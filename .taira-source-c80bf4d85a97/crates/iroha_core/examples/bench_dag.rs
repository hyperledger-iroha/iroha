//! Micro-benchmark for the conflict DAG builder under different key distributions.
#![allow(
    clippy::doc_markdown,
    clippy::struct_excessive_bools,
    clippy::unreadable_literal,
    clippy::double_parens,
    unused_parens,
    clippy::similar_names,
    unused_variables,
    clippy::explicit_iter_loop,
    clippy::explicit_into_iter_loop,
    clippy::items_after_statements,
    clippy::manual_midpoint,
    clippy::cast_precision_loss,
    clippy::same_item_push,
    clippy::redundant_closure_for_method_calls,
    clippy::ignored_unit_patterns,
    clippy::disallowed_types,
    clippy::type_complexity,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::cast_possible_truncation
)]
//!
//! Usage:
//!   cargo run -p iroha_core --example bench_dag -- [key=value ...]
//!
//! Params (all optional, defaults in parentheses):
//! - n: total transactions (10000)
//! - keys: total distinct keys in pool (1000)
//! - reads: reads per tx (0)
//! - writes: writes per tx (1)
//! - dist: distribution: uniform|zipf|all_same|all_unique|hot_read (uniform)
//! - sched: scheduler backing: vec|csr|both (vec)
//! - delta_index: merge index kind: hashmap|vec|both (hashmap)
//! - zipf_s: zipf exponent s > 0 (1.1)
//! - iters: iterations for each method (3)
//! - seed: deterministic RNG seed (42)
//!
//! Notes:
//! - This isolates DAG build and schedule; it doesn’t execute overlays.
//! - Compares per-wave sort vs BinaryHeap ready-queue schedulers (both
//!   deterministic by a synthetic key `(call_hash, idx)`).
//! - No external deps are used; timings are rough but comparable.

use std::time::{Duration, Instant};

use iroha_core::pipeline::access::AccessSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DistKind {
    Uniform,
    Zipf,
    AllSame,
    AllUnique,
    HotRead,
}

#[derive(Debug, Clone)]
struct Params {
    n: usize,
    keys: usize,
    reads: usize,
    writes: usize,
    dist: DistKind,
    zipf_s: f64,
    iters: usize,
    seed: u64,
    // Synthetic delta writes per tx (merge workload)
    delta_writes: usize,
    // Use CSR-backed scheduler comparison instead of Vec adjacency
    sched_csr: bool,
    // Run both scheduler backings
    sched_both: bool,
    // Delta index variant selection for merge benchmark
    delta_index_hashmap: bool,
    delta_index_vec: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            n: 10_000,
            keys: 1_000,
            reads: 0,
            writes: 1,
            dist: DistKind::Uniform,
            zipf_s: 1.1,
            iters: 3,
            seed: 42,
            delta_writes: 8,
            sched_csr: false,
            sched_both: false,
            delta_index_hashmap: true,
            delta_index_vec: false,
        }
    }
}

// Simple LCG for deterministic pseudo-random generation (no deps)
#[derive(Clone)]
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        // Constants from Numerical Recipes
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.0
    }
    fn next_usize(&mut self) -> usize {
        (self.next_u64() >> 1) as usize
    }
}

fn parse_params() -> Params {
    let mut p = Params::default();
    for arg in std::env::args().skip(1) {
        let mut it = arg.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        match k {
            "n" => p.n = v.parse().unwrap_or(p.n),
            "keys" => p.keys = v.parse().unwrap_or(p.keys),
            "reads" => p.reads = v.parse().unwrap_or(p.reads),
            "writes" => p.writes = v.parse().unwrap_or(p.writes),
            "zipf_s" => p.zipf_s = v.parse().unwrap_or(p.zipf_s),
            "iters" => p.iters = v.parse().unwrap_or(p.iters),
            "seed" => p.seed = v.parse().unwrap_or(p.seed),
            "delta_writes" | "delta-writes" | "dw" => {
                p.delta_writes = v.parse().unwrap_or(p.delta_writes)
            }
            "sched" => match v.to_ascii_lowercase().as_str() {
                "vec" => {
                    p.sched_csr = false;
                    p.sched_both = false;
                }
                "csr" => {
                    p.sched_csr = true;
                    p.sched_both = false;
                }
                "both" => {
                    p.sched_both = true;
                }
                _ => {}
            },
            "sched_csr" | "sched-csr" => p.sched_csr = matches!(v, "1" | "true" | "yes"),
            "delta_index" | "delta-index" => match v.to_ascii_lowercase().as_str() {
                "hashmap" => {
                    p.delta_index_hashmap = true;
                    p.delta_index_vec = false;
                }
                "vec" => {
                    p.delta_index_hashmap = false;
                    p.delta_index_vec = true;
                }
                "both" => {
                    p.delta_index_hashmap = true;
                    p.delta_index_vec = true;
                }
                _ => {}
            },
            "dist" => {
                p.dist = match v.to_ascii_lowercase().as_str() {
                    "uniform" => DistKind::Uniform,
                    "zipf" => DistKind::Zipf,
                    "all_same" | "all-same" => DistKind::AllSame,
                    "all_unique" | "all-unique" => DistKind::AllUnique,
                    "hot_read" | "hot-read" => DistKind::HotRead,
                    _ => p.dist,
                }
            }
            _ => {}
        }
    }
    p
}

