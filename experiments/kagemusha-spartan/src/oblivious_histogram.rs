//! Oblivious LogUp multiplicities.
//!
//! A table entry's multiplicity is the number of lookups whose tuple equals
//! it. Looked-up tuples are private, and so are a witness table's entries (a
//! fixed table is the public program's), so the count never indexes memory by
//! such a value and never branches on a comparison of one: for every input of
//! one public shape (entry count, key width, lookup count and group of each
//! lookup, and for a fixed table its values) it reads and writes the same
//! addresses in the same order and runs the same instructions. Equality is
//! exact: a key is its field elements' canonical representations, compared in
//! full; no hash or fingerprint stands in for a tuple.
//!
//! [`count`] picks one of three strategies from public sizes alone:
//! - [`Strategy::Trie`], fixed tables: lookups in batches of 256 as bit
//!   planes, run through a straight-line program compiled from the table's
//!   public values. Bit positions constant over the table are checked per
//!   lookup; the program then walks a binary trie of the varying positions,
//!   testing a position that all of a subtree's entries share once for the
//!   subtree, so every leaf has tested every varying position (exact
//!   equality) and counts its lanes. Every operation runs for every batch,
//!   whatever its lanes hold. A 2^11-entry range table compiles to 6,142
//!   operations: about 200 word operations per lookup, against 2^11 entry
//!   comparisons per lookup for a scan.
//! - [`Strategy::Linear`], small witness tables (the 512-row wallet and key
//!   tables): every lookup against every entry by masks, `N (2W + 3)` word
//!   operations per lookup, after `N^2 / 2` comparisons that find equal
//!   entries.
//! - [`Strategy::Sort`], large tables: entries and lookups sorted together by
//!   a bitonic network (a public schedule of compare-exchanges), counted by
//!   two linear scans and sorted back to their origin:
//!   `O((N + Q) log^2 (N + Q))` compare-exchanges, within [`MAX_SORT_RECORDS`]
//!   and [`MAX_SORT_BYTES`].
//!
//! Of equal entries the last takes every lookup of their tuple (the uniform
//! build's order). A lookup in no entry is reported for its group only; the
//! caller panics, as a malformed witness has no proof. Counts beyond
//! [`MAX_WORK`] panic before reading any lookup.
//!
//! The selects are mask arithmetic on 64-bit words (no `subtle` barrier; the
//! compiler's branch-free lowering of these patterns is assumed), and leaf
//! counts use `count_ones`, a single instruction on AArch64 and on x86-64
//! with POPCNT and a fixed bit-twiddling sequence otherwise.

use std::collections::HashMap;

use rayon::prelude::*;

use super::Scalar;

/// 64-bit words per field element of a key.
pub(super) const FIELD_WORDS: usize = 4;
/// Words per lane mask of a trie batch.
const LANES: usize = 4;
/// Lookups per trie batch.
const BATCH: usize = 64 * LANES;

/// The most estimated word operations a count may take.
pub(super) const MAX_WORK: u128 = 1 << 40;
/// The sort's most records (entries and lookups, padded to a power of two).
pub(super) const MAX_SORT_RECORDS: usize = 1 << 24;
/// The sort's most bytes: its records, their results and the origin sort.
pub(super) const MAX_SORT_BYTES: usize = 1 << 28;
/// A fixed table's most program operations.
const MAX_TRIE_OPS: usize = 1 << 22;
/// The most work (members times untested positions) a program's build takes.
const MAX_TRIE_BUILD: usize = 1 << 30;
/// Records per parallel sort task.
const SORT_GRAIN: usize = 1 << 12;
/// A sort record's origin word: lookups set the top bit, padding is all ones.
const LOOKUP: u64 = 1 << 63;

/// All ones when `x == 0`, else zero.
#[inline(always)]
fn zero_mask(x: u64) -> u64 {
    // `x | -x` has its top bit set exactly when `x != 0`.
    ((x | x.wrapping_neg()) >> 63).wrapping_sub(1)
}

/// All ones when `a < b` (unsigned), else zero.
#[inline(always)]
fn lt_mask(a: u64, b: u64) -> u64 {
    // The high half of the 128-bit difference is the borrow, spread.
    ((u128::from(a).wrapping_sub(u128::from(b))) >> 64) as u64
}

/// All ones when the word slices are equal (of one length), else zero.
#[inline(always)]
fn eq_mask(a: &[u64], b: &[u64]) -> u64 {
    let mut diff = 0u64;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    zero_mask(diff)
}

/// All ones when `a < b` as word strings, the first word most significant.
#[inline(always)]
fn lex_lt_mask(a: &[u64], b: &[u64]) -> u64 {
    let (mut lt, mut eq) = (0u64, !0u64);
    for (&x, &y) in a.iter().zip(b) {
        lt |= eq & lt_mask(x, y);
        eq &= zero_mask(x ^ y);
    }
    lt
}

/// Swaps two records of one length where `swap` is all ones.
#[inline(always)]
fn cswap(a: &mut [u64], b: &mut [u64], swap: u64) {
    for (x, y) in a.iter_mut().zip(b.iter_mut()) {
        let t = (*x ^ *y) & swap;
        *x ^= t;
        *y ^= t;
    }
}

