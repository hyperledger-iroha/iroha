//! Non-authorizing native40 ordinary-product and masked-quotient arithmetic.
//!
//! The parent declares this adapter only under `cfg(test)`. It uses the shared
//! constant-time transform; it does not promote the old MKHE NTT test oracle.
//! TODO: bind actual original aggregated K/R/E/M/C and the retained S replay to
//! this arithmetic through the source owner. Raw slices establish no source,
//! transcript, proof, key authority, or production correspondence.
//!
//! Every call charges the original ledger before allocating its five-N-word
//! buffers. The six borrowed input slices are additionally charged for this
//! call's lifetime. If their owner already charged them this is conservative
//! overlap, never a credit or a second budget. Retained output is three N words.
//! Work counts field add/sub/mul calls, public exponentiation calls expanded
//! into their actual field products, and coefficient checks. It is not a CPU
//! instruction, whole-prover work, provider-buffer, or process-RSS claim.

use core::mem::size_of;
use iroha_fhe::{constant_time::FixedModulus, ntt::cyclic_product_in_place_with};
use zeroize::{Zeroize, Zeroizing};

use super::{
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
        ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1,
    },
    rns_native_resource_budget::{
        RnsNativeProofResourceBudgetV1, RnsNativeResourceErrorV1, RnsNativeResourceReservationV1,
    },
};
use crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1;

/// Refusal of an arithmetic candidate, without any authority transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RelationPolynomialErrorV1 {
    /// Wrong native coordinate, length or transform parameters.
    Geometry,
    /// A source residue is noncanonical or S has nonzero top coefficient.
    Source,
    /// The unmasked ordinary-product relation does not hold.
    Relation,
    /// Fallible allocation did not provide the exact named storage.
    Allocation,
    /// The original work or workspace ledger refused admission.
    Resource(RnsNativeResourceErrorV1),
}

impl From<RnsNativeResourceErrorV1> for RelationPolynomialErrorV1 {
    fn from(value: RnsNativeResourceErrorV1) -> Self {
        Self::Resource(value)
    }
}

/// Aggregated coefficient inputs; these borrows supply no authentication.
#[derive(Clone, Copy)]
pub(super) struct RelationPolynomialInputsV1<'a> {
    /// B + beta*A.
    pub(super) key: &'a [u64],
    /// Sum gamma^j*r_j.
    pub(super) randomizer: &'a [u64],
    /// Sum gamma^j*(e0_j + beta*e1_j).
    pub(super) error: &'a [u64],
    /// Sum gamma^j times the centered message residue.
    pub(super) message: &'a [u64],
    /// Sum gamma^j*(C0_j + beta*C1_j).
    pub(super) ciphertext: &'a [u64],
    /// Original S of degree at most N-2; no entropy is drawn here.
    pub(super) mask: &'a [u64],
}

impl<'a> RelationPolynomialInputsV1<'a> {
    fn slices_v1(self) -> [&'a [u64]; 6] {
        [
            self.key,
            self.randomizer,
            self.error,
            self.message,
            self.ciphertext,
            self.mask,
        ]
    }
}

#[derive(Clone, Copy)]
struct GeometryV1 {
    degree: usize,
    modulus: u64,
    root: u64,
}

struct SecretWordsV1(Zeroizing<Vec<u64>>);

impl SecretWordsV1 {
    fn zeroed_v1(len: usize) -> Result<Self, RelationPolynomialErrorV1> {
        let injected = ALLOCATION_FAILURE_V1.with(|remaining| match remaining.get() {
            Some(0) => {
                remaining.set(None);
                true
            }
            Some(value) => {
                remaining.set(Some(value - 1));
                false
            }
            None => false,
        });
        if injected {
            return Err(RelationPolynomialErrorV1::Allocation);
        }
        let mut values = Zeroizing::new(Vec::new());
        values
            .try_reserve_exact(len)
            .map_err(|_| RelationPolynomialErrorV1::Allocation)?;
        if values.capacity() != len {
            return Err(RelationPolynomialErrorV1::Allocation);
        }
        values.resize(len, 0);
        Ok(Self(values))
    }
}

