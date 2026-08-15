// Allocation accounting for standard-library owned node collections.
/// Resources charged by one measured [`DecodeLimits`] scope.
///
/// The counters are cumulative allocation requests, not a sample of allocator
/// residency. A successful owned decode can therefore retain this value as a
/// conservative upper charge without inspecting the decoded Rust type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[doc(hidden)]
pub struct DecodeAllocationUsage {
    total_elements: usize,
    total_allocated_bytes: usize,
}
impl DecodeAllocationUsage {
    /// Sequence elements charged in the measured scope.
    #[must_use]
    pub const fn total_elements(self) -> usize {
        self.total_elements
    }
    /// Allocation bytes charged in the measured scope.
    #[must_use]
    pub const fn total_allocated_bytes(self) -> usize {
        self.total_allocated_bytes
    }
}
/// Run a synchronous decode scope and return its exact cumulative charges.
///
/// Nested decode limits still compose normally: this scope records its own counters while every
/// already-active outer scope observes the same charges. The guard is dropped before this function
/// returns, including during unwind. Lazy work returned by `decode` is outside the measured scope.
#[doc(hidden)]
pub fn with_decode_limits_measured<T>(
    limits: DecodeLimits,
    decode: impl FnOnce() -> T,
) -> (T, DecodeAllocationUsage) {
    let context = DecodeBudgetContext::new(limits);
    let counters = Arc::clone(&context.layers[0].budget.counters);
    let guard = DecodeLimitsGuard::enter_context(&context);
    let result = decode();
    drop(guard);
    let usage = DecodeAllocationUsage {
        total_elements: usize::try_from(counters.total_elements.load(Ordering::Relaxed))
            .unwrap_or(usize::MAX),
        total_allocated_bytes: usize::try_from(
            counters.total_allocated_bytes.load(Ordering::Relaxed),
        )
        .unwrap_or(usize::MAX),
    };
    (result, usage)
}
/// Build a fixed array in its final storage and drop every initialized element
/// if decoding fails or unwinds.
fn try_decode_array<T, const N: usize>(
    mut decode: impl FnMut() -> Result<T, Error>,
) -> Result<[T; N], Error> {
    struct Initialized<T> {
        first: *mut T,
        len: usize,
    }
    impl<T> Drop for Initialized<T> {
        fn drop(&mut self) {
            // SAFETY: `first` points to the array's final storage and exactly
            // `len` consecutive elements have been initialized there.
            unsafe {
                core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.first, self.len));
            }
        }
    }
    let mut array = core::mem::MaybeUninit::<[T; N]>::uninit();
    let mut initialized = Initialized {
        first: array.as_mut_ptr().cast::<T>(),
        len: 0,
    };
    for _ in 0..N {
        let value = decode()?;
        // SAFETY: `len < N`, this slot is inside the array and has not been
        // initialized yet. The guard records it immediately after the write.
        unsafe { initialized.first.add(initialized.len).write(value) };
        initialized.len += 1;
    }
    core::mem::forget(initialized);
    // SAFETY: all `N` elements were initialized above and their cleanup guard
    // was forgotten only after the final successful write.
    Ok(unsafe { array.assume_init() })
}
/// Allocator-visible bytes owned by one `Box<T>` allocation.
#[doc(hidden)]
pub const fn owned_box_allocation_bytes<T>() -> usize {
    core::mem::size_of::<T>()
}
fn owned_counted_pointer_allocation_bytes<T, Counter>() -> Result<usize, Error> {
    let counters = Layout::array::<Counter>(2).map_err(|_| Error::LengthMismatch)?;
    let (layout, _) = counters
        .extend(Layout::new::<T>())
        .map_err(|_| Error::LengthMismatch)?;
    Ok(layout.pad_to_align().size())
}
/// Allocator-visible bytes owned by one `Rc<T>` allocation.
///
/// The standard-library wrapper stores two `Cell<usize>` counters followed by
/// `T` in a C-layout allocation. `Layout::extend` accounts for padding before
/// `T` and at the allocation tail.
#[doc(hidden)]
pub fn owned_rc_allocation_bytes<T>() -> Result<usize, Error> {
    owned_counted_pointer_allocation_bytes::<T, Cell<usize>>()
}
/// Allocator-visible bytes owned by one `Arc<T>` allocation.
///
/// The standard-library wrapper stores two atomic counters followed by `T` in
/// a C-layout allocation.
#[doc(hidden)]
pub fn owned_arc_allocation_bytes<T>() -> Result<usize, Error> {
    owned_counted_pointer_allocation_bytes::<T, std::sync::atomic::AtomicUsize>()
}
/// Charge a decoded `Box<T>` before constructing its allocation.
#[doc(hidden)]
pub fn reserve_decode_box_allocation<T>() -> Result<(), Error> {
    reserve_decode_allocation(owned_box_allocation_bytes::<T>())
}
/// Charge a decoded `Rc<T>` before constructing its allocation.
#[doc(hidden)]
pub fn reserve_decode_rc_allocation<T>() -> Result<(), Error> {
    reserve_decode_allocation(owned_rc_allocation_bytes::<T>()?)
}
/// Charge a decoded `Arc<T>` before constructing its allocation.
#[doc(hidden)]
pub fn reserve_decode_arc_allocation<T>() -> Result<(), Error> {
    reserve_decode_allocation(owned_arc_allocation_bytes::<T>()?)
}
/// `std::collections::BTreeMap` uses a degree-six B-tree in the pinned Rust
/// toolchain: eleven key/value slots and twelve child edges per node.
///
/// These constants describe allocator-visible node storage, not the Norito wire layout. Keep them
/// in sync when the repository's pinned toolchain changes its B-tree node geometry.
const STD_BTREE_NODE_CAPACITY: usize = 11;
const STD_BTREE_NODE_MIN_ENTRIES: usize = 5;
const STD_BTREE_INTERNAL_EDGE_CAPACITY: usize = 12;
/// Hashbrown keeps seven usable slots per eight buckets.
const STD_HASH_TABLE_LOAD_NUMERATOR: usize = 7;
const STD_HASH_TABLE_LOAD_DENOMINATOR: usize = 8;
/// Hashbrown's control group matches the standard library's target-specific
/// implementation. SSE2 and LSX scan sixteen tags; all other std targets scan
/// one native word (including AArch64 NEON's eight-byte group).
#[cfg(any(
    all(
        target_feature = "sse2",
        any(target_arch = "x86", target_arch = "x86_64"),
        not(miri)
    ),
    all(target_arch = "loongarch64", target_feature = "lsx", not(miri))
))]
const STD_HASH_TABLE_CONTROL_GROUP_BYTES: usize = 16;
#[cfg(not(any(
    all(
        target_feature = "sse2",
        any(target_arch = "x86", target_arch = "x86_64"),
        not(miri)
    ),
    all(target_arch = "loongarch64", target_feature = "lsx", not(miri))
)))]
const STD_HASH_TABLE_CONTROL_GROUP_BYTES: usize = core::mem::size_of::<usize>();
/// Return an upper bound for one Rust-layout struct with `field_count` fields.
///
/// Rust-layout structs may reorder fields. Summing their sizes and allowing
/// `max_alignment - 1` bytes at every field boundary and at the tail bounds
/// every ordering without duplicating private standard-library node types.
fn checked_rust_struct_layout_upper_bound(
    field_bytes: usize,
    field_count: usize,
    max_alignment: usize,
) -> Result<usize, Error> {
    debug_assert!(max_alignment.is_power_of_two());
    let padding = field_count
        .checked_mul(max_alignment.saturating_sub(1))
        .ok_or(Error::LengthMismatch)?;
    field_bytes
        .checked_add(padding)
        .ok_or(Error::LengthMismatch)
}
/// Bytes reserved by all `LinkedList<T>` nodes created for `elements`.
fn linked_list_node_allocation_bytes<T>(elements: usize) -> Result<usize, Error> {
    let pointer_bytes = core::mem::size_of::<usize>();
    let field_bytes = pointer_bytes
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<T>()))
        .ok_or(Error::LengthMismatch)?;
    let max_alignment = core::mem::align_of::<T>().max(core::mem::align_of::<usize>());
    let node_bytes = checked_rust_struct_layout_upper_bound(field_bytes, 3, max_alignment)?;
    elements
        .checked_mul(node_bytes)
        .ok_or(Error::LengthMismatch)
}
/// Maximum live B-tree nodes while inserting `entries` distinct keys into one
/// standard-library B-tree.
///
/// Every non-root node produced by insertion contains at least five entries.
/// A split can transiently add a sibling and a new root, but at the split's
/// resulting entry count the same bound still covers all live nodes.
#[doc(hidden)]
pub fn owned_btree_node_count_upper_bound(entries: usize) -> Result<usize, Error> {
    btree_maps_node_count_upper_bound(1, entries)
}
fn btree_maps_node_count_upper_bound(maps: usize, entries: usize) -> Result<usize, Error> {
    let non_empty_maps = maps.min(entries);
    let remaining_entries = entries.saturating_sub(non_empty_maps);
    non_empty_maps
        .checked_add(remaining_entries / STD_BTREE_NODE_MIN_ENTRIES)
        .ok_or(Error::LengthMismatch)
}
/// Bytes reserved by all `BTreeMap<K, V>` nodes created for `entries`.
///
/// An internal node is the largest standard-library B-tree node: it owns the leaf header, eleven
/// key/value slots, and twelve child pointers. Charging that layout for the maximum live node count
/// also covers leaf-only trees and insertion splits.
#[doc(hidden)]
pub fn owned_btree_allocation_bytes<K, V>(entries: usize) -> Result<usize, Error> {
    let node_bytes = btree_node_allocation_bytes::<K, V>()?;
    owned_btree_node_count_upper_bound(entries)?
        .checked_mul(node_bytes)
        .ok_or(Error::LengthMismatch)
}
/// Bytes reserved by `maps` owned B-trees containing `entries` in aggregate.
///
/// Empty maps allocate no nodes. Distributing entries across as many non-empty
/// maps as possible maximizes the number of roots; every remaining group of
/// five entries can add at most one more live node.
#[doc(hidden)]
pub fn owned_btree_maps_allocation_bytes<K, V>(
    maps: usize,
    entries: usize,
) -> Result<usize, Error> {
    let node_bytes = btree_node_allocation_bytes::<K, V>()?;
    btree_maps_node_count_upper_bound(maps, entries)?
        .checked_mul(node_bytes)
        .ok_or(Error::LengthMismatch)
}
fn btree_node_allocation_bytes<K, V>() -> Result<usize, Error> {
    let pointer_bytes = core::mem::size_of::<usize>();
    let keys_bytes = core::mem::size_of::<K>()
        .checked_mul(STD_BTREE_NODE_CAPACITY)
        .ok_or(Error::LengthMismatch)?;
    let values_bytes = core::mem::size_of::<V>()
        .checked_mul(STD_BTREE_NODE_CAPACITY)
        .ok_or(Error::LengthMismatch)?;
    let edges_bytes = pointer_bytes
        .checked_mul(STD_BTREE_INTERNAL_EDGE_CAPACITY)
        .ok_or(Error::LengthMismatch)?;
    let field_bytes = pointer_bytes
        .checked_add(core::mem::size_of::<u16>())
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<u16>()))
        .and_then(|bytes| bytes.checked_add(keys_bytes))
        .and_then(|bytes| bytes.checked_add(values_bytes))
        .and_then(|bytes| bytes.checked_add(edges_bytes))
        .ok_or(Error::LengthMismatch)?;
    let max_alignment = core::mem::align_of::<K>()
        .max(core::mem::align_of::<V>())
        .max(core::mem::align_of::<usize>());
    checked_rust_struct_layout_upper_bound(field_bytes, 6, max_alignment)
}
/// Charge a decoded B-tree's complete node allocation before insertion.
#[doc(hidden)]
pub fn reserve_decode_btree_allocation<K, V>(entries: usize) -> Result<(), Error> {
    reserve_decode_allocation(owned_btree_allocation_bytes::<K, V>(entries)?)
}
fn hash_table_bucket_count<T>(entries: usize) -> Result<usize, Error> {
    debug_assert_ne!(entries, 0);
    if entries < 15 {
        // This is std/hashbrown's small-table policy. Tiny elements need more
        // buckets so their storage reaches the alignment of the control group.
        let minimum_capacity = match (
            STD_HASH_TABLE_CONTROL_GROUP_BYTES,
            core::mem::size_of::<T>(),
        ) {
            (16, 0..=1) => 14,
            (16, 2..=3) => 7,
            (8, 0..=1) => 7,
            _ => 3,
        };
        let capacity = entries.max(minimum_capacity);
        return Ok(if capacity < 4 {
            4
        } else if capacity < 8 {
            8
        } else {
            16
        });
    }
    let adjusted_capacity = entries
        .checked_mul(STD_HASH_TABLE_LOAD_DENOMINATOR)
        .ok_or(Error::LengthMismatch)?
        / STD_HASH_TABLE_LOAD_NUMERATOR;
    adjusted_capacity
        .checked_next_power_of_two()
        .ok_or(Error::LengthMismatch)
}
/// Bytes requested by the standard hash table for `entries` values of `T`.
///
/// This mirrors the repository's pinned std/hashbrown `capacity_to_buckets` and
/// `TableLayout::calculate_layout_for` implementations: their small-table policy, 7/8 load factor,
/// bucket/control alignment, one control byte per bucket, and one trailing target-specific control
/// group. It measures the allocator request rather than an allocator-specific usable-size class.
#[doc(hidden)]
pub fn owned_hash_table_allocation_bytes<T>(entries: usize) -> Result<usize, Error> {
    if entries == 0 {
        return Ok(0);
    }
    let buckets = hash_table_bucket_count::<T>(entries)?;
    let bucket_bytes = buckets
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(Error::LengthMismatch)?;
    let control_alignment = core::mem::align_of::<T>().max(STD_HASH_TABLE_CONTROL_GROUP_BYTES);
    let control_offset = bucket_bytes
        .checked_add(control_alignment - 1)
        .ok_or(Error::LengthMismatch)?
        & !(control_alignment - 1);
    let allocation_bytes = control_offset
        .checked_add(buckets)
        .and_then(|bytes| bytes.checked_add(STD_HASH_TABLE_CONTROL_GROUP_BYTES))
        .ok_or(Error::LengthMismatch)?;
    if allocation_bytes > isize::MAX as usize - (control_alignment - 1) {
        return Err(Error::LengthMismatch);
    }
    Ok(allocation_bytes)
}
/// Charge a decoded standard hash table's complete allocation before reserve.
#[doc(hidden)]
pub fn reserve_decode_hash_table_allocation<T>(entries: usize) -> Result<(), Error> {
    reserve_decode_allocation(owned_hash_table_allocation_bytes::<T>(entries)?)
}
#[cfg(test)]
mod owned_collection_allocation_tests {
    use super::*;
    fn allocation_limit_just_below(bytes: usize) -> DecodeLimits {
        DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            bytes.saturating_sub(1),
            usize::MAX,
        )
    }
    #[test]
    fn measured_scope_reports_its_own_collection_charges() {
        reset_decode_state();
        let value = vec![1_u32, 2, 3];
        let mut bytes = Vec::new();
        serialize_to_buffer(&value, &mut bytes).expect("serialize measured Vec");
        let limits = DecodeLimits::new(16, bytes.len(), 16, 1024, 16);
        let (decoded, usage) = with_decode_limits_measured(limits, || {
            <Vec<u32> as DecodeFromSlice>::decode_from_slice(&bytes)
        });
        assert_eq!(decoded.expect("decode measured Vec").0, value);
        assert_eq!(usage.total_elements(), value.len());
        assert!(
            usage.total_allocated_bytes()
                >= value
                    .len()
                    .checked_mul(core::mem::size_of::<u32>())
                    .expect("test allocation fits")
        );
        reset_decode_state();
    }
    #[test]
    fn linked_list_decoder_charges_all_nodes_before_insertion() {
        reset_decode_state();
        let value = LinkedList::from([1_u8, 2, 3]);
        let mut bytes = Vec::new();
        serialize_to_buffer(&value, &mut bytes).expect("serialize linked list");
        let node_bytes = linked_list_node_allocation_bytes::<u8>(value.len())
            .expect("linked-list node charge fits");
        let error = with_decode_limits(allocation_limit_just_below(node_bytes), || {
            <LinkedList<u8> as DecodeFromSlice>::decode_from_slice(&bytes).map(|_| ())
        })
        .expect_err("the node allocation must exceed the narrow limit");
        assert!(matches!(
            error,
            Error::TotalAllocationExceeded { attempted, limit }
                if attempted == node_bytes as u64 && limit == (node_bytes - 1) as u64
        ));
        reset_decode_state();
    }
    #[test]
    fn btree_set_decoder_charges_tree_nodes_before_insertion() {
        reset_decode_state();
        let value = BTreeSet::from([1_u16, 2, 3, 4, 5, 6]);
        let mut bytes = Vec::new();
        serialize_to_buffer(&value, &mut bytes).expect("serialize B-tree set");
        let node_bytes = owned_btree_allocation_bytes::<u16, ()>(value.len())
            .expect("B-tree set node charge fits");
        let error = with_decode_limits(allocation_limit_just_below(node_bytes), || {
            <BTreeSet<u16> as DecodeFromSlice>::decode_from_slice(&bytes).map(|_| ())
        })
        .expect_err("the node allocation must exceed the narrow limit");
        assert!(matches!(
            error,
            Error::TotalAllocationExceeded { attempted, limit }
                if attempted == node_bytes as u64 && limit == (node_bytes - 1) as u64
        ));
        reset_decode_state();
    }
    #[test]
    fn btree_map_decoder_charges_tree_nodes_before_insertion() {
        reset_decode_state();
        let value = BTreeMap::from([(1_u16, 2_u32), (3, 4), (5, 6)]);
        let mut bytes = Vec::new();
        serialize_to_buffer(&value, &mut bytes).expect("serialize B-tree map");
        let node_bytes = owned_btree_allocation_bytes::<u16, u32>(value.len())
            .expect("B-tree map node charge fits");
        let error = with_decode_limits(allocation_limit_just_below(node_bytes), || {
            <BTreeMap<u16, u32> as DecodeFromSlice>::decode_from_slice(&bytes).map(|_| ())
        })
        .expect_err("the node allocation must exceed the narrow limit");
        assert!(matches!(
            error,
            Error::TotalAllocationExceeded { attempted, limit }
                if attempted == node_bytes as u64 && limit == (node_bytes - 1) as u64
        ));
        reset_decode_state();
    }
    #[test]
    fn hash_set_decoder_charges_buckets_and_control_bytes_before_reserve() {
        reset_decode_state();
        let value = HashSet::from([1_u16, 2, 3, 4]);
        let mut bytes = Vec::new();
        serialize_to_buffer(&value, &mut bytes).expect("serialize hash set");
        let table_bytes = owned_hash_table_allocation_bytes::<u16>(value.len())
            .expect("hash-set table charge fits");
        let error = with_decode_limits(allocation_limit_just_below(table_bytes), || {
            <HashSet<u16> as DecodeFromSlice>::decode_from_slice(&bytes).map(|_| ())
        })
        .expect_err("the hash-table allocation must exceed the narrow limit");
        assert!(matches!(
            error,
            Error::TotalAllocationExceeded { attempted, limit }
                if attempted == table_bytes as u64 && limit == (table_bytes - 1) as u64
        ));
        reset_decode_state();
    }
    #[test]
    fn hash_table_allocation_matches_std_raw_table_layout() {
        let buckets = hash_table_bucket_count::<u8>(1).expect("small table bucket count fits");
        let expected_buckets = match STD_HASH_TABLE_CONTROL_GROUP_BYTES {
            16 => 16,
            8 => 8,
            _ => 4,
        };
        assert_eq!(buckets, expected_buckets);
        assert_eq!(
            owned_hash_table_allocation_bytes::<u8>(1).expect("small table allocation fits"),
            expected_buckets + expected_buckets + STD_HASH_TABLE_CONTROL_GROUP_BYTES
        );
        assert_eq!(
            hash_table_bucket_count::<u64>(15).expect("large table bucket count fits"),
            32
        );
    }
    #[test]
    fn hash_map_decoder_charges_buckets_and_control_bytes_before_reserve() {
        reset_decode_state();
        let value = HashMap::from([(1_u16, 2_u32), (3, 4), (5, 6), (7, 8)]);
        let mut bytes = Vec::new();
        serialize_to_buffer(&value, &mut bytes).expect("serialize hash map");
        let table_bytes = owned_hash_table_allocation_bytes::<(u16, u32)>(value.len())
            .expect("hash-map table charge fits");
        let error = with_decode_limits(allocation_limit_just_below(table_bytes), || {
            <HashMap<u16, u32> as DecodeFromSlice>::decode_from_slice(&bytes).map(|_| ())
        })
        .expect_err("the hash-table allocation must exceed the narrow limit");
        assert!(matches!(
            error,
            Error::TotalAllocationExceeded { attempted, limit }
                if attempted == table_bytes as u64 && limit == (table_bytes - 1) as u64
        ));
        reset_decode_state();
    }
    #[test]
    fn aggregate_btree_estimator_charges_each_non_empty_root() {
        let single =
            owned_btree_maps_allocation_bytes::<u8, u8>(1, 6).expect("single-map charge fits");
        let split =
            owned_btree_maps_allocation_bytes::<u8, u8>(6, 6).expect("multi-map charge fits");
        let node = owned_btree_allocation_bytes::<u8, u8>(1).expect("one node charge fits");
        assert_eq!(single, node.checked_mul(2).expect("test charge fits"));
        assert_eq!(split, node.checked_mul(6).expect("test charge fits"));
    }
    #[test]
    fn single_btree_node_estimator_handles_empty_root_and_split_boundary() {
        assert_eq!(
            owned_btree_node_count_upper_bound(0).expect("empty tree count fits"),
            0
        );
        assert_eq!(
            owned_btree_node_count_upper_bound(5).expect("one-node tree count fits"),
            1
        );
        assert_eq!(
            owned_btree_node_count_upper_bound(6).expect("split bound fits"),
            2
        );
    }
}
