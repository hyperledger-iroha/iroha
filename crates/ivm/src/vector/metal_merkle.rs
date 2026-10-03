//! Merkle attempts retain their original physical owner through host readback.

use super::sha256_cpu::context::{Sha256Baseline, Sha256Context};
use super::{
    MetalKernel, MetalSelection, metal_dispatch, metal_input_buffer, metal_output_buffer,
    metal_runtime, metal_runtime_allowed, with_metal_state_try,
};
use iroha_accel::{HostOutput, ProcessResources};
use objc2::rc::autoreleasepool;
use objc2_foundation::NSUInteger;
use objc2_metal::MTLBuffer as _;

struct Readback<T> {
    selection: MetalSelection,
    value: T,
}

impl<T> Readback<T> {
    fn publish(self) -> Option<T> {
        // Host decoding can outlast a concurrent policy change or quarantine.
        // Re-enter this exact original owner only after all staging is complete.
        // No caller-owned destination has been changed when acceptance declines.
        self.selection.run(|| self.value)
    }
}

pub(crate) fn metal_sha256_leaves(blocks: &[[u8; 64]]) -> Option<HostOutput<[u8; 32]>> {
    leaves_attempt(blocks)?.publish()
}

fn leaves_attempt(blocks: &[[u8; 64]]) -> Option<Readback<HostOutput<[u8; 32]>>> {
    if !metal_runtime_allowed() {
        return None;
    }
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let selection = metal_runtime::current_selection()?;
            let n = blocks.len();
            if n == 0 {
                return Some(Readback {
                    selection,
                    value: ProcessResources::get()?.try_host_output(0).ok()?,
                });
            }
            let flat = blocks.as_flattened();
            let buf_blocks = metal_input_buffer(&ctx.device, flat, flat.len())?;
            let buf_out = metal_output_buffer(&ctx.device, n * 8 * core::mem::size_of::<u32>())?;
            metal_dispatch(
                &ctx.queue,
                &ctx.sha256_leaves,
                &[&buf_blocks, &buf_out],
                n as NSUInteger,
                1,
                "metal sha256 leaves",
                Some(MetalKernel::Sha256Leaves),
            )?;
            let ptr = buf_out.contents().as_ptr() as *const u32;
            // SAFETY: the completed kernel initialized the exact output extent;
            // the retained native buffer owns it throughout this staged copy.
            let words = unsafe { std::slice::from_raw_parts(ptr, n * 8) };
            let mut out = ProcessResources::get()?
                .try_host_output::<[u8; 32]>(n)
                .ok()?;
            for i in 0..n {
                let w = &words[i * 8..i * 8 + 8];
                let mut d = [0u8; 32];
                for (j, &word) in w.iter().enumerate() {
                    d[j * 4..j * 4 + 4].copy_from_slice(&word.to_be_bytes());
                }
                out[i] = d;
            }
            Some(Readback {
                selection,
                value: out,
            })
        })
    })
}

/// Shared padded-leaf preparation and readback for the complete root and tree
/// producers. Every overlapping scratch allocation keeps original host credit.
fn chunk_leaves(data: &[u8], chunk: usize) -> Option<HostOutput<[u8; 32]>> {
    if !(1..=32).contains(&chunk) {
        return None;
    }
    let leaves = data.len().div_ceil(chunk).max(1);
    fixed_chunk_leaves(data, chunk, leaves)
}

/// Fixed retained backing can have empty tail leaves, or ignore an input suffix.
/// Root and construction callers derive the same exact count from their input.
fn fixed_chunk_leaves(data: &[u8], chunk: usize, leaves: usize) -> Option<HostOutput<[u8; 32]>> {
    if !(1..=32).contains(&chunk) || leaves == 0 {
        return None;
    }
    let owner = ProcessResources::get()?;
    let mut blocks = owner.try_host_output::<u8>(leaves.checked_mul(64)?).ok()?;
    let bit_len = (chunk as u64 * 8).to_be_bytes();
    for (index, block) in blocks.chunks_exact_mut(64).enumerate() {
        let start = index * chunk;
        let end = start.saturating_add(chunk).min(data.len());
        if start < end {
            block[..end - start].copy_from_slice(&data[start..end]);
        }
        block[chunk] = 0x80;
        block[56..].copy_from_slice(&bit_len);
    }
    let (blocks_view, tail) = blocks.as_chunks::<64>();
    debug_assert!(tail.is_empty());
    let digests = metal_sha256_leaves(blocks_view)?;
    if digests.len() != leaves {
        return None;
    }
    drop(blocks);
    Some(digests)
}

pub(super) fn root_from_bytes(data: &[u8], chunk: usize) -> Option<[u8; 32]> {
    let selection = metal_runtime::current_selection()?;
    let digests = chunk_leaves(data, chunk)?;
    let root = metal_merkle_root(&digests)?;
    drop(digests);
    Readback {
        selection,
        value: root,
    }
    .publish()
}

/// Whole retained-tree construction uses the same bounded native leaf adapter,
/// then the ordinary fixed CPU destination and its canonical node constructor.
pub(super) fn tree_from_bytes(data: &[u8], chunk: usize) -> Option<crate::ByteMerkleTree> {
    tree_attempt(data, chunk)?.publish()
}

fn tree_attempt(data: &[u8], chunk: usize) -> Option<Readback<crate::ByteMerkleTree>> {
    let selection = metal_runtime::current_selection()?;
    let digests = chunk_leaves(data, chunk)?;
    let tree = crate::ByteMerkleTree::from_leaf_digests(&digests, chunk).ok()?;
    drop(digests);
    Some(Readback {
        selection,
        value: tree,
    })
}

