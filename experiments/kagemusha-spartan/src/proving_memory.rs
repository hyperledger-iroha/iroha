//! Allocation accounting for the constructed prover, without constructing its
//! lazy plans. This is an admission estimate, not a whole-process RSS bound:
//! allocator fragmentation, opaque closure captures, matrix compiler scratch, other circuits, shared
//! generator tables and the MSM kernel's separately enforced budget are extra.

use super::*;
use core::mem::size_of;

#[path = "memory_plan_counts.rs"]
mod memory_plan_counts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvingMemoryAccounting {
    pub construction_bytes: u64,
    pub symbolic_bytes: u64,
    pub gather_bytes: u64,
    pub readers_bytes: u64,
    pub m_plan_bytes: u64,
    pub matrix_program_bytes: u64,
    /// Accounted heap storage already owned by this circuit.
    pub retained_bytes: u64,
    /// Largest additional phase allocation. The caller's witness and all
    /// already retained circuit plans are excluded: an OS availability query
    /// has already charged them. Shared MSM scratch is accounted separately.
    pub additional_peak_bytes: u64,
    pub accounted_peak_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_plans<F: Scalar>(c: UniformCircuit<F>) {
        let counts = c.memory_plan_counts();
        let before = c.proving_memory_accounting();
        let phases = c.phase_memory_accounting();
        let w = 1u64 << c.shape.sw();
        let x = 1u64 << c.shape.sx;
        assert!(phases.preparation_bytes >= (2 * w + 3 * x) * size_of::<F>() as u64);
        assert!(c.symbolic.get().is_none() && c.gather.get().is_none());
        assert!(c.readers.get().is_none() && c.m_plans.get().is_none());
        let _ = c.symbolic();
        let _ = c.gather_plan();
        let prepared = c.proving_memory_accounting();
        let prepared_phase = c.phase_memory_accounting();
        assert!(
            counts.symbolic >= prepared.symbolic_bytes as u128,
            "symbolic projection {} < materialized {}",
            counts.symbolic,
            prepared.symbolic_bytes
        );
        assert!(
            counts.gather >= prepared.gather_bytes as u128,
            "gather projection {} < materialized {}",
            counts.gather,
            prepared.gather_bytes
        );
        assert!(
            phases.preparation_bytes
                >= prepared_phase.preparation_bytes + prepared.retained_bytes
                    - before.retained_bytes
        );
        let _ = c.readers();
        let _ = c.m_plans();
        assert_eq!(
            counts.edge_count,
            c.readers()
                .at
                .iter()
                .flatten()
                .map(|(_, pairs)| pairs.len() as u128)
                .sum::<u128>(),
            "exact CSR edges"
        );
        assert_eq!(
            counts.entry_count,
            c.m_plans()
                .iter()
                .map(|p| p.entries.len() as u128)
                .sum::<u128>(),
            "exact present nonzero M entries"
        );
        assert_eq!(
            counts.coefficient_count,
            c.m_plans()
                .iter()
                .map(|p| p.coefs.len() as u128)
                .sum::<u128>(),
            "exact non-unit M coefficients"
        );
        let planned = c.proving_memory_accounting();
        assert!(
            counts.readers >= planned.readers_bytes as u128,
            "CSR projection {} < materialized {}",
            counts.readers,
            planned.readers_bytes
        );
        assert!(
            counts.m_plans >= planned.m_plan_bytes as u128,
            "M projection {} < materialized {}",
            counts.m_plans,
            planned.m_plan_bytes
        );
        assert_eq!(
            prepared_phase.preparation_bytes,
            c.preparation_memory_bytes(),
            "preparation never charges readers or M plans, cached or absent"
        );
        assert_eq!(
            prepared_phase.phase1_bytes,
            c.phase_memory_accounting().phase1_bytes,
            "prefix commitments never charge matrix plans"
        );
        assert!(
            planned.retained_bytes - before.retained_bytes
                <= before.additional_peak_bytes - planned.additional_peak_bytes
        );
    }

