//! Exact Fiat--Shamir manifest for the Phase-23 global lookup prerequisite.
//!
//! The schedule is deliberately private and move-only. It fixes every frame,
//! coordinate, challenge ordinal, retry rule, mask segment, and terminal
//! opening relation, but cannot mint any production proof or release receipt.
use super::*;
use crate::vega::{VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, sponge::Keccak256};
use core::{marker::PhantomData, ops::Range};
#[path = "challenge_v1/coefficient_residual_v1.rs"]
mod coefficient_residual_v1;
#[rustfmt::skip]
use coefficient_residual_v1::{COEFFICIENT_CHALLENGE_LANGUAGE_V1, COEFFICIENT_DIMENSIONS_V1, COEFFICIENT_GATE_LANGUAGE_V1, COEFFICIENT_RESIDUAL_COMMITMENT_LANGUAGE_V1, DELTA_ORDINAL_V1, CoefficientResidualCommitmentStageV1, challenge_is_outside_boolean_set_v1, coefficient_challenge_coordinate_v1, derive_coefficient_challenges_v1, hash_coefficient_manifest_suffix_v1};
#[cfg(test)]
use coefficient_residual_v1::{CoefficientResidualCommitmentSealV1, coefficient_gate_residuals_v1};
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.challenge\0";
const BOUND_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.bound-context\0";
const MANIFEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.challenge-manifest\0";
const FRAME_TAG_V1: u8 = 0x52;
const FRAME_CHALLENGE_MANIFEST_V1: &[u8] = b"challenge-manifest";
const FRAME_TOPOLOGY_V1: &[u8] = b"global-lookup-topology";
const FRAME_CONTEXT_V1: [&[u8]; 6] = [
    b"fixed-axes",
    b"source-binding",
    b"radix-range",
    b"packing",
    b"cross-field",
    b"qpcs-initial-root",
];
const FRAME_PRE_Z_COMMITMENTS_V1: &[u8] = b"pre-z-commitments";
const FRAME_POST_Z_INVERSES_V1: &[u8] = b"post-z-inverse-commitments";
const FRAME_SUMCHECK_COORDINATE_V1: &[u8] = b"sumcheck-coordinate";
const FRAME_SUMCHECK_GTILDE_V1: &[u8] = b"sumcheck-gtilde";
const FRAME_ENDPOINT_COORDINATE_V1: &[u8] = b"endpoint-coordinate";
const FRAME_ENDPOINT_COMMITMENT_V1: &[u8] = b"endpoint-commitment";
const FRAME_OPENING_PROOFS_V1: &[u8] = b"ordered-opening-proofs";
const FRAME_CHALLENGE_PURPOSE_V1: &[u8] = b"challenge-purpose";
const FRAME_CHALLENGE_ORDINAL_V1: &[u8] = b"challenge-ordinal";
const FRAME_CHALLENGE_ATTEMPT_V1: &[u8] = b"challenge-attempt";
const FRAME_CHALLENGE_SCALAR_V1: &[u8] = b"challenge-scalar";
const NO_COORDINATE_V1: u16 = u16::MAX;
const MAX_CHALLENGE_ATTEMPTS_V1: u8 = 128;
const LOOKUP_TABLE_VALUES_V1: u16 = 1 << 15;
const LOOKUP_DIMENSIONS_V1: usize = 29;
const ENDPOINT_BATCHES_V1: usize = 16;
const MASK_COMMITTED_SCALARS_V1: usize = 702;
const MASK_PADDED_SCALARS_V1: usize = 1_024;
const LOOKUP_Z_ORDINAL_V1: u32 = 0;
const RHO_FIRST_ORDINAL_V1: u32 = 1;
const RHO_LAST_ORDINAL_V1: u32 = 29;
const ALPHA_ORDINAL_V1: u32 = 30;
const LAMBDA_ORDINAL_V1: u32 = 31;
const MU_ORDINAL_V1: u32 = 32;
const FIRST_SUMCHECK_ORDINAL_V1: u32 = 49;
const LAST_SUMCHECK_ORDINAL_V1: u32 = 282;
const FIRST_ENDPOINT_BATCH_ORDINAL_V1: u32 = 283;
const LAST_ENDPOINT_BATCH_ORDINAL_V1: u32 = 298;
const MASK_BATCH_ORDINAL_V1: u32 = 299;
const COMPLETE_CHALLENGE_ORDINAL_V1: u32 = 300;
const LOOKUP_GATE_LANGUAGE_V1: &[u8] = b"lookup-statement=15;gate0:(z-A*)*U*-V*=0;gate1:R*-[alpha*chi_rho(r)*(V*-S(r_y))+lambda*(U*-E0(r_c)*M*Q_z(r_y))+mu*(E0(r_c)*M*-S(r_y))]=0;public-linear:R*+Z*-C15=0";
const MASK_OPENING_LANGUAGE_V1: &[u8] = b"mask-scalars=702,padded=1024;segments:s0..13=(14*s,14),s14=(196,9),s15=(205,29);h=1/2;w_(s,t)=(h^(len-1-t)*(r^3-h),h^(len-1-t)*(r^2-h),h^(len-1-t)*(r-h));xi-batch=sum_s(xi^s*w_s);padding-weight=0;s0..13:Z_s=<w_s,mask>,Cfinal_s-B_s-Z_s=0;s14:<w14,mask>+R14-C14=0;s15:<w15,mask>-Z*=0;R*+Z*-C15=0";
const LOOKUP_EVALUATION_LANGUAGE_V1: &[u8] = b"nu_s-derived-after-all-52-endpoint-commitments;s15:C_AU=sum_{p=0}^{32767}eq(r_y,bin15(p))*(C_A[p]+nu_15*C_U[p]);target=A*+nu_15*U*;coefficient-IPA-n=16384-at-r_c;mask-xi-derived-after-nu_15";
const ORDINAL_LANGUAGE_V1: &[u8] = b"outer-ordinals:z=0;rho[0..28]=1..29;alpha=30;lambda=31;mu=32;tau[0..13]=33..46;kappa=47;delta=48;for-j=0..233:absorb-exact-gtilde_j-96B-then-r_j=49+j;absorb-exact-52-endpoint-commitments;nu[0..15]=283..298;xi=299;complete=300";
const PURPOSE_LANGUAGE_V1: &[u8] = b"purpose-payload=literal-label||statement_be_u16||coordinate_be_u16;no-coordinate=ffff;tau=(unscoped-statement,coordinate0..13);kappa-delta=unscoped;equations=(statement0..13,round0..13);group=(statement14,round0..8);lookup=(statement15,round0..28);challenge-fork=state||challenge-domain||ordinal_be_u32||attempt_u8||branch_u8||framed-purpose;accepted-append=framed-purpose,ordinal,attempt,scalar_le;rejected-forks-do-not-mutate;attempts=0..127";
const _: () = {
    assert!(MASK_COMMITTED_SCALARS_V1 == REQUIRED_CUBIC_MESSAGES_V1 * 3);
    assert!(MASK_PADDED_SCALARS_V1 == 1 << 10);
    assert!(RHO_LAST_ORDINAL_V1 - RHO_FIRST_ORDINAL_V1 + 1 == LOOKUP_DIMENSIONS_V1 as u32);
    assert!(ALPHA_ORDINAL_V1 == RHO_LAST_ORDINAL_V1 + 1);
    assert!(LAMBDA_ORDINAL_V1 == ALPHA_ORDINAL_V1 + 1);
    assert!(MU_ORDINAL_V1 == LAMBDA_ORDINAL_V1 + 1);
    assert!(FIRST_SUMCHECK_ORDINAL_V1 == DELTA_ORDINAL_V1 + 1);
    assert!(LAST_SUMCHECK_ORDINAL_V1 + 1 == FIRST_ENDPOINT_BATCH_ORDINAL_V1);
    assert!(LAST_ENDPOINT_BATCH_ORDINAL_V1 - FIRST_ENDPOINT_BATCH_ORDINAL_V1 + 1 == 16);
    assert!(MASK_BATCH_ORDINAL_V1 == LAST_ENDPOINT_BATCH_ORDINAL_V1 + 1);
    assert!(COMPLETE_CHALLENGE_ORDINAL_V1 == MASK_BATCH_ORDINAL_V1 + 1);
};
#[rustfmt::skip]
#[derive(Clone, Copy)]
struct ChallengePurposeV1 { label: &'static [u8], statement: u16, coordinate: u16 }
#[rustfmt::skip]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChallengePredicateV1 { OutsideLookupTable = 1, Nonzero = 2, OutsideBooleanSet = 3 }
#[rustfmt::skip]
#[derive(Clone, Copy)]
struct ChallengeCoordinateV1 { ordinal: u32, purpose: ChallengePurposeV1, predicate: ChallengePredicateV1 }
#[rustfmt::skip]
impl ChallengePurposeV1 {
    const fn unscoped_v1(label: &'static [u8]) -> Self { Self { label, statement: NO_COORDINATE_V1, coordinate: NO_COORDINATE_V1 } }
    const fn scoped_v1(label: &'static [u8], statement: usize, coordinate: usize) -> Self { Self { label, statement: statement as u16, coordinate: coordinate as u16 } }
}
fn challenge_coordinate_v1(ordinal: u32) -> Result<ChallengeCoordinateV1, GlobalLookupErrorV1> {
    if let Some(coordinate) = coefficient_challenge_coordinate_v1(ordinal) {
        return Ok(coordinate);
    }
    let (purpose, predicate) = match ordinal {
        LOOKUP_Z_ORDINAL_V1 => (
            ChallengePurposeV1::unscoped_v1(b"lookup-z"),
            ChallengePredicateV1::OutsideLookupTable,
        ),
        RHO_FIRST_ORDINAL_V1..=RHO_LAST_ORDINAL_V1 => (
            ChallengePurposeV1::scoped_v1(
                b"lookup-rho-coordinate",
                15,
                (ordinal - RHO_FIRST_ORDINAL_V1) as usize,
            ),
            ChallengePredicateV1::Nonzero,
        ),
        ALPHA_ORDINAL_V1 => (
            ChallengePurposeV1::unscoped_v1(b"lookup-alpha"),
            ChallengePredicateV1::Nonzero,
        ),
        LAMBDA_ORDINAL_V1 => (
            ChallengePurposeV1::unscoped_v1(b"lookup-lambda"),
            ChallengePredicateV1::Nonzero,
        ),
        MU_ORDINAL_V1 => (
            ChallengePurposeV1::unscoped_v1(b"lookup-mu"),
            ChallengePredicateV1::Nonzero,
        ),
        FIRST_SUMCHECK_ORDINAL_V1..=LAST_SUMCHECK_ORDINAL_V1 => (
            sumcheck_purpose_v1((ordinal - FIRST_SUMCHECK_ORDINAL_V1) as usize)?,
            ChallengePredicateV1::Nonzero,
        ),
        FIRST_ENDPOINT_BATCH_ORDINAL_V1..=LAST_ENDPOINT_BATCH_ORDINAL_V1 => (
            ChallengePurposeV1::scoped_v1(
                b"evaluation-opening-batch",
                (ordinal - FIRST_ENDPOINT_BATCH_ORDINAL_V1) as usize,
                NO_COORDINATE_V1 as usize,
            ),
            ChallengePredicateV1::Nonzero,
        ),
        MASK_BATCH_ORDINAL_V1 => (
            ChallengePurposeV1::unscoped_v1(b"mask-opening-batch"),
            ChallengePredicateV1::Nonzero,
        ),
        _ => return Err(GlobalLookupErrorV1::Order),
    };
    Ok(ChallengeCoordinateV1 {
        ordinal,
        purpose,
        predicate,
    })
}
fn absorb_frame_header_v1(
    state: &mut Keccak256,
    label: &[u8],
    payload_len: usize,
) -> Result<(), GlobalLookupErrorV1> {
    let label_len = u16::try_from(label.len()).map_err(|_| GlobalLookupErrorV1::Shape)?;
    let payload_len = u64::try_from(payload_len).map_err(|_| GlobalLookupErrorV1::Shape)?;
    state.update(&[FRAME_TAG_V1]);
    state.update(&label_len.to_be_bytes());
    state.update(label);
    state.update(&payload_len.to_be_bytes());
    Ok(())
}
fn absorb_frame_v1(
    state: &mut Keccak256,
    label: &[u8],
    payload: &[u8],
) -> Result<(), GlobalLookupErrorV1> {
    absorb_frame_header_v1(state, label, payload.len())?;
    state.update(payload);
    Ok(())
}
fn absorb_purpose_v1(
    state: &mut Keccak256,
    purpose: ChallengePurposeV1,
) -> Result<(), GlobalLookupErrorV1> {
    let payload_len = purpose
        .label
        .len()
        .checked_add(4)
        .ok_or(GlobalLookupErrorV1::Shape)?;
    absorb_frame_header_v1(state, FRAME_CHALLENGE_PURPOSE_V1, payload_len)?;
    state.update(purpose.label);
    state.update(&purpose.statement.to_be_bytes());
    state.update(&purpose.coordinate.to_be_bytes());
    Ok(())
}
fn challenge_is_outside_table_v1(challenge: Scalar) -> bool {
    let bytes = challenge.to_le_bytes();
    bytes[2..].iter().any(|byte| *byte != 0)
        || u16::from_le_bytes([bytes[0], bytes[1]]) >= LOOKUP_TABLE_VALUES_V1
}
fn derive_challenge_with_policy_v1(
    state: &mut Keccak256,
    ordinal: &mut u32,
    purpose: ChallengePurposeV1,
    mut accepted: impl FnMut(u8, Scalar) -> bool,
) -> Result<Scalar, GlobalLookupErrorV1> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut wide = [0_u8; 64];
        for branch in 0_u8..=1 {
            let mut fork = state.fork_v1();
            fork.update(CHALLENGE_DOMAIN_V1);
            fork.update(&ordinal.to_be_bytes());
            fork.update(&[attempt, branch]);
            absorb_purpose_v1(&mut fork, purpose)?;
            let digest = fork.finalize();
            let start = usize::from(branch) * 32;
            wide[start..start + 32].copy_from_slice(&digest);
        }
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if accepted(attempt, challenge) {
            absorb_purpose_v1(state, purpose)?;
            absorb_frame_v1(state, FRAME_CHALLENGE_ORDINAL_V1, &ordinal.to_be_bytes())?;
            absorb_frame_v1(state, FRAME_CHALLENGE_ATTEMPT_V1, &[attempt])?;
            absorb_frame_v1(state, FRAME_CHALLENGE_SCALAR_V1, &challenge.to_le_bytes())?;
            *ordinal = ordinal
                .checked_add(1)
                .ok_or(GlobalLookupErrorV1::Arithmetic)?;
            return Ok(challenge);
        }
    }
    Err(GlobalLookupErrorV1::ChallengeExhausted)
}
fn derive_coordinate_challenge_v1(
    state: &mut Keccak256,
    ordinal: &mut u32,
) -> Result<Scalar, GlobalLookupErrorV1> {
    let coordinate = challenge_coordinate_v1(*ordinal)?;
    if coordinate.ordinal != *ordinal {
        return Err(GlobalLookupErrorV1::Order);
    }
    derive_challenge_with_policy_v1(
        state,
        ordinal,
        coordinate.purpose,
        |_, value| match coordinate.predicate {
            ChallengePredicateV1::OutsideLookupTable => challenge_is_outside_table_v1(value),
            ChallengePredicateV1::Nonzero => !value.is_zero(),
            ChallengePredicateV1::OutsideBooleanSet => challenge_is_outside_boolean_set_v1(value),
        },
    )
}
fn sumcheck_purpose_v1(ordinal: usize) -> Result<ChallengePurposeV1, GlobalLookupErrorV1> {
    let coordinate = cubic_message_coordinate_v1(ordinal)?;
    Ok(match coordinate.role {
        CubicMessageRoleV1::Equation => ChallengePurposeV1::scoped_v1(
            b"equation-sumcheck-round",
            ordinal / 14,
            coordinate.local_round,
        ),
        CubicMessageRoleV1::GroupBinder => {
            ChallengePurposeV1::scoped_v1(b"group-sumcheck-round", 14, coordinate.local_round)
        }
        CubicMessageRoleV1::GlobalLookup => {
            ChallengePurposeV1::scoped_v1(b"lookup-sumcheck-round", 15, coordinate.local_round)
        }
    })
}
fn absorb_sumcheck_coordinate_v1(
    state: &mut Keccak256,
    ordinal: usize,
) -> Result<(), GlobalLookupErrorV1> {
    let coordinate = cubic_message_coordinate_v1(ordinal)?;
    let statement = match coordinate.role {
        CubicMessageRoleV1::Equation => ordinal / 14,
        CubicMessageRoleV1::GroupBinder => 14,
        CubicMessageRoleV1::GlobalLookup => 15,
    };
    let encoded = [
        coordinate.role as u8,
        coordinate.equation.map(|role| role as u8).unwrap_or(0),
        u8::try_from(statement).map_err(|_| GlobalLookupErrorV1::Shape)?,
        u8::try_from(coordinate.local_round).map_err(|_| GlobalLookupErrorV1::Shape)?,
    ];
    absorb_frame_v1(state, FRAME_SUMCHECK_COORDINATE_V1, &encoded)
}
fn validate_gtilde_v1(bytes: &[u8; CUBIC_MESSAGE_BYTES_V1]) -> Result<(), GlobalLookupErrorV1> {
    for chunk in bytes.chunks_exact(32) {
        Scalar::from_le_bytes_exact(
            chunk
                .try_into()
                .map_err(|_| GlobalLookupErrorV1::Encoding)?,
        )
        .map_err(|_| GlobalLookupErrorV1::Encoding)?;
    }
    Ok(())
}
fn validate_endpoint_v1(bytes: &[u8; 33]) -> Result<(), GlobalLookupErrorV1> {
    Point::from_non_identity_wire_bytes_exact(bytes)
        .map(|_| ())
        .map_err(|_| GlobalLookupErrorV1::Encoding)
}
struct GlobalLookupChallengesV1 {
    z: Scalar,
    rho: [Scalar; LOOKUP_DIMENSIONS_V1],
    alpha: Scalar,
    lambda: Scalar,
    mu: Scalar,
    tau: [Scalar; COEFFICIENT_DIMENSIONS_V1],
    kappa: Scalar,
    delta: Scalar,
    sumcheck: [Scalar; REQUIRED_CUBIC_MESSAGES_V1],
    endpoint_batches: [Scalar; ENDPOINT_BATCHES_V1],
    mask_batch: Scalar,
}
impl GlobalLookupChallengesV1 {
    fn empty_v1() -> Self {
        Self {
            z: Scalar::zero(),
            rho: [Scalar::zero(); LOOKUP_DIMENSIONS_V1],
            alpha: Scalar::zero(),
            lambda: Scalar::zero(),
            mu: Scalar::zero(),
            tau: [Scalar::zero(); COEFFICIENT_DIMENSIONS_V1],
            kappa: Scalar::zero(),
            delta: Scalar::zero(),
            sumcheck: [Scalar::zero(); REQUIRED_CUBIC_MESSAGES_V1],
            endpoint_batches: [Scalar::zero(); ENDPOINT_BATCHES_V1],
            mask_batch: Scalar::zero(),
        }
    }
}
struct CommitmentStageV1;
struct LookupZStageV1;
struct SumcheckStageV1;
struct EndpointStageV1;
struct OpeningStageV1;
struct GlobalLookupTranscriptV1<S> {
    state: Keccak256,
    bound_context_digest: [u8; 32],
    challenge_ordinal: u32,
    next_sumcheck: usize,
    next_endpoint: usize,
    challenges: GlobalLookupChallengesV1,
    seals: BoundOwnerSealsV1,
    frames: BoundTranscriptFramesV1,
    stage: PhantomData<S>,
}
impl<S> GlobalLookupTranscriptV1<S> {
    fn transition_v1<T>(self) -> GlobalLookupTranscriptV1<T> {
        GlobalLookupTranscriptV1 {
            state: self.state,
            bound_context_digest: self.bound_context_digest,
            challenge_ordinal: self.challenge_ordinal,
            next_sumcheck: self.next_sumcheck,
            next_endpoint: self.next_endpoint,
            challenges: self.challenges,
            seals: self.seals,
            frames: self.frames,
            stage: PhantomData,
        }
    }
}
impl GlobalLookupTranscriptV1<CommitmentStageV1> {
    fn begin_v1(
        context: GlobalLookupContextV1,
        seals: BoundOwnerSealsV1,
        frames: BoundTranscriptFramesV1,
    ) -> Result<Self, GlobalLookupErrorV1> {
        frames.validate_v1()?;
        let bound_context_digest = bound_context_digest_v1(&context)?;
        let mut state = Keccak256::new();
        state.update(TRANSCRIPT_DOMAIN_V1);
        state.update(&[GLOBAL_LOOKUP_VERSION_V1]);
        absorb_frame_v1(
            &mut state,
            FRAME_CHALLENGE_MANIFEST_V1,
            &challenge_manifest_digest_v1(),
        )?;
        absorb_frame_v1(
            &mut state,
            FRAME_TOPOLOGY_V1,
            &global_lookup_topology_digest_v1(),
        )?;
        for (label, digest) in FRAME_CONTEXT_V1.into_iter().zip([
            context.fixed_axes_digest,
            context.source_binding_digest,
            context.radix_range_digest,
            context.packing_digest,
            context.cross_field_digest,
            context.qpcs_initial_root,
        ]) {
            absorb_frame_v1(&mut state, label, &require_nonzero_v1(digest)?)?;
        }
        Ok(Self {
            state,
            bound_context_digest,
            challenge_ordinal: 0,
            next_sumcheck: 0,
            next_endpoint: 0,
            challenges: GlobalLookupChallengesV1::empty_v1(),
            seals,
            frames,
            stage: PhantomData,
        })
    }
    fn absorb_commitments_and_derive_z_v1(
        mut self,
    ) -> Result<GlobalLookupTranscriptV1<LookupZStageV1>, GlobalLookupErrorV1> {
        absorb_frame_v1(
            &mut self.state,
            FRAME_PRE_Z_COMMITMENTS_V1,
            &self.frames.commitment_digest,
        )?;
        if self.challenge_ordinal != LOOKUP_Z_ORDINAL_V1 {
            return Err(GlobalLookupErrorV1::Order);
        }
        self.challenges.z =
            derive_coordinate_challenge_v1(&mut self.state, &mut self.challenge_ordinal)?;
        Ok(self.transition_v1())
    }
}
fn bound_context_digest_v1(
    context: &GlobalLookupContextV1,
) -> Result<[u8; 32], GlobalLookupErrorV1> {
    let mut state = Keccak256::new();
    state.update(BOUND_CONTEXT_DOMAIN_V1);
    state.update(&[GLOBAL_LOOKUP_VERSION_V1]);
    state.update(&challenge_manifest_digest_v1());
    state.update(&global_lookup_topology_digest_v1());
    for (label, digest) in FRAME_CONTEXT_V1.into_iter().zip([
        context.fixed_axes_digest,
        context.source_binding_digest,
        context.radix_range_digest,
        context.packing_digest,
        context.cross_field_digest,
        context.qpcs_initial_root,
    ]) {
        absorb_frame_v1(&mut state, label, &require_nonzero_v1(digest)?)?;
    }
    let digest = state.finalize();
    require_nonzero_v1(digest)
}
impl GlobalLookupTranscriptV1<LookupZStageV1> {
    #[rustfmt::skip]
    fn absorb_inverses_and_derive_relation_v1(
        mut self,
    ) -> Result<GlobalLookupTranscriptV1<CoefficientResidualCommitmentStageV1>, GlobalLookupErrorV1> {
        absorb_frame_v1(
            &mut self.state,
            FRAME_POST_Z_INVERSES_V1,
            &self.frames.inverse_digest,
        )?;
        for coordinate in 0..LOOKUP_DIMENSIONS_V1 {
            if self.challenge_ordinal != RHO_FIRST_ORDINAL_V1 + coordinate as u32 {
                return Err(GlobalLookupErrorV1::Order);
            }
            self.challenges.rho[coordinate] =
                derive_coordinate_challenge_v1(&mut self.state, &mut self.challenge_ordinal)?;
        }
        for (expected, destination) in [
            (ALPHA_ORDINAL_V1, &mut self.challenges.alpha),
            (LAMBDA_ORDINAL_V1, &mut self.challenges.lambda),
            (MU_ORDINAL_V1, &mut self.challenges.mu),
        ] {
            if self.challenge_ordinal != expected {
                return Err(GlobalLookupErrorV1::Order);
            }
            *destination =
                derive_coordinate_challenge_v1(&mut self.state, &mut self.challenge_ordinal)?;
        }
        derive_coefficient_challenges_v1(
            &mut self.state,
            &mut self.challenge_ordinal,
            &mut self.challenges,
        )?;
        if self.challenge_ordinal != FIRST_SUMCHECK_ORDINAL_V1 {
            return Err(GlobalLookupErrorV1::Order);
        }
        Ok(self.transition_v1())
    }
}
impl GlobalLookupTranscriptV1<SumcheckStageV1> {
    fn absorb_gtilde_v1(
        mut self,
        ordinal: usize,
        bytes: [u8; CUBIC_MESSAGE_BYTES_V1],
    ) -> Result<Self, GlobalLookupErrorV1> {
        if ordinal != self.next_sumcheck || ordinal >= REQUIRED_CUBIC_MESSAGES_V1 {
            return Err(GlobalLookupErrorV1::Order);
        }
        // The move-only stage takes the exact frame before parsing it; an
        // invalid frame cannot be retried against the same transcript state.
        self.next_sumcheck += 1;
        validate_gtilde_v1(&bytes)?;
        absorb_sumcheck_coordinate_v1(&mut self.state, ordinal)?;
        absorb_frame_v1(&mut self.state, FRAME_SUMCHECK_GTILDE_V1, &bytes)?;
        let expected = FIRST_SUMCHECK_ORDINAL_V1 + ordinal as u32;
        if self.challenge_ordinal != expected {
            return Err(GlobalLookupErrorV1::Order);
        }
        self.challenges.sumcheck[ordinal] =
            derive_coordinate_challenge_v1(&mut self.state, &mut self.challenge_ordinal)?;
        Ok(self)
    }
    fn finish_sumcheck_v1(
        self,
    ) -> Result<GlobalLookupTranscriptV1<EndpointStageV1>, GlobalLookupErrorV1> {
        if self.next_sumcheck != REQUIRED_CUBIC_MESSAGES_V1
            || self.challenge_ordinal != FIRST_ENDPOINT_BATCH_ORDINAL_V1
        {
            return Err(GlobalLookupErrorV1::Order);
        }
        Ok(self.transition_v1())
    }
}
impl GlobalLookupTranscriptV1<EndpointStageV1> {
    fn absorb_endpoint_commitment_v1(
        mut self,
        ordinal: usize,
        bytes: [u8; 33],
    ) -> Result<Self, GlobalLookupErrorV1> {
        if ordinal != self.next_endpoint || ordinal >= HIDDEN_ENDPOINTS_V1 {
            return Err(GlobalLookupErrorV1::Order);
        }
        // Advance before point validation so malformed proof material consumes
        // this move-only stage instead of exposing a validation oracle.
        self.next_endpoint += 1;
        validate_endpoint_v1(&bytes)?;
        let coordinate = [endpoint_tag_v1(hidden_endpoint_role_v1(ordinal)?)];
        absorb_frame_v1(&mut self.state, FRAME_ENDPOINT_COORDINATE_V1, &coordinate)?;
        absorb_frame_v1(&mut self.state, FRAME_ENDPOINT_COMMITMENT_V1, &bytes)?;
        Ok(self)
    }
    fn derive_opening_batches_v1(
        mut self,
    ) -> Result<GlobalLookupTranscriptV1<OpeningStageV1>, GlobalLookupErrorV1> {
        if self.next_endpoint != HIDDEN_ENDPOINTS_V1
            || self.challenge_ordinal != FIRST_ENDPOINT_BATCH_ORDINAL_V1
        {
            return Err(GlobalLookupErrorV1::Order);
        }
        for statement in 0..ENDPOINT_BATCHES_V1 {
            self.challenges.endpoint_batches[statement] =
                derive_coordinate_challenge_v1(&mut self.state, &mut self.challenge_ordinal)?;
        }
        if self.challenge_ordinal != MASK_BATCH_ORDINAL_V1 {
            return Err(GlobalLookupErrorV1::Order);
        }
        self.challenges.mask_batch =
            derive_coordinate_challenge_v1(&mut self.state, &mut self.challenge_ordinal)?;
        Ok(self.transition_v1())
    }
}
impl GlobalLookupTranscriptV1<OpeningStageV1> {
    fn absorb_openings_and_finish_v1(mut self) -> Result<[u8; 32], GlobalLookupErrorV1> {
        if self.challenge_ordinal != COMPLETE_CHALLENGE_ORDINAL_V1 {
            return Err(GlobalLookupErrorV1::Order);
        }
        absorb_frame_v1(
            &mut self.state,
            FRAME_OPENING_PROOFS_V1,
            &self.frames.opening_digest,
        )?;
        Ok(self.state.finalize())
    }
}
fn mask_segment_v1(statement: usize) -> Result<Range<usize>, GlobalLookupErrorV1> {
    match statement {
        0..=13 => Ok(statement * 14..statement * 14 + 14),
        14 => Ok(196..205),
        15 => Ok(205..234),
        _ => Err(GlobalLookupErrorV1::Shape),
    }
}
fn scalar_pow_v1(mut base: Scalar, mut exponent: usize) -> Scalar {
    let mut result = Scalar::one();
    while exponent != 0 {
        if exponent & 1 == 1 {
            result *= base;
        }
        base = base.square();
        exponent >>= 1;
    }
    result
}
fn mask_terminal_weight_v1(
    statement: usize,
    local_round: usize,
    coefficient: usize,
    challenges: &[Scalar; REQUIRED_CUBIC_MESSAGES_V1],
) -> Result<Scalar, GlobalLookupErrorV1> {
    let segment = mask_segment_v1(statement)?;
    if local_round >= segment.len() || coefficient >= 3 {
        return Err(GlobalLookupErrorV1::Shape);
    }
    let challenge = challenges[segment.start + local_round];
    let half = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| GlobalLookupErrorV1::Arithmetic)?;
    let monomial = match coefficient {
        0 => challenge.square() * challenge,
        1 => challenge.square(),
        2 => challenge,
        _ => return Err(GlobalLookupErrorV1::Shape),
    } - half;
    Ok(scalar_pow_v1(half, segment.len() - 1 - local_round) * monomial)
}
fn batched_mask_weight_v1(
    scalar_ordinal: usize,
    challenges: &[Scalar; REQUIRED_CUBIC_MESSAGES_V1],
    xi: Scalar,
) -> Result<Scalar, GlobalLookupErrorV1> {
    if scalar_ordinal >= MASK_PADDED_SCALARS_V1 {
        return Err(GlobalLookupErrorV1::Shape);
    }
    if scalar_ordinal >= MASK_COMMITTED_SCALARS_V1 {
        return Ok(Scalar::zero());
    }
    let message = scalar_ordinal / 3;
    let coefficient = scalar_ordinal % 3;
    let statement = match message {
        0..=195 => message / 14,
        196..=204 => 14,
        205..=233 => 15,
        _ => return Err(GlobalLookupErrorV1::Shape),
    };
    let local_round = message - mask_segment_v1(statement)?.start;
    Ok(scalar_pow_v1(xi, statement)
        * mask_terminal_weight_v1(statement, local_round, coefficient, challenges)?)
}
fn equality_weight_v1(index: usize, point: &[Scalar; 15]) -> Result<Scalar, GlobalLookupErrorV1> {
    if index >= 1 << 15 {
        return Err(GlobalLookupErrorV1::Shape);
    }
    Ok(point
        .iter()
        .enumerate()
        .fold(Scalar::one(), |weight, (coordinate, challenge)| {
            weight
                * if index >> coordinate & 1 == 1 {
                    *challenge
                } else {
                    Scalar::one() - *challenge
                }
        }))
}
fn lookup_evaluation_target_v1(candidate: Scalar, inverse: Scalar, nu: Scalar) -> Scalar {
    candidate + nu * inverse
}
fn lookup_evaluation_commitment_term_v1(
    candidate: Point,
    inverse: Point,
    equality: Scalar,
    nu: Scalar,
) -> Point {
    (candidate + inverse.mul_scalar(nu)).mul_scalar(equality)
}
fn multilinear_equality_v1(
    left: &[Scalar; LOOKUP_DIMENSIONS_V1],
    right: &[Scalar; LOOKUP_DIMENSIONS_V1],
) -> Scalar {
    left.iter()
        .zip(right)
        .fold(Scalar::one(), |weight, (left, right)| {
            weight * ((Scalar::one() - *left) * (Scalar::one() - *right) + *left * *right)
        })
}
fn active_selector_v1(
    point: &[Scalar; LOOKUP_DIMENSIONS_V1],
) -> Result<Scalar, GlobalLookupErrorV1> {
    let plane_point: &[Scalar; 15] = point[14..]
        .try_into()
        .map_err(|_| GlobalLookupErrorV1::Shape)?;
    let mut selector = Scalar::zero();
    for plane in 0..ACTIVE_LOOKUP_PLANES_V1 {
        selector += equality_weight_v1(plane, plane_point)?;
    }
    Ok(selector)
}
fn coordinate_zero_selector_v1(point: &[Scalar; LOOKUP_DIMENSIONS_V1]) -> Scalar {
    point[..14]
        .iter()
        .fold(Scalar::one(), |selector, coordinate| {
            selector * (Scalar::one() - *coordinate)
        })
}
fn fixed_table_inverse_mle_v1(
    z: Scalar,
    point: &[Scalar; LOOKUP_DIMENSIONS_V1],
) -> Result<Scalar, GlobalLookupErrorV1> {
    if !challenge_is_outside_table_v1(z) {
        return Err(GlobalLookupErrorV1::Context);
    }
    let plane_point: &[Scalar; 15] = point[14..]
        .try_into()
        .map_err(|_| GlobalLookupErrorV1::Shape)?;
    let mut evaluation = Scalar::zero();
    for table_value in 0..usize::from(LOOKUP_TABLE_VALUES_V1) {
        let inverse = (z - Scalar::from_u64(table_value as u64))
            .inverse()
            .map_err(|_| GlobalLookupErrorV1::Arithmetic)?;
        evaluation += equality_weight_v1(table_value, plane_point)? * inverse;
    }
    Ok(evaluation)
}
fn lookup_relation_residual_v1(
    z: Scalar,
    inverse: Scalar,
    inverse_product: Scalar,
    multiplicity: Scalar,
    residual: Scalar,
    alpha: Scalar,
    rho: &[Scalar; LOOKUP_DIMENSIONS_V1],
    point: &[Scalar; LOOKUP_DIMENSIONS_V1],
    lambda: Scalar,
    mu: Scalar,
) -> Result<Scalar, GlobalLookupErrorV1> {
    let chi = multilinear_equality_v1(rho, point);
    let active = active_selector_v1(point)?;
    let coordinate_zero = coordinate_zero_selector_v1(point);
    let table_inverse = fixed_table_inverse_mle_v1(z, point)?;
    let rhs = alpha * chi * (inverse_product - active)
        + lambda * (inverse - coordinate_zero * multiplicity * table_inverse)
        + mu * (coordinate_zero * multiplicity - active);
    Ok(residual - rhs)
}
fn lookup_gate_residuals_v1(
    z: Scalar,
    candidate: Scalar,
    inverse: Scalar,
    inverse_product: Scalar,
    multiplicity: Scalar,
    residual: Scalar,
    alpha: Scalar,
    rho: &[Scalar; LOOKUP_DIMENSIONS_V1],
    point: &[Scalar; LOOKUP_DIMENSIONS_V1],
    lambda: Scalar,
    mu: Scalar,
    masked_accumulator: Scalar,
    public_claim: Scalar,
) -> Result<[Scalar; 3], GlobalLookupErrorV1> {
    Ok([
        (z - candidate) * inverse - inverse_product,
        lookup_relation_residual_v1(
            z,
            inverse,
            inverse_product,
            multiplicity,
            residual,
            alpha,
            rho,
            point,
            lambda,
            mu,
        )?,
        residual + masked_accumulator - public_claim,
    ])
}
fn mask_constraint_residuals_v1(
    statement: usize,
    mask_evaluation: Scalar,
    residual: Scalar,
    public_claim: Scalar,
    lookup_masked_accumulator: Scalar,
) -> Result<[Scalar; 2], GlobalLookupErrorV1> {
    match statement {
        14 => Ok([mask_evaluation + residual - public_claim, Scalar::zero()]),
        15 => Ok([
            mask_evaluation - lookup_masked_accumulator,
            residual + lookup_masked_accumulator - public_claim,
        ]),
        _ => Err(GlobalLookupErrorV1::Shape),
    }
}
#[rustfmt::skip]
fn hash_manifest_frame_v1(hash: &mut Keccak256, label: &[u8]) {
    hash.update(&[0x46]);
    hash.update(&(label.len() as u16).to_be_bytes());
    hash.update(label);
}
fn hash_manifest_challenge_v1(hash: &mut Keccak256, ordinal: u32) {
    let coordinate = challenge_coordinate_v1(ordinal).expect("constant challenge schedule");
    hash.update(&[0x43]);
    hash.update(&coordinate.ordinal.to_be_bytes());
    hash.update(&[coordinate.predicate as u8]);
    hash.update(&(coordinate.purpose.label.len() as u16).to_be_bytes());
    hash.update(coordinate.purpose.label);
    hash.update(&coordinate.purpose.statement.to_be_bytes());
    hash.update(&coordinate.purpose.coordinate.to_be_bytes());
    hash.update(&[0x66]);
    hash_manifest_frame_v1(hash, FRAME_CHALLENGE_PURPOSE_V1);
    hash.update(&[0x61]);
    for label in [
        FRAME_CHALLENGE_PURPOSE_V1,
        FRAME_CHALLENGE_ORDINAL_V1,
        FRAME_CHALLENGE_ATTEMPT_V1,
        FRAME_CHALLENGE_SCALAR_V1,
    ] {
        hash_manifest_frame_v1(hash, label);
    }
}
pub(super) fn challenge_manifest_digest_v1() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V1);
    hash.update(&[GLOBAL_LOOKUP_VERSION_V1, FRAME_TAG_V1]);
    for value in [
        u64::from(MAX_CHALLENGE_ATTEMPTS_V1),
        LOOKUP_DIMENSIONS_V1 as u64,
        COEFFICIENT_DIMENSIONS_V1 as u64,
        REQUIRED_CUBIC_MESSAGES_V1 as u64,
        HIDDEN_ENDPOINTS_V1 as u64,
        ENDPOINT_BATCHES_V1 as u64,
        MASK_COMMITTED_SCALARS_V1 as u64,
        MASK_PADDED_SCALARS_V1 as u64,
        u64::from(COMPLETE_CHALLENGE_ORDINAL_V1),
    ] {
        hash.update(&value.to_be_bytes());
    }
    for language in [
        ORDINAL_LANGUAGE_V1,
        PURPOSE_LANGUAGE_V1,
        COEFFICIENT_CHALLENGE_LANGUAGE_V1,
        COEFFICIENT_RESIDUAL_COMMITMENT_LANGUAGE_V1,
        COEFFICIENT_GATE_LANGUAGE_V1,
        LOOKUP_GATE_LANGUAGE_V1,
        MASK_OPENING_LANGUAGE_V1,
        LOOKUP_EVALUATION_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u64).to_be_bytes());
        hash.update(language);
    }
    hash.update(TRANSCRIPT_DOMAIN_V1);
    hash.update(CHALLENGE_DOMAIN_V1);
    for label in [FRAME_CHALLENGE_MANIFEST_V1, FRAME_TOPOLOGY_V1] {
        hash_manifest_frame_v1(&mut hash, label);
    }
    for label in FRAME_CONTEXT_V1 {
        hash_manifest_frame_v1(&mut hash, label);
    }
    hash_manifest_frame_v1(&mut hash, FRAME_PRE_Z_COMMITMENTS_V1);
    hash_manifest_challenge_v1(&mut hash, LOOKUP_Z_ORDINAL_V1);
    hash_manifest_frame_v1(&mut hash, FRAME_POST_Z_INVERSES_V1);
    for ordinal in RHO_FIRST_ORDINAL_V1..=MU_ORDINAL_V1 {
        hash_manifest_challenge_v1(&mut hash, ordinal);
    }
    hash_coefficient_manifest_suffix_v1(&mut hash);
    for message in 0..REQUIRED_CUBIC_MESSAGES_V1 {
        hash_manifest_frame_v1(&mut hash, FRAME_SUMCHECK_COORDINATE_V1);
        hash_manifest_frame_v1(&mut hash, FRAME_SUMCHECK_GTILDE_V1);
        hash_manifest_challenge_v1(&mut hash, FIRST_SUMCHECK_ORDINAL_V1 + message as u32);
    }
    for _ in 0..HIDDEN_ENDPOINTS_V1 {
        hash_manifest_frame_v1(&mut hash, FRAME_ENDPOINT_COORDINATE_V1);
        hash_manifest_frame_v1(&mut hash, FRAME_ENDPOINT_COMMITMENT_V1);
    }
    for ordinal in FIRST_ENDPOINT_BATCH_ORDINAL_V1..=MASK_BATCH_ORDINAL_V1 {
        hash_manifest_challenge_v1(&mut hash, ordinal);
    }
    hash_manifest_frame_v1(&mut hash, FRAME_OPENING_PROOFS_V1);
    hash.finalize()
}
#[path = "challenge_v1/global_lookup_external_sumcheck_v1.rs"]
mod global_lookup_external_sumcheck_v1;
#[cfg(test)]
#[path = "challenge_v1_tests.rs"]
mod tests;
