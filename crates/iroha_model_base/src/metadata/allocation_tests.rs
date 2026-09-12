// The destination tree adds a conservative codec charge to the fully decoded entry vector.
mod allocation_tests {
    use super::*;
    use ncore::{
        Archived, ArchivedRef, DecodeAllocationUsage, DecodeFlagsGuard, DecodeLimits,
        DeserializePayload, PayloadCtxGuard, with_decode_limits_measured,
    };

    const GENEROUS_ALLOCATION: usize = 16 * 1024 * 1024;

    fn limits(allocation: usize) -> DecodeLimits {
        DecodeLimits::new(4096, 1024 * 1024, 16_384, allocation, 32)
    }

    fn layouts() -> Vec<u8> {
        let flags: Vec<_> = (0..=u8::MAX)
            .filter(|&flags| ncore::validate_header_flags(flags).is_ok())
            .collect();
        assert_eq!(flags.len(), 10);
        flags
    }

    fn entries(count: usize) -> Vec<(Name, Json)> {
        (0..count)
            .map(|index| {
                (
                    format!("key_{index:03}").parse().expect("valid key"),
                    Json::new(format!("value_{index:03}")),
                )
            })
            .collect()
    }

    fn with_staged_entries<T>(
        entries: &Vec<(Name, Json)>,
        requested: u8,
        inspect: impl FnOnce(&ArchivedRef<'_, Metadata>, usize, u8) -> T,
    ) -> T {
        let _layout = DecodeFlagsGuard::enter(requested);
        let (mut backing, flags) = norito::codec::encode_with_header_flags(entries);
        ncore::validate_header_flags(flags).expect("encoder advertises a valid layout");
        let payload_len = backing.len();
        // Stage once outside every measured scope. Both decoders see the same address and
        // logical payload length, including empty sequences shorter than a Rust container.
        let minimum = ncore::archived_payload_size::<Metadata>()
            .max(ncore::archived_payload_size::<Vec<(Name, Json)>>());
        backing.resize(backing.len().max(minimum), 0);
        let archived = ncore::archived_from_slice::<Metadata>(&backing).expect("staged payload");
        inspect(&archived, payload_len, flags)
    }

    fn measured<T>(
        archived: &ArchivedRef<'_, Metadata>,
        payload_len: usize,
        flags: u8,
        allocation: usize,
        decode: impl FnOnce(&Archived<Metadata>) -> T,
    ) -> (T, DecodeAllocationUsage) {
        let _layout = DecodeFlagsGuard::enter(flags);
        let _payload = PayloadCtxGuard::enter_with_len(archived.bytes(), payload_len);
        with_decode_limits_measured(limits(allocation), || decode(archived.archived()))
    }

    fn measured_entries(
        archived: &ArchivedRef<'_, Metadata>,
        payload_len: usize,
        flags: u8,
    ) -> (Vec<(Name, Json)>, usize) {
        let (decoded, usage) =
            measured(archived, payload_len, flags, GENEROUS_ALLOCATION, |value| {
                Vec::<(Name, Json)>::try_deserialize(value.cast())
            });
        (
            decoded.expect("entry vector"),
            usage.total_allocated_bytes(),
        )
    }

    #[test]
    fn metadata_tree_allocation_fallible_exact_boundary() {
        // Six entries already increase the conservative node bound; sixty-four also require
        // multiple actual nodes. The oracle is the shared owner, not private std node layout.
        for count in [0, 1, 6, 12, 64] {
            let entries = entries(count);
            let expected = Metadata(entries.iter().cloned().collect());
            let tree = ncore::owned_btree_allocation_bytes::<Name, Json>(count).unwrap();
            assert_eq!(tree == 0, count == 0);
            for flags in layouts() {
                with_staged_entries(&entries, flags, |archived, payload_len, flags| {
                    let (decoded, child) = measured_entries(archived, payload_len, flags);
                    assert_eq!(decoded, entries);
                    let exact = child.checked_add(tree).unwrap();
                    let (decoded, usage) = measured(archived, payload_len, flags, exact, |value| {
                        Metadata::try_deserialize(value)
                    });
                    assert_eq!(decoded.unwrap(), expected);
                    assert_eq!(usage.total_allocated_bytes(), exact);
                    if tree == 0 {
                        assert_eq!(exact, child);
                        return;
                    }
                    for budget in [child, exact - 1] {
                        let (result, _) = measured(archived, payload_len, flags, budget, |value| {
                            Metadata::try_deserialize(value)
                        });
                        assert!(matches!(
                            result,
                            Err(ncore::Error::TotalAllocationExceeded { attempted, limit })
                                if attempted == exact as u64 && limit == budget as u64
                        ));
                    }
                    // Rejection did not leave a nested budget or payload scope installed.
                    let (decoded, usage) = measured(archived, payload_len, flags, exact, |value| {
                        Metadata::try_deserialize(value)
                    });
                    assert_eq!(decoded.unwrap(), expected);
                    assert_eq!(usage.total_allocated_bytes(), exact);
                });
            }
        }
    }

    #[test]
    fn metadata_tree_allocation_infallible_exact_boundary() {
        for count in [0, 1, 6, 12, 64] {
            let entries = entries(count);
            let expected = Metadata(entries.iter().cloned().collect());
            let tree = ncore::owned_btree_allocation_bytes::<Name, Json>(count).unwrap();
            for flags in layouts() {
                with_staged_entries(&entries, flags, |archived, payload_len, flags| {
                    let (decoded, child) = measured_entries(archived, payload_len, flags);
                    assert_eq!(decoded, entries);
                    let exact = child.checked_add(tree).unwrap();
                    let (decoded, usage) = measured(archived, payload_len, flags, exact, |value| {
                        Metadata::deserialize(value)
                    });
                    assert_eq!(decoded, expected);
                    assert_eq!(usage.total_allocated_bytes(), exact);
                    if tree != 0 {
                        // This is the existing infallible Norito trait contract: a failed
                        // fallible decode panics. No panic payload or process hook is invented.
                        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            measured(archived, payload_len, flags, exact - 1, |value| {
                                Metadata::deserialize(value)
                            })
                        }));
                        assert!(panic.is_err());
                    }
                    let (decoded, usage) = measured(archived, payload_len, flags, exact, |value| {
                        Metadata::deserialize(value)
                    });
                    assert_eq!(decoded, expected);
                    assert_eq!(usage.total_allocated_bytes(), exact);
                });
            }
        }
    }

    #[test]
    fn metadata_tree_charge_obeys_outer_budget_before_duplicate_insertion() {
        let mut entries = entries(6);
        entries.push(entries[0].clone());
        for flags in layouts() {
            with_staged_entries(&entries, flags, |archived, payload_len, flags| {
                let (decoded, child) = measured_entries(archived, payload_len, flags);
                assert_eq!(
                    decoded, entries,
                    "the tuple sequence itself permits duplicates"
                );
                let tree =
                    ncore::owned_btree_allocation_bytes::<Name, Json>(entries.len()).unwrap();
                let exact = child.checked_add(tree).unwrap();
                let ((result, inner_usage), outer_usage) =
                    with_decode_limits_measured(limits(exact), || {
                        measured(archived, payload_len, flags, GENEROUS_ALLOCATION, |value| {
                            Metadata::try_deserialize(value)
                        })
                    });
                assert!(matches!(result, Err(ncore::Error::Message(message))
                    if message == "duplicate metadata key"));
                assert_eq!(inner_usage.total_allocated_bytes(), exact);
                assert_eq!(outer_usage.total_allocated_bytes(), exact);
                let ((result, _), _) = with_decode_limits_measured(limits(exact - 1), || {
                    measured(archived, payload_len, flags, GENEROUS_ALLOCATION, |value| {
                        Metadata::try_deserialize(value)
                    })
                });
                assert!(matches!(result,
                    Err(ncore::Error::TotalAllocationExceeded { attempted, limit })
                        if attempted == exact as u64 && limit == (exact - 1) as u64));
            });
        }
    }
}