/// Appends the canonical words of `v` (its representation, little-endian) to
/// a key.
pub(super) fn push_field<F: Scalar>(key: &mut Vec<u64>, v: F) {
    let repr = v.to_repr();
    for bytes in repr.chunks_exact(8) {
        let mut word = [0u8; 8];
        word.copy_from_slice(bytes);
        key.push(u64::from_le_bytes(word));
    }
}

/// A LogUp table's present entries as keys of `words` words each.
pub(super) struct Table {
    words: usize,
    entries: usize,
    keys: Vec<u64>,
    /// The entries are the public program's (a fixed table), not witness
    /// values: only then may the count's schedule follow them.
    public: bool,
}

impl Table {
    pub(super) fn new(words: usize, entries: usize, keys: Vec<u64>, public: bool) -> Self {
        assert_eq!(keys.len(), words * entries, "whole table keys");
        Table {
            words,
            entries,
            keys,
            public,
        }
    }

    /// The entries at `slots` (in order), each the `cells` values
    /// `value(cell, slot)`.
    pub(super) fn gather<F: Scalar>(
        cells: usize,
        slots: &[usize],
        public: bool,
        value: impl Fn(usize, usize) -> F,
    ) -> Self {
        let mut keys = Vec::with_capacity(slots.len() * cells * FIELD_WORDS);
        for &slot in slots {
            for cell in 0..cells {
                push_field(&mut keys, value(cell, slot));
            }
        }
        Table::new(cells * FIELD_WORDS, slots.len(), keys, public)
    }

    fn key(&self, e: usize) -> &[u64] {
        &self.keys[e * self.words..(e + 1) * self.words]
    }
}

/// The multiplicities of a table's entries (in table order) and, per lookup
/// group, whether one of its tuples is in no entry.
pub(super) struct Counts {
    pub(super) counts: Vec<u64>,
    pub(super) missing: Vec<bool>,
}

/// How a count runs (each oblivious; [`plan`] picks by public sizes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Strategy {
    Trie,
    Linear,
    Sort,
}

/// The multiplicities of `table`'s entries over `lookups` lookups in `groups`
/// groups. `visit` hands every lookup to its sink as (group, key), in an order
/// that depends on public structure only.
pub(super) fn count(
    table: &Table,
    groups: usize,
    lookups: usize,
    visit: impl FnOnce(&mut dyn FnMut(usize, &[u64])),
) -> Counts {
    let (strategy, program) = plan(table, lookups);
    count_with(table, strategy, program.as_ref(), groups, lookups, visit)
}

/// Estimated word operations of a scan (public sizes).
fn linear_cost(entries: usize, words: usize, lookups: usize, public: bool) -> u128 {
    if lookups == 0 {
        return 0;
    }
    let (n, w, q) = (entries as u128, words as u128, lookups as u128);
    // A witness table's equal entries are found by comparing every pair; a
    // fixed table's are public.
    let shadow = if public {
        n * w
    } else {
        n * n / 2 * (2 * w + 1)
    };
    q * n * (2 * w + 3) + shadow
}

/// The sort's records: entries and lookups, padded to a power of two.
fn sort_records(entries: usize, lookups: usize) -> usize {
    entries
        .saturating_add(lookups)
        .max(2)
        .checked_next_power_of_two()
        .unwrap_or(usize::MAX)
}

/// The bitonic network's compare-exchanges on `records` (a power of two).
fn exchanges(records: usize) -> u128 {
    let lg = u128::from(records.trailing_zeros());
    records as u128 / 2 * lg * (lg + 1) / 2
}

/// Estimated word operations of the sort, or `None` beyond its bounds.
fn sort_cost(entries: usize, words: usize, lookups: usize) -> Option<u128> {
    let p = sort_records(entries, lookups);
    let w = words as u128;
    // Records of w + 2 words, a result word each, the origin sort's pairs.
    let bytes = p as u128 * (w + 5) * 8;
    if p > MAX_SORT_RECORDS || bytes > MAX_SORT_BYTES as u128 {
        return None;
    }
    Some(exchanges(p) * (10 * (w + 2) + 20) + p as u128 * (4 * w + 16))
}

/// Estimated word operations of a fixed table's program.
fn trie_cost(program: &Program, words: usize, lookups: usize) -> u128 {
    let batches = lookups.div_ceil(BATCH) as u128;
    batches * program.ops.len() as u128 * (2 * LANES as u128)
        + lookups as u128 * (program.planes.len() as u128 + 2 * words as u128 + 4)
}

/// The cheapest strategy within [`MAX_WORK`] and its estimate (the trie's
/// estimate when a fixed table has a program).
fn choose(
    entries: usize,
    words: usize,
    lookups: usize,
    public: bool,
    trie: Option<u128>,
) -> Option<(u128, Strategy)> {
    let mut best: Option<(u128, Strategy)> = None;
    for (cost, strategy) in [
        (trie, Strategy::Trie),
        (
            Some(linear_cost(entries, words, lookups, public)),
            Strategy::Linear,
        ),
        (sort_cost(entries, words, lookups), Strategy::Sort),
    ] {
        if let Some(cost) = cost
            && cost <= MAX_WORK
            && best.is_none_or(|(b, _)| cost < b)
        {
            best = Some((cost, strategy));
        }
    }
    best
}