    fn check<F: Scalar + iroha_pasta::poseidon::PoseidonField>() {
        let (arrays, pi, _) = rp57::<F>(2, b"memory mapped rows");
        let counts = pi.iter().map(|p| 1 << p).collect();
        let (c, _, _) = UniformCircuit::build(
            "memory mapped rows",
            arrays,
            pi,
            counts,
            Vec::new(),
            11,
            4,
            b"memory mapped witness",
        );
        check_plans(c);
        let target = Array {
            name: "memory sparse target",
            family: 0,
            rho: 5,
            r_count: 32,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![vec![F::ONE; 32]],
        };
        let readers = Array {
            name: "memory absent sparse readers",
            family: 1,
            rho: 2,
            r_count: 3,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![[
                vec![Term {
                    src: Src::Link {
                        array: 0,
                        cell: 0,
                        map: vec![Some(4), Some(4), None, Some(31)],
                    },
                    coef: vec![F::ZERO, -F::ONE, F::from(7), F::from(9)],
                }],
                own(0, 4),
                constant(vec![F::ZERO; 4]),
            ]],
            values: vec![vec![F::ZERO; 16]],
        };
        let (c, _, _) = UniformCircuit::build(
            "memory sparse edges",
            vec![target, readers],
            vec![0, 2],
            vec![1, 3],
            Vec::new(),
            10,
            4,
            b"memory sparse witness",
        );
        check_plans(c);
        let (c, _, _) = crate::spartan::phase1_tests::phased_fixture::<F>(15);
        check_plans(c);
    }

    #[test]
    fn phase_memory_covers_materialized_plans_on_both_curves() {
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }
}

/// Allocation lifetimes of the actual native paths. Every value excludes the
/// existing caller witness and retained circuit storage. Missing plans are
/// counted only in phases that use them. Opaque fill closures, allocator
/// overhead and the separately bounded MSM kernel are not included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseMemoryAccounting {
    pub helper_bytes: u64,
    pub preparation_bytes: u64,
    pub phase1_bytes: u64,
    pub outer_bytes: u64,
    pub matrix_bytes: u64,
    pub inner_bytes: u64,
    pub opening_bytes: u64,
    pub additional_peak_bytes: u64,
}

fn bytes<T>(v: &Vec<T>) -> u128 {
    v.capacity() as u128 * size_of::<T>() as u128
}
fn nested<T>(v: &Vec<Vec<T>>) -> u128 {
    bytes(v) + v.iter().map(bytes).sum::<u128>()
}
fn term_heap<F>(t: &Term<F>) -> u128 {
    bytes(&t.coef)
        + match &t.src {
            Src::Link { map, .. } => bytes(map),
            _ => 0,
        }
}
fn lin_heap<F>(v: &Lin<F>) -> u128 {
    bytes(v) + v.iter().map(term_heap).sum::<u128>()
}
fn tuple_heap<F>(v: &Vec<Lin<F>>) -> u128 {
    bytes(v) + v.iter().map(lin_heap).sum::<u128>()
}
fn symbolic_heap<F>(rows: &SymbolicRows<F>) -> u128 {
    bytes(rows)
        + rows
            .iter()
            .map(|rs| {
                bytes(rs)
                    + rs.iter()
                        .map(|r| {
                            r.iter()
                                .map(|lin| {
                                    bytes(lin) + lin.iter().map(|(t, _)| term_heap(t)).sum::<u128>()
                                })
                                .sum::<u128>()
                        })
                        .sum::<u128>()
            })
            .sum::<u128>()
}
fn cap(x: u128) -> u64 {
    x.min(u64::MAX as u128) as u64
}