fn key_string(i: usize) -> String {
    format!("k{}", i)
}

// Build a Zipf CDF for keys in [0..keys)
fn zipf_cdf(keys: usize, s: f64) -> Vec<f64> {
    let mut w = Vec::with_capacity(keys);
    let mut sum = 0.0f64;
    for i in 0..keys {
        sum += 1.0f64 / ((i + 1) as f64).powf(s);
        w.push(sum);
    }
    for x in &mut w {
        *x /= sum;
    }
    w
}

fn sample_zipf(cdf: &[f64], r: f64) -> usize {
    use core::cmp::Ordering;
    let mut lo = 0usize;
    let mut hi = cdf.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        match r.partial_cmp(&cdf[mid]).unwrap_or(Ordering::Less) {
            Ordering::Greater => lo = mid + 1,
            _ => hi = mid,
        }
    }
    lo.min(cdf.len().saturating_sub(1))
}

fn gen_access_sets(p: &Params) -> Vec<AccessSet> {
    let mut rng = Lcg::new(p.seed);
    let zipf_cdf = if matches!(p.dist, DistKind::Zipf) {
        Some(zipf_cdf(p.keys, p.zipf_s))
    } else {
        None
    };
    let mut sets = Vec::with_capacity(p.n);
    let mut next_unique = 0usize;
    for _ in 0..p.n {
        let mut s = AccessSet::new();
        match p.dist {
            DistKind::AllSame => {
                for _ in 0..p.reads {
                    s.add_read(key_string(0));
                }
                for _ in 0..p.writes {
                    s.add_write(key_string(0));
                }
            }
            DistKind::AllUnique => {
                // Allocate disjoint key ranges per tx; fall back to modulo for large reads/writes
                for j in 0..p.reads {
                    s.add_read(key_string(next_unique + j));
                }
                for j in 0..p.writes {
                    s.add_write(key_string(next_unique + p.reads + j));
                }
                next_unique += p.reads + p.writes;
            }
            DistKind::HotRead => {
                for _ in 0..p.reads {
                    s.add_read(key_string(0));
                }
                for _ in 0..p.writes {
                    let k = (rng.next_usize() % p.keys).max(1); // avoid 0 to keep reads hot-only
                    s.add_write(key_string(k));
                }
            }
            DistKind::Uniform => {
                for _ in 0..p.reads {
                    s.add_read(key_string(rng.next_usize() % p.keys));
                }
                for _ in 0..p.writes {
                    s.add_write(key_string(rng.next_usize() % p.keys));
                }
            }
            DistKind::Zipf => {
                let cdf = zipf_cdf.as_ref().expect("zipf cdf");
                for _ in 0..p.reads {
                    let r = (rng.next_u64() as f64 / u64::MAX as f64).min(1.0);
                    s.add_read(key_string(sample_zipf(cdf, r)));
                }
                for _ in 0..p.writes {
                    let r = (rng.next_u64() as f64 / u64::MAX as f64).min(1.0);
                    s.add_write(key_string(sample_zipf(cdf, r)));
                }
            }
        }
        sets.push(s);
    }
    sets
}

// Synthetic delta: list of write-key indices for a tx
#[derive(Clone)]
struct Delta {
    keys: Vec<usize>,
}

fn gen_deltas(p: &Params, seed_offset: u64) -> Vec<Delta> {
    let mut rng = Lcg::new(p.seed.wrapping_add(seed_offset));
    let zipf_cdf = if matches!(p.dist, DistKind::Zipf) {
        Some(zipf_cdf(p.keys, p.zipf_s))
    } else {
        None
    };
    let mut deltas = Vec::with_capacity(p.n);
    for _ in 0..p.n {
        let mut keys = Vec::with_capacity(p.delta_writes);
        match p.dist {
            DistKind::AllSame => {
                for _ in 0..p.delta_writes {
                    keys.push(0);
                }
            }
            DistKind::AllUnique => {
                // best-effort unique: stride through space
                let base = rng.next_usize() % p.keys;
                for j in 0..p.delta_writes {
                    keys.push((base + j) % p.keys);
                }
            }
            DistKind::HotRead => {
                // writes avoid 0 to keep 0 as read-hot in the access dist
                for _ in 0..p.delta_writes {
                    keys.push(((rng.next_usize() % p.keys).max(1)));
                }
            }
            DistKind::Uniform => {
                for _ in 0..p.delta_writes {
                    keys.push(rng.next_usize() % p.keys);
                }
            }
            DistKind::Zipf => {
                let cdf = zipf_cdf.as_ref().expect("zipf cdf");
                for _ in 0..p.delta_writes {
                    let r = (rng.next_u64() as f64 / u64::MAX as f64).min(1.0);
                    keys.push(sample_zipf(cdf, r));
                }
            }
        }
        deltas.push(Delta { keys });
    }
    deltas
}

// Naive O(n^2) DAG build
fn dag_build_naive(access: &[AccessSet]) -> (Vec<Vec<usize>>, Vec<usize>) {
    let n = access.len();
    let mut indeg = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let conflicts = |a: &AccessSet, b: &AccessSet| {
        a.write_keys
            .iter()
            .any(|k| b.write_keys.contains(k) || b.read_keys.contains(k))
            || a.read_keys.iter().any(|k| b.write_keys.contains(k))
    };
    for i in 0..n {
        for j in (i + 1)..n {
            if conflicts(&access[i], &access[j]) {
                adj[i].push(j);
                indeg[j] += 1;
            }
        }
    }
    (adj, indeg)
}