/// The strategy for `lookups` lookups into `table`, from public sizes and,
/// for a fixed table, its public values (its program). Panics beyond every
/// bound.
fn plan(table: &Table, lookups: usize) -> (Strategy, Option<Program>) {
    let (n, w) = (table.entries, table.words);
    let other = choose(n, w, lookups, table.public, None);
    // A program takes about three operations per distinct entry: build one
    // only when that estimate beats the other strategies.
    let floor = lookups.div_ceil(BATCH) as u128 * 3 * n as u128 * (2 * LANES as u128);
    let worth = other.is_none_or(|(c, _)| floor < c);
    let program = if table.public && n > 0 && lookups > 0 && worth {
        Program::build(table)
    } else {
        None
    };
    let trie = program.as_ref().map(|p| trie_cost(p, w, lookups));
    let Some((_, strategy)) = choose(n, w, lookups, table.public, trie) else {
        panic!(
            "a LogUp table of {n} entries ({w} words) with {lookups} lookups exceeds the oblivious count's bounds"
        );
    };
    (strategy, program)
}

/// The lookups a count has taken, checked against their public shape.
struct Intake {
    groups: usize,
    words: usize,
    lookups: usize,
    taken: usize,
}

impl Intake {
    fn take(&mut self, group: usize, key: &[u64]) {
        assert!(group < self.groups, "a lookup in a declared group");
        assert_eq!(key.len(), self.words, "a lookup of the table's width");
        assert!(self.taken < self.lookups, "the declared lookup count");
        self.taken += 1;
    }
}

/// [`count`] by a given strategy (`program` for the trie).
fn count_with(
    table: &Table,
    strategy: Strategy,
    program: Option<&Program>,
    groups: usize,
    lookups: usize,
    visit: impl FnOnce(&mut dyn FnMut(usize, &[u64])),
) -> Counts {
    let n = table.entries;
    let mut missing = vec![0u64; groups];
    let mut intake = Intake {
        groups,
        words: table.words,
        lookups,
        taken: 0,
    };
    let counts = match strategy {
        Strategy::Trie => {
            let program = program.expect("a fixed table's program");
            let mut run = TrieRun::new(program, table.words);
            visit(&mut |group: usize, key: &[u64]| {
                intake.take(group, key);
                run.push(group, key, &mut missing);
            });
            run.flush(&mut missing);
            let mut counts = vec![0u64; n];
            for (&owner, &lanes) in program.owners.iter().zip(&run.leaves) {
                counts[owner as usize] = lanes;
            }
            counts
        }
        Strategy::Linear => {
            let shadow = if lookups > 0 {
                shadowed(table)
            } else {
                vec![0; n]
            };
            let mut counts = vec![0u64; n];
            visit(&mut |group: usize, key: &[u64]| {
                intake.take(group, key);
                let mut found = 0u64;
                for (e, (count, &shade)) in counts.iter_mut().zip(&shadow).enumerate() {
                    let hit = eq_mask(key, table.key(e));
                    *count += hit & !shade & 1;
                    found |= hit;
                }
                missing[group] |= !found & 1;
            });
            counts
        }
        Strategy::Sort => sort_count(table, lookups, &mut missing, &mut intake, visit),
    };
    assert_eq!(intake.taken, lookups, "the declared lookup count");
    Counts {
        counts,
        missing: missing.into_iter().map(|m| m != 0).collect(),
    }
}

/// Per entry, all ones when a later entry is equal (that one takes the
/// lookups).
fn shadowed(table: &Table) -> Vec<u64> {
    let n = table.entries;
    if table.public {
        // The public program's entries: their equalities are public.
        let mut last: HashMap<&[u64], usize> = HashMap::new();
        for e in 0..n {
            last.insert(table.key(e), e);
        }
        (0..n)
            .map(|e| if last[table.key(e)] == e { 0 } else { !0 })
            .collect()
    } else {
        (0..n)
            .map(|e| {
                let mut later = 0u64;
                for f in e + 1..n {
                    later |= eq_mask(table.key(e), table.key(f));
                }
                later
            })
            .collect()
    }
}

/// A fixed table's trie as a straight-line program over bit planes.
struct Program {
    /// Per key word, the bits every entry agrees on, and their values.
    fixed_mask: Vec<u64>,
    fixed_value: Vec<u64>,
    /// The varying positions (word, bit), one plane each.
    planes: Vec<(u32, u32)>,
    ops: Vec<Op>,
    /// The deepest mask an operation writes.
    depth: usize,
    /// Per leaf (distinct tuple), the entry that takes its lookups.
    owners: Vec<u32>,
}

/// A program step: `mask[d] = mask[d - 1] & (plane ^ flip)`, or a leaf
/// counting the lanes of `mask[d]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Op {
    head: u32,
    arg: u32,
}

const OP_LEAF: u32 = 1 << 31;
const OP_FLIP: u32 = 1 << 30;
const OP_DEPTH: u32 = OP_FLIP - 1;

