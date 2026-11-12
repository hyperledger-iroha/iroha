//! Shared recursive layout requirements for Receive, Send and real Bootstrap.
//!
//! A common target combines each phase's maximum, not only the largest natural
//! row count. The caller must rebuild every kind at this target and check the
//! actual layouts: placement/alignment and a larger sumcheck tail can require
//! another sizing pass. Oversize authentication must never select a weaker
//! Bootstrap relation. Spare rows become explicit fixed padding at construction
//! time: they contain known constants and are omitted through the verification
//! key, never by deleting commitments from an already constructed proof.

use crate::{
    fs::Scalar,
    r1cs::Circuit,
    uniform::{Layout, UniformCircuit},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirements {
    pub layout: Layout,
    pub second_phase: usize,
    /// Alignment of the ordinary constant region, excluding the wallet row.
    pub fixed_alignment: usize,
    pub wallet_rows: usize,
    pub col_bits: usize,
    pub io_len: usize,
    pub sumcheck_variables: usize,
}

impl Requirements {
    pub fn of<F: Scalar>(circuit: &UniformCircuit<F>) -> Self {
        let mut fixed_alignment = 1;
        for (a, array) in circuit.arrays.iter().enumerate().filter(|(_, a)| a.fixed) {
            let m = circuit.pi[array.family] + array.rho;
            for k in 0..array.cells() {
                if !circuit.cell_placed(a, k) {
                    continue;
                }
                // A placed block maps low instance bits to columns and the
                // remaining bits to consecutive rows. Include its full padded
                // extent, even if its final instances are absent. Moving the
                // constant region must preserve this power-of-two alignment.
                let first = circuit.cell_index(a, k, 0) >> circuit.shape.col_bits;
                let last = circuit.cell_index(a, k, (1usize << m) - 1) >> circuit.shape.col_bits;
                let height = last.checked_sub(first).expect("ordered constant block") + 1;
                assert!(
                    height.is_power_of_two(),
                    "constant block has aligned row height"
                );
                fixed_alignment = fixed_alignment.max(height);
            }
        }
        let layout = circuit.layout();
        let wallet_rows = circuit.wallet_rows();
        assert!(
            layout.fixed >= wallet_rows,
            "wallet rows belong to the fixed region"
        );
        assert_eq!(
            (layout.fixed - wallet_rows) % fixed_alignment,
            0,
            "constant-region alignment"
        );
        Self {
            layout,
            second_phase: circuit.regions().1,
            fixed_alignment,
            wallet_rows,
            col_bits: circuit.shape.col_bits,
            io_len: circuit.io_len,
            sumcheck_variables: circuit.shape.k,
        }
    }
}

/// Initial common target for one side of the recursive pair. P and S have
/// different column/IO profiles and must be planned separately.
pub fn common<F: Scalar>(circuits: &[&UniformCircuit<F>]) -> Layout {
    common_requirements(
        &circuits
            .iter()
            .map(|c| Requirements::of(c))
            .collect::<Vec<_>>(),
    )
}

pub fn common_requirements(requirements: &[Requirements]) -> Layout {
    let first = requirements.first().expect("at least one circuit kind");
    for r in requirements {
        assert_eq!(
            (r.col_bits, r.io_len, r.sumcheck_variables, r.wallet_rows),
            (
                first.col_bits,
                first.io_len,
                first.sumcheck_variables,
                first.wallet_rows
            ),
            "kinds share columns, early IO length, sumcheck grouping and wallet-row policy",
        );
        assert!(
            r.fixed_alignment.is_power_of_two(),
            "power-of-two constant alignment"
        );
        assert!(r.layout.fixed >= r.wallet_rows, "valid fixed region");
        assert_eq!((r.layout.fixed - r.wallet_rows) % r.fixed_alignment, 0);
    }
    let max = |f: fn(&Requirements) -> usize| requirements.iter().map(f).max().unwrap();
    let sx = max(|r| r.layout.sx);
    let p1 = max(|r| r.layout.p1);
    let pl = max(|r| r.layout.pl);
    let second_phase = max(|r| r.second_phase);
    let alignment = max(|r| r.fixed_alignment);
    let constants = max(|r| r.layout.fixed - r.wallet_rows);
    let minimum_fixed = constants.div_ceil(alignment) * alignment + first.wallet_rows;
    let phase_rows = p1 + pl + second_phase;
    let required_rows = phase_rows + minimum_fixed;
    let row_bits = max(|r| r.layout.row_bits)
        .max(required_rows.max(4).next_power_of_two().trailing_zeros() as usize);
    // Every kind reserves the same three phase bands. The remaining whole,
    // aligned rows may be VK constants. The builder rechecks placement, fills
    // each extra fixed row with [1, 0, ...], and excludes it from row blinding.
    // Leave any sub-alignment remainder in the witness region.
    let room = (1usize << row_bits) - phase_rows - first.wallet_rows;
    let fixed = (room / alignment) * alignment + first.wallet_rows;
    assert!(fixed >= minimum_fixed, "all natural constants still fit");
    Layout {
        sx,
        row_bits,
        p1,
        pl,
        fixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fs::SpikeRng,
        ipa::Gens,
        spartan,
        uniform::{Array, Copies, constant, own},
    };
    use ff::Field;
    use iroha_pasta::{Eq as Vesta, Fp};

    fn req(p1: usize, pl: usize, p2: usize, fixed: usize, alignment: usize) -> Requirements {
        Requirements {
            layout: Layout {
                sx: 19,
                row_bits: 6,
                p1,
                pl,
                fixed,
            },
            second_phase: p2,
            fixed_alignment: alignment,
            wallet_rows: 1,
            col_bits: 14,
            io_len: 64,
            sumcheck_variables: 3,
        }
    }

    #[test]
    fn phase_union_can_outgrow_each_natural_layout() {
        let receive = req(35, 5, 11, 3, 1);
        let authentication = req(28, 0, 27, 3, 2);
        let shared = common_requirements(&[receive, authentication]);
        assert_eq!(
            shared,
            Layout {
                sx: 19,
                row_bits: 7,
                p1: 35,
                pl: 5,
                fixed: 61
            }
        );
        assert_eq!(common_requirements(&[authentication, receive]), shared);
    }

    #[test]
    fn constant_padding_preserves_the_tallest_block_alignment() {
        let ordinary = req(12, 1, 8, 6, 1); // five ordinary constant rows
        let tall = req(13, 1, 7, 5, 4); // four rows aligned as one block
        let common = common_requirements(&[ordinary, tall]);
        assert_eq!(common.fixed, 41); // forty aligned constants plus the wallet
        assert_eq!(common.p1, 13);
        assert_eq!(common.pl, 1);
        assert_eq!((common.fixed - 1) % 4, 0);
        assert!(common.p1 + common.pl + 8 + common.fixed <= 64);
        assert!(common.p1 + common.pl + 8 + common.fixed + 4 > 64);
    }

    #[test]
    fn real_fixed_block_height_is_recovered_from_placement() {
        let arrays = vec![
            Array {
                name: "one constrained witness",
                family: 0,
                rho: 0,
                r_count: 1,
                fixed: false,
                lookups: vec![],
                widths: vec![255],
                rows: vec![[own(0, 1), constant(vec![Fp::ONE]), constant(vec![Fp::ONE])]],
                values: vec![vec![Fp::ONE]],
            },
            Array {
                name: "tall constant block",
                family: 0,
                rho: 12,
                r_count: 4096,
                fixed: true,
                lookups: vec![],
                widths: vec![255],
                rows: vec![],
                values: vec![vec![Fp::ONE; 4096]],
            },
        ];
        let (circuit, _, _) = UniformCircuit::build(
            "alignment fixture",
            arrays,
            vec![0],
            vec![1],
            vec![],
            10,
            4,
            b"alignment fixture",
        );
        let requirements = Requirements::of(&circuit);
        assert!(requirements.fixed_alignment >= 8);
        assert_eq!(requirements.wallet_rows, 0);
        let shared = common(&[&circuit, &circuit]);
        assert_eq!(shared.row_bits, circuit.shape.row_bits);
        assert!(shared.fixed >= circuit.layout().fixed);
        assert_eq!(shared.fixed % requirements.fixed_alignment, 0);
    }

    #[test]
    fn fixed_padding_saves_commitments_and_rejects_a_changed_omitted_row() {
        let build = |target| {
            UniformCircuit::build_with_copies_at(
                "fixed padding proof",
                vec![Array {
                    name: "one constrained witness",
                    family: 0,
                    rho: 0,
                    r_count: 1,
                    fixed: false,
                    lookups: vec![],
                    widths: vec![255],
                    rows: vec![
                        [own(0, 1), constant(vec![Fp::ONE]), constant(vec![Fp::ONE])],
                        [own(0, 1), own(0, 1), constant(vec![Fp::ONE])],
                    ],
                    values: vec![vec![Fp::ONE]],
                }],
                vec![0],
                vec![1],
                vec![],
                Copies::default(),
                10,
                4,
                b"fixed padding proof",
                target,
            )
        };
        let (natural, ni, nw) = build(None);
        let target = common(&[&natural]);
        let (padded, pi, pw) = build(Some(target));
        let added = padded.layout().fixed - natural.layout().fixed;
        assert!(added > 0);
        assert_eq!(padded.layout(), target);
        assert_eq!(padded.shape.rows(), natural.shape.rows());
        assert_eq!(
            padded.phase1_rows(&padded.shape),
            natural.phase1_rows(&natural.shape)
        );
        let start = (padded.shape.rows() - padded.layout().fixed) * padded.shape.cols();
        let known = padded.fixed_values(&padded.shape);
        assert_eq!(&pw.w[start..], known.as_slice());
        for row in known.chunks_exact(padded.shape.cols()) {
            assert_eq!(row[0], Fp::ONE);
            assert!(row[1..].iter().all(|v| *v == Fp::ZERO));
        }
        let mut filled = pw.w.clone();
        let _ = padded.instantiate(
            &padded.shape,
            &[Fp::from(31), Fp::from(47)],
            Some(&mut filled),
        );
        assert_eq!(
            &filled[start..],
            known.as_slice(),
            "no phase-two helper changes a fixed row"
        );
        let gens = Gens::<Vesta>::new(padded.shape.cols());
        let (natural_proof, _) = spartan::prove_circuit(
            &natural.shape,
            &gens,
            &ni,
            &nw,
            &mut SpikeRng::new(b"natural padding comparison"),
            &natural,
        );
        let (proof, _) = spartan::prove_circuit(
            &padded.shape,
            &gens,
            &pi,
            &pw,
            &mut SpikeRng::new(b"fixed padding comparison"),
            &padded,
        );
        assert_eq!(
            natural_proof.to_bytes().len() - proof.to_bytes().len(),
            32 * added
        );
        assert!(
            spartan::verify_circuit(&padded.shape, &gens, &pi, &proof.to_bytes(), &padded).is_ok()
        );
        let mut changed = crate::r1cs::Witness { w: pw.w.clone() };
        changed.w[start] += Fp::ONE;
        let (forged, _) = spartan::prove_circuit(
            &padded.shape,
            &gens,
            &pi,
            &changed,
            &mut SpikeRng::new(b"changed omitted row"),
            &padded,
        );
        assert!(
            spartan::verify_circuit(&padded.shape, &gens, &pi, &forged.to_bytes(), &padded)
                .is_err()
        );
    }
}
