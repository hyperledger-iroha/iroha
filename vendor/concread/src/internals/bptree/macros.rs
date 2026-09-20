macro_rules! debug_assert_leaf {
    ($x:expr) => {{
        debug_assert!($x.meta.is_leaf());
    }};
}

macro_rules! debug_assert_branch {
    ($x:expr) => {{
        debug_assert!($x.meta.is_branch());
    }};
}

macro_rules! self_meta {
    ($x:expr) => {{
        unsafe { &mut *($x as *mut Meta) }
    }};
}

/// Read node metadata without creating exclusive access to a shared generation.
macro_rules! self_meta_shared {
    ($x:expr) => {{
        unsafe { &*($x as *const Meta) }
    }};
}

macro_rules! branch_ref {
    ($x:expr, $k:ty, $v:ty, $c:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_branch() });
        unsafe { &mut *($x as *mut Branch<$k, $v, $c>) }
    }};
}

/// Borrow a shared branch directly, without creating an intermediate exclusive reference.
macro_rules! branch_ref_shared {
    ($x:expr, $k:ty, $v:ty, $c:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_branch() });
        unsafe { &*($x as *const Branch<$k, $v, $c>) }
    }};
}

/// Borrow a shared leaf directly, without creating an intermediate exclusive reference.
macro_rules! leaf_ref_shared {
    ($x:expr, $k:ty, $v:ty, $c:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_leaf() });
        unsafe { &*($x as *const Leaf<$k, $v, $c>) }
    }};
}

macro_rules! leaf_ref {
    ($x:expr, $k:ty, $v:ty, $c:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_leaf() });
        unsafe { &mut *($x as *mut Leaf<$k, $v, $c>) }
    }};
}

macro_rules! key_search {
    ($self:expr, $k:expr) => {{
        let (left, _) = $self.key.split_at($self.count());
        let inited: &[K] = unsafe { slice::from_raw_parts(left.as_ptr() as *const K, left.len()) };
        slice_search_linear(inited, $k)
    }};
}
