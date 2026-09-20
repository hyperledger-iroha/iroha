use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr;

#[cfg(all(test, not(miri)))]
use std::collections::BTreeSet;
#[cfg(all(test, not(miri)))]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(all(test, not(miri)))]
use std::sync::Mutex;

#[cfg(all(test, not(miri)))]
thread_local!(static LL_NODE_COUNTER: AtomicUsize = const { AtomicUsize::new(1) });
#[cfg(all(test, not(miri)))]
thread_local!(static LL_ALLOC_LIST: Mutex<BTreeSet<usize>> = const { Mutex::new(BTreeSet::new()) });

#[cfg(all(test, not(miri)))]
fn alloc_nid() -> usize {
    let nid: usize = LL_NODE_COUNTER.with(|nc| nc.fetch_add(1, Ordering::AcqRel));
    #[cfg(not(feature = "dhat-heap"))]
    {
        LL_ALLOC_LIST.with(|llist| llist.lock().unwrap().insert(nid));
    }
    // eprintln!("LL Allocate -> {:?}", nid);
    nid
}

#[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
fn release_nid(nid: usize) {
    {
        let r = LL_ALLOC_LIST.with(|llist| llist.lock().unwrap().remove(&nid));
        assert!(r);
    }
}

#[cfg(test)]
pub fn assert_released() {
    #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
    {
        let is_empt = LL_ALLOC_LIST.with(|llist| {
            let x = llist.lock().unwrap();
            eprintln!("Assert Released - Remaining -> {:?}", x);
            x.is_empty()
        });
        assert!(is_empt);
    }
}

pub trait LLWeight {
    fn ll_weight(&self) -> usize;
}

#[derive(Clone, Debug)]
pub(crate) struct LL<K>
where
    K: LLWeight + Clone + Debug,
{
    head: *mut LLNode<K>,
    tail: *mut LLNode<K>,
    size: usize,
    // tag: usize,
}

#[derive(Debug)]
struct LLNode<K>
where
    K: LLWeight + Clone + Debug,
{
    pub(crate) k: MaybeUninit<K>,
    next: *mut LLNode<K>,
    prev: *mut LLNode<K>,
    // tag: usize,
    #[cfg(all(test, not(miri)))]
    pub(crate) nid: usize,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct LLNodeRef<K>
where
    K: LLWeight + Clone + Debug,
{
    inner: *mut LLNode<K>,
}

impl<K> LLNodeRef<K>
where
    K: LLWeight + Clone + Debug,
{
    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn make_mut(&self) -> &mut K {
        &mut *(*self.inner).k.as_mut_ptr()
    }
}

impl<K> AsRef<K> for LLNodeRef<K>
where
    K: LLWeight + Clone + Debug,
{
    fn as_ref(&self) -> &K {
        unsafe { &*(*self.inner).k.as_ptr() }
    }
}

impl<K> PartialEq<&LLNodeOwned<K>> for &LLNodeRef<K>
where
    K: LLWeight + Clone + Debug,
{
    fn eq(&self, other: &&LLNodeOwned<K>) -> bool {
        self.inner == other.inner
    }
}

#[derive(Debug)]
pub(crate) struct LLNodeOwned<K>
where
    K: LLWeight + Clone + Debug,
{
    inner: *mut LLNode<K>,
}

impl<K> LLNodeOwned<K>
where
    K: LLWeight + Clone + Debug,
{
    fn into_inner(mut self) -> *mut LLNode<K> {
        let x = self.inner;
        self.inner = ptr::null_mut();
        x
    }

    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }
}

impl<K> PartialEq for &LLNodeOwned<K>
where
    K: LLWeight + Clone + Debug,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<K> AsRef<K> for LLNodeOwned<K>
where
    K: LLWeight + Clone + Debug,
{
    fn as_ref(&self) -> &K {
        unsafe { &*(*self.inner).k.as_ptr() }
    }
}

