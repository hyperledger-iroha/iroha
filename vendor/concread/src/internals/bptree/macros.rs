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

macro_rules! branch_ref {
    ($x:expr, $k:ty, $v:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_branch() });
        unsafe { &mut *($x as *mut Branch<$k, $v>) }
    }};
}

/// Like [`branch_ref`], but yields &Leaf without coercing from &mut Leaf. This is useful
/// to avoid triggering Miri's analysis.
macro_rules! branch_ref_shared {
    ($x:expr, $k:ty, $v:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_branch() });
        unsafe { &*($x as *const Branch<$k, $v>) }
    }};
}

/// Like [`leaf_ref`], but yields &Leaf without coercing from &mut Leaf. This is useful
/// to avoid triggering Miri's analysis.
macro_rules! leaf_ref_shared {
    ($x:expr, $k:ty, $v:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_leaf() });
        unsafe { &*($x as *const Leaf<$k, $v>) }
    }};
}

macro_rules! leaf_ref {
    ($x:expr, $k:ty, $v:ty) => {{
        debug_assert!(unsafe { (*$x).meta.is_leaf() });
        unsafe { &mut *($x as *mut Leaf<$k, $v>) }
    }};
}

macro_rules! key_search {
    ($self:expr, $k:expr) => {{
        let (left, _) = $self.key.split_at($self.count());
        let inited: &[K] = unsafe { slice::from_raw_parts(left.as_ptr() as *const K, left.len()) };
        slice_search_linear(inited, $k)
    }};
}
