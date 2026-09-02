//! Paired field-native Poseidon commitments for Offline Cash V1.
//!
//! The two Pasta scalar fields use the same reviewed width-3/rate-2 Pow5 construction and
//! independently generated field constants. A state or replay root is authoritative only as the
//! pair authenticated by both recursive proofs. Canonical field encodings are little-endian.

use ff::PrimeField;
use halo2_base::{
    AssignedValue, Context,
    gates::{RangeChip, RangeInstructions as _},
    poseidon::hasher::{PoseidonHasher, spec::OptimizedPoseidonSpec},
    utils::{BigPrimeField, ScalarField},
};
use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use iroha_data_model::offline::{
    OfflineCashPastaStateCommitmentV1, offline_cash_pasta_state_commitment_v1,
};
use snark_verifier::{loader::native::LOADER, util::hash::Poseidon};

/// Width of the fixed Offline Cash V1 native Poseidon permutation.
pub(crate) const OFFLINE_CASH_POSEIDON_WIDTH_V1: usize = 3;
/// Sponge rate of the fixed Offline Cash V1 native Poseidon permutation.
pub(crate) const OFFLINE_CASH_POSEIDON_RATE_V1: usize = 2;
/// Full rounds of the 128-bit-security Pow5 parameterization.
pub(crate) const OFFLINE_CASH_POSEIDON_FULL_ROUNDS_V1: usize = 8;
/// Partial rounds of the 128-bit-security Pow5 parameterization.
pub(crate) const OFFLINE_CASH_POSEIDON_PARTIAL_ROUNDS_V1: usize = 57;
/// Deterministic secure-MDS search selector.
pub(crate) const OFFLINE_CASH_POSEIDON_SECURE_MDS_V1: usize = 0;
/// Native fixed-list arity kept inline on the replay-tree hot path.
///
/// Keeping the domain, arity, and inputs in this stack buffer avoids one heap allocation for
/// every sparse-Merkle node without changing the Poseidon transcript.
const OFFLINE_CASH_POSEIDON_INLINE_ARITY_V1: usize = 32;

/// Empty consumed-credit leaf domain.
pub(crate) const OFFLINE_CASH_REPLAY_EMPTY_DOMAIN_V1: u64 = u64::from_le_bytes(*b"ocempty1");
/// Present consumed-credit leaf domain.
pub(crate) const OFFLINE_CASH_REPLAY_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"ocleaf_1");
/// Consumed-credit internal-node domain.
pub(crate) const OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1: u64 = u64::from_le_bytes(*b"ocnode_1");
/// Aggregate private-state commitment domain.
pub(crate) const OFFLINE_CASH_STATE_DOMAIN_V1: u64 = u64::from_le_bytes(*b"ocstate1");
/// Low limb of the canonical Pallas base/Vesta scalar field modulus.
pub(crate) const OFFLINE_CASH_FP_MODULUS_LOW_V1: u128 = 0x2246_98fc_094c_f91b_992d_30ed_0000_0001;
/// Low limb of the canonical Vesta base/Pallas scalar field modulus.
pub(crate) const OFFLINE_CASH_FQ_MODULUS_LOW_V1: u128 = 0x2246_98fc_0994_a8dd_8c46_eb21_0000_0001;

type Spec<F> =
    OptimizedPoseidonSpec<F, OFFLINE_CASH_POSEIDON_WIDTH_V1, OFFLINE_CASH_POSEIDON_RATE_V1>;
type Native<F> = Poseidon<F, F, OFFLINE_CASH_POSEIDON_WIDTH_V1, OFFLINE_CASH_POSEIDON_RATE_V1>;

fn fp_spec() -> &'static Spec<Fp> {
    static SPEC: std::sync::OnceLock<Spec<Fp>> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| {
        Spec::new::<
            OFFLINE_CASH_POSEIDON_FULL_ROUNDS_V1,
            OFFLINE_CASH_POSEIDON_PARTIAL_ROUNDS_V1,
            OFFLINE_CASH_POSEIDON_SECURE_MDS_V1,
        >()
    })
}

fn fq_spec() -> &'static Spec<Fq> {
    static SPEC: std::sync::OnceLock<Spec<Fq>> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| {
        Spec::new::<
            OFFLINE_CASH_POSEIDON_FULL_ROUNDS_V1,
            OFFLINE_CASH_POSEIDON_PARTIAL_ROUNDS_V1,
            OFFLINE_CASH_POSEIDON_SECURE_MDS_V1,
        >()
    })
}

std::thread_local! {
    static FP_HASHER: std::cell::RefCell<Native<Fp>> =
        std::cell::RefCell::new(Native::from_spec(&*LOADER, fp_spec().clone()));
    static FQ_HASHER: std::cell::RefCell<Native<Fq>> =
        std::cell::RefCell::new(Native::from_spec(&*LOADER, fq_spec().clone()));
}

