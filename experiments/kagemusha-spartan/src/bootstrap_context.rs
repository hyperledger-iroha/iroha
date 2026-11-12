//! Compact authenticated context shared by the research P proof kinds.
//!
//! Bootstrap binds these public words to its authenticated credential and
//! trusted scheme/relation inputs. Send and Receive inherit them from the
//! scalar verifier's opened IO for predecessor slot zero. This helper does not
//! authenticate an arbitrary input array, validate the credential, or supply
//! monetary transitions. The owning relation must provide those guarantees.

use ff::Field;
use iroha_pasta::Fp;

use crate::{
    full::Wiring,
    uniform::{Array, Lin, Src, constant, link, plus, scaled},
};

pub const CONTEXT_IO: usize = 16;
pub const FIELDS: usize = 13;
pub const SCHEME: usize = 0;
pub const RELATION: usize = 2;
pub const ASSET: usize = 4;
pub const WALLET: usize = 6;
pub const CREDENTIAL: usize = 8;
pub const PAYMENT_KEY: usize = 9;

/// Reserved dynamic lineage output block. Only real transition relations may
/// expose these claims; inheriting immutable context does not propagate them.
pub const LINEAGE_IO: usize = 32;
pub const LINEAGE_FIELDS: usize = 6;
pub const LIFECYCLE: usize = 0;
pub const POLICY_EPOCH: usize = 1;
pub const ENABLED_CONTROLS: usize = 2;
pub const BURNED_TOTAL: usize = 3;
pub const PENDING_ROOT: usize = 4;
pub const CREDIT_ROOT: usize = 5;

fn expose_at<const N: usize>(
    w: &mut Wiring<Fp>,
    family: usize,
    offset: usize,
    values: &[Fp; N],
    sources: &[Lin<Fp>; N],
) -> usize {
    assert_eq!((w.pi[family], w.counts[family]), (0, 1), "global context");
    // A source expression is read in a new, empty global binding array. Reject
    // local-cell references rather than silently interpreting them in that array.
    for source in sources {
        for term in source {
            assert_eq!(term.coef.len(), 1, "global source coefficient");
            match &term.src {
                Src::One => {}
                Src::Link { array, map, .. } => {
                    assert_eq!(w.arrays[*array].family, family, "same-family source");
                    assert_eq!(map.len(), 1, "one global source instance");
                    assert!(map[0].is_some(), "context source cannot be disabled");
                }
                _ => panic!("context sources must be explicit cell links or constants"),
            }
        }
    }
    let slots = N.next_power_of_two();
    assert_eq!(offset % slots, 0, "aligned context IO block");
    let mut public = vec![Fp::ZERO; slots];
    public[..N].copy_from_slice(values);
    let io = w.push(Array {
        name: "authenticated public context",
        family,
        rho: slots.trailing_zeros() as usize,
        r_count: N,
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        rows: vec![],
        values: vec![public],
    });
    w.io_arrays.push((io, offset));
    let rows = sources
        .iter()
        .enumerate()
        .map(|(i, source)| {
            [
                plus(
                    source,
                    &scaled(&link(io, 0, vec![Some(i)], Fp::ONE), -Fp::ONE),
                ),
                constant(vec![Fp::ONE]),
                vec![],
            ]
        })
        .collect();
    w.push(Array {
        name: "authenticated public context binding",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        rows,
        values: vec![],
    });
    io
}

/// Expose scheme/relation/asset/wallet (two little-endian 128-bit limbs each),
/// credential digest (one canonical Fp), and payment-key x/y (four numeric
/// 128-bit halves, x-low/x-high/y-low/y-high), at IO slots 16 through 28.
///
/// Every output has a mandatory equality to the supplied constrained source.
/// Bootstrap must source the credential claims from its authenticated byte
/// tape, and scheme/relation from the verifier's expected context. The caller
/// constrains source widths, canonical encodings and nonzero requirements.
pub fn expose(
    w: &mut Wiring<Fp>,
    family: usize,
    values: &[Fp; FIELDS],
    sources: &[Lin<Fp>; FIELDS],
) -> usize {
    expose_at(w, family, CONTEXT_IO, values, sources)
}

/// Copy immutable context from the opened IO of the *verified predecessor*
/// scalar proof. The owning Send/Receive must pass slot zero, never a payer's
/// proof or a host-populated array lacking verification constraints.
///
/// The original early inputs are the leaves in the second half of the scalar
/// verifier's IO-fold array, as in `full::predecessor_head_tie`. Changing any
/// output while retaining that verified predecessor makes the relation false.
pub fn inherit(
    w: &mut Wiring<Fp>,
    family: usize,
    values: &[Fp; FIELDS],
    verified_predecessor_io: usize,
) -> usize {
    let array = &w.arrays[verified_predecessor_io];
    assert_eq!(array.cells(), 1, "scalar IO-fold cell");
    assert!(array.rho >= 1, "IO-fold tree contains leaves");
    let leaves = (1usize << array.rho) / 2;
    assert!(
        CONTEXT_IO + FIELDS <= leaves,
        "predecessor context fits IO leaves"
    );
    assert!(
        leaves + CONTEXT_IO + FIELDS <= array.r_count,
        "present IO leaves"
    );
    let sources = std::array::from_fn(|i| {
        link(
            verified_predecessor_io,
            0,
            vec![Some(leaves + CONTEXT_IO + i)],
            Fp::ONE,
        )
    });
    expose(w, family, values, &sources)
}

