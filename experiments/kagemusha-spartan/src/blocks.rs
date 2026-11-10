//! Stage A2 block census: how the real Receive pair splits into repeated
//! gadget blocks, how many values cross blocks (copy lookups into a value
//! table, the uniform layout's wiring) and how large each block type's
//! template is (the verifier's matrix-MLE work).
//!
//! Gadgets open a scope with `let _s = blocks::scope("name");`. Scopes form
//! one process-wide tree; each thread keeps its own stack, and builders tag
//! every recorded row and lookup with the innermost open scope. Scopes are
//! recorded only while `ENABLED` is set (the census), so cost runs pay
//! nothing.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use ff::PrimeField;

use crate::sparse::{Assembly, SLc};

/// Record scopes (the block census sets it before assembling).
pub static ENABLED: AtomicBool = AtomicBool::new(false);

/// (name, parent) per scope id; id 0 is the root.
static TREE: Mutex<Vec<(&'static str, u32)>> = Mutex::new(Vec::new());

thread_local! {
    static STACK: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

/// The innermost open scope of this thread (0 outside every scope).
pub fn current() -> u32 {
    STACK.with(|s| s.borrow().last().copied().unwrap_or(0))
}

/// Closes its scope when dropped.
pub struct Guard(bool);

impl Drop for Guard {
    fn drop(&mut self) {
        if self.0 {
            STACK.with(|s| {
                s.borrow_mut().pop();
            });
        }
    }
}

/// Opens a scope named `name` under the current one.
pub fn scope(name: &'static str) -> Guard {
    if !ENABLED.load(Ordering::Relaxed) {
        return Guard(false);
    }
    let parent = current();
    let id = {
        let mut tree = TREE.lock().unwrap();
        if tree.is_empty() {
            tree.push(("root", u32::MAX));
        }
        tree.push((name, parent));
        (tree.len() - 1) as u32
    };
    STACK.with(|s| s.borrow_mut().push(id));
    Guard(true)
}

fn tree() -> Vec<(&'static str, u32)> {
    let mut tree = TREE.lock().unwrap();
    if tree.is_empty() {
        tree.push(("root", u32::MAX));
    }
    tree.clone()
}

/// Which scope a block is: the innermost scope of a row, or its outermost
/// gadget scope (depth 2: directly under an origin scope such as "own").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Inner,
    Outer,
}

struct TypeStats {
    name: &'static str,
    instances: usize,
    rows: usize,
    nnz: usize,
    lookups: usize,
    locals: usize,
    ports: usize,
    variants: std::collections::HashSet<u64>,
}

/// The census of one assembly at one level, as printable lines.
pub fn census<F: PrimeField>(asm: &Assembly<F>, level: Level) -> String {
    let tree = tree();
    let mut depth = vec![0u32; tree.len()];
    for id in 1..tree.len() {
        depth[id] = depth[tree[id].1 as usize] + 1;
    }
    // Scopes named in ZKS_FLATTEN are transparent: their rows belong to the
    // nearest enclosing scope that is not.
    let flatten: Vec<String> = std::env::var("ZKS_FLATTEN")
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let transparent: Vec<bool> = tree
        .iter()
        .map(|(name, _)| flatten.iter().any(|f| f == name))
        .collect();
    let block_of = |scope: u32| -> u32 {
        match level {
            Level::Inner => {
                let mut s = scope;
                while transparent[s as usize] && depth[s as usize] > 1 {
                    s = tree[s as usize].1;
                }
                s
            }
            Level::Outer => {
                let mut s = scope;
                while depth[s as usize] > 2 {
                    s = tree[s as usize].1;
                }
                s
            }
        }
    };
    let row_block: Vec<u32> = (0..asm.rows.len())
        .map(|i| block_of(asm.row_scope.get(i).copied().unwrap_or(0)))
        .collect();
    let lookup_block: Vec<u32> = (0..asm.lookups.len())
        .map(|i| block_of(asm.lookup_scope.get(i).copied().unwrap_or(0)))
        .collect();
    // Distinct referencing blocks per variable (the constant 0 is a
    // structured column every block reads directly).
    let n = asm.vals.len();
    let mut first = vec![u32::MAX; n];
    let mut more: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut note = |v: u32, b: u32| {
        if v == 0 {
            return;
        }
        let f = &mut first[v as usize];
        if *f == u32::MAX {
            *f = b;
        } else if *f != b {
            let e = more.entry(v).or_default();
            if !e.contains(&b) {
                e.push(b);
            }
        }
    };
    let terms = |lc: &SLc<F>| lc.iter().map(|(v, _)| *v).collect::<Vec<u32>>();
    for (i, (a, b, c)) in asm.rows.iter().enumerate() {
        for v in terms(a).into_iter().chain(terms(b)).chain(terms(c)) {
            note(v, row_block[i]);
        }
    }
    for (j, (_, tuple)) in asm.lookups.iter().enumerate() {
        for lc in tuple {
            for v in terms(lc) {
                note(v, lookup_block[j]);
            }
        }
    }
    let fanout = |v: u32| -> usize {
        if first[v as usize] == u32::MAX {
            0
        } else {
            1 + more.get(&v).map_or(0, |e| e.len())
        }
    };
    let wires = more.len();
    let copies: usize = more.values().map(|e| e.len() + 1).sum();
    // Rows and lookups per block, in order.
    let mut rows_of: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, b) in row_block.iter().enumerate() {
        rows_of.entry(*b).or_default().push(i);
    }
    let mut lookups_of: HashMap<u32, Vec<usize>> = HashMap::new();
    for (j, b) in lookup_block.iter().enumerate() {
        lookups_of.entry(*b).or_default().push(j);
    }
    let mut blocks: Vec<u32> = rows_of.keys().chain(lookups_of.keys()).copied().collect();
    blocks.sort_unstable();
    blocks.dedup();
    // Canonical forms: variables renumbered by first appearance; the shape
    // hash leaves out coefficient values, the full hash keeps them.
    let mut types: HashMap<(&'static str, u64), TypeStats> = HashMap::new();
    let mut block_type: HashMap<u32, (&'static str, u64)> = HashMap::new();
    let mut members: HashMap<(&'static str, u64), Vec<u32>> = HashMap::new();
    let (mut glue_rows, mut glue_nnz, mut glue_lookups) = (0usize, 0usize, 0usize);
    for b in &blocks {
        let name = tree[*b as usize].0;
        let origin = depth[*b as usize] <= 1;
        let rows = rows_of.get(b).map_or(&[][..], |v| v.as_slice());
        let looks = lookups_of.get(b).map_or(&[][..], |v| v.as_slice());
        let nnz: usize = rows
            .iter()
            .map(|&i| asm.rows[i].0.len() + asm.rows[i].1.len() + asm.rows[i].2.len())
            .sum();
        if origin {
            glue_rows += rows.len();
            glue_nnz += nnz;
            glue_lookups += looks.len();
            block_type.insert(*b, (name, 0));
            continue;
        }
        let mut local: HashMap<u32, u32> = HashMap::new();
        let mut order: Vec<u32> = Vec::new();
        let mut shape = std::collections::hash_map::DefaultHasher::new();
        let mut full = std::collections::hash_map::DefaultHasher::new();
        let mut put = |lc: &SLc<F>,
                       shape: &mut std::collections::hash_map::DefaultHasher,
                       full: &mut std::collections::hash_map::DefaultHasher| {
            lc.len().hash(shape);
            lc.len().hash(full);
            for (v, c) in lc {
                let id = if *v == 0 {
                    0
                } else {
                    *local.entry(*v).or_insert_with(|| {
                        order.push(*v);
                        order.len() as u32
                    })
                };
                id.hash(shape);
                id.hash(full);
                c.to_repr().as_ref().hash(full);
            }
        };
        for &i in rows {
            let (a, bb, c) = &asm.rows[i];
            put(a, &mut shape, &mut full);
            put(bb, &mut shape, &mut full);
            put(c, &mut shape, &mut full);
        }
        for &j in looks {
            let (tid, tuple) = &asm.lookups[j];
            tid.hash(&mut shape);
            tid.hash(&mut full);
            for lc in tuple {
                put(lc, &mut shape, &mut full);
            }
        }
        let ports: Vec<bool> = order.iter().map(|v| fanout(*v) >= 2).collect();
        ports.hash(&mut shape);
        ports.hash(&mut full);
        let key = (name, shape.finish());
        block_type.insert(*b, key);
        members.entry(key).or_default().push(*b);
        let t = types.entry(key).or_insert_with(|| TypeStats {
            name,
            instances: 0,
            rows: rows.len(),
            nnz,
            lookups: looks.len(),
            locals: order.len(),
            ports: ports.iter().filter(|p| **p).count(),
            variants: Default::default(),
        });
        t.instances += 1;
        t.variants.insert(full.finish());
    }
    // Wires: chains between consecutive blocks of one type (wirable by a
    // shift, no lookup), copies by block name, and the commonest flows.
    let mut index: HashMap<u32, usize> = HashMap::new();
    for blocks in members.values() {
        for (i, b) in blocks.iter().enumerate() {
            index.insert(*b, i);
        }
    }
    let (mut chain_wires, mut chain_copies) = (0usize, 0usize);
    let mut copies_by_name: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut flows: HashMap<String, usize> = HashMap::new();
    for (v, extra) in &more {
        let refs: Vec<u32> = std::iter::once(first[*v as usize])
            .chain(extra.iter().copied())
            .collect();
        let keys: Vec<(&'static str, u64)> = refs
            .iter()
            .map(|b| block_type.get(b).copied().unwrap_or(("?", 0)))
            .collect();
        if refs.len() == 2 && keys[0] == keys[1] && keys[0].1 != 0 {
            let (i, j) = (index[&refs[0]], index[&refs[1]]);
            if i.abs_diff(j) == 1 {
                chain_wires += 1;
                chain_copies += 2;
            }
        }
        for k in &keys {
            *copies_by_name.entry(k.0).or_insert(0) += 1;
        }
        let mut names: Vec<&str> = keys.iter().map(|k| k.0).collect();
        names.sort_unstable();
        names.dedup();
        *flows.entry(names.join("+")).or_insert(0) += 1;
    }
    let mut list: Vec<&TypeStats> = types.values().collect();
    list.sort_by_key(|t| std::cmp::Reverse(t.instances * (t.rows + t.lookups)));
    let instances: usize = list.iter().map(|t| t.instances).sum();
    let template_nnz: usize = list.iter().map(|t| t.nnz).sum();
    let varied = list.iter().filter(|t| t.variants.len() > 1).count();
    let mut by_name: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for t in &list {
        let e = by_name.entry(t.name).or_insert((0, 0));
        e.0 += 1;
        e.1 += t.instances;
    }
    let mut out = format!(
        "  level {level:?}: {} rows, {} lookups, filler {}; {} blocks in {} types ({} names, {} with coefficient variants); template nonzeros {}; glue {} rows, {} nonzeros, {} lookups; {} wires crossing blocks, {} copy lookups\n",
        asm.rows.len(),
        asm.lookups.len(),
        asm.filler,
        instances,
        list.len(),
        by_name.len(),
        varied,
        template_nnz,
        glue_rows,
        glue_nnz,
        glue_lookups,
        wires,
        copies
    );
    out += "    type                      blocks   rows  nnz  lookups locals ports variants   total rows\n";
    for t in list.iter().take(40) {
        out += &format!(
            "    {:<24} {:>7} {:>6} {:>5} {:>6} {:>6} {:>5} {:>8} {:>12}\n",
            t.name,
            t.instances,
            t.rows,
            t.nnz,
            t.lookups,
            t.locals,
            t.ports,
            t.variants.len(),
            t.instances * t.rows
        );
    }
    if list.len() > 40 {
        let rest: usize = list.iter().skip(40).map(|t| t.instances * t.rows).sum();
        out += &format!("    ... {} more types, {} rows\n", list.len() - 40, rest);
    }
    out += &format!(
        "    chain wires (consecutive blocks of one type, wirable by a shift): {chain_wires}, saving {chain_copies} copy lookups\n"
    );
    let mut by_copies: Vec<(&&'static str, &usize)> = copies_by_name.iter().collect();
    by_copies.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    out += "    copy lookups by block name: ";
    out += &by_copies
        .iter()
        .take(14)
        .map(|(n, c)| format!("{n} {c}"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push('\n');
    let mut top: Vec<(&String, &usize)> = flows.iter().collect();
    top.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    out += "    wires by the blocks that share them: ";
    out += &top
        .iter()
        .take(14)
        .map(|(n, c)| format!("{n} {c}"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push('\n');
    out += "    by name (types, blocks): ";
    out += &by_name
        .iter()
        .map(|(n, (t, b))| format!("{n} {t}/{b}"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push('\n');
    out
}