impl Op {
    fn head(depth: usize) -> u32 {
        u32::try_from(depth)
            .ok()
            .filter(|d| *d <= OP_DEPTH)
            .expect("a trie depth")
    }
    /// Tests `plane` at `depth` against the entries' bit `one`.
    fn test(depth: usize, plane: u32, one: bool) -> Op {
        let flip = if one { 0 } else { OP_FLIP };
        Op {
            head: Op::head(depth) | flip,
            arg: plane,
        }
    }
    fn leaf(depth: usize, leaf: u32) -> Op {
        Op {
            head: Op::head(depth) | OP_LEAF,
            arg: leaf,
        }
    }
    fn depth(self) -> usize {
        (self.head & OP_DEPTH) as usize
    }
    fn is_leaf(self) -> bool {
        self.head & OP_LEAF != 0
    }
    /// The plane's complement mask (program data, public).
    fn flip(self) -> u64 {
        if self.head & OP_FLIP != 0 { !0 } else { 0 }
    }
}

/// A trie node to compile: its distinct tuples, the varying positions its
/// path has not tested, the depth of its parent's last mask, and the split
/// that leads to it.
struct Node {
    leaves: Vec<u32>,
    untested: Vec<u32>,
    depth: usize,
    split: Option<(u32, bool)>,
}

impl Program {
    /// The program of a fixed table's entries; `None` for an empty table or
    /// beyond the trie's bounds.
    fn build(table: &Table) -> Option<Program> {
        let (n, w) = (table.entries, table.words);
        if n == 0 || n.saturating_mul(3) > MAX_TRIE_OPS {
            return None;
        }
        // Distinct tuples; the last equal entry owns each.
        let mut leaf_of: HashMap<&[u64], u32> = HashMap::new();
        let mut first: Vec<usize> = Vec::new();
        let mut owners: Vec<u32> = Vec::new();
        for e in 0..n {
            let owner = u32::try_from(e).ok()?;
            match leaf_of.get(table.key(e)) {
                Some(&leaf) => owners[leaf as usize] = owner,
                None => {
                    leaf_of.insert(table.key(e), u32::try_from(owners.len()).ok()?);
                    first.push(e);
                    owners.push(owner);
                }
            }
        }
        let (mut and, mut or) = (vec![!0u64; w], vec![0u64; w]);
        for &e in &first {
            for (t, &x) in table.key(e).iter().enumerate() {
                and[t] &= x;
                or[t] |= x;
            }
        }
        let fixed_mask: Vec<u64> = and.iter().zip(&or).map(|(a, o)| !(a ^ o)).collect();
        let fixed_value: Vec<u64> = and.iter().zip(&fixed_mask).map(|(a, m)| a & m).collect();
        let mut planes: Vec<(u32, u32)> = Vec::new();
        for (t, (a, o)) in and.iter().zip(&or).enumerate() {
            for b in 0..64u32 {
                if ((a ^ o) >> b) & 1 == 1 {
                    planes.push((u32::try_from(t).ok()?, b));
                }
            }
        }
        let bit = |leaf: u32, plane: u32| {
            let (t, b) = planes[plane as usize];
            (table.key(first[leaf as usize])[t as usize] >> b) & 1 == 1
        };
        let mut ops: Vec<Op> = Vec::new();
        let mut deepest = 0usize;
        let mut work = 0usize;
        let mut stack = vec![Node {
            leaves: (0..u32::try_from(owners.len()).ok()?).collect(),
            untested: (0..u32::try_from(planes.len()).ok()?).collect(),
            depth: 0,
            split: None,
        }];
        // Depth first: a node's masks sit above its parent's, which stay
        // valid while its subtree runs.
        while let Some(Node {
            leaves,
            untested,
            mut depth,
            split: entering,
        }) = stack.pop()
        {
            if let Some((plane, one)) = entering {
                depth += 1;
                ops.push(Op::test(depth, plane, one));
            }
            work = work.saturating_add(leaves.len().saturating_mul(untested.len()));
            if work > MAX_TRIE_BUILD {
                return None;
            }
            // Positions all of the node's tuples share: tested once for them.
            let mut rest = Vec::new();
            let mut best: Option<(usize, u32)> = None;
            for &plane in &untested {
                let ones = leaves.iter().filter(|&&l| bit(l, plane)).count();
                if ones == 0 || ones == leaves.len() {
                    depth += 1;
                    ops.push(Op::test(depth, plane, ones != 0));
                } else {
                    rest.push(plane);
                    let imbalance = leaves.len().abs_diff(2 * ones);
                    if best.is_none_or(|(i, _)| imbalance < i) {
                        best = Some((imbalance, plane));
                    }
                }
            }
            deepest = deepest.max(depth);
            if ops.len() > MAX_TRIE_OPS {
                return None;
            }
            let Some((_, split)) = best else {
                // Tuples agreeing on every position are one tuple.
                assert_eq!(leaves.len(), 1, "distinct tuples differ");
                ops.push(Op::leaf(depth, leaves[0]));
                continue;
            };
            rest.retain(|&p| p != split);
            let (ones, zeros): (Vec<u32>, Vec<u32>) =
                leaves.iter().copied().partition(|&l| bit(l, split));
            stack.push(Node {
                leaves: zeros,
                untested: rest.clone(),
                depth,
                split: Some((split, false)),
            });
            stack.push(Node {
                leaves: ones,
                untested: rest,
                depth,
                split: Some((split, true)),
            });
        }
        if ops.len() > MAX_TRIE_OPS {
            return None;
        }
        Some(Program {
            fixed_mask,
            fixed_value,
            planes,
            ops,
            depth: deepest,
            owners,
        })
    }
}