// Optimized O(n + E) DAG build (example-local copy matching block.rs strategy)
fn dag_build_optimized(access: &[AccessSet]) -> (Vec<Vec<usize>>, Vec<usize>) {
    use std::collections::HashMap;
    let n = access.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    let mut last_writer: HashMap<&str, usize> = HashMap::new();
    let mut open_readers: HashMap<&str, Vec<usize>> = HashMap::new();

    for (idx, aset) in access.iter().enumerate() {
        let mut parents: Vec<usize> = Vec::new();
        for k in aset.read_keys.iter() {
            let ks = k.as_str();
            if let Some(&w) = last_writer.get(ks) {
                parents.push(w);
            }
            open_readers.entry(ks).or_default().push(idx);
        }
        for k in aset.write_keys.iter() {
            let ks = k.as_str();
            if let Some(&w) = last_writer.get(ks) {
                parents.push(w);
            }
            if let Some(readers) = open_readers.remove(ks) {
                parents.extend(readers);
            }
            last_writer.insert(ks, idx);
        }
        if !parents.is_empty() {
            parents.sort_unstable();
            parents.dedup();
            for p in parents {
                adj[p].push(idx);
                indeg[idx] += 1;
            }
        }
    }
    (adj, indeg)
}

fn schedule_kahn(adj: &[Vec<usize>], indeg: &[usize]) -> Vec<usize> {
    use std::collections::BTreeSet;
    let n = indeg.len();
    let mut indeg = indeg.to_vec();
    let mut ready: BTreeSet<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(&i) = ready.iter().next() {
        ready.take(&i);
        order.push(i);
        for &v in &adj[i] {
            indeg[v] -= 1;
            if indeg[v] == 0 {
                ready.insert(v);
            }
        }
    }
    order
}

fn layers_from_dag(adj: &[Vec<usize>], indeg: &[usize]) -> Vec<Vec<usize>> {
    use std::collections::BTreeSet;
    let n = indeg.len();
    let mut indeg = indeg.to_vec();
    let mut ready: BTreeSet<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut layers: Vec<Vec<usize>> = Vec::new();
    while !ready.is_empty() {
        let layer: Vec<usize> = ready.iter().copied().collect();
        // consume this layer
        for &i in &layer {
            let _ = ready.take(&i);
        }
        for &i in &layer {
            for &v in &adj[i] {
                indeg[v] -= 1;
                if indeg[v] == 0 {
                    ready.insert(v);
                }
            }
        }
        layers.push(layer);
    }
    layers
}

// Deterministic synthetic call-hash generator for tie-breaking
fn gen_call_keys(n: usize, seed: u64) -> Vec<u64> {
    let mut rng = Lcg::new(seed);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(rng.next_u64());
    }
    out
}

