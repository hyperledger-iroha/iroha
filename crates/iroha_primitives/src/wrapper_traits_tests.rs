//! Formatting, ownership and borrowed-storage contracts for primitive wrappers.

use std::{fmt, fmt::Write as _};

use crate::{
    addr::{
        IpAddr, Ipv4Addr, Ipv6Addr, ParseError, SocketAddr, SocketAddrHost, SocketAddrV4,
        SocketAddrV6,
    },
    conststr::ConstString,
    must_use::MustUse,
    small::SmallStr,
};

struct FailingWriter;
impl fmt::Write for FailingWriter {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Err(fmt::Error)
    }
}

fn assert_composite_display(value: &impl fmt::Display, expected: &str) {
    assert_eq!(format!("{value}"), expected);
    assert_eq!(format!("{value:>80.3}"), expected);
    assert_eq!(format!("{value:🦀^80.3}"), expected);
    assert_eq!(format!("{value:+#080.3}"), expected);
    if !expected.is_empty() {
        assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
    }
}

fn assert_composite_debug(value: &impl fmt::Debug, expected: &str) {
    assert_eq!(format!("{value:?}"), expected);
    assert_eq!(format!("{value:>80.3?}"), expected);
    assert_eq!(format!("{value:🦀^80.3?}"), expected);
    assert_eq!(format!("{value:+#080.3?}"), expected);
    assert_eq!(write!(FailingWriter, "{value:?}"), Err(fmt::Error));
}

#[test]
fn ipv4_array_conversions_borrow_storage_and_iterate_owned_octets() {
    let octets = [0, 127, 128, 255];
    let addr = Ipv4Addr::from(octets);
    let borrowed: &[u8; 4] = addr.as_ref();
    assert_eq!(borrowed, &octets);
    assert!(std::ptr::eq(
        borrowed,
        std::ptr::from_ref(&addr).cast::<[u8; 4]>()
    ));
    let mut iter: std::array::IntoIter<u8, 4> = IntoIterator::into_iter(addr);
    assert_eq!(iter.len(), 4);
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next_back(), Some(255));
    assert_eq!(iter.collect::<Vec<_>>(), vec![127, 128]);
    let mut exhausted = addr.into_iter();
    assert_eq!(exhausted.nth(3), Some(255));
    assert_eq!(exhausted.next(), None);
    assert_eq!(exhausted.next(), None);
}

#[test]
fn ipv6_array_conversions_borrow_storage_and_iterate_owned_segments() {
    let segments = [0, 1, 0x1234, 0xffff, 7, 8, 9, 10];
    let addr = Ipv6Addr::from(segments);
    let borrowed: &[u16; 8] = addr.as_ref();
    assert_eq!(borrowed, &segments);
    assert!(std::ptr::eq(
        borrowed,
        std::ptr::from_ref(&addr).cast::<[u16; 8]>()
    ));
    let mut iter: std::array::IntoIter<u16, 8> = IntoIterator::into_iter(addr);
    assert_eq!(iter.len(), 8);
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next_back(), Some(10));
    assert_eq!(iter.collect::<Vec<_>>(), vec![1, 0x1234, 0xffff, 7, 8, 9]);
    let mut exhausted = addr.into_iter();
    assert_eq!(exhausted.nth(7), Some(10));
    assert_eq!(exhausted.next(), None);
    assert_eq!(exhausted.next(), None);
}

#[test]
fn address_display_and_custom_debug_preserve_complete_text() {
    let ip = Ipv4Addr::new([192, 0, 2, 255]);
    let v4 = SocketAddrV4 { ip, port: u16::MAX };
    let v6 = SocketAddrV6 {
        ip: Ipv6Addr::LOOPBACK,
        port: 443,
    };
    assert_composite_display(&ip, "192.0.2.255");
    assert_composite_debug(&ip, "192.0.2.255");
    assert_composite_display(&v4, "192.0.2.255:65535");
    assert_composite_debug(&v4, "192.0.2.255:65535");
    assert_composite_display(&v6, "[::1]:443");
    assert_composite_debug(&v6, "[::1]:443");
    assert_composite_display(&SocketAddr::Ipv4(v4), "192.0.2.255:65535");
    assert_composite_display(&SocketAddr::Ipv6(v6), "[::1]:443");
    let host = SocketAddrHost {
        host: "μ\n.example".into(),
        port: 0,
    };
    assert_composite_display(&SocketAddr::Host(host), "μ\n.example:0");
}