/// Field abstraction shared by host replay/state code and the two native circuits.
pub(crate) trait OfflineCashPoseidonFieldV1:
    snark_verifier::util::arithmetic::FieldExt
    + PrimeField
    + From<u64>
    + ScalarField
    + BigPrimeField
    + Sized
    + 'static
{
    /// True for the Eq/Fp half and false for the Ep/Fq half.
    const IS_EQ_PARITY: bool;
    /// Borrow the exact generated field specification.
    fn offline_cash_poseidon_spec_v1() -> &'static Spec<Self>;
    /// Execute with the thread-local native sponge.
    fn with_offline_cash_poseidon_v1<R>(callback: impl FnOnce(&mut Native<Self>) -> R) -> R;
    /// Select this parity's canonical component from a paired state/replay commitment.
    fn select_component(components: OfflineCashPastaStateCommitmentV1) -> [u8; 32];
}

/// Circuit-side sponge using exactly the native Offline Cash V1 parameters.
///
/// All protocol hashes prepend the fixed domain and exact arity. The helper is deliberately
/// field-generic so Eq/Fp and Ep/Fq execute an identical constraint schedule while using their
/// own field constants.
pub(crate) struct OfflineCashPoseidonChipV1<F: OfflineCashPoseidonFieldV1> {
    hasher: PoseidonHasher<F, OFFLINE_CASH_POSEIDON_WIDTH_V1, OFFLINE_CASH_POSEIDON_RATE_V1>,
}

impl<F: OfflineCashPoseidonFieldV1> OfflineCashPoseidonChipV1<F> {
    /// Construct the unique circuit sponge for this parity.
    pub(crate) fn new(ctx: &mut Context<F>, range: &RangeChip<F>) -> Self {
        let mut hasher = PoseidonHasher::new(F::offline_cash_poseidon_spec_v1().clone());
        hasher.initialize_consts(ctx, range.gate());
        Self { hasher }
    }

    /// Hash a fixed list with the same domain-and-arity prefix as [`hash`].
    pub(crate) fn hash(
        &self,
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        domain: u64,
        inputs: &[AssignedValue<F>],
    ) -> AssignedValue<F> {
        let mut cells = Vec::with_capacity(inputs.len() + 2);
        cells.push(ctx.load_constant(F::from(domain)));
        cells.push(ctx.load_constant(F::from(
            u64::try_from(inputs.len()).expect("bounded Poseidon arity fits u64"),
        )));
        cells.extend_from_slice(inputs);
        self.hasher.hash_fix_len_array(ctx, range.gate(), &cells)
    }
}

impl OfflineCashPoseidonFieldV1 for Fp {
    const IS_EQ_PARITY: bool = true;
    fn offline_cash_poseidon_spec_v1() -> &'static Spec<Self> {
        fp_spec()
    }

    fn with_offline_cash_poseidon_v1<R>(callback: impl FnOnce(&mut Native<Self>) -> R) -> R {
        FP_HASHER.with(|hasher| callback(&mut hasher.borrow_mut()))
    }

    fn select_component(components: OfflineCashPastaStateCommitmentV1) -> [u8; 32] {
        components.eq
    }
}

impl OfflineCashPoseidonFieldV1 for Fq {
    const IS_EQ_PARITY: bool = false;
    fn offline_cash_poseidon_spec_v1() -> &'static Spec<Self> {
        fq_spec()
    }

    fn with_offline_cash_poseidon_v1<R>(callback: impl FnOnce(&mut Native<Self>) -> R) -> R {
        FQ_HASHER.with(|hasher| callback(&mut hasher.borrow_mut()))
    }

    fn select_component(components: OfflineCashPastaStateCommitmentV1) -> [u8; 32] {
        components.ep
    }
}

/// Hash a fixed semantic list with explicit domain and arity words.
pub(crate) fn hash<F: OfflineCashPoseidonFieldV1>(domain: u64, inputs: &[F]) -> F {
    let mut inline = [F::ZERO; OFFLINE_CASH_POSEIDON_INLINE_ARITY_V1 + 2];
    let mut allocated = Vec::new();
    let preimage = if inputs.len() <= OFFLINE_CASH_POSEIDON_INLINE_ARITY_V1 {
        &mut inline[..inputs.len() + 2]
    } else {
        allocated.resize(inputs.len() + 2, F::ZERO);
        allocated.as_mut_slice()
    };
    preimage[0] = F::from(domain);
    preimage[1] = F::from(u64::try_from(inputs.len()).expect("bounded Poseidon arity fits u64"));
    preimage[2..].copy_from_slice(inputs);
    F::with_offline_cash_poseidon_v1(|hasher| {
        hasher.clear();
        hasher.update(preimage);
        hasher.squeeze()
    })
}