// Per-wave sort scheduler by (key, idx)
fn schedule_wave_by_key(adj: &[Vec<usize>], indeg: &[usize], key: &[u64]) -> Vec<usize> {
    let n = indeg.len();
    let mut indeg_s = indeg.to_vec();
    let mut ready: Vec<usize> = (0..n).filter(|&i| indeg_s[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while !ready.is_empty() {
        ready.sort_unstable_by(|&a, &b| key[a].cmp(&key[b]).then_with(|| a.cmp(&b)));
        let current = ready.split_off(0);
        for &i in &current {
            order.push(i);
            for &v in &adj[i] {
                indeg_s[v] = indeg_s[v].saturating_sub(1);
                if indeg_s[v] == 0 {
                    ready.push(v);
                }
            }
        }
    }
    order
}

// BinaryHeap ready-queue scheduler by (key, idx)
fn schedule_heap_by_key(adj: &[Vec<usize>], indeg: &[usize], key: &[u64]) -> Vec<usize> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    let n = indeg.len();
    let mut indeg_s = indeg.to_vec();
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::with_capacity(n);
    for i in 0..n {
        if indeg_s[i] == 0 {
            heap.push(Reverse((key[i], i)));
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(Reverse((_k, i))) = heap.pop() {
        order.push(i);
        for &v in &adj[i] {
            indeg_s[v] = indeg_s[v].saturating_sub(1);
            if indeg_s[v] == 0 {
                heap.push(Reverse((key[v], v)));
            }
        }
    }
    order
}

// Convert adjacency Vec<Vec<usize>> to CSR (row_offsets, cols) and indeg vector
fn adj_to_csr(adj: &[Vec<usize>]) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let n = adj.len();
    let mut row_offsets = vec![0usize; n + 1];
    for i in 0..n {
        row_offsets[i + 1] = row_offsets[i] + adj[i].len();
    }
    let e = row_offsets[n];
    let mut cols = vec![0usize; e];
    let mut indeg = vec![0usize; n];
    let mut cur = row_offsets.clone();
    for (u, edges) in adj.iter().enumerate() {
        for &v in edges {
            let pos = cur[u];
            cols[pos] = v;
            cur[u] = pos + 1;
            indeg[v] = indeg[v].saturating_add(1);
        }
    }
    (row_offsets, cols, indeg)
}

fn schedule_wave_by_key_csr(
    row_offsets: &[usize],
    cols: &[usize],
    indeg: &[usize],
    key: &[u64],
) -> Vec<usize> {
    let n = indeg.len();
    let mut indeg_s = indeg.to_vec();
    let mut ready: Vec<usize> = (0..n).filter(|&i| indeg_s[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while !ready.is_empty() {
        ready.sort_unstable_by(|&a, &b| key[a].cmp(&key[b]).then_with(|| a.cmp(&b)));
        let current = ready.split_off(0);
        for &i in &current {
            order.push(i);
            let (s, e) = (row_offsets[i], row_offsets[i + 1]);
            for &v in &cols[s..e] {
                indeg_s[v] = indeg_s[v].saturating_sub(1);
                if indeg_s[v] == 0 {
                    ready.push(v);
                }
            }
        }
    }
    order
}

fn schedule_heap_by_key_csr(
    row_offsets: &[usize],
    cols: &[usize],
    indeg: &[usize],
    key: &[u64],
) -> Vec<usize> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    let n = indeg.len();
    let mut indeg_s = indeg.to_vec();
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::with_capacity(n);
    for i in 0..n {
        if indeg_s[i] == 0 {
            heap.push(Reverse((key[i], i)));
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(Reverse((_k, i))) = heap.pop() {
        order.push(i);
        let (s, e) = (row_offsets[i], row_offsets[i + 1]);
        for &v in &cols[s..e] {
            indeg_s[v] = indeg_s[v].saturating_sub(1);
            if indeg_s[v] == 0 {
                heap.push(Reverse((key[v], v)));
            }
        }
    }
    order
}

// Intern keys per block (strings -> compact usize IDs), deterministically.
#[derive(Clone)]
struct AccessIds {
    reads: Vec<usize>,
    writes: Vec<usize>,
}

fn intern_access_sets(access: &[AccessSet]) -> (usize, Vec<AccessIds>) {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<&str, usize> = BTreeMap::new();
    for aset in access.iter() {
        for k in aset.read_keys.iter() {
            map.entry(k.as_str()).or_insert(usize::MAX);
        }
        for k in aset.write_keys.iter() {
            map.entry(k.as_str()).or_insert(usize::MAX);
        }
    }
    let mut next = 0usize;
    for v in map.values_mut() {
        *v = next;
        next += 1;
    }
    let key_count = next;
    let mut out: Vec<AccessIds> = Vec::with_capacity(access.len());
    for aset in access.iter() {
        let mut rs = Vec::with_capacity(aset.read_keys.len());
        let mut ws = Vec::with_capacity(aset.write_keys.len());
        for k in aset.read_keys.iter() {
            rs.push(*map.get(k.as_str()).unwrap());
        }
        for k in aset.write_keys.iter() {
            ws.push(*map.get(k.as_str()).unwrap());
        }
        out.push(AccessIds {
            reads: rs,
            writes: ws,
        });
    }
    (key_count, out)
}

// Build CSR adjacency from interned access ids
fn build_csr_from_ids(ids: &[AccessIds], key_count: usize) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let n = ids.len();
    let mut outdeg = vec![0usize; n];
    // Pass 1: count edges
    {
        let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
        let mut open_readers: Vec<Vec<usize>> = vec![Vec::new(); key_count];
        for (idx, aset) in ids.iter().enumerate() {
            let mut parents: Vec<usize> = Vec::new();
            for &k in &aset.reads {
                if let Some(w) = last_writer[k] {
                    parents.push(w);
                }
                open_readers[k].push(idx);
            }
            for &k in &aset.writes {
                if let Some(w) = last_writer[k] {
                    parents.push(w);
                }
                if !open_readers[k].is_empty() {
                    for r in open_readers[k].drain(..) {
                        parents.push(r);
                    }
                }
                last_writer[k] = Some(idx);
            }
            if !parents.is_empty() {
                parents.sort_unstable();
                parents.dedup();
                for p in parents {
                    outdeg[p] += 1;
                }
            }
        }
    }
    let mut row_offsets = vec![0usize; n + 1];
    for i in 0..n {
        row_offsets[i + 1] = row_offsets[i] + outdeg[i];
    }
    let e = row_offsets[n];
    let mut cols = vec![0usize; e];
    let mut indeg = vec![0usize; n];
    // Pass 2: fill cols
    {
        let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
        let mut open_readers: Vec<Vec<usize>> = vec![Vec::new(); key_count];
        let mut cursor = row_offsets.clone();
        for (idx, aset) in ids.iter().enumerate() {
            let mut parents: Vec<usize> = Vec::new();
            for &k in &aset.reads {
                if let Some(w) = last_writer[k] {
                    parents.push(w);
                }
                open_readers[k].push(idx);
            }
            for &k in &aset.writes {
                if let Some(w) = last_writer[k] {
                    parents.push(w);
                }
                if !open_readers[k].is_empty() {
                    for r in open_readers[k].drain(..) {
                        parents.push(r);
                    }
                }
                last_writer[k] = Some(idx);
            }
            if !parents.is_empty() {
                parents.sort_unstable();
                parents.dedup();
                for p in parents {
                    let pos = cursor[p];
                    cols[pos] = idx;
                    cursor[p] = pos + 1;
                    indeg[idx] += 1;
                }
            }
        }
    }
    (row_offsets, cols, indeg)
}

// DSF components from interned access ids
fn dsu_components(ids: &[AccessIds], key_count: usize) -> Vec<Vec<usize>> {
    let n = ids.len();
    struct Dsu {
        p: Vec<usize>,
        r: Vec<u8>,
    }
    impl Dsu {
        fn new(n: usize) -> Self {
            Self {
                p: (0..n).collect(),
                r: vec![0; n],
            }
        }
        fn find(&mut self, x: usize) -> usize {
            if self.p[x] != x {
                let px = self.p[x];
                let rx = self.find(px);
                self.p[x] = rx;
            }
            self.p[x]
        }
        fn union(&mut self, a: usize, b: usize) {
            let mut ra = self.find(a);
            let mut rb = self.find(b);
            if ra == rb {
                return;
            }
            let ra_r = self.r[ra];
            let rb_r = self.r[rb];
            if ra_r < rb_r {
                std::mem::swap(&mut ra, &mut rb);
            }
            self.p[rb] = ra;
            if ra_r == rb_r {
                self.r[ra] = ra_r + 1;
            }
        }
    }
    let mut dsu = Dsu::new(n);
    {
        let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
        let mut open_readers: Vec<Vec<usize>> = vec![Vec::new(); key_count];
        for (idx, aset) in ids.iter().enumerate() {
            for &k in &aset.reads {
                if let Some(w) = last_writer[k] {
                    dsu.union(idx, w);
                }
                open_readers[k].push(idx);
            }
            for &k in &aset.writes {
                if let Some(w) = last_writer[k] {
                    dsu.union(idx, w);
                }
                if !open_readers[k].is_empty() {
                    for r in open_readers[k].drain(..) {
                        dsu.union(idx, r);
                    }
                }
                last_writer[k] = Some(idx);
            }
        }
    }
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut dsu2 = Dsu {
        p: dsu.p.clone(),
        r: dsu.r.clone(),
    };
    for i in 0..n {
        let root = dsu2.find(i);
        buckets.entry(root).or_default().push(i);
    }
    let mut comps: Vec<Vec<usize>> = buckets.into_values().collect();
    for c in comps.iter_mut() {
        c.sort_unstable();
    }
    comps.sort_by_key(|c| c[0]);
    comps
}

fn layers_monolithic_csr(
    row_offsets: &[usize],
    cols: &[usize],
    indeg: &[usize],
) -> Vec<Vec<usize>> {
    let n = indeg.len();
    let mut indeg_m = indeg.to_vec();
    let mut ready: Vec<usize> = (0..n).filter(|&i| indeg_m[i] == 0).collect();
    let mut layers = Vec::new();
    while !ready.is_empty() {
        ready.sort_unstable();
        let wave: Vec<usize> = ready.split_off(0);
        for &i in &wave {
            let (s, e) = (row_offsets[i], row_offsets[i + 1]);
            for &v in &cols[s..e] {
                indeg_m[v] -= 1;
                if indeg_m[v] == 0 {
                    ready.push(v);
                }
            }
        }
        layers.push(wave);
    }
    layers
}

fn layers_per_component_vec(adj: &[Vec<usize>], components: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut per: Vec<Vec<Vec<usize>>> = Vec::new();
    let mut maxd = 0usize;
    for comp in components {
        let mut in_comp = vec![false; n];
        for &i in comp {
            in_comp[i] = true;
        }
        let mut indeg_c = vec![0usize; n];
        for &u in comp {
            for &v in &adj[u] {
                if in_comp[v] {
                    indeg_c[v] += 1;
                }
            }
        }
        let mut ready: Vec<usize> = comp.iter().copied().filter(|&i| indeg_c[i] == 0).collect();
        let mut cls: Vec<Vec<usize>> = Vec::new();
        while !ready.is_empty() {
            ready.sort_unstable();
            let wave: Vec<usize> = ready.split_off(0);
            for &i in &wave {
                for &v in &adj[i] {
                    if in_comp[v] {
                        indeg_c[v] -= 1;
                        if indeg_c[v] == 0 {
                            ready.push(v);
                        }
                    }
                }
            }
            cls.push(wave);
        }
        maxd = maxd.max(cls.len());
        per.push(cls);
    }
    let mut merged: Vec<Vec<usize>> = Vec::new();
    for d in 0..maxd {
        let mut wave = Vec::new();
        for cls in &per {
            if d < cls.len() {
                wave.extend_from_slice(&cls[d]);
            }
        }
        if !wave.is_empty() {
            wave.sort_unstable();
            merged.push(wave);
        }
    }
    merged
}

fn layers_per_component_csr(
    row_offsets: &[usize],
    cols: &[usize],
    components: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let n = row_offsets.len() - 1;
    let mut per: Vec<Vec<Vec<usize>>> = Vec::new();
    let mut maxd = 0usize;
    for comp in components {
        let mut in_comp = vec![false; n];
        for &i in comp {
            in_comp[i] = true;
        }
        let mut indeg_c = vec![0usize; n];
        for &u in comp {
            let (s, e) = (row_offsets[u], row_offsets[u + 1]);
            for &v in &cols[s..e] {
                if in_comp[v] {
                    indeg_c[v] += 1;
                }
            }
        }
        let mut ready: Vec<usize> = comp.iter().copied().filter(|&i| indeg_c[i] == 0).collect();
        let mut cls: Vec<Vec<usize>> = Vec::new();
        while !ready.is_empty() {
            ready.sort_unstable();
            let wave: Vec<usize> = ready.split_off(0);
            for &i in &wave {
                let (s, e) = (row_offsets[i], row_offsets[i + 1]);
                for &v in &cols[s..e] {
                    if in_comp[v] {
                        indeg_c[v] -= 1;
                        if indeg_c[v] == 0 {
                            ready.push(v);
                        }
                    }
                }
            }
            cls.push(wave);
        }
        maxd = maxd.max(cls.len());
        per.push(cls);
    }
    let mut merged: Vec<Vec<usize>> = Vec::new();
    for d in 0..maxd {
        let mut wave = Vec::new();
        for cls in &per {
            if d < cls.len() {
                wave.extend_from_slice(&cls[d]);
            }
        }
        if !wave.is_empty() {
            wave.sort_unstable();
            merged.push(wave);
        }
    }
    merged
}

fn time<F, R>(label: &str, mut f: F) -> (Duration, R)
where
    F: FnMut() -> R,
{
    let t0 = Instant::now();
    let r = f();
    (t0.elapsed(), r)
}

fn fmt_dur(d: Duration) -> String {
    format!("{} ms", d.as_secs_f64() * 1000.0)
}

fn main() {
    let p = parse_params();
    let sched_label = if p.sched_both {
        "both"
    } else if p.sched_csr {
        "csr"
    } else {
        "vec"
    };
    let delta_label = if p.delta_index_hashmap && p.delta_index_vec {
        "both"
    } else if p.delta_index_vec {
        "vec"
    } else {
        "hashmap"
    };
    println!(
        "bench_dag: n={} keys={} reads={} writes={} dist={:?} zipf_s={} iters={} seed={} sched={} delta_index={}",
        p.n, p.keys, p.reads, p.writes, p.dist, p.zipf_s, p.iters, p.seed, sched_label, delta_label
    );
    println!("delta_writes per tx={}", p.delta_writes);

    // Generate once and reuse for fairness
    let (gen_dur, access) = time("gen", || gen_access_sets(&p));
    println!("generated access sets in {}", fmt_dur(gen_dur));

    let mut naive_times = Vec::new();
    let mut opt_times = Vec::new();
    // Accumulators for scheduler summaries
    let mut sched_wave_times_vec = Vec::new();
    let mut sched_heap_times_vec = Vec::new();
    let mut sched_wave_times_csr = Vec::new();
    let mut sched_heap_times_csr = Vec::new();
    let mut naive_edges = 0usize;
    let mut opt_edges = 0usize;

    for i in 0..p.iters {
        let (t1, (adj_n, indeg_n)) = time("naive", || dag_build_naive(&access));
        let e1: usize = indeg_n.iter().sum();
        let _ = schedule_kahn(&adj_n, &indeg_n);
        naive_times.push(t1);
        naive_edges = e1; // same every run

        let (t2, (adj_o, indeg_o)) = time("opt", || dag_build_optimized(&access));
        let e2: usize = indeg_o.iter().sum();
        let _ = schedule_kahn(&adj_o, &indeg_o);
        opt_times.push(t2);
        opt_edges = e2;
        println!(
            "iter {}: naive={} (E={}), opt={} (E={})",
            i + 1,
            fmt_dur(t1),
            e1,
            fmt_dur(t2),
            e2
        );

        // Scheduler variants: per-wave vs heap using synthetic call keys
        let keys = gen_call_keys(p.n, p.seed.wrapping_add(1337 + i as u64));
        if p.sched_both || !p.sched_csr {
            let (t_wave, _order_wave) = time("sched_wave", || {
                schedule_wave_by_key(&adj_o, &indeg_o, &keys)
            });
            let (t_heap, _order_heap) = time("sched_heap", || {
                schedule_heap_by_key(&adj_o, &indeg_o, &keys)
            });
            println!(
                "iter {}: sched(vec) wave={} heap={}",
                i + 1,
                fmt_dur(t_wave),
                fmt_dur(t_heap)
            );
            sched_wave_times_vec.push(t_wave);
            sched_heap_times_vec.push(t_heap);
        }
        if p.sched_both || p.sched_csr {
            let (ro, cols, indeg_c) = adj_to_csr(&adj_o);
            let (t_wave, _order_wave) = time("sched_wave_csr", || {
                schedule_wave_by_key_csr(&ro, &cols, &indeg_c, &keys)
            });
            let (t_heap, _order_heap) = time("sched_heap_csr", || {
                schedule_heap_by_key_csr(&ro, &cols, &indeg_c, &keys)
            });
            println!(
                "iter {}: sched(csr) wave={} heap={}",
                i + 1,
                fmt_dur(t_wave),
                fmt_dur(t_heap)
            );
            sched_wave_times_csr.push(t_wave);
            sched_heap_times_csr.push(t_heap);
        }
    }

    let avg = |ts: &[Duration]| -> Duration {
        let total = ts.iter().fold(Duration::ZERO, |a, &b| a.saturating_add(b));
        if ts.is_empty() {
            Duration::ZERO
        } else {
            total / (ts.len() as u32)
        }
    };
    let min = |ts: &[Duration]| -> Duration { ts.iter().copied().min().unwrap_or(Duration::ZERO) };

    println!("-- summary --");
    println!(
        "naive: avg={} min={} edges={}",
        fmt_dur(avg(&naive_times)),
        fmt_dur(min(&naive_times)),
        naive_edges
    );
    println!(
        "opt:   avg={} min={} edges={}",
        fmt_dur(avg(&opt_times)),
        fmt_dur(min(&opt_times)),
        opt_edges
    );

    // Scheduler summaries
    if !sched_wave_times_vec.is_empty() {
        println!(
            "sched(vec): wave avg={} min={} | heap avg={} min={}",
            fmt_dur(avg(&sched_wave_times_vec)),
            fmt_dur(min(&sched_wave_times_vec)),
            fmt_dur(avg(&sched_heap_times_vec)),
            fmt_dur(min(&sched_heap_times_vec)),
        );
    }
    if !sched_wave_times_csr.is_empty() {
        println!(
            "sched(csr): wave avg={} min={} | heap avg={} min={}",
            fmt_dur(avg(&sched_wave_times_csr)),
            fmt_dur(min(&sched_wave_times_csr)),
            fmt_dur(avg(&sched_heap_times_csr)),
            fmt_dur(min(&sched_heap_times_csr)),
        );
    }

    // Interning + CSR + DSF benchmarks
    let (t_intern, (key_count, access_ids)) = time("intern", || intern_access_sets(&access));
    println!("intern: {} (keys={})", fmt_dur(t_intern), key_count);
    let (t_csr, (row_offsets, cols, indeg_csr)) =
        time("csr", || build_csr_from_ids(&access_ids, key_count));
    println!("csr: {} (E={})", fmt_dur(t_csr), cols.len());
    let (t_dsu, components) = time("dsu", || dsu_components(&access_ids, key_count));
    let comp_count = components.len();
    let max_comp = components.iter().map(|c| c.len()).max().unwrap_or(0);
    // Component size histogram (cumulative) over thresholds
    let thresholds: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
    let mut buckets = [0usize; 8];
    for c in &components {
        let sz = c.len();
        for (i, &t) in thresholds.iter().enumerate() {
            if sz <= t {
                buckets[i] += 1;
            }
        }
    }
    println!(
        "dsu: {} (components={} max={}) buckets(le): [1:{} 2:{} 4:{} 8:{} 16:{} 32:{} 64:{} 128:{}]",
        fmt_dur(t_dsu),
        comp_count,
        max_comp,
        buckets[0],
        buckets[1],
        buckets[2],
        buckets[3],
        buckets[4],
        buckets[5],
        buckets[6],
        buckets[7]
    );

    // Layers: monolithic Vec adjacency (optimized builder)
    let (adj_opt, indeg_opt) = dag_build_optimized(&access);
    let (t_layers_vec_mono, layers_vec_mono) =
        time("layers_vec_mono", || layers_from_dag(&adj_opt, &indeg_opt));
    let (t_layers_csr_mono, layers_csr_mono) = time("layers_csr_mono", || {
        layers_monolithic_csr(&row_offsets, &cols, &indeg_csr)
    });
    // Layers: per-component
    let (t_layers_vec_comp, layers_vec_comp) = time("layers_vec_comp", || {
        layers_per_component_vec(&adj_opt, &components)
    });
    let (t_layers_csr_comp, layers_csr_comp) = time("layers_csr_comp", || {
        layers_per_component_csr(&row_offsets, &cols, &components)
    });
    println!(
        "layers: vec_mono={} (L={}) | csr_mono={} (L={}) | vec_comp={} (L={}) | csr_comp={} (L={})",
        fmt_dur(t_layers_vec_mono),
        layers_vec_mono.len(),
        fmt_dur(t_layers_csr_mono),
        layers_csr_mono.len(),
        fmt_dur(t_layers_vec_comp),
        layers_vec_comp.len(),
        fmt_dur(t_layers_csr_comp),
        layers_csr_comp.len(),
    );
    // Peak layer widths
    let peak = |ls: &Vec<Vec<usize>>| -> usize { ls.iter().map(|w| w.len()).max().unwrap_or(0) };
    let avg_w = |ls: &Vec<Vec<usize>>| -> usize {
        let c = ls.len();
        if c == 0 {
            0
        } else {
            (ls.iter().map(|w| w.len()).sum::<usize>() + c / 2) / c
        }
    };
    let med_w = |ls: &Vec<Vec<usize>>| -> usize {
        let mut v: Vec<usize> = ls.iter().map(|w| w.len()).collect();
        if v.is_empty() {
            0
        } else {
            v.sort_unstable();
            if v.len() % 2 == 1 {
                v[v.len() / 2]
            } else {
                (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2
            }
        }
    };
    println!(
        "peaks: vec_mono={} | csr_mono={} | vec_comp={} | csr_comp={}",
        peak(&layers_vec_mono),
        peak(&layers_csr_mono),
        peak(&layers_vec_comp),
        peak(&layers_csr_comp)
    );
    println!(
        "avg:   vec_mono={} | csr_mono={} | vec_comp={} | csr_comp={}",
        avg_w(&layers_vec_mono),
        avg_w(&layers_csr_mono),
        avg_w(&layers_vec_comp),
        avg_w(&layers_csr_comp)
    );
    println!(
        "median:vec_mono={} | csr_mono={} | vec_comp={} | csr_comp={}",
        med_w(&layers_vec_mono),
        med_w(&layers_csr_mono),
        med_w(&layers_vec_comp),
        med_w(&layers_csr_comp)
    );
    // Simple per-layer-width cumulative hist (≤ [1,2,4,8,16,32,64,128]) for csr_comp variant
    let thresholds: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
    let mut buckets = [0usize; 8];
    for w in layers_csr_comp.iter().map(|l| l.len()) {
        for (i, &t) in thresholds.iter().enumerate() {
            if w <= t {
                buckets[i] += 1;
            }
        }
    }
    println!(
        "layer-width buckets(le) csr_comp: [1:{} 2:{} 4:{} 8:{} 16:{} 32:{} 64:{} 128:{}]",
        buckets[0],
        buckets[1],
        buckets[2],
        buckets[3],
        buckets[4],
        buckets[5],
        buckets[6],
        buckets[7]
    );

    // Build layers from optimized DAG (same edges) for merge benchmark
    let (adj_o, indeg_o) = dag_build_optimized(&access);
    let layers = layers_from_dag(&adj_o, &indeg_o);
    let layer_vertices: usize = layers.iter().map(|l| l.len()).sum();
    println!("layers: {} (vertices={})", layers.len(), layer_vertices);

    // Generate synthetic deltas once
    let (t_delta_gen, deltas) = time("delta_gen", || gen_deltas(&p, 777));
    let total_ops: usize = deltas.iter().map(|d| d.keys.len()).sum();
    println!(
        "generated deltas in {} (ops={})",
        fmt_dur(t_delta_gen),
        total_ops
    );

    // As in block.rs: deltas come as a Vec<(idx, Option<Result<Delta, _>>)>; here: all Ok
    let deltas_vec: Vec<(usize, Delta)> = (0..p.n).map(|i| (i, deltas[i].clone())).collect();

    // Merge benchmarks (multi-iteration): naive linear scan vs indexed lookups
    let mut merge_naive_times = Vec::new();
    let mut merge_build_map_times = Vec::new();
    let mut merge_opt_times = Vec::new();
    let mut merge_build_vec_times = Vec::new();
    let mut merge_vec_times = Vec::new();
    for i in 0..p.iters {
        // Naive scan per prepared index
        let mut state_naive: std::collections::HashMap<usize, i64> =
            std::collections::HashMap::new();
        let (t_merge_naive, _) = time("merge_naive", || {
            for layer in &layers {
                for &idx in layer {
                    if let Some((_, d)) = deltas_vec.iter().find(|(j, _)| *j == idx) {
                        for &k in &d.keys {
                            *state_naive.entry(k).or_insert(0) += 1;
                        }
                    }
                }
            }
        });
        merge_naive_times.push(t_merge_naive);

        if p.delta_index_hashmap {
            // Build index map and do O(1) lookups per prepared index
            let (t_build_map, deltas_map) = time("build_map", || {
                let mut m: std::collections::HashMap<usize, &Delta> =
                    std::collections::HashMap::with_capacity(deltas_vec.len() * 2);
                for (idx, d) in &deltas_vec {
                    m.insert(*idx, d);
                }
                m
            });
            merge_build_map_times.push(t_build_map);

            let mut state_opt: std::collections::HashMap<usize, i64> =
                std::collections::HashMap::new();
            let (t_merge_opt, _) = time("merge_opt", || {
                for layer in &layers {
                    for &idx in layer {
                        if let Some(d) = deltas_map.get(&idx) {
                            for &k in &d.keys {
                                *state_opt.entry(k).or_insert(0) += 1;
                            }
                        }
                    }
                }
            });
            merge_opt_times.push(t_merge_opt);

            println!(
                "iter {}: merge naive={} | build_map={} + opt={}",
                i + 1,
                fmt_dur(t_merge_naive),
                fmt_dur(t_build_map),
                fmt_dur(t_merge_opt)
            );
        }

        if p.delta_index_vec {
            // Build vec index and do direct lookups
            let (t_build_vec, deltas_idx) = time("build_vec", || {
                let mut v: Vec<Option<&Delta>> = vec![None; deltas_vec.len()];
                for (idx, d) in &deltas_vec {
                    v[*idx] = Some(d);
                }
                v
            });
            merge_build_vec_times.push(t_build_vec);
            let mut state_vec: std::collections::HashMap<usize, i64> =
                std::collections::HashMap::new();
            let (t_merge_vec, _) = time("merge_vec", || {
                for layer in &layers {
                    for &idx in layer {
                        if let Some(Some(d)) = deltas_idx.get(idx) {
                            for &k in &d.keys {
                                *state_vec.entry(k).or_insert(0) += 1;
                            }
                        }
                    }
                }
            });
            merge_vec_times.push(t_merge_vec);
            println!(
                "iter {}: merge naive={} | build_vec={} + vec={}",
                i + 1,
                fmt_dur(t_merge_naive),
                fmt_dur(t_build_vec),
                fmt_dur(t_merge_vec)
            );
        }
    }
    println!("-- merge summary -- (ops={})", total_ops);
    println!(
        "merge-naive: avg={} min={}",
        fmt_dur(avg(&merge_naive_times)),
        fmt_dur(min(&merge_naive_times))
    );
    if !merge_build_map_times.is_empty() {
        println!(
            "merge-opt(map):   build avg={} min={} | merge avg={} min={}",
            fmt_dur(avg(&merge_build_map_times)),
            fmt_dur(min(&merge_build_map_times)),
            fmt_dur(avg(&merge_opt_times)),
            fmt_dur(min(&merge_opt_times))
        );
    }
    if !merge_build_vec_times.is_empty() {
        println!(
            "merge-opt(vec):   build avg={} min={} | merge avg={} min={}",
            fmt_dur(avg(&merge_build_vec_times)),
            fmt_dur(min(&merge_build_vec_times)),
            fmt_dur(avg(&merge_vec_times)),
            fmt_dur(min(&merge_vec_times))
        );
    }
}