impl<K> AsMut<K> for LLNodeOwned<K>
where
    K: LLWeight + Clone + Debug,
{
    fn as_mut(&mut self) -> &mut K {
        unsafe { &mut *(*self.inner).k.as_mut_ptr() }
    }
}

impl<K> Drop for LLNodeOwned<K>
where
    K: LLWeight + Clone + Debug,
{
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe {
                debug_assert!((*self.inner).next.is_null());
                debug_assert!((*self.inner).prev.is_null());
            }
            panic!("dropping LLNodeOwned<K> which has valid content, this should never happen!");
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LLIterMut<'a, K>
where
    K: LLWeight + Clone + Debug,
{
    next: *mut LLNode<K>,
    end: *mut LLNode<K>,
    phantom: PhantomData<&'a K>,
}

impl<K> LL<K>
where
    K: LLWeight + Clone + Debug,
{
    pub(crate) fn new(// tag: usize
    ) -> Self {
        // assert!(tag > 0);
        let (head, tail) = LLNode::create_markers();
        LL {
            head,
            tail,
            size: 0,
            // tag,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn iter_mut(&self) -> LLIterMut<'_, K> {
        LLIterMut {
            next: unsafe { (*self.head).next },
            end: self.tail,
            phantom: PhantomData,
        }
    }

    // Append a k to the set, and return its pointer.
    pub(crate) fn append_k(&mut self, k: K) -> LLNodeRef<K> {
        let n = LLNode::new(k);
        self.append_n(n)
    }

    // Append an arbitrary node into this set, at the tail end.
    pub(crate) fn append_n(&mut self, owned: LLNodeOwned<K>) -> LLNodeRef<K> {
        // Who is to the left of tail?
        let n = owned.into_inner();
        unsafe {
            self.size += (*(*n).k.as_ptr()).ll_weight();
            // must be untagged
            // assert!((*n).tag == 0);
            // Tail has no trailing nodes.
            debug_assert!((*self.tail).next.is_null());
            // Tail must have a previous node.
            debug_assert!(!(*self.tail).prev.is_null());
            // What is that predecessor?
            let pred = (*self.tail).prev;
            debug_assert!(!pred.is_null());
            // Assert that the predecessor points at tail correctly.
            // We know now that:
            //
            // pred <-> tail
            debug_assert!((*pred).next == self.tail);

            // Tell our node where we are.
            // pred <-> n <-> tail
            (*n).prev = pred;
            (*n).next = self.tail;
            // (*n).tag = self.tag;
            (*pred).next = n;
            (*self.tail).prev = n;
            // We should have a prev and next
            debug_assert!(!(*n).prev.is_null());
            debug_assert!(!(*n).next.is_null());
            // And that prev's next is us, and next's prev is us.
            debug_assert!(!(*(*n).prev).next.is_null());
            debug_assert!(!(*(*n).next).prev.is_null());
            debug_assert!((*(*n).prev).next == n);
            debug_assert!((*(*n).next).prev == n);
            // We have asserted the list is sane.
        };
        LLNodeRef { inner: n }
    }

    // Given a node ptr, extract and put it at the tail. IE hit.
    pub(crate) fn touch(&mut self, n: LLNodeRef<K>) {
        debug_assert!(self.size > 0);
        if n.inner == unsafe { (*self.tail).prev } {
            // Done, no-op
        } else {
            let previous_size = self.size;

            let owned = self.extract(n);
            self.append_n(owned);

            let after_size = self.size;

            // Ensure that we didn't actually alter the size of the list during the operation,
            // we only re-arranged the content.
            debug_assert_eq!(previous_size, after_size);
        }
    }

    // remove this node from the ll, and return it's ptr.
    pub(crate) fn pop(&mut self) -> Option<LLNodeOwned<K>> {
        let next = unsafe { (*self.head).next };
        if next == self.tail {
            None
        } else {
            let n = unsafe { (*self.head).next };
            let owned = self.extract(LLNodeRef { inner: n });
            debug_assert!(!owned.is_null());
            debug_assert!(owned.inner != self.head);
            debug_assert!(owned.inner != self.tail);
            Some(owned)
        }
    }

    pub(crate) fn pop_n_free(&mut self) -> Option<K> {
        if let Some(owned) = self.pop() {
            let ll_node = owned.into_inner();
            let k = LLNode::into_inner(ll_node);
            Some(k)
        } else {
            None
        }
    }

    // Cut a node out from this list from any location.
    pub(crate) fn extract(&mut self, n: LLNodeRef<K>) -> LLNodeOwned<K> {
        assert!(!n.is_null());
        assert!(self.size > 0);
        unsafe {
            // We should have a prev and next
            debug_assert!(!(*n.inner).prev.is_null());
            debug_assert!(!(*n.inner).next.is_null());
            // And that prev's next is us, and next's prev is us.
            // This is asserting that we have a proper construction
            //
            // prev <-> n <-> next
            //
            debug_assert!(!(*(*n.inner).prev).next.is_null());
            debug_assert!(!(*(*n.inner).next).prev.is_null());
            debug_assert!((*(*n.inner).prev).next == n.inner);
            debug_assert!((*(*n.inner).next).prev == n.inner);
            // And we belong to this set
            // assert!((*n).tag == self.tag);
            self.size -= (*(*n.inner).k.as_ptr()).ll_weight();
        }

        unsafe {
            let prev = (*n.inner).prev;
            let next = (*n.inner).next;
            // Currently is:
            //
            // prev <-> n <-> next
            (*next).prev = prev;
            (*prev).next = next;
            // Now is
            // prev <-> next

            // Null things for paranoia.
            if cfg!(test) || cfg!(debug_assertions) {
                (*n.inner).prev = ptr::null_mut();
                (*n.inner).next = ptr::null_mut();
            }
            // Finally, we have
            //
            // null <- n -> null
            // prev <-> next

            // (*n).tag = 0;
        }

        LLNodeOwned { inner: n.inner }
    }

    pub(crate) fn len(&self) -> usize {
        self.size
    }

    pub(crate) fn drop_head(&mut self) {
        if let Some(owned) = self.pop() {
            let n = owned.into_inner();
            LLNode::free(n);
        }
    }

    pub(crate) fn peek_head(&self) -> Option<&K> {
        debug_assert!(!self.head.is_null());
        let next = unsafe { (*self.head).next };
        if next == self.tail {
            None
        } else {
            let l = unsafe {
                let ptr = (*next).k.as_ptr();
                &(*ptr) as &K
            };
            Some(l)
        }
    }

    #[cfg(test)]
    pub(crate) fn peek_tail(&self) -> Option<&K> {
        debug_assert!(!self.tail.is_null());
        let prev = unsafe { (*self.tail).prev };
        if prev == self.head {
            None
        } else {
            let l = unsafe {
                let ptr = (*prev).k.as_ptr();
                &(*ptr) as &K
            };
            Some(l)
        }
    }

    #[cfg(test)]
    pub(crate) fn verify(&self) {
        // Walk the list to ensure it's sane.

        let head = self.head;
        let tail = self.tail;

        let expect_size = self.size;
        let mut size = 0;

        // Establish that the sentinel nodes exist, and that they
        // have the correct out bound null/non-null pointers.
        assert_ne!(head, tail);

        unsafe {
            assert!((*head).prev.is_null());
            assert!(!(*head).next.is_null());

            assert!((*tail).next.is_null());
            assert!(!(*tail).prev.is_null());
        }

        // Now we walk each item to assert that they have the correct structure.
        let mut n = unsafe { (*head).next };
        // We have to manually check head here.
        unsafe {
            assert_eq!((*n).prev, head);
        }

        while n != tail {
            unsafe {
                // Setup the pointer for the next iteration.
                let next = (*n).next;

                // Add the size.
                size += (*(*n).k.as_ptr()).ll_weight();

                // assert we have outbound pointers.
                assert!(!(*n).prev.is_null());
                assert!(!(*n).next.is_null());

                // Assert that the previous node points to us.
                assert!(!(*(*n).prev).next.is_null());
                assert!((*(*n).prev).next == n);
                // Assert that the next node points back to us.
                // NOTE: This accounts for tail pointing back to us.
                assert!(!(*(*n).next).prev.is_null());
                assert!((*(*n).next).prev == n);

                n = next;
            }
        }

        assert_eq!(expect_size, size);
    }
}

impl<K> Drop for LL<K>
where
    K: LLWeight + Clone + Debug,
{
    fn drop(&mut self) {
        let head = self.head;
        let tail = self.tail;

        debug_assert!(head != tail);

        // Get the first node.
        let mut n = unsafe { (*head).next };
        while n != tail {
            unsafe {
                let next = (*n).next;
                // For sanity - we want to check that the node preceding us is the correct link.
                debug_assert!((*next).prev == n);

                // K is not a null pointer.
                debug_assert!(!(*n).k.as_mut_ptr().is_null());
                // ptr::drop_in_place((*n).k.as_mut_ptr());
                // Now we can proceed.
                LLNode::free(n);
                n = next;
            }
        }

        LLNode::free_marker(head);
        LLNode::free_marker(tail);

        // #[cfg(all(test, not(miri)))]
        // assert_released();
    }
}

impl<K> LLNode<K>
where
    K: LLWeight + Clone + Debug,
{
    #[inline]
    pub(crate) fn create_markers() -> (*mut Self, *mut Self) {
        let head = Box::into_raw(Box::new(LLNode {
            k: MaybeUninit::uninit(),
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
            // tag: 0,
            #[cfg(all(test, not(miri)))]
            nid: alloc_nid(),
        }));
        let tail = Box::into_raw(Box::new(LLNode {
            k: MaybeUninit::uninit(),
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
            // tag: 0,
            #[cfg(all(test, not(miri)))]
            nid: alloc_nid(),
        }));
        unsafe {
            (*head).next = tail;
            (*tail).prev = head;
        }
        (head, tail)
    }

    #[inline]
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new(
        k: K,
        // tag: usize
    ) -> LLNodeOwned<K> {
        let b = Box::new(LLNode {
            k: MaybeUninit::new(k),
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
            // tag,
            #[cfg(all(test, not(miri)))]
            nid: alloc_nid(),
        });
        let inner = Box::into_raw(b);
        LLNodeOwned { inner }
    }

    #[inline]
    fn into_inner(v: *mut Self) -> K {
        debug_assert!(!v.is_null());
        let llnode = unsafe { Box::from_raw(v) };
        let k = unsafe { llnode.k.assume_init() };
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        release_nid(llnode.nid);

        k
    }

    #[inline]
    fn free(v: *mut Self) {
        // drop the inner k.
        let _ = Self::into_inner(v);
    }

    #[inline]
    fn free_marker(v: *mut Self) {
        debug_assert!(!v.is_null());
        let _llnode = unsafe { Box::from_raw(v) };
        // Markers never have a k to drop.
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        release_nid(_llnode.nid)
    }
}

impl<K> AsRef<K> for LLNode<K>
where
    K: LLWeight + Clone + Debug,
{
    fn as_ref(&self) -> &K {
        unsafe {
            let ptr = self.k.as_ptr();
            &(*ptr) as &K
        }
    }
}

impl<K> AsMut<K> for LLNode<K>
where
    K: LLWeight + Clone + Debug,
{
    fn as_mut(&mut self) -> &mut K {
        unsafe {
            let ptr = self.k.as_mut_ptr();
            &mut (*ptr) as &mut K
        }
    }
}

impl<'a, K> Iterator for LLIterMut<'a, K>
where
    K: LLWeight + Clone + Debug,
{
    type Item = &'a mut K;

    fn next(&mut self) -> Option<Self::Item> {
        debug_assert!(!self.next.is_null());
        if self.next == self.end {
            None
        } else {
            let r = Some(unsafe { (*self.next).as_mut() });
            self.next = unsafe { (*self.next).next };
            r
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{assert_released, LLWeight, LL};

    impl LLWeight for Box<usize> {
        #[inline]
        fn ll_weight(&self) -> usize {
            1
        }
    }

    #[test]
    fn test_cache_arc_ll_basic() {
        // We test with box so that we leak on error
        let mut ll: LL<Box<usize>> = LL::new();
        assert!(ll.len() == 0);
        // Allocate new nodes
        let n1 = ll.append_k(Box::new(1));
        let n2 = ll.append_k(Box::new(2));
        let _n3 = ll.append_k(Box::new(3));
        let n4 = ll.append_k(Box::new(4));
        // Check that n1 is the head, n3 is tail.
        assert!(ll.len() == 4);
        assert!(ll.peek_head().unwrap().as_ref() == &1);
        assert!(ll.peek_tail().unwrap().as_ref() == &4);

        // Touch 2, it's now tail.
        ll.touch(n2.clone());
        assert!(ll.len() == 4);
        assert!(ll.peek_head().unwrap().as_ref() == &1);
        assert!(ll.peek_tail().unwrap().as_ref() == &2);

        // Touch 1 (head), it's the tail now.
        ll.touch(n1.clone());
        assert!(ll.len() == 4);
        assert!(ll.peek_head().unwrap().as_ref() == &3);
        assert!(ll.peek_tail().unwrap().as_ref() == &1);

        // Touch 1 (tail), it stays as tail.
        ll.touch(n1.clone());
        assert!(ll.len() == 4);
        assert!(ll.peek_head().unwrap().as_ref() == &3);
        assert!(ll.peek_tail().unwrap().as_ref() == &1);

        // pop from head
        let n3 = ll.pop().unwrap();
        assert!(ll.len() == 3);
        assert!(ll.peek_head().unwrap().as_ref() == &4);
        assert!(ll.peek_tail().unwrap().as_ref() == &1);

        // cut a node out from any (head, mid, tail)
        let n2 = ll.extract(n2);
        assert!(ll.len() == 2);
        assert!(ll.peek_head().unwrap().as_ref() == &4);
        assert!(ll.peek_tail().unwrap().as_ref() == &1);

        let n1 = ll.extract(n1);
        assert!(ll.len() == 1);
        assert!(ll.peek_head().unwrap().as_ref() == &4);
        assert!(ll.peek_tail().unwrap().as_ref() == &4);

        // test touch on ll of size 1
        ll.touch(n4);
        assert!(ll.len() == 1);
        assert!(ll.peek_head().unwrap().as_ref() == &4);
        assert!(ll.peek_tail().unwrap().as_ref() == &4);
        // Remove last
        let n4 = ll.pop().unwrap();
        assert!(ll.len() == 0);
        assert!(ll.peek_head().is_none());
        assert!(ll.peek_tail().is_none());

        // Add them all back so they are dropped.
        ll.append_n(n1);
        ll.append_n(n2);
        ll.append_n(n3);
        ll.append_n(n4);

        drop(ll);

        assert_released();
    }

    #[derive(Clone, Debug)]
    struct Weighted {
        _i: u64,
    }

    impl LLWeight for Weighted {
        fn ll_weight(&self) -> usize {
            8
        }
    }

    #[test]
    fn test_cache_arc_ll_weighted() {
        // We test with box so that we leak on error
        let mut ll: LL<Weighted> = LL::new();
        assert!(ll.len() == 0);
        let _n1 = ll.append_k(Weighted { _i: 1 });
        assert!(ll.len() == 8);
        let _n2 = ll.append_k(Weighted { _i: 2 });
        assert!(ll.len() == 16);
        let n1 = ll.pop().unwrap();
        assert!(ll.len() == 8);
        let n2 = ll.pop().unwrap();
        assert!(ll.len() == 0);
        // Add back so they drop
        ll.append_n(n1);
        ll.append_n(n2);

        drop(ll);

        assert_released();
    }
}
