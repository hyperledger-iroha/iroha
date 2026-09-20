//! Allocation-free traversal and admitted empty-root reset of the original cursor.

use super::*;

// All pointers belong to one valid tree held by the original writer. Like final
// tree destruction, traversal uses a bounded stack: a balanced B+tree cannot
// have more branch levels than bits in its addressable entry count. It never
// constructs the allocating general iterator or clones a key/value.
fn visit_tree<K: Clone + Ord + Debug, V: Clone, C>(
    root: *mut Node<K, V, C>,
    mut visit: impl FnMut(*mut Node<K, V, C>),
) -> Option<usize> {
    let mut path = [(std::ptr::null_mut::<Node<K, V, C>>(), 0usize); usize::BITS as usize + 1];
    path[0].0 = root;
    let mut depth = 0usize;
    let mut count = 1usize;
    visit(root);
    loop {
        let (node, next) = path[depth];
        // SAFETY: the original held cursor retains this entire unchanged tree.
        if !unsafe { &*node }.is_leaf() {
            let branch = unsafe { &*node.cast::<Branch<K, V, C>>() };
            if next <= branch.count() {
                let child = branch.get_idx_unchecked(next);
                path[depth].1 = next.checked_add(1)?;
                depth = depth.checked_add(1)?;
                if depth >= path.len() {
                    return None;
                }
                count = count.checked_add(1)?;
                path[depth] = (child, 0);
                visit(child);
                continue;
            }
        }
        if depth == 0 {
            return Some(count);
        }
        depth -= 1;
    }
}

impl<K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>>
    CursorWrite<K, V, crate::bptree::Prepaid<P>>
{
    /// Count exact retirement slots under the unchanged original writer.
    pub(crate) fn admitted_clear_node_count(&self) -> Option<usize> {
        self.assert_operable();
        visit_tree(self.root, |_| {})
    }

    /// Execute only after the same cursor's complete reset demand was admitted.
    /// The caller owns a child checkpoint and keeps failure armed through funding
    /// cleanup. No payload or node is freed here; retirement goes to the existing
    /// original reader on publication, while abort restores the parent's suffix.
    pub(crate) fn clear_admitted_root(&mut self, nodes: usize) {
        assert!(self.edit_failed, "reset must be guarded before mutation");
        assert!(self.first_seen.remaining_capacity().unwrap() >= 1);
        let retired = self.last_seen.as_mut().expect("original retirement buffer");
        assert!(retired.remaining_capacity().unwrap() >= nodes);
        let root = Node::<K, V, P::Charge>::new_leaf(self.txid, &mut self.funding).cast();
        // Install allocation custody before any later assertion can unwind.
        self.first_seen.push(root);
        assert_eq!(
            visit_tree(self.root, |node| retired.push(node)),
            Some(nodes),
            "same held tree must match its complete retirement plan"
        );
        self.root = root;
        self.length = 0;
    }
}