impl Drop for SecretWordsV1 {
    fn drop(&mut self) {
        self.0.as_mut_slice().zeroize();
        // Test-only observation after clearing, before the backing allocation is
        // released. The disabled observer allocates no storage.
        CLEAR_AUDIT_V1.with(|audit| {
            if let Some(events) = audit.borrow_mut().as_mut() {
                events.push((self.0.len(), self.0.iter().all(|value| *value == 0)));
            }
        });
    }
}

/// Clearing outputs and their original retained-workspace reservation.
pub(super) struct RelationPolynomialsV1 {
    product: SecretWordsV1,
    quotient: SecretWordsV1,
    // Drop order is significant: both payloads clear before their credit ends.
    reservation: RnsNativeResourceReservationV1,
}

struct ScratchV1<'a> {
    right: SecretWordsV1,
    inputs: RelationPolynomialInputsV1<'a>,
    arithmetic: FixedModulus,
    words: Zeroizing<[u64; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AdmissionV1 {
    work: u64,
    retained: u64,
    scratch: u64,
}

fn power_products_v1(exponent: u64) -> u64 {
    u64::from(u64::BITS - exponent.leading_zeros()) + u64::from(exponent.count_ones())
}

fn admission_v1(geometry: GeometryV1) -> Result<AdmissionV1, RelationPolynomialErrorV1> {
    let n = u64::try_from(geometry.degree).map_err(|_| RelationPolynomialErrorV1::Geometry)?;
    let length = n
        .checked_mul(2)
        .ok_or(RelationPolynomialErrorV1::Geometry)?;
    if n < 2
        || !n.is_power_of_two()
        || n > ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64
        || geometry.modulus < 3
        || geometry.modulus >= 1 << 63
        || geometry.modulus.is_multiple_of(2)
        || geometry.root >= geometry.modulus
    {
        return Err(RelationPolynomialErrorV1::Geometry);
    }
    let log = u64::from(length.ilog2());
    // Per transform: two products plus add/sub for each butterfly, and
    // stage powers 2^k, k=0..log-1, each costing k+2 field products.
    let transform = 2 * length * log + log * (log + 3) / 2;
    let work = 3 * transform
        + 2 * length // pointwise product and inverse normalization
        + 5 * n // p*E, three low-relation operations and H+S
        + 2 * power_products_v1(geometry.modulus - 2) // inverse root and length
        + power_products_v1(length) + power_products_v1(n) // root order checks
        + 64 // public plaintext modulus: 32 byte-wise products and additions
        + 7 * n + 2; // six canonical-input scans, relation scan and two top checks
    Ok(AdmissionV1 {
        work,
        retained: 3 * n * 8 + size_of::<RelationPolynomialsV1>() as u64,
        scratch: 8 * n * 8 + size_of::<ScratchV1<'_>>() as u64,
    })
}

/// Construct one native40 row; success grants only clearing coefficient owners.
pub(super) fn construct_native_relation_polynomials_v1(
    resources: &mut RnsNativeProofResourceBudgetV1,
    limb: usize,
    repetition: usize,
    inputs: RelationPolynomialInputsV1<'_>,
) -> Result<RelationPolynomialsV1, RelationPolynomialErrorV1> {
    if repetition >= usize::from(ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1) {
        return Err(RelationPolynomialErrorV1::Geometry);
    }
    let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .get(limb)
        .ok_or(RelationPolynomialErrorV1::Geometry)?;
    let root = *ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1
        .get(limb)
        .ok_or(RelationPolynomialErrorV1::Geometry)?;
    construct_with_geometry_v1(
        resources,
        GeometryV1 {
            degree: ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
            modulus,
            root,
        },
        inputs,
    )
}

fn construct_with_geometry_v1(
    resources: &mut RnsNativeProofResourceBudgetV1,
    geometry: GeometryV1,
    inputs: RelationPolynomialInputsV1<'_>,
) -> Result<RelationPolynomialsV1, RelationPolynomialErrorV1> {
    let admission = admission_v1(geometry)?;
    if inputs
        .slices_v1()
        .iter()
        .any(|input| input.len() != geometry.degree)
    {
        return Err(RelationPolynomialErrorV1::Geometry);
    }
    // Declared first so every later secret allocation clears before this drops.
    let mut reservation = resources.admit(admission.work, admission.retained, admission.scratch)?;
    let arithmetic =
        FixedModulus::derive(geometry.modulus).ok_or(RelationPolynomialErrorV1::Geometry)?;
    let n = geometry.degree;
    if arithmetic.power(geometry.root, (2 * n) as u64) != 1
        || arithmetic.power(geometry.root, n as u64) != geometry.modulus - 1
    {
        return Err(RelationPolynomialErrorV1::Geometry);
    }
    let mut bad = u64::from(inputs.mask[n - 1] != 0);
    for input in inputs.slices_v1() {
        for &value in input {
            bad |= u64::from(value >= geometry.modulus);
        }
    }
    if bad != 0 {
        return Err(RelationPolynomialErrorV1::Source);
    }
    let inverse_root = arithmetic.power(geometry.root, geometry.modulus - 2);
    let inverse_degree = arithmetic.power((2 * n) as u64, geometry.modulus - 2);
    let mut plain_modulus = 0;
    for byte in VEGA_T256_SCALAR_MODULUS_BE_V1 {
        plain_modulus = arithmetic.add_canonical(
            arithmetic.multiply(plain_modulus, 256 % geometry.modulus),
            u64::from(byte) % geometry.modulus,
        );
    }
    let mut product = SecretWordsV1::zeroed_v1(2 * n)?;
    let mut quotient = SecretWordsV1::zeroed_v1(n)?;
    let mut scratch = ScratchV1 {
        right: SecretWordsV1::zeroed_v1(2 * n)?,
        inputs,
        arithmetic,
        words: Zeroizing::new([0; 4]),
    };
    fault_point_v1(FaultV1::Allocated);
    product.0[..n].copy_from_slice(inputs.key);
    scratch.right.0[..n].copy_from_slice(inputs.randomizer);
    cyclic_product_in_place_with(
        &mut product.0,
        &mut scratch.right.0,
        &scratch.arithmetic,
        geometry.root,
        inverse_root,
        inverse_degree,
    )
    .ok_or(RelationPolynomialErrorV1::Geometry)?;
    fault_point_v1(FaultV1::Product);
    scratch.words[3] = u64::from(product.0[2 * n - 1] != 0);
    for i in 0..n {
        scratch.words[0] = scratch
            .arithmetic
            .multiply(plain_modulus, scratch.inputs.error[i]);
        scratch.words[1] = scratch
            .arithmetic
            .add_canonical(product.0[i], scratch.words[0]);
        scratch.words[1] = scratch
            .arithmetic
            .add_canonical(scratch.words[1], scratch.inputs.message[i]);
        scratch.words[1] = scratch
            .arithmetic
            .sub_canonical(scratch.words[1], scratch.inputs.ciphertext[i]);
        scratch.words[3] |= u64::from(scratch.words[1] != product.0[n + i]);
        scratch.words[2] = scratch
            .arithmetic
            .add_canonical(product.0[n + i], scratch.inputs.mask[i]);
        quotient.0[i] = scratch.words[2];
        product.0[i] = scratch.words[2];
        product.0[n + i] = scratch.words[2];
    }
    if scratch.words[3] != 0 {
        return Err(RelationPolynomialErrorV1::Relation);
    }
    drop(scratch);
    reservation.release_scratch();
    Ok(RelationPolynomialsV1 {
        product,
        quotient,
        reservation,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FaultV1 {
    Allocated,
    Product,
}

thread_local! {
    static CLEAR_AUDIT_V1: std::cell::RefCell<Option<Vec<(usize, bool)>>> = const { std::cell::RefCell::new(None) };
    static FAULT_V1: std::cell::Cell<Option<FaultV1>> = const { std::cell::Cell::new(None) };
    static ALLOCATION_FAILURE_V1: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

fn fault_point_v1(point: FaultV1) {
    FAULT_V1.with(|fault| {
        if fault.get() == Some(point) {
            fault.set(None);
            panic!("injected relation coefficient unwind");
        }
    });
}

#[path = "rns_native_relation_polynomials_tests.rs"]
mod tests;