impl<F: Scalar> UniformCircuit<F> {
    pub fn proving_memory_accounting(&self) -> ProvingMemoryAccounting {
        let mut retained = size_of::<Self>() as u128 + bytes(&self.arrays);
        for a in &self.arrays {
            retained += bytes(&a.widths) + nested(&a.values) + bytes(&a.rows) + bytes(&a.lookups);
            for r in &a.rows {
                for lin in r {
                    retained += lin_heap(lin);
                }
            }
            for (_, tuple) in &a.lookups {
                retained += tuple_heap(tuple);
            }
        }
        retained += bytes(&self.pi)
            + bytes(&self.counts)
            + bytes(&self.product_masks)
            + nested(&self.row_base)
            + nested(&self.blocks)
            + bytes(&self.bounds)
            + self.fixed_values.heap_bytes()
            + nested(&self.phase2)
            + bytes(&self.lookup_arrays)
            + bytes(&self.tables)
            + bytes(&self.ch_rows)
            + bytes(&self.io_arrays)
            + bytes(&self.late_io)
            + bytes(&self.exports)
            + bytes(&self.copy_exports)
            + bytes(&self.copy_imports)
            + bytes(&self.index_arrays)
            + bytes(&self.link_tables)
            + bytes(&self.link_io)
            + bytes(&self.rfield)
            + nested(&self.moved)
            + bytes(&self.split_of);
        for (_, _, row) in &self.ch_rows {
            for lin in row {
                retained += bytes(lin);
                for (t, _) in lin {
                    retained += term_heap(t);
                }
            }
        }
        for (_, _, _, tuple) in &self.exports {
            retained += tuple_heap(tuple);
        }
        for (_, tuple) in &self.copy_exports {
            retained += tuple_heap(tuple);
        }
        for (_, tuple, from) in &self.copy_imports {
            retained += tuple_heap(tuple) + bytes(from);
        }
        if let Some((v, _)) = &self.link_binding {
            retained += bytes(v);
        }
        // Include hash-table bucket storage conservatively, independent of the
        // implementation's load factor, plus each separately allocated value.
        retained += (self.lookup_mask.capacity() as u128 + self.lookup_mask_col.capacity() as u128)
            * 2
            * 64;
        retained += self.lookup_mask.values().map(bytes).sum::<u128>();

        let construction_bytes = retained;
        if let Some(rows) = self.symbolic.get() {
            retained += symbolic_heap(rows);
        }
        let symbolic_bytes = retained - construction_bytes;
        let start = retained;
        if let Some(jobs) = self.gather.get() {
            retained += bytes(jobs);
            for j in jobs {
                retained += bytes(&j.targets) + bytes(&j.rs) + bytes(&j.factors) + bytes(&j.sums);
            }
        }
        let gather_bytes = retained - start;
        let start = retained;
        if let Some(rd) = self.readers.get() {
            retained += bytes(&rd.readers)
                + bytes(&rd.at)
                + bytes(&rd.all_readers)
                + bytes(&rd.one_readers);
            for row in &rd.readers {
                retained += nested(row);
            }
            for row in &rd.all_readers {
                retained += nested(row);
            }
            for row in &rd.at {
                retained += bytes(row);
                for (offsets, pairs) in row {
                    retained += bytes(offsets) + bytes(pairs);
                }
            }
        }
        let readers_bytes = retained - start;
        let start = retained;
        if let Some(plans) = self.m_plans.get() {
            retained += bytes(plans);
            for p in plans {
                retained +=
                    bytes(&p.offsets) + bytes(&p.entries) + bytes(&p.coefs) + bytes(&p.readers);
            }
        }
        let m_plan_bytes = retained - start;
        let start = retained;
        if let Some(t) = self.plan.get() {
            retained += bytes(&t.ops) + bytes(&t.pool) + bytes(&t.regs.inputs) + bytes(&t.regs.ops);
        }
        let matrix_program_bytes = retained - start;

        let additional = self.phase_memory_accounting().additional_peak_bytes as u128;
        ProvingMemoryAccounting {
            construction_bytes: cap(construction_bytes),
            symbolic_bytes: cap(symbolic_bytes),
            gather_bytes: cap(gather_bytes),
            readers_bytes: cap(readers_bytes),
            m_plan_bytes: cap(m_plan_bytes),
            matrix_program_bytes: cap(matrix_program_bytes),
            retained_bytes: cap(retained),
            additional_peak_bytes: cap(additional),
            accounted_peak_bytes: cap(retained.saturating_add(additional)),
        }
    }