/// A fixed table's program over batches of lookups.
struct TrieRun<'p> {
    program: &'p Program,
    words: usize,
    keys: Vec<u64>,
    groups: Vec<usize>,
    filled: usize,
    planes: Vec<[u64; LANES]>,
    masks: Vec<[u64; LANES]>,
    /// Lanes counted per leaf.
    leaves: Vec<u64>,
}

impl<'p> TrieRun<'p> {
    fn new(program: &'p Program, words: usize) -> Self {
        TrieRun {
            program,
            words,
            keys: vec![0; BATCH * words],
            groups: vec![0; BATCH],
            filled: 0,
            planes: vec![[0; LANES]; program.planes.len()],
            masks: vec![[0; LANES]; program.depth + 1],
            leaves: vec![0; program.owners.len()],
        }
    }

    fn push(&mut self, group: usize, key: &[u64], missing: &mut [u64]) {
        let (w, at) = (self.words, self.filled);
        self.keys[at * w..(at + 1) * w].copy_from_slice(key);
        self.groups[at] = group;
        self.filled += 1;
        if self.filled == BATCH {
            self.flush(missing);
        }
    }

    /// Runs the filled lanes through every operation of the program.
    fn flush(&mut self, missing: &mut [u64]) {
        let (p, w, filled) = (self.program, self.words, self.filled);
        if filled == 0 {
            return;
        }
        let mut root = [0u64; LANES];
        for plane in &mut self.planes {
            *plane = [0; LANES];
        }
        for lane in 0..filled {
            let key = &self.keys[lane * w..(lane + 1) * w];
            let mut off = 0u64;
            for ((k, m), v) in key.iter().zip(&p.fixed_mask).zip(&p.fixed_value) {
                off |= (k & m) ^ v;
            }
            let (x, b) = (lane / 64, lane % 64);
            root[x] |= (zero_mask(off) & 1) << b;
            for (plane, &(t, bit)) in self.planes.iter_mut().zip(&p.planes) {
                plane[x] |= ((key[t as usize] >> bit) & 1) << b;
            }
        }
        self.masks[0] = root;
        let mut found = [0u64; LANES];
        for &op in &p.ops {
            let d = op.depth();
            if op.is_leaf() {
                let m = self.masks[d];
                let mut lanes = 0u64;
                for (f, &mx) in found.iter_mut().zip(&m) {
                    lanes += u64::from(mx.count_ones());
                    *f |= mx;
                }
                self.leaves[op.arg as usize] += lanes;
            } else {
                let parent = self.masks[d - 1];
                let plane = self.planes[op.arg as usize];
                let flip = op.flip();
                self.masks[d] = std::array::from_fn(|x| parent[x] & (plane[x] ^ flip));
            }
        }
        for (lane, &group) in self.groups[..filled].iter().enumerate() {
            missing[group] |= !(found[lane / 64] >> (lane % 64)) & 1;
        }
        self.filled = 0;
    }
}

/// Sorts records of `stride` words (a power-of-two count) ascending by `lt`
/// (an all-ones mask for less) through a bitonic network: the
/// compare-exchanges and their order follow the record count alone.
fn bitonic_sort(data: &mut [u64], stride: usize, lt: impl Fn(&[u64], &[u64]) -> u64 + Sync) {
    let n = data.len() / stride;
    assert!(
        n.is_power_of_two() && data.len() == n * stride,
        "a power-of-two count of whole records"
    );
    let mut k = 2;
    while k <= n {
        let mut j = k / 2;
        while j > 0 {
            // Blocks of 2j records: record t < j pairs with t + j, in the
            // direction of bit k of the block's first index.
            let block = 2 * j;
            let task = block.max(SORT_GRAIN).min(n);
            data.par_chunks_mut(task * stride)
                .enumerate()
                .for_each(|(c, chunk)| {
                    for (b, pair) in chunk.chunks_mut(block * stride).enumerate() {
                        let ascending = ((c * task + b * block) & k) == 0;
                        let (lo, hi) = pair.split_at_mut(j * stride);
                        for (x, y) in lo.chunks_mut(stride).zip(hi.chunks_mut(stride)) {
                            let swap = if ascending {
                                lt(&*y, &*x)
                            } else {
                                lt(&*x, &*y)
                            };
                            cswap(x, y, swap);
                        }
                    }
                });
            j /= 2;
        }
        k *= 2;
    }
}