/// Expose lifecycle, policy epoch, enabled controls, burned total, pending root
/// and credit root at IO slots 32 through 37. Bootstrap supplies its constrained
/// initial-state values. Synthetic Send/Receive relations must not use this to
/// assert unimplemented monetary state transitions or copy dynamic state.
pub fn expose_lineage(
    w: &mut Wiring<Fp>,
    family: usize,
    values: &[Fp; LINEAGE_FIELDS],
    sources: &[Lin<Fp>; LINEAGE_FIELDS],
) -> usize {
    expose_at(w, family, LINEAGE_IO, values, sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        r1cs::{Circuit, Instance, Witness},
        uniform::UniformCircuit,
    };

    type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>);

    fn values(seed: u64) -> [Fp; FIELDS] {
        std::array::from_fn(|i| Fp::from(seed + i as u64))
    }
    fn build(predecessor: [Fp; FIELDS], own_context: [Fp; FIELDS]) -> Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        // This isolated fixture tests the equality boundary only. The full
        // recursive circuit supplies an IO-fold array constrained by its verifier.
        let mut leaves = vec![Fp::ZERO; 128];
        leaves[64 + CONTEXT_IO..64 + CONTEXT_IO + FIELDS].copy_from_slice(&predecessor);
        let verified = w.push(Array {
            name: "verified predecessor IO fixture",
            family,
            rho: 7,
            r_count: 128,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![leaves],
        });
        inherit(&mut w, family, &own_context, verified);
        let (arrays, pi, counts, tables, copies) = w.finish();
        UniformCircuit::build_with_copies(
            "context continuity",
            arrays,
            pi,
            counts,
            tables,
            copies,
            10,
            64,
            b"context continuity",
        )
    }
    fn holds(b: &Built) -> bool {
        let (circuit, instance, witness) = b;
        let mut w = witness.w.clone();
        let relation =
            circuit.instantiate(&circuit.shape, &[Fp::from(31), Fp::from(47)], Some(&mut w));
        let z = crate::r1cs::z_vector(&circuit.shape, instance, &Witness { w });
        let (a, b, c) = relation.abc(&circuit.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }

    #[test]
    fn every_context_word_is_bound_to_the_predecessor() {
        let old = values(100);
        let mut built = build(old, old);
        assert!(holds(&built));
        for i in 0..FIELDS {
            built.1.io[CONTEXT_IO + i] += Fp::ONE;
            assert!(!holds(&built), "changed context word {i}");
            built.1.io[CONTEXT_IO + i] -= Fp::ONE;
            let mut replacement = old;
            replacement[i] += Fp::ONE;
            assert!(
                !holds(&build(old, replacement)),
                "rebuilt wrong context word {i}"
            );
        }
        assert!(
            !holds(&build(old, values(200))),
            "payer context cannot replace predecessor context"
        );
    }

    #[test]
    fn context_values_do_not_select_the_constraint_matrix() {
        let a = build(values(100), values(100));
        let b = build(values(200), values(200));
        assert!(holds(&a) && holds(&b));
        assert_eq!(a.0.layout(), b.0.layout());
        assert_eq!(
            a.0.entries(&[Fp::from(31), Fp::from(47)]),
            b.0.entries(&[Fp::from(31), Fp::from(47)])
        );
    }

    #[test]
    fn lineage_outputs_require_their_actual_sources() {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let values = [
            Fp::ONE,
            Fp::ZERO,
            Fp::ZERO,
            Fp::ZERO,
            Fp::from(47),
            Fp::from(59),
        ];
        let sources = values.map(|value| constant(vec![value]));
        expose_lineage(&mut w, family, &values, &sources);
        let (arrays, pi, counts, tables, copies) = w.finish();
        let mut b = UniformCircuit::build_with_copies(
            "initial lineage",
            arrays,
            pi,
            counts,
            tables,
            copies,
            10,
            64,
            b"initial lineage",
        );
        assert!(holds(&b));
        for i in 0..LINEAGE_FIELDS {
            b.1.io[LINEAGE_IO + i] += Fp::ONE;
            assert!(!holds(&b), "changed initial lineage word {i}");
            b.1.io[LINEAGE_IO + i] -= Fp::ONE;
        }
    }
}
