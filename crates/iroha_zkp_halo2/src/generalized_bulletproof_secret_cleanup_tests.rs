use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
static TEST_LOCK: Mutex<()> = Mutex::new(());
static CLEAR_CALLS: AtomicUsize = AtomicUsize::new(0);
static POINT_CLEAR_CALLS: AtomicUsize = AtomicUsize::new(0);
static POINT_ADD_CALLS: AtomicUsize = AtomicUsize::new(0);
static PANIC_ON_POINT_ADD: AtomicUsize = AtomicUsize::new(usize::MAX);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrackingScalar(u64);
impl Add for TrackingScalar {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_add(rhs.0))
    }
}
impl Sub for TrackingScalar {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_sub(rhs.0))
    }
}
impl Mul for TrackingScalar {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_mul(rhs.0))
    }
}
impl Neg for TrackingScalar {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(self.0.wrapping_neg())
    }
}
impl AddAssign for TrackingScalar {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for TrackingScalar {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for TrackingScalar {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl ProofScalar for TrackingScalar {
    const ZERO: Self = Self(0);
    const ONE: Self = Self(1);
    const SCALAR_BITS: usize = 64;
    fn from_u64(value: u64) -> Self {
        Self(value)
    }
    fn decode(bytes: [u8; 32]) -> Option<Self> {
        if bytes == [u8::MAX; 32] {
            return None;
        }
        Some(Self(u64::from_le_bytes(
            bytes[..8]
                .try_into()
                .expect("eight-byte tracking scalar encoding"),
        )))
    }
    fn encode(self) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        bytes[..8].copy_from_slice(&self.0.to_le_bytes());
        bytes
    }
    fn reduce_wide(bytes: [u8; 64]) -> Self {
        Self(u64::from_le_bytes(
            bytes[..8]
                .try_into()
                .expect("eight-byte tracking scalar reduction"),
        ))
    }
    fn invert(self) -> Option<Self> {
        (!self.is_zero()).then_some(Self::ONE)
    }
    fn sqrt(self) -> Option<Self> {
        Some(self)
    }
    fn square(self) -> Self {
        self * self
    }
    fn double(self) -> Self {
        self + self
    }
    fn is_zero(self) -> bool {
        self == Self::ZERO
    }
    fn is_odd(self) -> bool {
        self.0 & 1 == 1
    }
    fn clear_secret(&mut self) {
        self.0 = 0;
        CLEAR_CALLS.fetch_add(1, Ordering::SeqCst);
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrackingPoint(u64);
impl Add for TrackingPoint {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let call = POINT_ADD_CALLS.fetch_add(1, Ordering::SeqCst) + 1;
        assert_ne!(
            PANIC_ON_POINT_ADD.load(Ordering::SeqCst),
            call,
            "deliberate secret-MSM point-operation panic"
        );
        Self(self.0.wrapping_add(rhs.0))
    }
}
impl Sub for TrackingPoint {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_sub(rhs.0))
    }
}
impl Neg for TrackingPoint {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(self.0.wrapping_neg())
    }
}
impl AddAssign for TrackingPoint {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for TrackingPoint {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl ProofPoint for TrackingPoint {
    type Scalar = TrackingScalar;
    type Encoded = [u8; 32];
    const POINT_BYTES: usize = 32;
    fn identity() -> Self {
        Self(0)
    }
    fn is_identity(self) -> bool {
        self.0 == 0
    }
    fn double(self) -> Self {
        Self(self.0.wrapping_mul(2))
    }
    fn scale(self, scalar: Self::Scalar) -> Self {
        Self(self.0.wrapping_mul(scalar.0))
    }
    fn conditional_select(a: &Self, b: &Self, choice: u8) -> Self {
        let mask = 0_u64.wrapping_sub(u64::from(choice & 1));
        Self((a.0 & !mask) | (b.0 & mask))
    }
    fn clear_secret(&mut self) {
        self.0 = 0;
        POINT_CLEAR_CALLS.fetch_add(1, Ordering::SeqCst);
    }
    fn encode(self) -> Self::Encoded {
        let mut bytes = [0_u8; 32];
        bytes[..8].copy_from_slice(&self.0.to_le_bytes());
        bytes
    }
    fn decode(
        bytes: impl AsRef<[u8]>,
        allow_identity: bool,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let bytes = bytes.as_ref();
        if bytes.len() != 32 || bytes[8..].iter().any(|byte| *byte != 0) {
            return Err(GeneralizedBulletproofErrorV1::PointEncoding);
        }
        let point = Self(u64::from_le_bytes(
            bytes[..8]
                .try_into()
                .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?,
        ));
        if !allow_identity && point.is_identity() {
            return Err(GeneralizedBulletproofErrorV1::PointIdentity);
        }
        Ok(point)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrackingSuite;
impl ProofSuite for TrackingSuite {
    type Scalar = TrackingScalar;
    type Point = TrackingPoint;
    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: std::sync::OnceLock<ProofGenerators<TrackingSuite>> =
            std::sync::OnceLock::new();
        GENERATORS.get_or_init(|| {
            ProofGenerators::new(
                TrackingPoint(1),
                TrackingPoint(2),
                vec![TrackingPoint(3)],
                vec![TrackingPoint(4)],
            )
            .expect("tracking generator basis")
        })
    }
}
struct ScriptedRandom {
    requests: usize,
    fail_at: Option<usize>,
}
impl ProofRandomSource for ScriptedRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        let request = self.requests;
        self.requests += 1;
        if self.fail_at == Some(request) {
            return Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable);
        }
        destination.fill((request + 1) as u8);
        Ok(())
    }
}
struct PanickingRandom {
    requests: usize,
    panic_at: usize,
}
struct FixedRandom(u8);
impl ProofRandomSource for FixedRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        destination.fill(self.0);
        Ok(())
    }
}
impl ProofRandomSource for PanickingRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        let request = self.requests;
        self.requests += 1;
        assert_ne!(request, self.panic_at, "deliberate entropy-source panic");
        destination.fill((request + 1) as u8);
        Ok(())
    }
}
#[test]
fn secret_scalar_owner_clears_constructor_and_transfer_slots() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let owned = SecretScalar::new(TrackingScalar::ZERO);
    assert!(owned.is_zero());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(owned);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut source = TrackingScalar(7);
    let owned = SecretScalar::take(&mut source);
    assert_eq!(source, TrackingScalar::ZERO);
    assert!(!owned.is_zero());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(owned);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    let source = include_str!("generalized_bulletproof.rs");
    assert!(source.contains("fn new(mut value: F) -> Self"));
    assert!(source.contains("fn take(value: &mut F) -> Self"));
    let production = source
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let secret_owner = production
        .split_once("impl<F: ProofScalar> SecretScalar<F> {")
        .expect("secret scalar owner")
        .1
        .split_once("impl<F: ProofScalar> Drop for SecretScalar<F>")
        .expect("secret scalar owner boundary")
        .0;
    assert_eq!(
        secret_owner.matches("fn is_zero(&self) -> bool {").count(),
        1
    );
    assert!(secret_owner.contains("self.0.eq(&F::ZERO)"));
    assert!(!secret_owner.contains("fn expose_copy(&self) -> F"));
    assert!(!secret_owner.contains("self.0.is_zero()"));
    let constraint_precheck = production
        .split_once("        for constraint in &self.constraints {")
        .expect("constraint precheck")
        .1
        .split_once("        let alpha = random_scalar::<S::Scalar, _>(rng)?;")
        .expect("constraint precheck boundary")
        .0;
    let final_accumulation = constraint_precheck
        .rfind("*evaluation.expose_mut() +=")
        .expect("final constraint accumulation");
    let inspection = constraint_precheck
        .find("if !evaluation.is_zero() {")
        .expect("borrowed constraint inspection");
    let error = constraint_precheck[inspection..]
        .find("return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);")
        .map(|position| inspection + position)
        .expect("constraint error");
    assert!(final_accumulation < inspection && inspection < error);
    assert_eq!(
        constraint_precheck.matches("evaluation.is_zero()").count(),
        1
    );
    for forbidden in [
        "evaluation.expose_copy",
        "*evaluation.expose_ref",
        ".clone(",
        ".cloned(",
        ".copied(",
        ".to_vec(",
        "Vec::",
        "alloc",
        "random",
        "rng",
        "transcript",
        "unsafe",
        "callback",
        "FnOnce",
        "FnMut",
        "?",
    ] {
        assert!(
            !constraint_precheck.contains(forbidden),
            "borrowed constraint zero-check {forbidden}"
        );
    }
    let random = source
        .split_once("pub fn random_scalar<F, R>(")
        .expect("random scalar function")
        .1
        .split_once("/// Scalar operations required")
        .expect("random scalar boundary")
        .0;
    assert!(random.contains("if let Some(scalar) = F::random(rng)?"));
    assert!(random.contains("Result<SecretScalar<F>, GeneralizedBulletproofErrorV1>"));
    assert!(random.contains("return Ok(scalar);"));
    assert!(!random.contains("SecretScalar::take"));
    assert!(!random.contains("expose_copy"));
}
#[test]
fn proof_scalar_one_attempt_returns_only_owned_candidates() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut success = FixedRandom(7);
    let sampled = TrackingScalar::random(&mut success)
        .expect("fixed entropy succeeds")
        .expect("fixed entropy is canonical");
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(sampled);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut zero = FixedRandom(0);
    let sampled = TrackingScalar::random(&mut zero)
        .expect("zero entropy succeeds")
        .expect("zero is canonical");
    assert_eq!(sampled.expose_ref(), &TrackingScalar::ZERO);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(sampled);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 1);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut rejected = FixedRandom(u8::MAX);
    assert!(
        TrackingScalar::random(&mut rejected)
            .expect("rejection is not an entropy failure")
            .is_none()
    );
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 1);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut failure = ScriptedRandom {
        requests: 0,
        fail_at: Some(0),
    };
    assert!(matches!(
        TrackingScalar::random(&mut failure),
        Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
    ));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 1);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let returned_owner_unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut random = FixedRandom(11);
        let _sampled = TrackingScalar::random(&mut random)
            .expect("fixed entropy succeeds")
            .expect("fixed entropy is canonical");
        panic!("exercise one-attempt owner unwind");
    }));
    assert!(returned_owner_unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 1);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let entropy_unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut random = PanickingRandom {
            requests: 0,
            panic_at: 0,
        };
        let _ = TrackingScalar::random(&mut random);
    }));
    assert!(entropy_unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 1);

    let production = include_str!("generalized_bulletproof.rs")
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let trait_random = production
        .split_once("    /// Sample one canonical scalar from the supplied entropy source")
        .expect("trait random method")
        .1
        .split_once("\n}\n/// One owned secret scalar")
        .expect("trait random boundary")
        .0;
    assert!(
        trait_random
            .contains(") -> Result<Option<SecretScalar<Self>>, GeneralizedBulletproofErrorV1>")
    );
    assert!(trait_random.contains("if let Some(mut scalar) = Self::decode(bytes.0)"));
    assert!(trait_random.contains("Ok(Some(SecretScalar::take(&mut scalar)))"));
    assert!(trait_random.contains("Ok(None)"));
    assert!(!trait_random.contains("Result<Option<Self>"));
}
#[test]
fn random_scalar_owner_clears_success_error_and_unwind() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut success = ScriptedRandom {
        requests: 0,
        fail_at: None,
    };
    let sampled =
        random_scalar::<TrackingScalar, _>(&mut success).expect("scripted scalar sample succeeds");
    assert_eq!(success.requests, 1);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(sampled);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut failure = ScriptedRandom {
        requests: 0,
        fail_at: Some(0),
    };
    assert!(matches!(
        random_scalar::<TrackingScalar, _>(&mut failure),
        Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
    ));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut random = ScriptedRandom {
            requests: 0,
            fail_at: None,
        };
        let _sampled =
            random_scalar::<TrackingScalar, _>(&mut random).expect("sample owner before unwind");
        panic!("exercise sampled-scalar owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
}
#[test]
fn scoped_guards_clear_named_scalar_and_direct_msm_owners() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    {
        let _scalar = SecretScalar::new(TrackingScalar(7));
        let mut terms =
            SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
        terms
            .push(&TrackingScalar(11), &TrackingPoint(17))
            .expect("first term");
        terms
            .push(&TrackingScalar(13), &TrackingPoint(19))
            .expect("second term");
    }
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 2);
}
#[test]
fn secret_builder_private_push_copy_handoffs_and_clears_every_exit() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    // Exercise the same owner-first handoff used by `push_copy` directly so
    // both vacated source slots remain observable before their guard drops.
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut source_scalar = TrackingScalar(29);
    let mut source_point = TrackingPoint(31);
    let retained = {
        let mut incoming =
            BorrowedSecretMsmTerm::<TrackingSuite>::new(&mut source_scalar, &mut source_point);
        let retained = incoming.take_term();
        assert_eq!(*incoming.scalar, TrackingScalar::ZERO);
        assert_eq!(*incoming.point, TrackingPoint::identity());
        assert_eq!(retained.scalar, TrackingScalar(29));
        assert_eq!(retained.point, TrackingPoint(31));
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 0);
        drop(incoming);
        retained
    };
    assert_eq!(source_scalar, TrackingScalar::ZERO);
    assert_eq!(source_point, TrackingPoint::identity());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(retained);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 2);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut terms =
        SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
    let allocation = terms.terms.as_ptr();
    let allocation_capacity = terms.terms.capacity();
    terms
        .push_copy(TrackingScalar(37), TrackingPoint(41))
        .expect("first term fits exact capacity");
    terms
        .push_copy(TrackingScalar(43), TrackingPoint(47))
        .expect("second term fits exact capacity");
    assert_eq!(terms.terms.as_ptr(), allocation);
    assert_eq!(terms.terms.capacity(), allocation_capacity);
    assert_eq!(terms.terms.len(), 2);
    assert_eq!(terms.terms[0].scalar, TrackingScalar(37));
    assert_eq!(terms.terms[0].point, TrackingPoint(41));
    assert_eq!(terms.terms[1].scalar, TrackingScalar(43));
    assert_eq!(terms.terms[1].point, TrackingPoint(47));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(
        terms.push_copy(TrackingScalar(53), TrackingPoint(59)),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    );
    assert_eq!(terms.terms.as_ptr(), allocation);
    assert_eq!(terms.terms.capacity(), allocation_capacity);
    assert_eq!(terms.terms.len(), 2);
    assert_eq!(terms.terms[0].scalar, TrackingScalar(37));
    assert_eq!(terms.terms[0].point, TrackingPoint(41));
    assert_eq!(terms.terms[1].scalar, TrackingScalar(43));
    assert_eq!(terms.terms[1].point, TrackingPoint(47));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 3);
    drop(terms);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 5);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 5);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut terms =
            SecretMultiexpBuilder::<TrackingSuite>::new(1).expect("unwind tracking capacity");
        let allocation = terms.terms.as_ptr();
        let allocation_capacity = terms.terms.capacity();
        terms
            .push_copy(TrackingScalar(61), TrackingPoint(67))
            .expect("unwind term fits exact capacity");
        assert_eq!(terms.terms.as_ptr(), allocation);
        assert_eq!(terms.terms.capacity(), allocation_capacity);
        assert_eq!(terms.terms[0].scalar, TrackingScalar(61));
        assert_eq!(terms.terms[0].point, TrackingPoint(67));
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 1);
        panic!("exercise owner-first computed-term unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 2);
}
#[test]
fn secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values() {
    let source = include_str!("generalized_bulletproof.rs");
    let production = source
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let owner = production
        .split_once("impl<S: ProofSuite> SecretMsmTerm<S> {")
        .expect("retained MSM term owner")
        .1
        .split_once("impl<S: ProofSuite> Drop for SecretMsmTerm<S>")
        .expect("retained MSM term owner boundary")
        .0;
    assert!(owner.contains("fn copy_from_borrowed(scalar: &S::Scalar, point: &S::Point)"));
    assert!(owner.contains("scalar: *scalar,"));
    assert!(owner.contains("point: *point,"));

    let borrowed_push = production
        .split_once("pub fn push(\n")
        .expect("borrowed MSM insertion")
        .1
        .split_once("fn push_copy(")
        .expect("borrowed MSM insertion boundary")
        .0;
    assert!(borrowed_push.contains("scalar: &S::Scalar,\n        point: &S::Point,"));
    let capacity_preflight = borrowed_push
        .find("if self.terms.len() >= self.exact_capacity")
        .expect("capacity preflight");
    let retained_copy = borrowed_push
        .find("SecretMsmTerm::<S>::copy_from_borrowed(scalar, point)")
        .expect("direct retained owner copy");
    assert!(capacity_preflight < retained_copy);
    assert!(!borrowed_push.contains("push_copy("));
    assert!(!borrowed_push.contains("*scalar"));
    assert!(!borrowed_push.contains("*point"));

    let incoming_owner = production
        .split_once("impl<'a, S: ProofSuite> BorrowedSecretMsmTerm<'a, S> {")
        .expect("computed-value parameter owner")
        .1
        .split_once("impl<S: ProofSuite> Drop for BorrowedSecretMsmTerm<'_, S>")
        .expect("computed-value parameter owner boundary")
        .0;
    let handoff = incoming_owner
        .split_once("fn take_term(&mut self) -> SecretMsmTerm<S> {")
        .expect("computed-value owner handoff")
        .1;
    let mut cursor = 0;
    for step in [
        "let mut retained = SecretMsmTerm",
        "scalar: S::Scalar::ZERO,",
        "point: S::Point::identity(),",
        "core::mem::swap(&mut retained.scalar, &mut *self.scalar);",
        "core::mem::swap(&mut retained.point, &mut *self.point);",
        "retained",
    ] {
        let offset = handoff[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing owner-first MSM handoff step {step}"));
        cursor += offset + step.len();
    }
    assert_eq!(handoff.matches("core::mem::swap(").count(), 2);

    let owned_push = production
        .split_once("fn push_copy(")
        .expect("computed-value MSM insertion")
        .1
        .split_once("/// Evaluate exactly")
        .expect("computed-value MSM insertion boundary")
        .0;
    assert!(owned_push.contains("mut scalar: S::Scalar,"));
    assert!(owned_push.contains("mut point: S::Point,"));
    let mut cursor = 0;
    for step in [
        "let mut incoming = BorrowedSecretMsmTerm::<S>::new(&mut scalar, &mut point);",
        "if self.terms.len() >= self.exact_capacity",
        "return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);",
        "let retained = incoming.take_term();",
        "self.terms.push(retained);",
        "drop(incoming);",
        "Ok(())",
    ] {
        let offset = owned_push[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing computed-term insertion step {step}"));
        cursor += offset + step.len();
    }
    assert_eq!(owned_push.matches("incoming.take_term()").count(), 1);
    assert_eq!(owned_push.matches("self.terms.push(retained);").count(), 1);
    assert_eq!(owned_push.matches("drop(incoming);").count(), 1);
    for forbidden in [
        "scalar_copy",
        "point_copy",
        "scalar: *",
        "point: *",
        "expose_copy",
        ".clone(",
        ".cloned(",
        ".copied(",
        ".to_owned(",
        "copy_from_slice",
        "core::ptr",
        "copy_nonoverlapping",
        "core::mem::replace",
        "unsafe",
        "Vec::",
        "vec![",
        ".reserve(",
        ".reserve_exact(",
        ".try_reserve",
        ".collect",
        "callback",
        "FnOnce",
        "FnMut",
        "random_scalar",
        "rng",
        "transcript",
        "?",
    ] {
        assert!(
            !handoff.contains(forbidden) && !owned_push.contains(forbidden),
            "owner-first computed-term path {forbidden}"
        );
    }

    assert_eq!(production.matches(".push_copy(").count(), 13);
    let prover = production
        .split_once("pub fn prove<R, T>(")
        .expect("generalized prover")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("generalized prover boundary")
        .0;
    let p_terms_start = prover
        .find("let mut p_terms = SecretMultiexpBuilder::<S>::new(1 + (2 * n))?;")
        .expect("prover P-term owner");
    let p_terms_end = prover
        .find("transcript.push_scalar(tau_x.expose_ref())?;")
        .expect("prover P-term boundary");
    let p_terms = &prover[p_terms_start..p_terms_end];
    let mut cursor = 0;
    for step in [
        "let mut p_terms = SecretMultiexpBuilder::<S>::new(1 + (2 * n))?;",
        "for (index, (left, right)) in l_eval.0.iter().zip(&r_eval.0).enumerate()",
        "p_terms.push(left, &self.generators.g_bold[index])?;",
        "p_terms.push_copy(y_inverse[index] * *right, self.generators.h_bold[index])?;",
    ] {
        let offset = p_terms[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing fixed P-term step {step}"));
        cursor += offset + step.len();
    }
    assert_eq!(p_terms.matches("p_terms.push(").count(), 1);
    assert_eq!(p_terms.matches("p_terms.push_copy(").count(), 1);

    let secret_straus = production
        .split_once("fn secret_straus_chunk<S: ProofSuite>(")
        .expect("secret Straus chunk")
        .1
        .split_once("/// Encoded scalar material cached by Pippenger")
        .expect("secret Straus chunk boundary")
        .0;
    let scalar_encodings = secret_straus
        .split_once(
            "let mut encodings = SecretScalarEncodings([[0_u8; 32]; SECRET_MSM_CHUNK_TERMS_V1]);",
        )
        .expect("prezeroed secret scalar encodings")
        .1
        .split_once("let mut accumulator = SecretPoint::new(S::Point::identity());")
        .expect("secret scalar encoding boundary")
        .0;
    let mut cursor = 0;
    for step in [
        "let mut encoding = SecretBytes(term.scalar.bits_le());",
        "core::mem::swap(&mut encodings.0[index], &mut encoding.0);",
        "drop(encoding);",
    ] {
        let offset = scalar_encodings[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing scalar-encoding owner-transfer step {step}"));
        cursor += offset + step.len();
    }
    for (needle, expected) in [
        ("let mut encoding = SecretBytes(term.scalar.bits_le());", 1),
        (
            "core::mem::swap(&mut encodings.0[index], &mut encoding.0);",
            1,
        ),
        ("drop(encoding);", 1),
        ("term.scalar.bits_le()", 1),
        ("encodings.0[index]", 1),
        ("encoding.0", 1),
    ] {
        assert_eq!(scalar_encodings.matches(needle).count(), expected);
    }
    for forbidden in [
        "encodings.0[index] = encoding.0;",
        "copy",
        "clone",
        "*",
        "unsafe",
        "ptr",
        "replace",
        "Vec",
        "vec!",
        "reserve",
        "push(",
        "insert",
        "resize",
        "append",
        "extend",
        "collect",
        "alloc",
        "Box",
        "String",
        "format!",
        "to_string",
        "callback",
        "Fn",
        "|",
        "random",
        "rng",
        "entropy",
        "transcript",
        "?",
    ] {
        assert!(
            !scalar_encodings.contains(forbidden),
            "owner-first scalar-encoding path {forbidden}"
        );
    }

    let nibble_comparator = production
        .split_once(
            "fn ct_eq_window_nibble(encoded_byte: &u8, shift: usize, candidate: u8) -> u8 {",
        )
        .expect("borrowed secret-window nibble comparator")
        .1
        .split_once("fn secret_straus_chunk<S: ProofSuite>(")
        .expect("borrowed secret-window nibble comparator boundary")
        .0;
    assert!(nibble_comparator.contains(
        "((*encoded_byte >> shift) & (SECRET_MSM_TABLE_ENTRIES_V1 as u8 - 1)) ^ candidate"
    ));
    assert!(nibble_comparator.contains("difference.wrapping_sub(1)"));
    for forbidden in [
        "let digit",
        "digit:",
        "left: u8",
        "SecretBytes",
        "clone",
        "copy",
        "unsafe",
        "ptr",
        "Vec",
        "alloc",
        "callback",
        "Fn",
        "random",
        "rng",
        "entropy",
        "transcript",
        "?",
    ] {
        assert!(
            !nibble_comparator.contains(forbidden),
            "borrowed secret-window nibble comparator {forbidden}"
        );
    }

    let window_scan = secret_straus
        .split_once("let mut accumulator = SecretPoint::new(S::Point::identity());")
        .expect("secret-window scan")
        .1
        .split_once("Ok(accumulator)")
        .expect("secret-window scan boundary")
        .0;
    let mut cursor = 0;
    for step in [
        "for window in (0..SECRET_MSM_WINDOWS_V1).rev()",
        "for _ in 0..SECRET_MSM_WINDOW_BITS_V1",
        "accumulator.double_assign();",
        "let byte_index = window / 2;",
        "let shift = (window % 2) * SECRET_MSM_WINDOW_BITS_V1;",
        "for index in 0..terms.len()",
        "let mut selected = SecretPoint::new(S::Point::identity());",
        "for candidate in 0..SECRET_MSM_TABLE_ENTRIES_V1",
        "ct_eq_window_nibble(",
        "&encodings.0[index][byte_index],",
        "shift,",
        "candidate as u8,",
        "accumulator.add_assign_secret(selected);",
    ] {
        let offset = window_scan[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing fused secret-window scan step {step}"));
        cursor += offset + step.len();
    }
    for (needle, expected) in [
        ("for index in 0..terms.len()", 1),
        ("ct_eq_window_nibble(", 1),
        ("&encodings.0[index][byte_index]", 1),
        ("candidate as u8", 1),
    ] {
        assert_eq!(window_scan.matches(needle).count(), expected);
    }
    for forbidden in [
        "SecretDigits",
        "digits",
        "let digit",
        "ct_eq_u8",
        "clone",
        "copy",
        "SecretBytes",
        "Vec",
        "vec!",
        "reserve",
        "push(",
        "insert",
        "resize",
        "append",
        "extend",
        "collect",
        "alloc",
        "Box",
        "String",
        "format!",
        "to_string",
        "unsafe",
        "ptr",
        "replace",
        "callback",
        "Fn",
        "random",
        "rng",
        "entropy",
        "transcript",
        "?",
    ] {
        assert!(
            !window_scan.contains(forbidden),
            "fused secret-window scan {forbidden}"
        );
    }
    assert!(!production.contains("struct SecretDigits("));
    assert!(!production.contains("fn ct_eq_u8("));
    assert_eq!(production.matches("ct_eq_window_nibble(").count(), 2);
}
#[test]
fn secret_builder_rejects_overflow_without_reallocation_and_wipes_terms() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut terms =
        SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
    terms
        .push(&TrackingScalar(3), &TrackingPoint(5))
        .expect("first term");
    terms
        .push(&TrackingScalar(7), &TrackingPoint(11))
        .expect("second term");
    let allocation = terms.terms.as_ptr();
    let allocation_capacity = terms.terms.capacity();
    assert_eq!(
        terms.push(&TrackingScalar(13), &TrackingPoint(17)),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    );
    assert_eq!(terms.terms.as_ptr(), allocation);
    assert_eq!(terms.terms.capacity(), allocation_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(terms);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 2);
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut incomplete =
        SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
    incomplete
        .push(&TrackingScalar(19), &TrackingPoint(23))
        .expect("partial term");
    assert!(matches!(
        incomplete.evaluate(),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 1);
}
#[test]
fn secret_builder_returned_owner_clears_on_success_and_comparison_mismatch() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    let evaluate = || {
        let mut terms = SecretMultiexpBuilder::<TrackingSuite>::new(1).expect("one-term capacity");
        terms
            .push(&TrackingScalar(3), &TrackingPoint(5))
            .expect("one retained term");
        terms.evaluate().expect("complete one-term MSM")
    };

    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let point = evaluate();
    assert!(point.equals(&TrackingPoint(15)));
    let live_owner_clears = POINT_CLEAR_CALLS.load(Ordering::SeqCst);
    drop(point);
    let completed_clears = POINT_CLEAR_CALLS.load(Ordering::SeqCst);
    assert_eq!(completed_clears, live_owner_clears + 1);

    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mismatch = (|| -> Result<(), GeneralizedBulletproofErrorV1> {
        let point = evaluate();
        if !point.equals(&TrackingPoint(16)) {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        Ok(())
    })();
    assert_eq!(
        mismatch,
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    );
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), completed_clears);
}
#[test]
fn secret_msm_point_owner_and_borrowed_publication_boundary_are_static() {
    let production = include_str!("generalized_bulletproof.rs")
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let evaluate = production
        .split_once("    /// Evaluate exactly the declared number of terms")
        .expect("secret MSM evaluation")
        .1
        .split_once("/// Exact-capacity collection")
        .expect("secret MSM evaluation boundary")
        .0;
    assert!(evaluate.contains(
        "pub fn evaluate(self) -> Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1>"
    ));
    assert!(!evaluate.contains("Result<S::Point"));
    assert!(production.contains("pub fn move_into(mut self, destination: &mut P)"));
    assert!(!production.contains("pub fn transfer"));
    let owner = production
        .split_once("impl<P: ProofPoint> SecretPoint<P> {")
        .expect("secret point owner")
        .1
        .split_once("impl<P: ProofPoint> Drop for SecretPoint<P>")
        .expect("secret point owner boundary")
        .0;
    let identity = owner
        .split_once("pub fn is_identity(&self) -> bool {")
        .expect("borrowed identity inspection")
        .1
        .split_once("/// Compare the retained point")
        .expect("borrowed identity inspection boundary")
        .0;
    assert!(identity.contains("self.0.eq(&P::identity())"));
    assert!(!identity.contains("expose_copy"));
    let owned_add = owner
        .split_once("fn add_assign_secret(&mut self, rhs: Self) {")
        .expect("owned secret-point addition")
        .1
        .split_once("fn add_scaled_pair_assign(")
        .expect("owned secret-point addition boundary")
        .0;
    let mut cursor = 0;
    for step in [
        "let mut sum = self.0 + rhs.0;",
        "drop(rhs);",
        "self.replace(&mut sum);",
    ] {
        let offset = owned_add[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing owned secret-point addition step {step}"));
        cursor += offset + step.len();
    }
    for (needle, expected) in [
        ("let mut sum = self.0 + rhs.0;", 1),
        ("drop(rhs);", 1),
        ("self.replace(&mut sum);", 1),
    ] {
        assert_eq!(owned_add.matches(needle).count(), expected);
    }
    for forbidden in [
        "rhs: &Self",
        "let mut rhs",
        "BorrowedSecretPoint",
        "expose_copy",
        "clone",
        "copy",
        "core::mem",
        "unsafe",
        "ptr",
        "Vec",
        "alloc",
        "callback",
        "Fn",
        "random",
        "rng",
        "entropy",
        "transcript",
        "?",
    ] {
        assert!(
            !owned_add.contains(forbidden),
            "owned secret-point addition {forbidden}"
        );
    }
    let owned_scaled_pair_signature = concat!(
        "fn add_scaled_pair_assign(\n",
        "        &mut self,\n",
        "        left: Self,\n",
        "        left_scalar: P::Scalar,\n",
        "        right: Self,\n",
        "        right_scalar: P::Scalar,\n",
        "    ) {",
    );
    assert_eq!(owner.matches(owned_scaled_pair_signature).count(), 1);
    assert!(!owner.contains("left: &Self"));
    assert!(!owner.contains("right: &Self"));
    let owned_scaled_pair = owner
        .split_once(owned_scaled_pair_signature)
        .expect("owned scaled-pair point addition")
        .1
        .split_once("fn select_assign(")
        .expect("owned scaled-pair point addition boundary")
        .0;
    let mut cursor = 0;
    for step in [
        "let mut updated =",
        "left.0.scale(left_scalar) + self.0 + right.0.scale(right_scalar);",
        "drop(right);",
        "drop(left);",
        "self.replace(&mut updated);",
    ] {
        let offset = owned_scaled_pair[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing owned scaled-pair addition step {step}"));
        cursor += offset + step.len();
    }
    for (needle, expected) in [
        ("let mut updated =", 1),
        (
            "left.0.scale(left_scalar) + self.0 + right.0.scale(right_scalar);",
            1,
        ),
        ("drop(right);", 1),
        ("drop(left);", 1),
        ("self.replace(&mut updated);", 1),
    ] {
        assert_eq!(owned_scaled_pair.matches(needle).count(), expected);
    }
    for forbidden in [
        "left_point",
        "current_point",
        "right_point",
        "BorrowedSecretPoint",
        "expose_copy",
        "clone",
        "copy",
        "core::mem",
        "unsafe",
        "ptr",
        "Vec",
        "alloc",
        "callback",
        "Fn",
        "random",
        "rng",
        "entropy",
        "transcript",
        "?",
    ] {
        assert!(
            !owned_scaled_pair.contains(forbidden),
            "owned scaled-pair point addition {forbidden}"
        );
    }
    let fold = production
        .split_once("    fn fold_in_order(")
        .expect("secret chunk fold")
        .1
        .split_once("struct SecretScalarEncodings")
        .expect("secret chunk fold boundary")
        .0;
    assert!(fold.contains("Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1>"));
    assert!(fold.contains("Ok(result)"));
    assert!(!fold.contains("Ok(result.expose_copy())"));
    assert!(!fold.contains("Result<S::Point"));
    assert_eq!(fold.matches("result.add_assign_secret(chunk);").count(), 1);
    assert!(!fold.contains("result.add_assign_secret(&chunk);"));
    assert_eq!(production.matches(".add_assign_secret(").count(), 2);
    assert!(!production.contains(".add_assign_secret(&"));

    let transcript_trait = production
        .split_once("pub trait ProverTranscript<S: ProofSuite>")
        .expect("prover transcript trait")
        .1
        .split_once("/// Transcript reads required by the verifier")
        .expect("prover transcript trait boundary")
        .0;
    assert!(transcript_trait.contains("point: &S::Point"));
    assert!(!transcript_trait.contains("point: S::Point"));
    assert_eq!(production.matches("transcript.push_point(").count(), 9);
    for publication in [
        "transcript.push_point(ai.expose_ref())?;",
        "transcript.push_point(ao.expose_ref())?;",
        "transcript.push_point(s_point.expose_ref())?;",
        "transcript.push_point(commitment.expose_ref())?;",
        "transcript.push_point(left.expose_ref())?;",
        "transcript.push_point(right.expose_ref())?;",
    ] {
        assert!(production.contains(publication));
    }
    assert_eq!(
        production
            .matches("transcript.push_point(commitment.expose_ref())?;")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("transcript.push_point(left.expose_ref())?;")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("transcript.push_point(right.expose_ref())?;")
            .count(),
        2
    );
    for forbidden in [
        "transcript.push_point(ai)?",
        "transcript.push_point(ao)?",
        "transcript.push_point(s_point)?",
        "transcript.push_point(commitment)?",
        "transcript.push_point(left)?",
        "transcript.push_point(right)?",
    ] {
        assert!(!production.contains(forbidden));
    }
}
#[test]
fn secret_builder_matches_public_and_naive_msm_across_chunks() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    POINT_ADD_CALLS.store(0, Ordering::SeqCst);
    let mut public_terms = Vec::with_capacity(260);
    let edges = [0_u64, 1, 2, u64::MAX];
    for index in 0..260_u64 {
        let scalar = if index < edges.len() as u64 {
            edges[index as usize]
        } else {
            index
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .rotate_left((index % 64) as u32)
        };
        public_terms.push((
            TrackingScalar(scalar),
            TrackingPoint(index.wrapping_mul(17).wrapping_add(3)),
        ));
    }
    let expected = public_terms
        .iter()
        .fold(TrackingPoint::identity(), |sum, term| {
            TrackingPoint(sum.0.wrapping_add(term.0.0.wrapping_mul(term.1.0)))
        });
    assert_eq!(multiexp::<TrackingSuite>(&public_terms), expected);
    let evaluate_secret = || {
        let mut secret = SecretMultiexpBuilder::<TrackingSuite>::new(public_terms.len())
            .expect("fixed cross-chunk capacity");
        for (scalar, point) in &public_terms {
            secret
                .push(scalar, point)
                .expect("term fits exact capacity");
        }
        secret.evaluate().expect("complete secret MSM")
    };
    #[cfg(feature = "parallel")]
    let single_thread = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("single-thread Rayon pool")
        .install(&evaluate_secret);
    #[cfg(not(feature = "parallel"))]
    let single_thread = evaluate_secret();
    assert!(single_thread.equals(&expected));
    #[cfg(feature = "parallel")]
    {
        let four_threads = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .expect("four-thread Rayon pool")
            .install(&evaluate_secret);
        assert!(four_threads.equals(&expected));
        assert_eq!(
            (*single_thread.expose_ref()).encode(),
            (*four_threads.expose_ref()).encode()
        );
    }
}
#[test]
fn public_two_term_straus_matches_independent_scaling_at_scalar_edges() {
    let scalars = [
        0_u64,
        1,
        2,
        3,
        4,
        0x5555_5555_5555_5555,
        0xaaaa_aaaa_aaaa_aaaa,
        0x8000_0000_0000_0001,
        u64::MAX,
    ];
    for left in scalars {
        for right in scalars {
            let terms = [
                (TrackingScalar(left), TrackingPoint(0x1234_5678)),
                (TrackingScalar(right), TrackingPoint(0x9abc_def0)),
            ];
            let expected = terms[0].1.scale(terms[0].0) + terms[1].1.scale(terms[1].0);
            assert_eq!(
                multiexp::<TrackingSuite>(&terms),
                expected,
                "two-term public fold diverged for ({left:#x}, {right:#x})"
            );
        }
    }
}
fn tracking_msm(terms: impl IntoIterator<Item = (TrackingScalar, TrackingPoint)>) -> TrackingPoint {
    terms
        .into_iter()
        .fold(TrackingPoint::identity(), |sum, (scalar, point)| {
            sum + point.scale(scalar)
        })
}
fn tracking_inner_product(left: &[TrackingScalar], right: &[TrackingScalar]) -> TrackingScalar {
    left.iter()
        .copied()
        .zip(right.iter().copied())
        .fold(TrackingScalar::ZERO, |sum, (left, right)| {
            sum + (left * right)
        })
}
#[test]
fn symbolic_initial_h_matches_eager_materialization_at_small_powers_of_two() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    for n in [1_usize, 2, 4, 8] {
        let g_bold = (0..n)
            .map(|index| TrackingPoint(17 + (index as u64 * 6)))
            .collect::<Vec<_>>();
        let h_bold = (0..n)
            .map(|index| TrackingPoint(71 + (index as u64 * 10)))
            .collect::<Vec<_>>();
        let a = (0..n)
            .map(|index| TrackingScalar(3 + index as u64))
            .collect::<Vec<_>>();
        let b = (0..n)
            .map(|index| TrackingScalar(19 + (index as u64 * 3)))
            .collect::<Vec<_>>();
        let weight_edges = [
            0_u64,
            1,
            u64::MAX,
            0x8000_0000_0000_0001,
            2,
            3,
            0x5555_5555_5555_5555,
            0xaaaa_aaaa_aaaa_aaaa,
        ];
        let weights = weight_edges[..n]
            .iter()
            .copied()
            .map(TrackingScalar)
            .collect::<Vec<_>>();
        let eager_h = h_bold
            .iter()
            .copied()
            .zip(weights.iter().copied())
            .map(|(point, weight)| point.scale(weight))
            .collect::<Vec<_>>();
        let g = TrackingPoint(211);
        let u_scalar = TrackingScalar(13);
        let u = g.scale(u_scalar);
        let product = tracking_inner_product(&a, &b);
        let eager_opening = tracking_msm(
            a.iter()
                .copied()
                .zip(g_bold.iter().copied())
                .chain(b.iter().copied().zip(eager_h.iter().copied()))
                .chain(core::iter::once((product, u))),
        );
        let symbolic_opening = tracking_msm(
            a.iter()
                .copied()
                .zip(g_bold.iter().copied())
                .chain(
                    b.iter()
                        .copied()
                        .zip(weights.iter().copied())
                        .map(|(scalar, weight)| scalar * weight)
                        .zip(h_bold.iter().copied()),
                )
                .chain(core::iter::once((product * u_scalar, g))),
        );
        assert_eq!(symbolic_opening, eager_opening, "opening diverged at n={n}");
        if n == 1 {
            continue;
        }
        let half = n / 2;
        let (a_left, a_right) = a.split_at(half);
        let (b_left, b_right) = b.split_at(half);
        let (g_left, g_right) = g_bold.split_at(half);
        let (h_left, h_right) = h_bold.split_at(half);
        let (eager_h_left, eager_h_right) = eager_h.split_at(half);
        let (weight_left, weight_right) = weights.split_at(half);
        let c_left = tracking_inner_product(a_left, b_right);
        let c_right = tracking_inner_product(a_right, b_left);
        let eager_left = tracking_msm(
            a_left
                .iter()
                .copied()
                .zip(g_right.iter().copied())
                .chain(b_right.iter().copied().zip(eager_h_left.iter().copied()))
                .chain(core::iter::once((c_left, u))),
        );
        let symbolic_left = tracking_msm(
            a_left
                .iter()
                .copied()
                .zip(g_right.iter().copied())
                .chain(
                    b_right
                        .iter()
                        .copied()
                        .zip(weight_left.iter().copied())
                        .map(|(scalar, weight)| scalar * weight)
                        .zip(h_left.iter().copied()),
                )
                .chain(core::iter::once((c_left * u_scalar, g))),
        );
        assert_eq!(symbolic_left, eager_left, "L0 diverged at n={n}");
        let eager_right = tracking_msm(
            a_right
                .iter()
                .copied()
                .zip(g_left.iter().copied())
                .chain(b_left.iter().copied().zip(eager_h_right.iter().copied()))
                .chain(core::iter::once((c_right, u))),
        );
        let symbolic_right = tracking_msm(
            a_right
                .iter()
                .copied()
                .zip(g_left.iter().copied())
                .chain(
                    b_left
                        .iter()
                        .copied()
                        .zip(weight_right.iter().copied())
                        .map(|(scalar, weight)| scalar * weight)
                        .zip(h_right.iter().copied()),
                )
                .chain(core::iter::once((c_right * u_scalar, g))),
        );
        assert_eq!(symbolic_right, eager_right, "R0 diverged at n={n}");
        let challenge = TrackingScalar(7);
        let inverse = TrackingScalar(11);
        for index in 0..half {
            let eager_fold =
                eager_h_left[index].scale(challenge) + eager_h_right[index].scale(inverse);
            let symbolic_fold = h_left[index].scale(challenge * weight_left[index])
                + h_right[index].scale(inverse * weight_right[index]);
            assert_eq!(
                symbolic_fold, eager_fold,
                "first H fold diverged at n={n}, index={index}"
            );
        }
    }
}
#[derive(Default)]
struct RecordingProverTranscript(Vec<u8>);
impl ProverTranscript<TrackingSuite> for RecordingProverTranscript {
    fn push_scalar(
        &mut self,
        scalar: &TrackingScalar,
    ) -> Result<(), GeneralizedBulletproofErrorV1> {
        self.0.push(0);
        self.0.extend_from_slice(&scalar.encode());
        Ok(())
    }
    fn push_point(&mut self, point: &TrackingPoint) -> Result<(), GeneralizedBulletproofErrorV1> {
        self.0.push(1);
        self.0.extend_from_slice(&(*point).encode());
        Ok(())
    }
    fn challenge(&mut self) -> Result<TrackingScalar, GeneralizedBulletproofErrorV1> {
        Ok(TrackingScalar::ONE)
    }
}
#[test]
fn prover_scalar_publication_borrows_every_private_response() {
    let production = include_str!("generalized_bulletproof.rs")
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let transcript_trait = production
        .split_once("pub trait ProverTranscript<S: ProofSuite>")
        .expect("prover transcript trait")
        .1
        .split_once("/// Transcript reads required by the verifier")
        .expect("prover transcript trait boundary")
        .0;
    assert!(transcript_trait.contains("scalar: &S::Scalar"));
    assert!(!transcript_trait.contains("scalar: S::Scalar"));
    assert_eq!(production.matches("transcript.push_scalar(").count(), 7);
    for publication in [
        "transcript.push_scalar(tau_x.expose_ref())?;",
        "transcript.push_scalar(u.expose_ref())?;",
        "transcript.push_scalar(t_caret.expose_ref())?;",
    ] {
        assert!(production.contains(publication));
    }
    assert_eq!(
        production
            .matches("transcript.push_scalar(&a[0])?;")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("transcript.push_scalar(&b[0])?;")
            .count(),
        2
    );
    for forbidden in [
        "transcript.push_scalar(tau_x.expose_copy())",
        "transcript.push_scalar(u.expose_copy())",
        "transcript.push_scalar(*t_caret.expose_ref())",
        "transcript.push_scalar(a[0])",
        "transcript.push_scalar(b[0])",
    ] {
        assert!(!production.contains(forbidden));
    }
}
#[test]
fn symbolic_h_proof_bytes_are_worker_count_independent() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    let g_bold = [
        TrackingPoint(17),
        TrackingPoint(19),
        TrackingPoint(23),
        TrackingPoint(29),
    ];
    let h_bold = [
        TrackingPoint(31),
        TrackingPoint(37),
        TrackingPoint(41),
        TrackingPoint(43),
    ];
    let generators = ProofGeneratorView::<TrackingSuite> {
        g: TrackingPoint(47),
        h: TrackingPoint(53),
        g_bold: &g_bold,
        h_bold: &h_bold,
    };
    let a = [
        TrackingScalar(2),
        TrackingScalar(3),
        TrackingScalar(4),
        TrackingScalar(5),
    ];
    let b = [
        TrackingScalar(6),
        TrackingScalar(7),
        TrackingScalar(8),
        TrackingScalar(9),
    ];
    let weights = [
        TrackingScalar(10),
        TrackingScalar(11),
        TrackingScalar(12),
        TrackingScalar(13),
    ];
    let u_scalar = TrackingScalar(3);
    let product = tracking_inner_product(&a, &b);
    let p = tracking_msm(
        a.iter()
            .copied()
            .zip(g_bold.iter().copied())
            .chain(
                b.iter()
                    .copied()
                    .zip(weights.iter().copied())
                    .map(|(scalar, weight)| scalar * weight)
                    .zip(h_bold.iter().copied()),
            )
            .chain(core::iter::once((product * u_scalar, generators.g))),
    );
    let prove = || {
        let mut transcript = RecordingProverTranscript::default();
        prove_inner_product::<TrackingSuite, _>(
            generators,
            ScalarVector(weights.to_vec()),
            u_scalar,
            SecretPoint::new(p),
            ScalarVector(a.to_vec()),
            ScalarVector(b.to_vec()),
            &mut transcript,
        )
        .expect("symbolic-H tracking proof");
        transcript.0
    };
    #[cfg(feature = "parallel")]
    let single_thread = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("single-thread Rayon pool")
        .install(&prove);
    #[cfg(not(feature = "parallel"))]
    let single_thread = prove();
    assert!(!single_thread.is_empty());
    #[cfg(feature = "parallel")]
    {
        let four_threads = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .expect("four-thread Rayon pool")
            .install(&prove);
        assert_eq!(single_thread, four_threads);
    }
}
#[test]
fn secret_chunk_fold_clears_successes_after_peer_error() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    let mut chunks = SecretMsmChunkResults::<TrackingSuite>::new(3).expect("fixed chunk capacity");
    let allocation = chunks.values.as_ptr();
    let allocation_capacity = chunks.values.capacity();
    chunks.values.push(Ok(SecretPoint::new(TrackingPoint(11))));
    chunks
        .values
        .push(Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant));
    chunks.values.push(Ok(SecretPoint::new(TrackingPoint(13))));
    assert_eq!(chunks.values.as_ptr(), allocation);
    assert_eq!(chunks.values.capacity(), allocation_capacity);
    assert!(matches!(
        chunks.fold_in_order(),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    // Three constructor parameter slots, the consumed first result owner,
    // replaced accumulator value, sum source, error-path accumulator, and
    // buffered result all clear.
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 8);
}
#[test]
fn secret_point_owner_clears_constructor_scaled_pair_success_and_unwind() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    let point = SecretPoint::new(TrackingPoint(17));
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(point);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 2);
    let source = include_str!("generalized_bulletproof.rs");
    let constructor = source
        .split_once("impl<P: ProofPoint> SecretPoint<P> {")
        .expect("secret point owner")
        .1
        .split_once("impl<P: ProofPoint> Drop for SecretPoint<P>")
        .expect("secret point owner boundary")
        .0;
    assert!(constructor.contains("fn new(mut point: P) -> Self"));
    assert!(constructor.contains("BorrowedSecretPoint::new(&mut point)"));
    assert!(constructor.contains("drop(incoming);"));

    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_ADD_CALLS.store(0, Ordering::SeqCst);
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    let mut p = SecretPoint::new(TrackingPoint(5));
    let left = SecretPoint::new(TrackingPoint(7));
    let right = SecretPoint::new(TrackingPoint(11));
    p.add_scaled_pair_assign(left, TrackingScalar(2), right, TrackingScalar(3));
    assert_eq!(p.expose_ref(), &TrackingPoint(52));
    assert_eq!(POINT_ADD_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 7);
    drop(p);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 8);

    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_ADD_CALLS.store(0, Ordering::SeqCst);
    PANIC_ON_POINT_ADD.store(1, Ordering::SeqCst);
    let mut p = SecretPoint::new(TrackingPoint(13));
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let left = SecretPoint::new(TrackingPoint(17));
        let right = SecretPoint::new(TrackingPoint(19));
        p.add_scaled_pair_assign(left, TrackingScalar(2), right, TrackingScalar(3));
    }));
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    assert!(unwind.is_err());
    assert_eq!(p.expose_ref(), &TrackingPoint(13));
    assert_eq!(POINT_ADD_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 5);
    drop(p);
    assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 6);
}
#[test]
fn secret_builder_unwind_wipes_terms_encodings_tables_and_named_points() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
    POINT_ADD_CALLS.store(0, Ordering::SeqCst);
    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);

    // Exercise the same owner-first encoding transfer directly so the
    // vacated source and retained bytes remain observable before Drop.
    assert_eq!(core::mem::size_of::<SecretScalarEncodings>(), 8192);
    let mut retained = SecretScalarEncodings([[0_u8; 32]; SECRET_MSM_CHUNK_TERMS_V1]);
    let mut source = SecretBytes([0x5a_u8; 32]);
    core::mem::swap(&mut retained.0[1], &mut source.0);
    assert_eq!(source.0, [0_u8; 32]);
    assert_eq!(retained.0[1], [0x5a_u8; 32]);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(source);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(retained);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 257);

    SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
    // Two 16-entry tables require 30 additions. Panic on the first
    // scalar-dependent accumulator addition after its nibble was extracted.
    PANIC_ON_POINT_ADD.store(31, Ordering::SeqCst);
    let mut secret =
        SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
    secret
        .push(&TrackingScalar(0x1234), &TrackingPoint(5))
        .expect("first term");
    secret
        .push(&TrackingScalar(0xabcd), &TrackingPoint(7))
        .expect("second term");
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = secret.evaluate();
    }));
    PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst), 258);
    assert!(POINT_CLEAR_CALLS.load(Ordering::SeqCst) > 40);
}
#[test]
fn inner_product_owner_clears_success_error_length_panic_and_unwind() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let left = ScalarVector(vec![TrackingScalar(2), TrackingScalar(3)]);
    let right = [TrackingScalar(5), TrackingScalar(7)];
    let product = left.inner_product(right.iter());
    assert_eq!(*product.expose_ref(), TrackingScalar(31));
    // The constructor clears its incoming zero slot; the returned owner
    // remains live until the caller explicitly drops it.
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(product);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(left);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let left = ScalarVector(vec![TrackingScalar(11), TrackingScalar(13)]);
    let right = [TrackingScalar(17), TrackingScalar(19)];
    let error = (|| -> Result<(), GeneralizedBulletproofErrorV1> {
        let _product = left.inner_product(right.iter());
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    })();
    assert_eq!(
        error,
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    );
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(left);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let left = ScalarVector(vec![TrackingScalar(23), TrackingScalar(29)]);
    let short = [TrackingScalar(31)];
    let length_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _product = left.inner_product(short.iter());
    }));
    assert!(length_panic.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(left);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let left = ScalarVector(vec![TrackingScalar(37)]);
    let right = [TrackingScalar(41)];
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _product = left.inner_product(right.iter());
        panic!("exercise returned inner-product owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(left);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);
}
#[test]
fn scalar_vector_borrowed_scaled_accumulation_clears_without_copy_or_allocation() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let coefficient = ScalarVector(vec![TrackingScalar(2), TrackingScalar(3)]);
    let coefficient_pointer = coefficient.0.as_ptr();
    let coefficient_capacity = coefficient.0.capacity();
    let mut result = ScalarVector(vec![TrackingScalar(5), TrackingScalar(7)]);
    let result_pointer = result.0.as_ptr();
    let result_capacity = result.0.capacity();
    result.add_scaled_assign(&coefficient, &TrackingScalar(11));
    assert_eq!(
        result.0.as_slice(),
        &[TrackingScalar(27), TrackingScalar(40)]
    );
    assert_eq!(
        coefficient.0.as_slice(),
        &[TrackingScalar(2), TrackingScalar(3)]
    );
    assert_eq!(coefficient.0.as_ptr(), coefficient_pointer);
    assert_eq!(coefficient.0.capacity(), coefficient_capacity);
    assert_eq!(result.0.as_ptr(), result_pointer);
    assert_eq!(result.0.capacity(), result_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(result);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(coefficient);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let length_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let coefficient = ScalarVector(vec![TrackingScalar(13)]);
        let mut result = ScalarVector(vec![TrackingScalar(17), TrackingScalar(19)]);
        result.add_scaled_assign(&coefficient, &TrackingScalar(23));
    }));
    assert!(length_panic.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let post_success_unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let coefficient = ScalarVector(vec![TrackingScalar(29), TrackingScalar(31)]);
        let mut result = ScalarVector(vec![TrackingScalar(37), TrackingScalar(41)]);
        result.add_scaled_assign(&coefficient, &TrackingScalar(43));
        assert_eq!(
            result.0.as_slice(),
            &[TrackingScalar(1284), TrackingScalar(1374)]
        );
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
        panic!("exercise borrowed scaled accumulation owner unwind");
    }));
    assert!(post_success_unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);
}
#[test]
fn scalar_vector_borrowed_product_preallocates_and_clears_every_exit() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let left = ScalarVector(vec![TrackingScalar(2), TrackingScalar(3)]);
    let right = ScalarVector(vec![TrackingScalar(5), TrackingScalar(7)]);
    let left_pointer = left.0.as_ptr();
    let left_capacity = left.0.capacity();
    let right_pointer = right.0.as_ptr();
    let right_capacity = right.0.capacity();
    let product =
        ScalarVector::product_from_borrowed(&left, &right).expect("borrowed elementwise product");
    assert_eq!(
        product.0.as_slice(),
        &[TrackingScalar(10), TrackingScalar(21)]
    );
    assert_eq!(left.0.as_slice(), &[TrackingScalar(2), TrackingScalar(3)]);
    assert_eq!(right.0.as_slice(), &[TrackingScalar(5), TrackingScalar(7)]);
    assert_eq!(left.0.as_ptr(), left_pointer);
    assert_eq!(left.0.capacity(), left_capacity);
    assert_eq!(right.0.as_ptr(), right_pointer);
    assert_eq!(right.0.capacity(), right_capacity);
    assert_ne!(product.0.as_ptr(), left_pointer);
    assert_ne!(product.0.as_ptr(), right_pointer);
    assert_eq!(product.len(), left.len());
    assert!(product.0.capacity() >= product.len());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(product);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(left);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);
    drop(right);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mismatched_left = ScalarVector(vec![TrackingScalar(11)]);
    let mismatched_right = ScalarVector(vec![TrackingScalar(13), TrackingScalar(17)]);
    let left_pointer = mismatched_left.0.as_ptr();
    let right_pointer = mismatched_right.0.as_ptr();
    assert!(matches!(
        ScalarVector::product_from_borrowed(&mismatched_left, &mismatched_right),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(mismatched_left.0.as_ptr(), left_pointer);
    assert_eq!(mismatched_right.0.as_ptr(), right_pointer);
    assert_eq!(mismatched_left.0.as_slice(), &[TrackingScalar(11)]);
    assert_eq!(
        mismatched_right.0.as_slice(),
        &[TrackingScalar(13), TrackingScalar(17)]
    );
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(mismatched_left);
    drop(mismatched_right);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let left = ScalarVector(vec![TrackingScalar(19), TrackingScalar(23)]);
        let right = ScalarVector(vec![TrackingScalar(29), TrackingScalar(31)]);
        let product = ScalarVector::product_from_borrowed(&left, &right)
            .expect("borrowed product unwind fixture");
        assert_eq!(
            product.0.as_slice(),
            &[TrackingScalar(551), TrackingScalar(713)]
        );
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
        panic!("exercise borrowed-product owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);

    let source = include_str!("generalized_bulletproof.rs");
    let production = source
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let borrowed_product = production
        .split_once("fn product_from_borrowed(")
        .expect("borrowed product owner")
        .1
        .split_once("/// Add one borrowed vector multiplied by one borrowed scalar")
        .expect("borrowed product owner boundary")
        .0;
    let mut cursor = 0;
    for step in [
        "if left.len() != right.len()",
        "return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);",
        "let exact_len = left.len();",
        "let mut product = Self(Vec::new());",
        ".try_reserve_exact(exact_len)",
        ".map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;",
        "let allocation_capacity = product.0.capacity();",
        "if allocation_capacity < exact_len",
        "return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);",
        "let allocation_pointer = product.0.as_ptr();",
        "for _ in 0..exact_len",
        "product.0.push(F::ZERO);",
        "for ((output, left), right) in product.0.iter_mut().zip(&left.0).zip(&right.0)",
        "*output = *left;",
        "*output *= *right;",
        "Ok(product)",
    ] {
        let offset = borrowed_product[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing borrowed-product step {step}"));
        cursor += offset + step.len();
    }
    assert_eq!(
        borrowed_product.matches("product.0.push(F::ZERO);").count(),
        1
    );
    assert_eq!(
        borrowed_product
            .matches("debug_assert_eq!(product.0.len(), exact_len);")
            .count(),
        2
    );
    assert_eq!(
        borrowed_product
            .matches("debug_assert_eq!(product.0.capacity(), allocation_capacity);")
            .count(),
        2
    );
    assert_eq!(
        borrowed_product
            .matches("debug_assert_eq!(product.0.as_ptr(), allocation_pointer);")
            .count(),
        2
    );
    for forbidden in [
        ".clone(",
        ".cloned(",
        ".copied(",
        ".to_vec(",
        "Self::zero(",
        "Vec::with_capacity",
        "vec![",
        "resize",
        "copy_from_slice",
        "extend_from_slice",
        "collect",
        "*output = *left * *right;",
        "product.0.push(*left",
        "core::mem",
        "unsafe",
        "callback",
        "FnOnce",
        "FnMut",
    ] {
        assert!(
            !borrowed_product.contains(forbidden),
            "borrowed product path {forbidden}"
        );
    }
    let witness_constructor = production
        .split_once("pub(crate) fn new_with_scalar_commitments(")
        .expect("generalized witness constructor")
        .1
        .split_once("/// One constrainable circuit variable")
        .expect("generalized witness constructor boundary")
        .0;
    let length_check = witness_constructor
        .find("if a_l.len() != a_r.len()")
        .expect("witness input length check");
    let product_call = witness_constructor
        .find("let a_o = ScalarVector::product_from_borrowed(&a_l, &a_r)?;")
        .expect("borrowed witness-output product");
    let aggregate = witness_constructor
        .find("Ok(Self {")
        .expect("completed witness owner");
    assert!(length_check < product_call && product_call < aggregate);
    assert_eq!(
        witness_constructor
            .matches("ScalarVector::product_from_borrowed(&a_l, &a_r)?")
            .count(),
        1
    );
    assert!(!witness_constructor.contains("a_l.clone()"));
    assert!(!witness_constructor.contains("a_r.clone()"));
    assert_eq!(production.matches("product_from_borrowed(").count(), 2);
}
#[test]
fn output_witness_polynomial_rehome_moves_allocation_and_clears_exactly_once() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut witness = ArithmeticCircuitWitness::<TrackingSuite>::new(
        vec![TrackingScalar(2), TrackingScalar(3)],
        vec![TrackingScalar(5), TrackingScalar(7)],
        Vec::new(),
    )
    .expect("bounded output-witness fixture");
    let source_pointer = witness.a_o.0.as_ptr();
    let source_capacity = witness.a_o.0.capacity();
    let io = 2;
    let mut l = vec![ScalarVector(Vec::new()); io + 2];
    assert!(l[io].0.is_empty());
    assert_eq!(l[io].0.capacity(), 0);
    l[io] = core::mem::replace(&mut witness.a_o, ScalarVector(Vec::new()));
    assert!(witness.a_o.0.is_empty());
    assert_eq!(witness.a_o.0.capacity(), 0);
    assert_eq!(
        l[io].0.as_slice(),
        &[TrackingScalar(10), TrackingScalar(21)]
    );
    assert_eq!(l[io].0.as_ptr(), source_pointer);
    assert_eq!(l[io].0.capacity(), source_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(l);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(witness);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut witness = ArithmeticCircuitWitness::<TrackingSuite>::new(
            vec![TrackingScalar(11), TrackingScalar(13)],
            vec![TrackingScalar(17), TrackingScalar(19)],
            Vec::new(),
        )
        .expect("bounded output-witness unwind fixture");
        let source_pointer = witness.a_o.0.as_ptr();
        let source_capacity = witness.a_o.0.capacity();
        let io = 2;
        let mut l = vec![ScalarVector(Vec::new()); io + 2];
        assert!(l[io].0.is_empty());
        assert_eq!(l[io].0.capacity(), 0);
        l[io] = core::mem::replace(&mut witness.a_o, ScalarVector(Vec::new()));
        assert!(witness.a_o.0.is_empty());
        assert_eq!(witness.a_o.0.capacity(), 0);
        assert_eq!(
            l[io].0.as_slice(),
            &[TrackingScalar(187), TrackingScalar(247)]
        );
        assert_eq!(l[io].0.as_ptr(), source_pointer);
        assert_eq!(l[io].0.capacity(), source_capacity);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
        panic!("exercise output-witness polynomial-owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);
}
#[test]
fn right_witness_polynomial_rehome_scales_without_copy_or_allocation() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut witness = ArithmeticCircuitWitness::<TrackingSuite>::new(
        vec![TrackingScalar(2), TrackingScalar(3)],
        vec![TrackingScalar(5), TrackingScalar(7)],
        Vec::new(),
    )
    .expect("bounded right-witness fixture");
    let source_pointer = witness.a_r.0.as_ptr();
    let source_capacity = witness.a_r.0.capacity();
    let y_powers = ScalarVector::powers(TrackingScalar(11), 2);
    let y_pointer = y_powers.0.as_ptr();
    let y_capacity = y_powers.0.capacity();
    let l_weights = ScalarVector(vec![TrackingScalar(17), TrackingScalar(19)]);
    let result_pointer = l_weights.0.as_ptr();
    let result_capacity = l_weights.0.capacity();
    let jlr = 1;
    let mut r = vec![ScalarVector(Vec::new()); 4];
    assert!(r[jlr].0.is_empty());
    assert_eq!(r[jlr].0.capacity(), 0);
    let a_r = core::mem::replace(&mut witness.a_r, ScalarVector(Vec::new()));
    assert!(witness.a_r.0.is_empty());
    assert_eq!(witness.a_r.0.capacity(), 0);
    assert_eq!(a_r.0.as_ptr(), source_pointer);
    assert_eq!(a_r.0.capacity(), source_capacity);
    let scaled_a_r = a_r * &y_powers;
    assert_eq!(scaled_a_r.0.as_ptr(), source_pointer);
    assert_eq!(scaled_a_r.0.capacity(), source_capacity);
    assert_eq!(
        scaled_a_r.0.as_slice(),
        &[TrackingScalar(5), TrackingScalar(77)]
    );
    r[jlr] = l_weights + &scaled_a_r;
    assert_eq!(
        r[jlr].0.as_slice(),
        &[TrackingScalar(22), TrackingScalar(96)]
    );
    assert_eq!(r[jlr].0.as_ptr(), result_pointer);
    assert_eq!(r[jlr].0.capacity(), result_capacity);
    assert_eq!(
        y_powers.0.as_slice(),
        &[TrackingScalar(1), TrackingScalar(11)]
    );
    assert_eq!(y_powers.0.as_ptr(), y_pointer);
    assert_eq!(y_powers.0.capacity(), y_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(scaled_a_r);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(r);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);
    drop(y_powers);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);
    drop(witness);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 10);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut witness = ArithmeticCircuitWitness::<TrackingSuite>::new(
            vec![TrackingScalar(11), TrackingScalar(13)],
            vec![TrackingScalar(17), TrackingScalar(19)],
            Vec::new(),
        )
        .expect("bounded right-witness unwind fixture");
        let source_pointer = witness.a_r.0.as_ptr();
        let source_capacity = witness.a_r.0.capacity();
        let y_powers = ScalarVector::powers(TrackingScalar(23), 2);
        let l_weights = ScalarVector(vec![TrackingScalar(29), TrackingScalar(31)]);
        let result_pointer = l_weights.0.as_ptr();
        let result_capacity = l_weights.0.capacity();
        let jlr = 1;
        let mut r = vec![ScalarVector(Vec::new()); 4];
        let a_r = core::mem::replace(&mut witness.a_r, ScalarVector(Vec::new()));
        assert!(witness.a_r.0.is_empty());
        assert_eq!(witness.a_r.0.capacity(), 0);
        assert_eq!(a_r.0.as_ptr(), source_pointer);
        assert_eq!(a_r.0.capacity(), source_capacity);
        r[jlr] = l_weights + &(a_r * &y_powers);
        assert_eq!(
            r[jlr].0.as_slice(),
            &[TrackingScalar(46), TrackingScalar(468)]
        );
        assert_eq!(r[jlr].0.as_ptr(), result_pointer);
        assert_eq!(r[jlr].0.capacity(), result_capacity);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
        panic!("exercise right-witness polynomial-owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 10);
}
#[test]
fn scalar_vector_borrowed_scaled_accumulation_source_boundary() {
    let source = include_str!("generalized_bulletproof.rs");
    let production = source
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let accumulation = production
        .split_once("fn add_scaled_assign(&mut self, coefficient: &Self, scalar: &F) {")
        .expect("borrowed scaled accumulation")
        .1
        .split_once("/// Compute an inner product")
        .expect("borrowed scaled accumulation boundary")
        .0;
    let length_check = accumulation
        .find("assert_eq!(self.len(), coefficient.len());")
        .expect("borrowed scaled accumulation length check");
    let coordinate_loop = accumulation
        .find("for (result, coefficient) in self.0.iter_mut().zip(&coefficient.0)")
        .expect("ordered borrowed coordinate accumulation");
    let coordinate_update = accumulation
        .find("*result += *coefficient * *scalar;")
        .expect("borrowed scaled coordinate update");
    assert!(length_check < coordinate_loop && coordinate_loop < coordinate_update);
    for forbidden in [
        ".clone(",
        ".cloned(",
        ".copied(",
        ".to_vec(",
        "Vec::",
        "vec![",
        "reserve",
        "collect",
        "copy_from_slice",
        "extend_from_slice",
        "core::mem",
        "unsafe",
        "callback",
        "FnOnce",
        "FnMut",
    ] {
        assert!(
            !accumulation.contains(forbidden),
            "borrowed scaled accumulation path {forbidden}"
        );
    }

    let prover = production
        .split_once("pub fn prove<R, T>(")
        .expect("generalized prover")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("generalized prover boundary")
        .0;
    let polynomial_evaluation = prover
        .split_once("let x = ScalarVector::powers(transcript.challenge()?, t_poly_len);")
        .expect("polynomial evaluation challenge")
        .1
        .split_once("let mut tau_ni = SecretScalar::new(S::Scalar::ZERO);")
        .expect("polynomial evaluation boundary")
        .0;
    assert_eq!(
        polynomial_evaluation
            .matches("ScalarVector::zero(n)")
            .count(),
        1
    );
    assert_eq!(
        polynomial_evaluation
            .matches("result.add_scaled_assign(coefficient, &x[index]);")
            .count(),
        1
    );
    let evaluate = polynomial_evaluation
        .find("let evaluate = |polynomial: &[ScalarVector<S::Scalar>]| {")
        .expect("borrowed polynomial evaluation closure");
    let result_owner = polynomial_evaluation
        .find("let mut result = ScalarVector::zero(n);")
        .expect("polynomial result owner");
    let coefficient_loop = polynomial_evaluation
        .find("for (index, coefficient) in polynomial.iter().enumerate()")
        .expect("ordered polynomial coefficient loop");
    let accumulation_call = polynomial_evaluation
        .find("result.add_scaled_assign(coefficient, &x[index]);")
        .expect("borrowed polynomial accumulation call");
    let l_eval = polynomial_evaluation
        .find("let l_eval = evaluate(&l);")
        .expect("left polynomial evaluation");
    let r_eval = polynomial_evaluation
        .find("let r_eval = evaluate(&r);")
        .expect("right polynomial evaluation");
    let drop_l = polynomial_evaluation
        .find("drop(l);")
        .expect("left polynomial owner drop");
    let drop_r = polynomial_evaluation
        .find("drop(r);")
        .expect("right polynomial owner drop");
    let t_caret = polynomial_evaluation
        .find("let t_caret = l_eval.inner_product(r_eval.0.iter());")
        .expect("evaluated polynomial inner product");
    assert!(evaluate < result_owner);
    assert!(result_owner < coefficient_loop && coefficient_loop < accumulation_call);
    assert!(accumulation_call < l_eval && l_eval < r_eval);
    assert!(r_eval < drop_l && drop_l < drop_r && drop_r < t_caret);
    for forbidden in [
        "coefficient.clone()",
        ".cloned(",
        ".copied(",
        ".to_vec(",
        "Vec::",
        "vec![",
        "reserve",
        "collect",
        "copy_from_slice",
        "extend_from_slice",
        "core::mem",
        "unsafe",
        "callback",
        "FnOnce",
        "FnMut",
    ] {
        assert!(
            !polynomial_evaluation.contains(forbidden),
            "borrowed polynomial evaluation path {forbidden}"
        );
    }
    assert_eq!(production.matches(".add_scaled_assign(").count(), 1);
    assert_eq!(production.matches("coefficient.clone()").count(), 0);
    assert_eq!(production.matches(".clone()").count(), 0);
    let borrowed_product = "let a_o = ScalarVector::product_from_borrowed(&a_l, &a_r)?;";
    assert_eq!(production.matches(borrowed_product).count(), 1);
    assert!(!production.contains("let a_o = a_l.clone() * &a_r;"));
    assert!(!production.contains("witness.a_r.clone()"));
    let left_scale = "let scaled_r_weights = r_weights * &y_inverse;";
    let left_handoff = "let a_l = core::mem::replace(&mut witness.a_l, ScalarVector(Vec::new()));";
    let left_product = "l[ilr] = a_l + &scaled_r_weights;";
    let left_release = "drop(scaled_r_weights);";
    for step in [left_scale, left_handoff, left_product, left_release] {
        assert_eq!(prover.matches(step).count(), 1);
    }
    assert_eq!(prover.matches("witness.a_l").count(), 6);
    let left_shape = prover
        .find("if witness.a_l.len() > n")
        .expect("left-witness shape check");
    let left_pair_shape = prover
        .find("|| witness.a_l.len() != witness.a_r.len()")
        .expect("left/right witness shape check");
    let left_padding = prover
        .find("witness.a_l.pad_with_zeroes(n)?;")
        .expect("left-witness padding owner");
    let left_constraint = prover
        .find("*evaluation.expose_mut() += witness.a_l[*index] * *weight;")
        .expect("left-witness constraint read");
    let left_commitment = prover
        .find("for (scalar, point) in witness.a_l.0.iter().zip(self.generators.g_bold)")
        .expect("left-witness AI commitment read");
    let left_commitment_evaluation = prover[left_commitment..]
        .find("terms.evaluate()?")
        .map(|position| left_commitment + position)
        .expect("left-witness AI commitment evaluation");
    let left_scale_index = prover.find(left_scale).expect("left-weight scaling");
    let left_handoff_index = prover.find(left_handoff).expect("left-witness handoff");
    let left_product_index = prover.find(left_product).expect("left polynomial product");
    let left_release_index = prover.find(left_release).expect("scaled-left release");
    let left_rehome_region = prover
        .split_once(left_scale)
        .expect("left-witness rehome start")
        .1
        .split_once("l[io] =")
        .expect("left-witness rehome end")
        .0;
    assert_eq!(left_rehome_region.matches("Vec::new()").count(), 1);
    assert_eq!(left_rehome_region.matches("core::mem::replace").count(), 1);
    for forbidden in [
        ".clone(",
        ".cloned(",
        ".copied(",
        ".to_vec(",
        "Vec::with_capacity",
        "vec![",
        "reserve",
        "collect",
        "copy_from_slice",
        "extend_from_slice",
        "unsafe",
        "callback",
        "FnOnce",
        "FnMut",
    ] {
        assert!(
            !left_rehome_region.contains(forbidden),
            "left-witness rehome path {forbidden}"
        );
    }
    let output_handoff = "l[io] = core::mem::replace(&mut witness.a_o, ScalarVector(Vec::new()));";
    assert_eq!(prover.matches(output_handoff).count(), 1);
    assert!(!prover.contains("l[io] = witness.a_o.clone();"));
    assert_eq!(prover.matches("witness.a_o").count(), 4);
    let output_padding = prover
        .find("witness.a_o.pad_with_zeroes(n)?;")
        .expect("output-wire padding owner");
    let output_constraint = prover
        .find("*evaluation.expose_mut() += witness.a_o[*index] * *weight;")
        .expect("output-wire constraint read");
    let output_commitment = prover
        .find("for (scalar, point) in witness.a_o.0.iter().zip(self.generators.g_bold)")
        .expect("output-wire commitment read");
    let output_commitment_evaluation = prover[output_commitment..]
        .find("terms.evaluate()?")
        .map(|position| output_commitment + position)
        .expect("output-wire commitment evaluation");
    let left_polynomial_allocation = prover
        .find("let mut l = vec![ScalarVector(Vec::new()); is + 1];")
        .expect("left polynomial owner allocation");
    let output_handoff_index = prover
        .find(output_handoff)
        .expect("output-wire polynomial-owner handoff");
    let left_randomness = prover
        .find("l[is] = s_l;")
        .expect("left polynomial randomness owner");
    let polynomial_product = prover
        .find("let t_poly_len")
        .expect("polynomial product boundary");
    let left_drop = prover.find("drop(l);").expect("left polynomial owner drop");
    let witness_drop = prover
        .find("drop(witness);")
        .expect("emptied witness owner drop");
    let right_handoff = "let a_r = core::mem::replace(&mut witness.a_r, ScalarVector(Vec::new()));";
    let right_product = "r[jlr] = l_weights + &(a_r * &y_powers);";
    assert_eq!(prover.matches(right_handoff).count(), 1);
    assert_eq!(prover.matches(right_product).count(), 1);
    assert_eq!(prover.matches("witness.a_r").count(), 5);
    let right_shape = prover
        .find("|| witness.a_l.len() != witness.a_r.len()")
        .expect("right-witness shape check");
    let right_padding = prover
        .find("witness.a_r.pad_with_zeroes(n)?;")
        .expect("right-witness padding owner");
    let right_constraint = prover
        .find("*evaluation.expose_mut() += witness.a_r[*index] * *weight;")
        .expect("right-witness constraint read");
    let right_commitment = prover
        .find("for (scalar, point) in witness.a_r.0.iter().zip(self.generators.h_bold)")
        .expect("right-witness AI commitment read");
    let right_commitment_evaluation = prover[right_commitment..]
        .find("terms.evaluate()?")
        .map(|position| right_commitment + position)
        .expect("right-witness AI commitment evaluation");
    let right_polynomial_allocation = prover
        .find("let mut r = vec![ScalarVector(Vec::new()); is + 1];")
        .expect("right polynomial owner allocation");
    let right_handoff_index = prover
        .find(right_handoff)
        .expect("right-witness polynomial-owner handoff");
    let right_product_index = prover
        .find(right_product)
        .expect("right-witness polynomial product");
    let right_drop = prover
        .find("drop(r);")
        .expect("right polynomial owner drop");
    let right_handoff_region = prover
        .split_once("l[is] = s_l;")
        .expect("right-witness handoff region start")
        .1
        .split_once("r[jo] = o_weights - &y_powers;")
        .expect("right-witness handoff region end")
        .0;
    assert_eq!(right_handoff_region.matches("Vec::new()").count(), 1);
    assert_eq!(
        right_handoff_region.matches("core::mem::replace").count(),
        1
    );
    for forbidden in [
        ".clone(",
        ".cloned(",
        ".copied(",
        ".to_vec(",
        "Vec::with_capacity",
        "vec![",
        "reserve",
        "collect",
        "copy_from_slice",
        "extend_from_slice",
        "unsafe",
        "callback",
        "FnOnce",
        "FnMut",
    ] {
        assert!(
            !right_handoff_region.contains(forbidden),
            "right-witness handoff path {forbidden}"
        );
    }
    assert!(output_padding < output_constraint && output_constraint < output_commitment);
    assert!(output_commitment < output_commitment_evaluation);
    assert!(output_commitment_evaluation < left_polynomial_allocation);
    assert!(left_polynomial_allocation < output_handoff_index);
    assert!(output_handoff_index < left_randomness);
    assert!(left_randomness < polynomial_product);
    assert!(polynomial_product < left_drop && left_drop < witness_drop);
    assert!(!prover[output_handoff_index + output_handoff.len()..].contains("witness.a_o"));
    assert!(left_shape < left_pair_shape && left_pair_shape < left_padding);
    assert!(left_padding < left_constraint && left_constraint < left_commitment);
    assert!(left_commitment < left_commitment_evaluation);
    assert!(left_commitment_evaluation < left_polynomial_allocation);
    assert!(left_polynomial_allocation < left_scale_index);
    assert!(left_scale_index < left_handoff_index && left_handoff_index < left_product_index);
    assert!(left_product_index < left_release_index && left_release_index < output_handoff_index);
    assert!(!prover[left_handoff_index + left_handoff.len()..].contains("witness.a_l"));
    assert!(right_shape < right_padding && right_padding < right_constraint);
    assert!(right_constraint < right_commitment);
    assert!(right_commitment < right_commitment_evaluation);
    assert!(right_commitment_evaluation < right_polynomial_allocation);
    assert!(right_polynomial_allocation < right_handoff_index);
    assert!(right_handoff_index < right_product_index);
    assert!(right_product_index < right_drop && right_drop < witness_drop);
    assert!(!prover[right_handoff_index + right_handoff.len()..].contains("witness.a_r"));
}
