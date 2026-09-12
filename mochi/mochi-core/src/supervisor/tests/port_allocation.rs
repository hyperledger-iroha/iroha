//! Deterministic shared cursor ordering, collision, exhaustion and wrap contracts.
use super::*;

#[test]
fn shared_port_allocators_preserve_exact_order_and_wrap_without_reuse() {
    for (torii_base, p2p_base, expected) in [
        (40_000, 40_002, (40_000..40_008).collect::<Vec<_>>()),
        (
            u16::MAX - 3,
            u16::MAX - 1,
            vec![1, 2, 3, 4, 65_532, 65_533, 65_534, 65_535],
        ),
    ] {
        let mut torii = PortAllocator::new(torii_base);
        let mut p2p = PortAllocator::new(p2p_base);
        let mut reserved = HashSet::new();
        let mut torii_ports = Vec::new();
        let mut p2p_ports = Vec::new();
        let allocate = |cursor: &mut PortAllocator, reserved: &mut HashSet<u16>| {
            SupervisorBuilder::reserve_unique_port(
                || {
                    cursor
                        .allocate_with_probe(|_| true)
                        .ok_or_else(|| io::Error::other("exhausted test cursor"))
                },
                reserved,
                "controlled",
            )
            .expect("free controlled range")
        };
        for _ in 0..4 {
            torii_ports.push(allocate(&mut torii, &mut reserved));
            p2p_ports.push(allocate(&mut p2p, &mut reserved));
        }
        assert_eq!(torii_ports.first().copied(), Some(torii_base));
        assert_eq!(p2p_ports.first().copied(), Some(p2p_base));
        assert_eq!(reserved.len(), 8);
        let mut all_ports: Vec<_> = reserved.into_iter().collect();
        all_ports.sort_unstable();
        assert_eq!(
            all_ports, expected,
            "shared cursors must cover the free range without reusing or skipping ports"
        );
        assert!(!all_ports.contains(&0));
    }
}

#[test]
fn shared_port_allocator_preserves_exhaustion_error_without_reserving_a_port() {
    let mut cursor = PortAllocator::new(u16::MAX);
    let mut reserved = HashSet::new();
    let error = SupervisorBuilder::reserve_unique_port(
        || {
            cursor.allocate_with_probe(|_| false).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    "exhausted controlled range",
                )
            })
        },
        &mut reserved,
        "Torii",
    )
    .expect_err("unavailable range must terminate");
    assert!(
        matches!(error, SupervisorError::Config(message) if message == "failed to allocate Torii port: exhausted controlled range")
    );
    assert!(reserved.is_empty());
}
