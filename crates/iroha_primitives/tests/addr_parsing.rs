//! Regression tests for strict network-address parsing.
#[path = "numeric_inspect.rs"]
mod numeric_inspect;

use iroha_primitives::addr::{Ipv6Addr, ParseError, SocketAddrV6};

#[test]
fn ipv6_rejects_empty_groups_without_shorthand() {
    for malformed in ["", "1:2:3:4:5:6:7:", "1:2:3:4:5:6::7:"] {
        assert!(
            malformed.parse::<Ipv6Addr>().is_err(),
            "malformed IPv6 address was accepted: {malformed:?}"
        );
    }
}

#[test]
fn ipv6_shorthand_must_replace_a_segment() {
    for malformed in ["1:2:3:4:5:6:7::8", "1:2:3:4:5:6:7:8::", "::1:2:3:4:5:6:7:8"] {
        assert_eq!(
            malformed.parse::<Ipv6Addr>(),
            Err(ParseError::TooManySegments),
            "zero-width IPv6 shorthand was accepted: {malformed}"
        );
    }
}

#[test]
fn ipv6_shorthand_may_replace_exactly_one_segment() {
    assert_eq!(
        "1:2:3:4:5:6:7::".parse::<Ipv6Addr>().unwrap(),
        Ipv6Addr::new([1, 2, 3, 4, 5, 6, 7, 0])
    );
    assert_eq!(
        "::2:3:4:5:6:7:8".parse::<Ipv6Addr>().unwrap(),
        Ipv6Addr::new([0, 2, 3, 4, 5, 6, 7, 8])
    );
}

#[test]
fn socket_ipv6_requires_exactly_one_opening_bracket() {
    assert!("2001:db8::]:9019".parse::<SocketAddrV6>().is_err());
    assert!("[[2001:db8::]:9019".parse::<SocketAddrV6>().is_err());
}
