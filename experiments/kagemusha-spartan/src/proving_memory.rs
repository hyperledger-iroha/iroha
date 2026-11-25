//! Allocation accounting for the constructed prover, without constructing its
//! lazy plans. This is an admission estimate, not a whole-process RSS bound:
//! allocator fragmentation, opaque closure captures, matrix compiler scratch, other circuits, shared
//! generator tables and the MSM kernel's separately enforced budget are extra.

use super::*;
use core::mem::size_of;
use memory_plan_counts::{PlanCounts, PlanScope};

#[path = "memory_plan_counts.rs"]
mod memory_plan_counts;

/// Constraint rows per task of the engine's preparation relation check (its
/// per-task `A`, `B`, `C` tables).
pub const PREPARATION_CHECK_ROWS: usize = 1 << 14;

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
        let counts = c.memory_plan_counts(PlanScope::CountMatrixEntries);
        let before = c.proving_memory_accounting();
        let phases = c.phase_memory_accounting();
        assert_eq!(c.first_phase_memory_bytes(), phases.phase1_bytes);
        assert_eq!(c.preparation_memory_bytes(), phases.preparation_bytes);
        let k = c.shape.inner_blocks()[0];
        let cube = 1u64 << k;
        let grid = 3u64.pow(k as u32);
        // All these field arrays coexist in Projection::finish.
        let low = (2u64 << c.shape.sw()) >> k;
        let final_grid = low + cube * cube + grid * cube + cube + 2 * grid;
        assert!(phases.matrix_bytes >= phases.phase1_bytes + final_grid * size_of::<F>() as u64);
        let tasks = rayon::current_num_threads() as u64;
        let rows = (PREPARATION_CHECK_ROWS as u64).min(1 << c.shape.sx);
        assert!(phases.preparation_bytes >= tasks * 3 * rows * size_of::<F>() as u64);
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
                >= prepared_phase.preparation_bytes + prepared.symbolic_bytes
                    - before.symbolic_bytes
        );
        let _ = c.readers();
        let _ = c.m_plans();
        // The flat plans themselves are built per proof, a group at a time:
        // each within its block's bound, the largest group within the count.
        let plans: Vec<_> = c
            .m_plans()
            .iter()
            .map(|b| (b, c.m_plan(c.readers(), b.a2, b.k2)))
            .collect();
        assert_eq!(
            counts.entry_count,
            plans
                .iter()
                .map(|(_, p)| p.entries.len() as u128)
                .sum::<u128>(),
            "exact present nonzero M entries"
        );
        assert_eq!(
            counts.coefficient_count,
            plans
                .iter()
                .map(|(_, p)| p.coefs.len() as u128)
                .sum::<u128>(),
            "exact non-unit M coefficients"
        );
        for (b, p) in &plans {
            let actual = size_of::<MPlan<F>>() as u128
                + bytes(&p.offsets)
                + bytes(&p.entries)
                + bytes(&p.coefs)
                + bytes(&p.readers);
            assert!(actual <= b.bytes as u128, "block plan within its bound");
            assert!(b.bytes as u128 <= counts.m_plan_group.max(b.bytes as u128));
        }
        assert!(
            plans
                .iter()
                .map(|(b, _)| b.bytes as u128)
                .max()
                .unwrap_or(0)
                <= counts.m_plan_group
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
            c.first_phase_memory_bytes(),
            "prefix commitments never charge matrix plans"
        );
        // A proof builds its own gather, readers and block plans whatever
        // the circuit retains: retained plans add to the retained bytes and
        // leave the phases as they were (but for the symbolic rows).
        assert!(planned.retained_bytes > prepared.retained_bytes);
        assert_eq!(
            planned.additional_peak_bytes,
            prepared.additional_peak_bytes
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
                    coef: vec![F::ZERO, -F::ONE, F::from(7), F::from(9)].into(),
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
fn term_heap<F>(t: &Term<F>, coefficients: &mut HashSet<*const F>) -> u128 {
    t.coef.heap_bytes(coefficients)
        + match &t.src {
            Src::Link { map, .. } => bytes(map),
            _ => 0,
        }
}
fn lin_heap<F>(v: &Lin<F>, coefficients: &mut HashSet<*const F>) -> u128 {
    bytes(v) + v.iter().map(|t| term_heap(t, coefficients)).sum::<u128>()
}
fn tuple_heap<F>(v: &Vec<Lin<F>>, coefficients: &mut HashSet<*const F>) -> u128 {
    bytes(v)
        + v.iter()
            .map(|lin| lin_heap(lin, coefficients))
            .sum::<u128>()
}
fn symbolic_heap(rows: &SymbolicRows) -> u128 {
    nested(rows)
}
fn cap(x: u128) -> u64 {
    x.min(u64::MAX as u128) as u64
}

impl<F: Scalar> UniformCircuit<F> {
    pub fn proving_memory_accounting(&self) -> ProvingMemoryAccounting {
        let mut coefficients = HashSet::new();
        let mut retained = size_of::<Self>() as u128 + bytes(&self.arrays);
        for a in &self.arrays {
            retained += bytes(&a.widths) + nested(&a.values) + bytes(&a.rows) + bytes(&a.lookups);
            for r in &a.rows {
                for lin in r {
                    retained += lin_heap(lin, &mut coefficients);
                }
            }
            for (_, tuple) in &a.lookups {
                retained += tuple_heap(tuple, &mut coefficients);
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
                    retained += term_heap(t, &mut coefficients);
                }
            }
        }
        for (_, _, _, tuple) in &self.exports {
            retained += tuple_heap(tuple, &mut coefficients);
        }
        for (_, tuple) in &self.copy_exports {
            retained += tuple_heap(tuple, &mut coefficients);
        }
        for (_, tuple, from) in &self.copy_imports {
            retained += tuple_heap(tuple, &mut coefficients) + bytes(from);
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
        }
        let gather_bytes = retained - start;
        let start = retained;
        if let Some(rd) = self.readers.get() {
            retained += bytes(&rd.readers) + bytes(&rd.all_readers) + bytes(&rd.one_readers);
            for row in &rd.readers {
                retained += nested(row);
            }
            for row in &rd.all_readers {
                retained += nested(row);
            }
        }
        let readers_bytes = retained - start;
        let start = retained;
        if let Some(blocks) = self.m_plans.get() {
            retained += bytes(blocks);
        }
        let m_plan_bytes = retained - start;
        let start = retained;
        if let Some(t) = self.plan.get() {
            retained += bytes(&t.pool) + bytes(&t.regs.inputs);
            retained += match &t.regs.ops {
                RegisterInstructions::Packed(words) => bytes(words),
                RegisterInstructions::Wide(instructions) => bytes(instructions),
            };
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

    /// Construction storage by kind (measurement lines): private array
    /// values, fixed array values, template rows and lookups, and the dense
    /// fixed tail.
    pub fn construction_breakdown(&self) -> [u64; 4] {
        let mut coefficients = HashSet::new();
        let (mut private, mut fixed, mut templates) = (0u128, 0u128, 0u128);
        for a in &self.arrays {
            let v = nested(&a.values);
            if a.fixed {
                fixed += v;
            } else {
                private += v;
            }
            templates += bytes(&a.rows) + bytes(&a.lookups);
            for r in &a.rows {
                for lin in r {
                    templates += lin_heap(lin, &mut coefficients);
                }
            }
            for (_, tuple) in &a.lookups {
                templates += tuple_heap(tuple, &mut coefficients);
            }
        }
        [
            cap(private),
            cap(fixed),
            cap(templates),
            cap(self.fixed_values.heap_bytes()),
        ]
    }

    /// Template storage by part (measurement lines): terms, coefficient and
    /// link-map bytes by capacity and by length, and the term vectors'.
    pub fn template_census(&self) -> String {
        let mut coefficients = HashSet::new();
        let (mut terms, mut coef_cap, mut coef_len, mut map_cap, mut map_len) =
            (0u128, 0u128, 0u128, 0u128, 0u128);
        let (mut lin_cap, mut lin_len, mut ones) = (0u128, 0u128, 0u128);
        let mut single_one = 0u128;
        let mut count = |lin: &Lin<F>| {
            lin_cap += bytes(lin);
            lin_len += (lin.len() * size_of::<Term<F>>()) as u128;
            for t in lin {
                terms += 1;
                coef_cap += t.coef.heap_bytes(&mut coefficients);
                coef_len += (t.coef.len() * size_of::<F>()) as u128;
                if t.coef.iter().all(|c| *c == F::ONE) {
                    ones += 1;
                    if t.coef.len() == 1 {
                        single_one += 1;
                    }
                }
                if let Src::Link { map, .. } = &t.src {
                    map_cap += bytes(map);
                    map_len += (map.len() * size_of::<Option<usize>>()) as u128;
                }
            }
        };
        for a in &self.arrays {
            for r in &a.rows {
                for lin in r {
                    count(lin);
                }
            }
            for (_, tuple) in &a.lookups {
                for lin in tuple {
                    count(lin);
                }
            }
        }
        let mb = |b: u128| b as f64 / 1e6;
        format!(
            "templates: {terms} terms ({ones} all-one, {single_one} single-one), coefficients {:.0} MB (length {:.0}), maps {:.0} MB (length {:.0}), term vectors {:.0} MB (length {:.0})",
            mb(coef_cap),
            mb(coef_len),
            mb(map_cap),
            mb(map_len),
            mb(lin_cap),
            mb(lin_len)
        )
    }

    /// Workspace of the engine's relation check: the second phase filled into
    /// the prover's own rows, then `A z * B z = C z` checked a task of
    /// [`PREPARATION_CHECK_ROWS`] rows at a time (no `z` copy, no whole
    /// tables). This path does not build a reader, M-table or matrix tape.
    pub fn preparation_memory_bytes(&self) -> u64 {
        let plans = self.memory_plan_counts(PlanScope::Preparation);
        let (owned, _) = self.witness_and_first_phase_bytes();
        cap(self.preparation_workspace(owned, &plans))
    }

    fn preparation_workspace(&self, owned: u128, plans: &PlanCounts) -> u128 {
        let symbolic = if self.symbolic.get().is_none() {
            plans.symbolic
        } else {
            0
        };
        let rows = 1u128.checked_shl(self.shape.sx as u32).unwrap_or(u128::MAX);
        let task_rows = (PREPARATION_CHECK_ROWS as u128).min(rows);
        let workers = rayon::current_num_threads() as u128;
        let check = plans.gather_index
            + plans.gather_scratch
            + workers * 3 * task_rows * size_of::<F>() as u128;
        owned + symbolic + plans.gather + plans.helpers.max(check)
    }

    fn witness_and_first_phase_bytes(&self) -> (u128, u128) {
        // Rows the prover writes: masks, the link phase, the second phase.
        let cols = 1usize << self.shape.col_bits;
        let mut owned = vec![false; self.shape.rows()];
        let written_rows = self.p1_rows..(self.p1_rows + self.pl_rows + self.p2_rows);
        for row in owned
            [written_rows.start.min(self.shape.rows())..written_rows.end.min(self.shape.rows())]
            .iter_mut()
        {
            *row = true;
        }
        for &(array, cells, instance) in &self.product_masks {
            for cell in cells {
                owned[self.cell_index(array, cell, instance) / cols] = true;
            }
        }
        let field = size_of::<F>() as u128;
        let owned_bytes = owned.iter().filter(|x| **x).count() as u128 * cols as u128 * field;
        let phase1 = owned_bytes
            + self.bounds.len() as u128 * size_of::<usize>() as u128
            + self.shape.rows() as u128 * field;
        (owned_bytes, phase1)
    }

    /// Bound the first commitment phase from its row layout before querying
    /// available memory. Later phases perform their own full admission check.
    pub(crate) fn first_phase_memory_bytes(&self) -> u64 {
        cap(self.witness_and_first_phase_bytes().1)
    }

    /// Count phase workspaces without building lazy plans or changing a witness.
    pub fn phase_memory_accounting(&self) -> PhaseMemoryAccounting {
        let p = self.memory_plan_counts(PlanScope::FullProof);
        let field = size_of::<F>() as u128;
        let w = 1u128
            .checked_shl(self.shape.sw() as u32)
            .unwrap_or(u128::MAX);
        let x = 1u128.checked_shl(self.shape.sx as u32).unwrap_or(u128::MAX);
        let cols = 1u128
            .checked_shl(self.shape.col_bits as u32)
            .unwrap_or(u128::MAX);
        // The symbolic rows are built on first use and stay with the circuit.
        // A proof's other plans are its own, each dropped after its phase:
        // the gather's jobs and index after the outer rounds, the readers and
        // block plans after the M table (plans a caller built on the circuit
        // are retained besides, and counted there).
        let symbolic = if self.symbolic.get().is_none() {
            p.symbolic
        } else {
            0
        };
        let gather = p.gather;
        let matrix_plans = p.readers + p.m_plans;
        // The prover reads its caller's witness in place and copies only the
        // rows it writes (`kern::Rows`): product masks, the link phase, the
        // second phase.
        let (owned, phase1) = self.witness_and_first_phase_bytes();
        let preparation = self.preparation_workspace(owned, &p);
        let helper = phase1 + p.helpers + symbolic;
        // Outer: the first round and its fold stream Az, Bz, Cz a chunk of
        // rows at a time (per worker: three tables of 2^k corners of a
        // chunk); the folded tables and eq_low remain, and each later fold
        // allocates its replacements.
        let outer_k = self.shape.outer_blocks().first().copied().unwrap_or(0);
        let outer_low = x >> outer_k;
        let chunk = (crate::kern::CHUNK as u128).min(outer_low.max(1));
        let tasks = (rayon::current_num_threads() as u128).min(outer_low.div_ceil(chunk));
        let chunk_tables = tasks * 3 * (1u128 << outer_k) * chunk * field;
        let outer = phase1
            + (3 * outer_low + outer_low) * field
            + chunk_tables
            + p.gather_index
            + symbolic
            + gather
            + p.gather_scratch;
        // Matrix: equality ex as its high and low tables (the low one over
        // the largest row block), the pages of M's dense half its plans write
        // and its written-run bitmap, the plans a group at a time.
        let block_bits = self
            .arrays
            .iter()
            .enumerate()
            .filter(|(_, arr)| !arr.rows.is_empty())
            .map(|(a, _)| self.m(a))
            .max()
            .unwrap_or(0)
            .min(self.shape.sx);
        let eq_tables = (1u128 << block_bits) + (x >> block_bits);
        let half_runs = w.div_ceil(crate::kern::SEGMENT as u128);
        let m_dense = p.m_pages * memory_plan_counts::M_PAGE as u128;
        let inner_k = self.shape.inner_blocks().first().copied().unwrap_or(0);
        let inner_low = (2 * w) >> inner_k;
        let low_runs = inner_low.div_ceil(crate::kern::SEGMENT as u128);
        let cube = 1u128 << inner_k;
        let grid = 3u128.pow(inner_k as u32);
        // An IO array occupies one contiguous row starting at its declared
        // offset in the public half. The constant occupies its first entry.
        let tail = self
            .io_arrays
            .iter()
            .map(|&(a, off)| off as u128 + self.arrays[a].r_count as u128)
            .fold(1, u128::max);
        // The parallel pass owns one corner cross-product per running worker
        // and one merged matrix. Finalization extends its columns on the
        // ternary grid while holding the unit vector, its extension and result.
        let workers = rayon::current_num_threads() as u128;
        let parallel_products = (workers + 1) * cube * cube;
        let projected_grid = cube * cube + grid * cube + cube + 2 * grid;
        // The projected table keeps its lowest corner. Its atomic run marks
        // overlap the collected word bitmap and both overlap the support scan.
        let marks = 2 * half_runs.div_ceil(64) * size_of::<u64>() as u128;
        let support = low_runs * 17;
        // Basis::new stores corner indices, up to one triple per extension
        // and a list of the non-Boolean grid positions. The latter vectors grow.
        let basis = (cube + 8 * grid) * size_of::<usize>() as u128;
        let streamed = (inner_low + tail + parallel_products.max(projected_grid)) * field
            + basis
            + marks
            + low_runs * size_of::<u32>() as u128
            + support;
        let matrix = phase1
            + eq_tables * field
            + (m_dense + half_runs.div_ceil(8)).max(streamed)
            + symbolic
            + matrix_plans
            + p.m_plan_group
            + p.matrix_scratch;
        // Inner: M's written pages while its first fold is allocated, then
        // (M's pages dropped) the folds of M and z (the caller's rows are read
        // in place).
        // Per run: the corner masks of both tables and of z alone (8 bytes),
        // their scan's pairs while they are split (8) and z's live flag
        // carried to the next round (1).
        let inner =
            phase1 + symbolic + support + (m_dense + inner_low * field).max(2 * inner_low * field);
        // At opening only the folded tables remain; IPA x/b, generator copy
        // and two L/R scalar/base buffers overlap. Pasta affine points contain
        // two base-field elements on both supported curves. The matrix value
        // is the folded M table's (no program run).
        let opening = phase1 + symbolic + (7 * cols) * field;
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