/// Return the protocol-fixed root of the empty depth-256 consumed-credit tree.
pub(crate) fn empty_replay_root<F: OfflineCashPoseidonFieldV1>() -> F {
    let mut root = hash(OFFLINE_CASH_REPLAY_EMPTY_DOMAIN_V1, &[]);
    for _ in 0..256 {
        root = hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &[root, root]);
    }
    root
}

/// Inject one unsigned 128-bit limb into either Pasta scalar field.
pub(crate) fn from_u128<F: PrimeField + From<u64>>(value: u128) -> F {
    F::from(value as u64) + F::from((value >> 64) as u64) * F::from_u128(1_u128 << 64)
}

/// Split a 256-bit protocol digest into two injective 128-bit field limbs.
pub(crate) fn digest_limbs<F: PrimeField + From<u64>>(digest: [u8; 32]) -> [F; 2] {
    [
        from_u128(u128::from_le_bytes(
            digest[..16].try_into().expect("fixed digest half"),
        )),
        from_u128(u128::from_le_bytes(
            digest[16..].try_into().expect("fixed digest half"),
        )),
    ]
}

/// Encode one native scalar using the sole canonical little-endian representation.
pub(crate) fn encode<F: PrimeField>(value: F) -> [u8; 32] {
    value
        .to_repr()
        .as_ref()
        .try_into()
        .expect("Pasta scalar representations are 32 bytes")
}

/// Strictly decode one canonical scalar without modular reduction.
pub(crate) fn decode<F: PrimeField>(bytes: [u8; 32]) -> Option<F> {
    let mut repr = F::Repr::default();
    repr.as_mut().copy_from_slice(&bytes);
    Option::from(F::from_repr(repr))
}

/// Build the paired components and their sole public 32-byte wire head.
pub(crate) fn paired_commitment(eq: Fp, ep: Fq) -> (OfflineCashPastaStateCommitmentV1, [u8; 32]) {
    let components = OfflineCashPastaStateCommitmentV1 {
        eq: encode(eq),
        ep: encode(ep),
    };
    let head = offline_cash_pasta_state_commitment_v1(components);
    (components, head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allocated_reference_hash<F: OfflineCashPoseidonFieldV1>(domain: u64, inputs: &[F]) -> F {
        let mut preimage = Vec::with_capacity(inputs.len() + 2);
        preimage.push(F::from(domain));
        preimage.push(F::from(
            u64::try_from(inputs.len()).expect("test arity fits u64"),
        ));
        preimage.extend_from_slice(inputs);
        F::with_offline_cash_poseidon_v1(|hasher| {
            hasher.clear();
            hasher.update(&preimage);
            hasher.squeeze()
        })
    }

    #[test]
    fn inline_native_preimage_matches_allocated_transcript() {
        for arity in [0_usize, 2, 4, 23, OFFLINE_CASH_POSEIDON_INLINE_ARITY_V1 + 1] {
            let fp_inputs = (0..arity)
                .map(|value| Fp::from(u64::try_from(value + 1).expect("test value fits")))
                .collect::<Vec<_>>();
            let fq_inputs = (0..arity)
                .map(|value| Fq::from(u64::try_from(value + 1).expect("test value fits")))
                .collect::<Vec<_>>();
            assert_eq!(
                hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &fp_inputs),
                allocated_reference_hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &fp_inputs),
            );
            assert_eq!(
                hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &fq_inputs),
                allocated_reference_hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &fq_inputs),
            );
        }
    }

    #[test]
    fn parity_domains_are_deterministic_and_non_aliasing() {
        let inputs_fp = [Fp::from(7), Fp::from(9)];
        let inputs_fq = [Fq::from(7), Fq::from(9)];
        let first_fp = hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &inputs_fp);
        let first_fq = hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &inputs_fq);
        assert_eq!(
            first_fp,
            hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &inputs_fp)
        );
        assert_eq!(
            first_fq,
            hash(OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, &inputs_fq)
        );
        assert_ne!(encode(first_fp), encode(first_fq));
        assert_ne!(
            first_fp,
            hash(OFFLINE_CASH_REPLAY_LEAF_DOMAIN_V1, &inputs_fp)
        );
    }

    #[test]
    fn paired_wire_head_binds_both_components() {
        let (components, head) = paired_commitment(Fp::from(3), Fq::from(5));
        assert_eq!(head, offline_cash_pasta_state_commitment_v1(components));
        let mut substituted = components;
        substituted.ep = encode(Fq::from(6));
        assert_ne!(head, offline_cash_pasta_state_commitment_v1(substituted));
    }

    #[test]
    fn empty_replay_root_is_deterministic_and_parity_native() {
        assert_eq!(empty_replay_root::<Fp>(), empty_replay_root::<Fp>());
        assert_eq!(empty_replay_root::<Fq>(), empty_replay_root::<Fq>());
        assert_ne!(
            encode(empty_replay_root::<Fp>()),
            encode(empty_replay_root::<Fq>())
        );
    }
}