/// The original tree and physical owner stay inseparable from staged digests.
struct RehashReadback<'tree> {
    tree: &'tree crate::ByteMerkleTree,
    selection: MetalSelection,
    digests: HostOutput<[u8; 32]>,
    baseline: Sha256Baseline,
    context: Sha256Context,
}

impl RehashReadback<'_> {
    fn install(self) -> bool {
        self.install_after_lock(|| {})
    }

    // The empty ordinary callback compiles away. Required regressions change
    // policy here to exercise the real boundary after both destination locks.
    fn install_after_lock(self, before_accept: impl FnOnce()) -> bool {
        let Some(update) = self.tree.lock_leaf_update(&self.digests) else {
            return false;
        };
        before_accept();
        self.selection
            .run(|| {
                if !self.baseline.is_current(self.context) {
                    return false;
                }
                update.install();
                true
            })
            .unwrap_or(false)
    }
}

fn rehash_attempt<'tree>(
    tree: &'tree crate::ByteMerkleTree,
    data: &[u8],
    baseline: Sha256Baseline,
    context: Sha256Context,
) -> Option<RehashReadback<'tree>> {
    if !baseline.is_current(context) {
        return None;
    }
    let selection = metal_runtime::current_selection()?;
    let digests = fixed_chunk_leaves(data, tree.chunk_size(), tree.leaf_count())?;
    Some(RehashReadback {
        tree,
        selection,
        digests,
        baseline,
        context,
    })
}

/// Full ordinary retained update, accepting the original owner only after locks.
pub(super) fn rehash_tree(
    tree: &crate::ByteMerkleTree,
    data: &[u8],
    baseline: Sha256Baseline,
    context: Sha256Context,
) -> bool {
    rehash_attempt(tree, data, baseline, context).is_some_and(RehashReadback::install)
}

#[cfg(test)]
pub(crate) fn metal_sha256_pairs_reduce(digests: &[[u8; 32]]) -> Option<[u8; 32]> {
    reduce(digests, false)
}

pub(crate) fn metal_merkle_root(digests: &[[u8; 32]]) -> Option<[u8; 32]> {
    reduce(digests, true)
}

fn reduce(digests: &[[u8; 32]], canonical_merkle: bool) -> Option<[u8; 32]> {
    if !metal_runtime_allowed() || digests.is_empty() {
        return None;
    }
    if digests.len() == 1 {
        let mut root = digests[0];
        if canonical_merkle {
            root[31] |= 1;
        }
        return Some(root);
    }
    reduce_attempt(digests, canonical_merkle)?.publish()
}

fn reduce_attempt(digests: &[[u8; 32]], canonical_merkle: bool) -> Option<Readback<[u8; 32]>> {
    if !metal_runtime_allowed() || digests.len() < 2 {
        return None;
    }
    let owner = ProcessResources::get()?;
    let mut cur = owner
        .try_host_output::<u8>(digests.len().checked_mul(32)?)
        .ok()?;
    cur.copy_from_slice(digests.as_flattened());
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let selection = metal_runtime::current_selection()?;
            let mut count = cur.len() / 32;
            while count > 1 {
                if canonical_merkle {
                    // Hash::prehashed marks every child before canonical parent
                    // hashing. Raw SHA pair callers retain their distinct semantics.
                    for node in cur.chunks_exact_mut(32) {
                        node[31] |= 1;
                    }
                }
                let pairs = count / 2;
                let has_leftover = (count & 1) != 0;
                let mut next = owner
                    .try_host_output::<u8>((pairs + usize::from(has_leftover)).checked_mul(32)?)
                    .ok()?;
                let pair_len = pairs * 64;
                let in_buf = metal_input_buffer(&ctx.device, &cur[..], pair_len)?;
                let out_buf =
                    metal_output_buffer(&ctx.device, pairs * 8 * core::mem::size_of::<u32>())?;
                metal_dispatch(
                    &ctx.queue,
                    &ctx.sha256_pairs,
                    &[&in_buf, &out_buf],
                    pairs as NSUInteger,
                    1,
                    "metal sha256 pairs reduce",
                    Some(MetalKernel::Sha256Pairs),
                )?;
                // SAFETY: the completed kernel initialized every pair, and the
                // original output owner outlives decoding into staged host bytes.
                let words = unsafe {
                    std::slice::from_raw_parts(out_buf.contents().as_ptr().cast::<u32>(), pairs * 8)
                };
                for pair in 0..pairs {
                    for j in 0..8 {
                        next[pair * 32 + j * 4..pair * 32 + j * 4 + 4]
                            .copy_from_slice(&words[pair * 8 + j].to_be_bytes());
                    }
                }
                if has_leftover {
                    let src_idx = (count - 1) * 32;
                    let dst_idx = pairs * 32;
                    next[dst_idx..dst_idx + 32].copy_from_slice(&cur[src_idx..src_idx + 32]);
                }
                cur = next;
                count = cur.len() / 32;
            }
            let mut root = [0u8; 32];
            root.copy_from_slice(&cur[..32]);
            if canonical_merkle {
                root[31] |= 1;
            }
            Some(Readback {
                selection,
                value: root,
            })
        })
    })
}

#[cfg(all(test, feature = "metal-hardware-tests"))]
#[path = "metal_merkle/qualification.rs"]
mod qualification;