    /// Workspace for `instantiate(..., Some(w))`, then `z_vector` and `abc`.
    /// This path does not build a reader, M-table or matrix-evaluation tape.
    pub fn preparation_memory_bytes(&self) -> u64 {
        self.phase_memory_accounting().preparation_bytes
    }

    /// Count phase workspaces without building lazy plans or changing a witness.
    pub fn phase_memory_accounting(&self) -> PhaseMemoryAccounting {
        let p = self.memory_plan_counts();
        let field = size_of::<F>() as u128;
        let w = 1u128
            .checked_shl(self.shape.sw() as u32)
            .unwrap_or(u128::MAX);
        let x = 1u128.checked_shl(self.shape.sx as u32).unwrap_or(u128::MAX);
        let cols = 1u128
            .checked_shl(self.shape.col_bits as u32)
            .unwrap_or(u128::MAX);
        let symbolic = if self.symbolic.get().is_none() {
            p.symbolic
        } else {
            0
        };
        let symbolic_scratch = if self.symbolic.get().is_none() {
            p.symbolic_build_scratch
        } else {
            0
        };
        let gather = if self.gather.get().is_none() {
            p.gather
        } else {
            0
        };
        let readers = if self.readers.get().is_none() {
            p.readers
        } else {
            0
        };
        let reader_scratch = if self.readers.get().is_none() {
            p.readers_build_scratch
        } else {
            0
        };
        let m_plans = if self.m_plans.get().is_none() {
            p.m_plans
        } else {
            0
        };
        let before_outer = symbolic + gather;
        let all_plans = before_outer + readers + m_plans;
        let preparation = p
            .helpers
            .max((2 * w + 3 * x) * field + before_outer + p.gather_scratch)
            .max(2 * w * field + symbolic + symbolic_scratch);
        // The full prover owns one cloned witness. Its caller's witness is
        // resident before admission. Helper buffers die before z/A/B/C exist.
        let phase1 = w * field
            + self.bounds.len() as u128 * size_of::<usize>() as u128
            + self.shape.rows() as u128 * field;
        let helper = phase1 + p.helpers;
        let outer_k = self.shape.outer_blocks().first().copied().unwrap_or(0);
        let outer_low = x >> outer_k;
        // eq_low is alive while each A/B/C fold allocates its replacement.
        let outer =
            (3 * w + 3 * x + 2 * outer_low) * field + before_outer + p.gather_scratch + phase1
                - w * field;
        let outer = outer.max((3 * w) * field + symbolic + symbolic_scratch);
        // readers() precedes the M output; its temporary edge buckets are
        // released before m_plans() is built. Equality ex is alive in both.
        let matrix_readers = (3 * w + x) * field + before_outer + readers + reader_scratch;
        let matrix_values = (5 * w + x) * field + all_plans + p.matrix_scratch;
        let matrix = matrix_readers.max(matrix_values);
        let inner_k = self.shape.inner_blocks().first().copied().unwrap_or(0);
        let inner_low = (2 * w) >> inner_k;
        let support = inner_low.div_ceil(crate::kern::SEGMENT as u128) * 4;
        let inner = (5 * w + inner_low) * field + all_plans + support;
        // At opening only folded m/z/A/B/C remain; witness clone, IPA x/b,
        // generator copy and two L/R scalar/base buffers overlap. Pasta affine
        // points contain two base-field elements on both supported curves.
        let tape_scratch = self
            .plan
            .get()
            .map_or(0, |t| (t.inputs + t.ops.len()) as u128 * field);
        let opening = w * field + all_plans + (7 * cols) * field + tape_scratch;
        let additional = phase1
            .max(helper)
            .max(outer)
            .max(matrix)
            .max(inner)
            .max(opening);
        PhaseMemoryAccounting {
            helper_bytes: cap(p.helpers),
            preparation_bytes: cap(preparation),
            phase1_bytes: cap(phase1),
            outer_bytes: cap(outer),
            matrix_bytes: cap(matrix),
            inner_bytes: cap(inner),
            opening_bytes: cap(opening),
            additional_peak_bytes: cap(additional),
        }
    }
}