#[test]
fn default_address_debug_keeps_names_fields_and_pretty_layout() {
    let ipv6 = Ipv6Addr::LOOPBACK;
    assert_eq!(format!("{ipv6:?}"), "Ipv6Addr([0, 0, 0, 0, 0, 0, 0, 1])");
    assert_eq!(
        format!("{ipv6:#?}"),
        "Ipv6Addr(\n    [\n        0,\n        0,\n        0,\n        0,\n        0,\n        0,\n        0,\n        1,\n    ],\n)"
    );
    assert_eq!(
        format!("{:?}", IpAddr::V4(Ipv4Addr::LOCALHOST)),
        "V4(127.0.0.1)"
    );
    assert_eq!(
        format!("{:?}", IpAddr::V6(ipv6)),
        "V6(Ipv6Addr([0, 0, 0, 0, 0, 0, 0, 1]))"
    );
    let host = SocketAddrHost {
        host: "μ\n.example".into(),
        port: 80,
    };
    assert_eq!(
        format!("{host:?}"),
        "SocketAddrHost { host: \"μ\\n.example\", port: 80 }"
    );
    assert_eq!(
        format!("{:?}", SocketAddr::Host(host)),
        "Host(SocketAddrHost { host: \"μ\\n.example\", port: 80 })"
    );
    assert_eq!(
        format!(
            "{:?}",
            SocketAddr::Ipv4(SocketAddrV4 {
                ip: Ipv4Addr::LOCALHOST,
                port: 80
            })
        ),
        "Ipv4(127.0.0.1:80)"
    );
    assert_eq!(
        format!(
            "{:?}",
            SocketAddr::Ipv6(SocketAddrV6 { ip: ipv6, port: 80 })
        ),
        "Ipv6([::1]:80)"
    );
    for (error, expected) in [
        (ParseError::NotEnoughSegments, "NotEnoughSegments"),
        (ParseError::TooManySegments, "TooManySegments"),
        (ParseError::InvalidSegment, "InvalidSegment"),
        (ParseError::InvalidPort, "InvalidPort"),
        (ParseError::NoPort, "NoPort"),
        (ParseError::UnexpectedAbbreviation, "UnexpectedAbbreviation"),
    ] {
        assert_composite_debug(&error, expected);
    }
}

#[test]
fn address_tuple_debug_keeps_pretty_fields_independent_of_outer_flags() {
    let segments = [0, 1, 12, 128, 0x1234, u16::MAX, 9, 10];
    let ipv6 = Ipv6Addr::from(segments);
    let ip = IpAddr::V6(ipv6);
    let socket = SocketAddr::Host(SocketAddrHost {
        host: "μ\n.example".into(),
        port: 1234,
    });
    let values: [&dyn fmt::Debug; 3] = [&ipv6, &ip, &socket];
    for value in values {
        let pretty = format!("{value:#?}");
        assert_eq!(format!("{value:+#080.3?}"), pretty);
        assert_eq!(format!("{value:🦀^#80.3?}"), pretty);
        assert_eq!(format!("{value:#x?}"), pretty);
        assert_eq!(format!("{value:#X?}"), pretty);
        assert_eq!(write!(FailingWriter, "{value:+#080.3?}"), Err(fmt::Error));
    }
    // Compact tuple fields still receive the surrounding numeric formatter flags.
    assert_eq!(format!("{ipv6:x?}"), format!("Ipv6Addr({segments:x?})"));
    assert_eq!(format!("{ipv6:X?}"), format!("Ipv6Addr({segments:X?})"));
    assert_eq!(
        format!("{ipv6:>80.3?}"),
        format!("Ipv6Addr({segments:>80.3?})")
    );
}

#[test]
fn compact_string_display_keeps_inline_heap_unicode_and_outer_flag_behavior() {
    for text in [
        String::new(),
        "a".repeat(15),
        "a".repeat(16),
        "a".repeat(33),
        "μ🔥\nquote\"\\".repeat(8),
    ] {
        assert_composite_display(&ConstString::from(text.as_str()), &text);
        assert_composite_display(&SmallStr::from_str(&text), &text);
    }
}

#[test]
fn must_use_constructor_and_borrows_preserve_unbounded_owned_storage() {
    const WRAPPED: MustUse<u8> = MustUse::new(7);
    assert_eq!(*WRAPPED, 7);
    struct NoTraits(String);
    let mut text = String::with_capacity(128);
    text.push_str("owned");
    let pointer = text.as_ptr();
    let mut wrapped = MustUse::new(NoTraits(text));
    let as_ref: &NoTraits = wrapped.as_ref();
    let dereferenced: &NoTraits = &wrapped;
    assert!(std::ptr::eq(as_ref, &wrapped.0));
    assert!(std::ptr::eq(dereferenced, &wrapped.0));
    let original = std::ptr::from_mut(&mut wrapped.0);
    let as_mut: &mut NoTraits = wrapped.as_mut();
    assert!(std::ptr::eq(as_mut, original));
    as_mut.0.push_str(" value");
    let inner = wrapped.into_inner();
    assert_eq!(inner.0, "owned value");
    assert_eq!(inner.0.as_ptr(), pointer);
}

#[test]
fn must_use_display_only_requires_display_and_forwards_formatter_options() {
    struct DisplayOnly<'a>(&'a str);
    impl fmt::Display for DisplayOnly<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Display::fmt(self.0, f)
        }
    }
    for text in ["", "abc", "μ🔥\nquote\"\\"] {
        let value = MustUse::new(DisplayOnly(text));
        assert_eq!(format!("{value}"), text);
        assert_eq!(format!("{value:>80.3}"), format!("{text:>80.3}"));
        assert_eq!(format!("{value:🦀^80.3}"), format!("{text:🦀^80.3}"));
        assert_eq!(format!("{value:+#080.3}"), format!("{text:+#080.3}"));
        if !text.is_empty() {
            assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
        }
    }
}