/// [`Strategy::Sort`]: entries and lookups sorted together by key, counted,
/// and sorted back to their origin.
fn sort_count(
    table: &Table,
    lookups: usize,
    missing: &mut [u64],
    intake: &mut Intake,
    visit: impl FnOnce(&mut dyn FnMut(usize, &[u64])),
) -> Vec<u64> {
    let (n, w) = (table.entries, table.words);
    // A record: padding flag, key, origin (entry index, or LOOKUP | lookup).
    let s = w + 2;
    let p = sort_records(n, lookups);
    assert!(sort_cost(n, w, lookups).is_some(), "the sort's bounds hold");
    let mut records = vec![0u64; p * s];
    for (e, r) in records.chunks_exact_mut(s).take(n).enumerate() {
        r[1..=w].copy_from_slice(table.key(e));
        r[s - 1] = e as u64;
    }
    let mut group_of: Vec<usize> = Vec::with_capacity(lookups);
    visit(&mut |group: usize, key: &[u64]| {
        intake.take(group, key);
        let q = group_of.len();
        let r = &mut records[(n + q) * s..(n + q + 1) * s];
        r[1..=w].copy_from_slice(key);
        r[s - 1] = LOOKUP | q as u64;
        group_of.push(group);
    });
    assert_eq!(group_of.len(), lookups, "the declared lookup count");
    for r in records.chunks_exact_mut(s).skip(n + lookups) {
        r[0] = 1;
        r[s - 1] = u64::MAX;
    }
    // Equal keys sit together, their entries first (by index), then their
    // lookups; padding last.
    bitonic_sort(&mut records, s, lex_lt_mask);
    let key = |i: usize| &records[i * s..i * s + s - 1];
    let lookup = |i: usize| (records[i * s + s - 1] >> 63).wrapping_neg();
    let mut out = vec![0u64; p];
    // Backwards: the lookups of a tuple run up to its last entry, which takes
    // them; an earlier equal entry sees none.
    let mut run = 0u64;
    for i in (0..p).rev() {
        let same = if i + 1 < p {
            eq_mask(key(i), key(i + 1))
        } else {
            0
        };
        let (is_lookup, carried) = (lookup(i), run & same);
        out[i] = carried & !is_lookup;
        run = carried.wrapping_add(1) & is_lookup;
    }
    // Forwards: a lookup is found when an entry of its tuple precedes it.
    let mut seen = 0u64;
    for i in 0..p {
        let same = if i > 0 {
            eq_mask(key(i), key(i - 1))
        } else {
            0
        };
        let is_lookup = lookup(i);
        seen = !is_lookup | (same & seen);
        out[i] |= seen & is_lookup & 1;
    }
    let mut back = vec![0u64; 2 * p];
    for (i, pair) in back.chunks_exact_mut(2).enumerate() {
        pair[0] = records[i * s + s - 1];
        pair[1] = out[i];
    }
    drop(records);
    drop(out);
    // Back to origin: entries by index, then lookups by order, then padding.
    bitonic_sort(&mut back, 2, lex_lt_mask);
    let counts = (0..n)
        .map(|e| {
            assert_eq!(back[2 * e], e as u64, "entries return in order");
            back[2 * e + 1]
        })
        .collect();
    for (q, &group) in group_of.iter().enumerate() {
        let at = 2 * (n + q);
        assert_eq!(back[at], LOOKUP | q as u64, "lookups return in order");
        missing[group] |= (back[at + 1] & 1) ^ 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic test stream (splitmix64).
    struct Stream(u64);

    impl Stream {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// The count the uniform build used to take (variable time, tests only):
    /// a hash index of the entries, the last equal one kept.
    fn indexed(table: &Table, groups: usize, lookups: &[(usize, Vec<u64>)]) -> Counts {
        let mut index: HashMap<Vec<u64>, usize> = HashMap::new();
        for e in 0..table.entries {
            index.insert(table.key(e).to_vec(), e);
        }
        let mut counts = vec![0u64; table.entries];
        let mut missing = vec![false; groups];
        for (group, key) in lookups {
            match index.get(key) {
                Some(&e) => counts[e] += 1,
                None => missing[*group] = true,
            }
        }
        Counts { counts, missing }
    }

    fn run(
        table: &Table,
        strategy: Strategy,
        groups: usize,
        lookups: &[(usize, Vec<u64>)],
    ) -> Counts {
        let program =
            (strategy == Strategy::Trie).then(|| Program::build(table).expect("a test program"));
        count_with(
            table,
            strategy,
            program.as_ref(),
            groups,
            lookups.len(),
            |sink| {
                for (group, key) in lookups {
                    sink(*group, key.as_slice());
                }
            },
        )
    }

    fn strategies(table: &Table) -> Vec<Strategy> {
        let mut all = vec![Strategy::Linear, Strategy::Sort];
        if table.public && table.entries > 0 {
            all.push(Strategy::Trie);
        }
        all
    }

    /// The field elements `0..2^bits` as one-element keys.
    fn range_table(bits: u32, public: bool) -> Table {
        let keys = (0..1u64 << bits).flat_map(|v| [v, 0, 0, 0]).collect();
        Table::new(FIELD_WORDS, 1 << bits, keys, public)
    }

    fn element(v: u64) -> Vec<u64> {
        vec![v, 0, 0, 0]
    }

    #[test]
    fn masks_compare_words_exactly() {
        let edge = [
            0,
            1,
            2,
            (1 << 63) - 1,
            1 << 63,
            (1 << 63) + 1,
            u64::MAX - 1,
            u64::MAX,
        ];
        let mask = |b: bool| if b { !0u64 } else { 0 };
        for &a in &edge {
            for &b in &edge {
                assert_eq!(lt_mask(a, b), mask(a < b), "{a} < {b}");
                assert_eq!(zero_mask(a ^ b), mask(a == b), "{a} == {b}");
                for &c in &edge {
                    assert_eq!(
                        lex_lt_mask(&[a, c], &[b, a]),
                        mask([a, c] < [b, a]),
                        "[{a}, {c}] < [{b}, {a}]"
                    );
                    assert_eq!(eq_mask(&[c, a], &[c, b]), mask(a == b));
                    let (mut x, mut y) = ([a, c], [b, a]);
                    cswap(&mut x, &mut y, mask(c & 1 == 1));
                    let swapped = c & 1 == 1;
                    assert_eq!(
                        (x, y),
                        if swapped {
                            ([b, a], [a, c])
                        } else {
                            ([a, c], [b, a])
                        }
                    );
                }
            }
        }
        assert_eq!(eq_mask(&[], &[]), !0, "empty keys are equal");
    }

    #[test]
    fn keys_are_canonical_field_words() {
        use ff::PrimeField;
        use iroha_pasta::Fp;
        let mut key = Vec::new();
        push_field(&mut key, Fp::from(5u64));
        push_field(&mut key, Fp::from_u128((1u128 << 64) + 3));
        assert_eq!(key, vec![5, 0, 0, 0, 3, 1, 0, 0]);
        let (mut a, mut b) = (Vec::new(), Vec::new());
        push_field(&mut a, Fp::from(7u64));
        push_field(&mut b, Fp::from(3u64) + Fp::from(4u64));
        assert_eq!(a, b, "one element, one key");
    }

    #[test]
    fn the_bitonic_network_sorts_on_a_fixed_schedule() {
        let mut s = Stream(3);
        for (records, stride) in [(2, 1), (4, 3), (64, 2), (1 << 13, 3)] {
            let mut data: Vec<u64> = (0..records * stride).map(|_| s.below(5)).collect();
            let mut want: Vec<Vec<u64>> = data.chunks(stride).map(<[u64]>::to_vec).collect();
            want.sort();
            bitonic_sort(&mut data, stride, lex_lt_mask);
            let got: Vec<Vec<u64>> = data.chunks(stride).map(<[u64]>::to_vec).collect();
            assert_eq!(got, want, "{records} records of {stride} words");
        }
        // The compare-exchanges, by record index, are the same for any values.
        let trace = |data: &mut Vec<u64>| {
            let base = data.as_ptr() as usize;
            let seen = std::sync::Mutex::new(Vec::new());
            bitonic_sort(data, 2, |a: &[u64], b: &[u64]| {
                let at = |r: &[u64]| (r.as_ptr() as usize - base) / 16;
                seen.lock().unwrap().push((at(a), at(b)));
                lex_lt_mask(a, b)
            });
            seen.into_inner().unwrap()
        };
        let mut x: Vec<u64> = (0..128).map(|_| s.next()).collect();
        let mut y = vec![0u64; 128];
        let (tx, ty) = (trace(&mut x), trace(&mut y));
        assert_eq!(tx, ty, "one schedule");
        assert_eq!(tx.len() as u128, exchanges(64));
    }

    #[test]
    fn every_strategy_counts_like_a_hash_index() {
        let mut s = Stream(7);
        for trial in 0..120 {
            let words = [0, 1, 4, 8, 12][trial % 5];
            let public = trial % 2 == 0;
            let entries = s.below(48) as usize;
            // Few values per word: repeated entries, shared prefixes and
            // near misses in both halves of a word.
            let palette: Vec<u64> = (0..1 + s.below(4))
                .map(|_| s.next())
                .chain([0, 1 << 63])
                .collect();
            let keys = (0..entries * words)
                .map(|_| palette[s.below(palette.len() as u64) as usize])
                .collect();
            let table = Table::new(words, entries, keys, public);
            let groups = 1 + s.below(3) as usize;
            // Past two trie batches, groups changing within a batch.
            let lookups: Vec<(usize, Vec<u64>)> = (0..s.below(600))
                .map(|_| {
                    let group = s.below(groups as u64) as usize;
                    let mut key = if entries > 0 {
                        table.key(s.below(entries as u64) as usize).to_vec()
                    } else {
                        (0..words).map(|_| s.next()).collect()
                    };
                    // One in eight off by one bit (unless that lands on
                    // another entry, which the index sees too).
                    if words > 0 && s.below(8) == 0 {
                        let t = s.below(words as u64) as usize;
                        key[t] ^= 1 << s.below(64);
                    }
                    (group, key)
                })
                .collect();
            let want = indexed(&table, groups, &lookups);
            for strategy in strategies(&table) {
                let got = run(&table, strategy, groups, &lookups);
                assert_eq!(got.counts, want.counts, "trial {trial} {strategy:?} counts");
                assert_eq!(
                    got.missing, want.missing,
                    "trial {trial} {strategy:?} missing"
                );
            }
        }
    }

    #[test]
    fn equal_entries_leave_every_lookup_to_the_last() {
        // Entries 0 and 2 hold 5; entries 1, 3 and 4 hold 9.
        let values = [5u64, 9, 5, 9, 9, 2];
        let lookups: Vec<(usize, Vec<u64>)> = [9u64, 5, 9, 2, 9, 5, 11]
            .iter()
            .map(|&v| (usize::from(v == 11), element(v)))
            .collect();
        for public in [true, false] {
            let keys = values.iter().flat_map(|&v| element(v)).collect();
            let table = Table::new(FIELD_WORDS, values.len(), keys, public);
            for strategy in strategies(&table) {
                let got = run(&table, strategy, 2, &lookups);
                assert_eq!(got.counts, vec![0, 0, 2, 0, 3, 1], "{public} {strategy:?}");
                assert_eq!(got.missing, vec![false, true], "{public} {strategy:?}");
            }
        }
    }

    #[test]
    fn a_tuple_off_the_table_is_missing_from_its_group_alone() {
        let members = vec![(0, element(5)), (2, element(2047))];
        // Off by a bit every entry shares: 2^11, a high word, the top bit,
        // the second word (every range value is an entry, so no tuple is off
        // in the varying bits alone).
        let off = [
            element(2048),
            vec![5, 0, 0, 1 << 40],
            vec![5 | (1 << 63), 0, 0, 0],
            vec![5, 1, 0, 0],
        ];
        for public in [true, false] {
            let table = range_table(11, public);
            for (case, key) in off.iter().enumerate() {
                let mut lookups = members.clone();
                lookups.push((1, key.clone()));
                for strategy in strategies(&table) {
                    let got = run(&table, strategy, 3, &lookups);
                    assert_eq!(
                        got.missing,
                        vec![false, true, false],
                        "{case} {public} {strategy:?}"
                    );
                    assert_eq!(got.counts.iter().sum::<u64>(), 2);
                    assert_eq!((got.counts[5], got.counts[2047]), (1, 1));
                }
            }
        }
    }

    #[test]
    fn a_range_table_compiles_to_a_complete_trie() {
        let table = range_table(11, true);
        let program = Program::build(&table).expect("a range program");
        let low_bits: Vec<(u32, u32)> = (0..11u32).map(|b| (0, b)).collect();
        assert_eq!(program.planes, low_bits);
        assert_eq!(
            program.ops.len(),
            (1 << 12) - 2 + (1 << 11),
            "6,142 operations"
        );
        assert_eq!(program.depth, 11);
        assert_eq!(program.fixed_mask, vec![!0x7ffu64, !0, !0, !0]);
        let per_lookup = trie_cost(&program, FIELD_WORDS, 1 << 20) >> 20;
        assert!(per_lookup < 250, "{per_lookup} word operations per lookup");
        for lookups in [300, 100_000, 1_000_000] {
            assert_eq!(plan(&table, lookups).0, Strategy::Trie, "{lookups} lookups");
        }
        // A spread-like second column, a function of the first, adds one
        // shared test per split rather than one chain per leaf.
        let keys = (0..1u64 << 11)
            .flat_map(|d| {
                let spread = (0..11).fold(0u64, |s, b| s | (((d >> b) & 1) << (2 * b)));
                [d, 0, 0, 0, spread, 0, 0, 0]
            })
            .collect();
        let spread = Program::build(&Table::new(8, 1 << 11, keys, true)).expect("a spread program");
        assert_eq!(spread.planes.len(), 22);
        assert!(spread.ops.len() < 3 * (1 << 12), "{}", spread.ops.len());
    }

    #[test]
    fn the_strategy_follows_public_sizes() {
        let pick = |n, w, q, public| choose(n, w, q, public, None).map(|(_, s)| s);
        assert_eq!(pick(16, 8, 18, false), Some(Strategy::Linear));
        assert_eq!(pick(1 << 12, 4, 100_000, false), Some(Strategy::Sort));
        assert_eq!(pick(1 << 20, 4, 1 << 30, false), None, "beyond every bound");
        // Witness tables never take the trie, whatever their values.
        let mut s = Stream(11);
        for _ in 0..4 {
            let keys = (0..512 * 28).map(|_| s.next()).collect();
            let table = Table::new(28, 512, keys, false);
            for lookups in [0, 64, 4096] {
                let (strategy, program) = plan(&table, lookups);
                assert!(program.is_none() && strategy != Strategy::Trie);
                assert_eq!(
                    Some(strategy),
                    pick(512, 28, lookups, false),
                    "the same choice for any values"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "exceeds the oblivious count's bounds")]
    fn a_count_beyond_its_bounds_is_refused_before_reading() {
        let table = range_table(12, false);
        count(&table, 1, 1 << 25, |_| unreachable!("no lookup is read"));
    }

    #[test]
    fn empty_tables_and_tables_without_lookups() {
        for public in [true, false] {
            let empty = Table::new(FIELD_WORDS, 0, Vec::new(), public);
            let got = count(&empty, 2, 1, |sink| sink(1, &[1u64, 2, 3, 4][..]));
            assert_eq!((got.counts, got.missing), (vec![], vec![false, true]));
            let table = range_table(4, public);
            let got = count(&table, 1, 0, |_| {});
            assert_eq!((got.counts, got.missing), (vec![0; 16], vec![false]));
        }
    }

    #[test]
    #[should_panic(expected = "the declared lookup count")]
    fn a_count_takes_exactly_its_declared_lookups() {
        let table = range_table(4, true);
        count(&table, 1, 2, |sink| sink(0, &[3u64, 0, 0, 0][..]));
    }
}
