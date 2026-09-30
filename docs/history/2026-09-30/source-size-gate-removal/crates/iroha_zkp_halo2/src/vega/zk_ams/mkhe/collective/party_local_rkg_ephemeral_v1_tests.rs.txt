use super::*;
use crate::{
    generalized_bulletproof::{GeneralizedBulletproofErrorV1 as BpError, ProofRandomSource},
    vega::{
        MaskedRelaxedRandomErrorV1 as RandomError, MaskedRelaxedRandomSourceV1,
        zk_ams::mkhe::{
            active::ZkAmsMkheGovernedActiveRosterV1,
            active_exact_binding::VerifiedPersistentWitnessBindingSetV1,
            direct_rkg_ephemeral_membership::tests::{
                creator_evidence, creator_replacement_binding, creator_state_fixture,
            },
        },
    },
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

const U_BYTES: usize = ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 / 4;
const B_BYTES: usize = RKG_EPHEMERAL_BLINDING_ENTROPY_BYTES_V1;
const ATTEMPTS: usize = MAX_RANDOM_REJECTION_ATTEMPTS_V1;
type Roster = ZkAmsMkheGovernedActiveRosterV1;
type Bindings = VerifiedPersistentWitnessBindingSetV1;
type State = ZkAmsMkheCollectivePartyStateV1;
type Evidence = ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1;
type DirectError = ZkAmsMkheDirectRkgEphemeralMembershipErrorV1;
type MkheError = ZkAmsMkheErrorV1;
type Context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1;
type Precursor = StateOwnedDirectRkgEphemeralMembershipPrecursorV1;

type DropAudit = [usize; 4];
#[derive(Clone, Copy)]
pub(super) enum Inject {
    None,
    Good,
    BadWire,
    Error(BpError),
    Panic,
}
std::thread_local! {
    static DROPS: Cell<DropAudit> = const { Cell::new([0; 4]) };
    static INJECT: Cell<Inject> = const { Cell::new(Inject::None) };
}
pub(super) fn record_drop_v1(index: usize, zero: bool) {
    if zero {
        DROPS.with(|cell| {
            let mut value = cell.get();
            value[index] += 1;
            cell.set(value);
        });
    }
}
pub(super) fn drops() -> DropAudit {
    DROPS.with(Cell::get)
}
pub(super) fn begin(value: Inject) {
    DROPS.with(|cell| cell.set([0; 4]));
    INJECT.with(|cell| cell.set(value));
}

pub(super) fn injected_membership_v1(
    context: Context,
    coefficients: &[i8],
    blindings: &[Scalar; 8],
    random: &mut dyn ProofRandomSource,
) -> Option<Result<Evidence, DirectError>> {
    match INJECT.with(|cell| cell.replace(Inject::None)) {
        Inject::None => None,
        Inject::Good => Some(creator_evidence(context, coefficients, blindings, false)),
        Inject::BadWire => Some(creator_evidence(context, coefficients, blindings, true)),
        Inject::Error(error) => Some(Err(ExactEightChunkMembershipErrorV1::Membership(
            ZkAmsT256MembershipErrorV1::Backend(error),
        )
        .into())),
        Inject::Panic => {
            random.fill_bytes(&mut [0]).unwrap();
            unreachable!()
        }
    }
}

pub(super) struct Rng(u8, usize, Option<usize>, Option<usize>);
impl Rng {
    pub(super) const fn new(byte: u8) -> Self {
        Self(byte, 0, None, None)
    }
    pub(super) const fn fail(byte: u8, at: usize) -> Self {
        Self(byte, 0, Some(at), None)
    }
    pub(super) const fn panic(byte: u8, at: usize) -> Self {
        Self(byte, 0, None, Some(at))
    }
    fn fill(&mut self, out: &mut [u8]) -> Result<(), ()> {
        if self.3.is_some_and(|at| self.1 >= at) {
            panic!("injected RNG panic");
        }
        if self.2.is_some_and(|at| self.1 >= at) {
            let partial = out.len().div_ceil(2);
            out[..partial].fill(self.0);
            self.1 += partial;
            return Err(());
        }
        out.fill(self.0);
        self.1 += out.len();
        Ok(())
    }
}
macro_rules! random_source {
    ($source:ident, $error:ty, $value:expr) => {
        impl $source for Rng {
            fn fill_bytes(&mut self, out: &mut [u8]) -> Result<(), $error> {
                self.fill(out).map_err(|()| $value)
            }
        }
    };
}
random_source!(
    MaskedRelaxedRandomSourceV1,
    RandomError,
    RandomError::Unavailable
);
random_source!(ProofRandomSource, BpError, BpError::RandomnessUnavailable);

struct Fx {
    r: Roster,
    b: Bindings,
    s: State,
}
impl Fx {
    fn new(label: &[u8]) -> Self {
        let (r, b, s) = creator_state_fixture(label);
        Self { r, b, s }
    }
    fn status(&self) -> (bool, u64) {
        (
            self.s.party_local_rkg_ephemeral_opening.is_some(),
            self.s.party_local_rkg_ephemeral_creation_mask,
        )
    }
    fn call(&mut self, digit: usize, random: &mut Rng) -> Result<Precursor, MkheError> {
        self.s
            .prepare_state_owned_direct_rkg_ephemeral_membership_v1(&self.r, &self.b, digit, random)
    }
    fn reject(&mut self, bindings: Option<&Bindings>, digit: usize) {
        begin(Inject::Good);
        let before = self.status();
        let mut random = Rng::new(0xaa);
        let bindings = bindings.unwrap_or(&self.b);
        let result = self
            .s
            .prepare_state_owned_direct_rkg_ephemeral_membership_v1(
                &self.r,
                bindings,
                digit,
                &mut random,
            );
        assert!(result.is_err());
        assert_eq!((random.1, self.status()), (0, before));
    }
    fn fail(&mut self, mode: Inject, random: &mut Rng, want: MkheError, audit: DropAudit) {
        begin(mode);
        assert_eq!(self.call(0, random).err(), Some(want));
        assert_eq!((drops(), self.status()), (audit, (false, 0)));
    }
}

#[test]
fn pre_entropy_failures_leave_slot_and_mask_unchanged() {
    let mut f = Fx::new(b"pre");
    let other = Fx::new(b"other");
    f.reject(Some(&other.b), 0);
    let cached =
        f.s.persistent_direct_opening
            .verified_binding
            .take()
            .unwrap();
    f.reject(None, 0);
    f.s.persistent_direct_opening.verified_binding = Some(cached);
    let replacement = creator_replacement_binding(&f.r, &f.s);
    let cached =
        f.s.persistent_direct_opening
            .verified_binding
            .replace(replacement)
            .unwrap();
    f.reject(None, 0);
    f.s.persistent_direct_opening.verified_binding = Some(cached);
    let original = f.s.persistent_direct_opening.secret.coefficients[0];
    f.s.persistent_direct_opening.secret.coefficients[0] = if original == 1 { 0 } else { 1 };
    f.reject(None, 0);
    f.s.persistent_direct_opening.secret.coefficients[0] = original;
    f.s.persistent_direct_opening.axes.cpk_transcript_digest[0] ^= 1;
    f.reject(None, 0);
    f.s.persistent_direct_opening.axes.cpk_transcript_digest[0] ^= 1;
    f.reject(None, 38);
    f.s.persistent_direct_opening.axes.party_index = 8;
    f.reject(None, 0);
}

#[test]
fn partial_and_exhausted_entropy_clear_partial_owners() {
    let mut f = Fx::new(b"entropy");
    f.fail(
        Inject::Good,
        &mut Rng::new(0x55),
        MkheError::RandomUnavailable,
        [ATTEMPTS, 0, 0, 0],
    );
    let mut partial_u = Rng::fail(0xaa, 0);
    f.fail(
        Inject::Good,
        &mut partial_u,
        MkheError::RandomUnavailable,
        [0; 4],
    );
    assert_eq!(partial_u.1, 1);
    for completed in 0..8 {
        f.fail(
            Inject::Good,
            &mut Rng::fail(0xaa, U_BYTES + completed * B_BYTES),
            MkheError::RandomUnavailable,
            [1, completed + 1, 1, 0],
        );
    }
    f.fail(
        Inject::Good,
        &mut Rng::new(0),
        MkheError::RandomUnavailable,
        [1, ATTEMPTS, 1, 0],
    );
}
#[test]
fn proof_verify_wire_and_unwind_failures_drop_every_transient() {
    let mut f = Fx::new(b"post");
    begin(Inject::Good);
    let mut random = Rng::panic(0xaa, U_BYTES);
    assert!(catch_unwind(AssertUnwindSafe(|| f.call(0, &mut random))).is_err());
    assert_eq!((drops(), f.status()), ([1, 1, 1, 0], (false, 0)));
    for (mode, error) in [
        (
            Inject::Error(BpError::RandomnessUnavailable),
            MkheError::RandomUnavailable,
        ),
        (
            Inject::Error(BpError::CircuitEquation),
            MkheError::InvalidKeyMaterial,
        ),
        (Inject::BadWire, MkheError::InvalidKeyMaterial),
    ] {
        f.fail(mode, &mut Rng::new(0xaa), error, [1, 8, 1, 1]);
    }
    begin(Inject::Panic);
    let mut random = Rng::panic(0xaa, U_BYTES + 8 * B_BYTES);
    assert!(catch_unwind(AssertUnwindSafe(|| f.call(0, &mut random))).is_err());
    assert_eq!((drops(), f.status()), ([1, 8, 1, 1], (false, 0)));
}

#[test]
fn success_installs_once_take_keeps_bit_and_drop_clears_owner() {
    let mut f = Fx::new(b"success");
    begin(Inject::Good);
    let public = f.call(37, &mut Rng::new(0xaa)).unwrap();
    assert_eq!(public.membership.to_wire_bytes().unwrap().len(), 12_291);
    assert_eq!((drops(), f.status()), ([0, 8, 0, 1], (true, 1_u64 << 37)));
    f.reject(None, 36);
    let owner = f.s.party_local_rkg_ephemeral_opening.take().unwrap();
    assert_eq!(
        (
            owner.context.record_index(),
            core::mem::size_of_val(&owner.retained_commitment_wire)
        ),
        (297, 264)
    );
    begin(Inject::None);
    drop(owner);
    assert_eq!((drops(), f.status()), ([1, 0, 1, 0], (false, 1_u64 << 37)));
    f.reject(None, 37);
}

#[test]
fn caps_resources_errors_and_single_slot_are_pinned() {
    let production = include_str!("party_local_rkg_ephemeral_v1.rs");
    let tests = include_str!("party_local_rkg_ephemeral_v1_tests.rs");
    assert!(production.lines().count() <= 500 && production.len() <= 24 * 1024);
    assert!(tests.lines().count() <= 500 && tests.len() <= 24 * 1024);
    assert_eq!(
        [
            RKG_EPHEMERAL_RETAINED_PAYLOAD_BYTES_V1,
            RKG_EPHEMERAL_WITH_NARROWING_BYTES_V1,
            ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1
        ],
        [1_049_096, 1_180_168, 12_291]
    );
    assert_eq!(
        map_t256_error_v1(ZkAmsT256MembershipErrorV1::Backend(
            BpError::ResourceOverflow
        )),
        MkheError::ResourceCeilingExceeded
    );
    assert_eq!(
        map_t256_error_v1(ZkAmsT256MembershipErrorV1::Backend(
            BpError::ProverRandomnessExhausted
        )),
        MkheError::RandomUnavailable
    );
    let mkhe = include_str!("../../mkhe.rs");
    for owner in [
        "impl Drop for SecretPolynomial",
        "impl<const N: usize> Drop for ZeroizingRandomBytesV1<N>",
    ] {
        assert!(mkhe.contains(owner));
    }
    assert_eq!(
        include_str!("../collective.rs")
            .matches("Option<PartyLocalRkgEphemeralOpeningV1>")
            .count(),
        1
    );
}
