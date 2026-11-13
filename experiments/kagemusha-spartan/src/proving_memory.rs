//! Allocation accounting for the constructed prover, without constructing its
//! lazy plans. This is an admission estimate, not a whole-process RSS bound:
//! allocator fragmentation, opaque closure captures, matrix compiler scratch, other circuits, shared
//! generator tables and the MSM kernel's separately enforced budget are extra.

use super::*;
use core::mem::size_of;

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
    /// Dense proof workspace and a conservative projection of all lazy plans.
    /// Includes one caller witness even if the caller has already allocated it.
    pub additional_peak_bytes: u64,
    pub accounted_peak_bytes: u64,
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

/// Upper counts for all original and replacement symbolic terms. Replaced
/// rows are deliberately counted twice, which avoids materializing them merely
/// to decide whether there is enough memory to prove.
#[derive(Default)]
struct Terms {
    count: u128,
    coefficients: u128,
    maps: u128,
}
impl Terms {
    fn term<F>(&mut self, t: &Term<F>) {
        self.count += 1;
        self.coefficients += t.coef.len() as u128;
        if let Src::Link { map, .. } = &t.src {
            self.maps += map.len() as u128;
        }
    }
    fn synthetic(&mut self, n: usize, count: usize, links: usize) {
        self.count += count as u128;
        self.coefficients += n as u128 * count as u128;
        self.maps += n as u128 * links as u128;
    }
}

impl<F: Scalar> UniformCircuit<F> {
    pub fn proving_memory_accounting(&self) -> ProvingMemoryAccounting {
        let field = size_of::<F>() as u128;
        let mut retained = size_of::<Self>() as u128 + bytes(&self.arrays);
        let mut terms = Terms::default();
        let mut template_rows = 0u128;
        let mut template_instances = 0u128;
        let mut cell_spans = 0u128;
        let mut cells = 0u128;
        for a in &self.arrays {
            retained += bytes(&a.widths) + nested(&a.values) + bytes(&a.rows) + bytes(&a.lookups);
            template_rows += a.rows.len() as u128;
            template_instances += a.rows.len() as u128 * (1u128 << a.rho);
            cells += a.cells() as u128;
            cell_spans += a.cells() as u128 * ((1u128 << a.rho) + 1);
            for r in &a.rows {
                for lin in r {
                    retained += lin_heap(lin);
                    for t in lin {
                        terms.term(t);
                    }
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
            + bytes(&self.fixed_values)
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
                    terms.term(t);
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

        for &(a, _, _) in &self.lookup_arrays {
            let arr = &self.arrays[a];
            let n = 1usize << arr.rho;
            for (j, (_, tuple)) in arr.lookups.iter().enumerate() {
                terms.synthetic(
                    n,
                    3,
                    usize::from(self.lookup_mask_col.contains_key(&(a, j))),
                );
                for lin in tuple {
                    for t in lin {
                        terms.term(t);
                    }
                }
            }
        }
        for &(va, _, wa) in &self.tables {
            terms.synthetic(
                1usize << self.arrays[wa].rho,
                3 + self.arrays[va].cells(),
                self.arrays[va].cells(),
            );
        }
        for (_, wa, _, tuple) in &self.exports {
            terms.synthetic(1usize << self.arrays[*wa].rho, 3, 0);
            for lin in tuple {
                for t in lin {
                    terms.term(t);
                }
            }
        }
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

        // Growing vectors include their minimum allocation and capacity slack. The
        // source coefficients bound gather/CSR edges, even when all maps repeat.
        let symbolic = 4 * terms.count * size_of::<(Term<F>, Factor)>() as u128
            + terms.coefficients * field
            + terms.maps * size_of::<Option<usize>>() as u128
            + template_rows * 3 * size_of::<Vec<(Term<F>, Factor)>>() as u128
            + self.arrays.len() as u128 * size_of::<Vec<[Vec<(Term<F>, Factor)>; 3]>>() as u128;
        let gather = 4 * template_rows * 3 * size_of::<GatherJob>() as u128
            + 4 * terms.count
                * (size_of::<Option<GatherTarget>>() + size_of::<Factor>() + size_of::<usize>())
                    as u128
            + template_instances * 3 * size_of::<usize>() as u128;
        let readers = 4
            * terms.count
            * (size_of::<Reader>() + size_of::<(usize, usize, usize, usize)>()) as u128
            + cell_spans * 8
            + terms.coefficients * (4 * 12 + 8)
            + cells * 256;
        let m_plans = 4 * cells * size_of::<MPlan<F>>() as u128
            + cell_spans * 4
            + terms.count * size_of::<MReader>() as u128
            + 4 * terms.coefficients * (size_of::<(u32, u8, u32, u32)>() as u128 + field);
        let mut additional = 0u128;
        if self.symbolic.get().is_none() {
            additional += symbolic;
        }
        if self.gather.get().is_none() {
            additional += gather;
        }
        if self.readers.get().is_none() {
            additional += readers;
        }
        if self.m_plans.get().is_none() {
            additional += m_plans;
        }
        // Per-proof matrix weights and the gather's per-term factor/base
        // workspace remain necessary even when their public plans are cached.
        additional += terms.count * (2 * field + size_of::<usize>() as u128);
        if let Some(t) = self.plan.get() {
            additional += (t.inputs + t.ops.len()) as u128 * field;
        }
        // Caller + cloned witness, z and M (2W each), helper filling and the
        // largest folded allocation; A/B/C, equality tables and first fold.
        // Summing stage maxima deliberately overcounts overlapping lifetimes.
        let w = 1u128
            .checked_shl(self.shape.sw() as u32)
            .unwrap_or(u128::MAX);
        let x = 1u128.checked_shl(self.shape.sx as u32).unwrap_or(u128::MAX);
        additional = additional.saturating_add(
            w.saturating_mul(8)
                .saturating_add(x.saturating_mul(5))
                .saturating_mul(field),
        );
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
}
