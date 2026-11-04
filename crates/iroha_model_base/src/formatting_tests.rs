//! Foundational formatter, constructor and wire-parity contracts.

use crate::{
    chain::ChainId,
    domain::DomainId,
    metadata::Metadata,
    name::Name,
    peer::{Peer, PeerId},
    state_path::StatePath,
    topology::{DataSpaceId, LaneId, ShardId},
};
use core::fmt::{self, Debug, Display, Write as _};
use iroha_crypto::PublicKey;
use iroha_primitives::json::Json;

const KEY: &str = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";

struct FailingWriter;
impl fmt::Write for FailingWriter {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Err(fmt::Error)
    }
}

fn assert_transparent_display(value: &impl Display, inner: &impl Display) {
    assert_eq!(format!("{value}"), format!("{inner}"));
    assert_eq!(format!("{value:>100.3}"), format!("{inner:>100.3}"));
    assert_eq!(format!("{value:🦀^100.3}"), format!("{inner:🦀^100.3}"));
    assert_eq!(format!("{value:+#0100.3}"), format!("{inner:+#0100.3}"));
    assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
}

fn assert_fixed_display(value: &impl Display, expected: &str) {
    assert_eq!(value.to_string(), expected);
    assert_eq!(format!("{value:>100.3}"), expected);
    assert_eq!(format!("{value:🦀^100.3}"), expected);
    assert_eq!(format!("{value:+#0100.3}"), expected);
    assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
}

fn assert_debug(value: &impl Debug, expected: &str, pretty: &str) {
    assert_eq!(format!("{value:?}"), expected);
    assert_eq!(format!("{value:>100.3?}"), expected);
    assert_eq!(format!("{value:+0100.3?}"), expected);
    assert_eq!(format!("{value:#?}"), pretty);
    assert_eq!(format!("{value:🦀^100.3?}"), expected);
    assert_eq!(write!(FailingWriter, "{value:?}"), Err(fmt::Error));
    assert_eq!(write!(FailingWriter, "{value:#?}"), Err(fmt::Error));
}

#[test]
fn text_display_preserves_existing_padding_and_precision_behavior() {
    for text in ["a", "café", "long-name-that-requires-owned-storage"] {
        let name: Name = text.parse().expect("canonical name");
        // The backing ConstString formatter ignores outer padding and precision.
        assert_fixed_display(&name, text);
        let path: StatePath = text.parse().expect("canonical state path");
        assert_fixed_display(&path, text);
    }
    for text in [
        "a",
        "Test_CHAIN:123",
        "long-chain-identifier-with-owned-storage",
    ] {
        let chain: ChainId = text.parse().expect("canonical chain label");
        assert_transparent_display(&chain, &text);
    }
}

#[test]
fn numeric_display_preserves_inner_flags_and_full_unsigned_domains() {
    for raw in [0, 1, 42, u32::MAX] {
        assert_transparent_display(&LaneId::new(raw), &raw);
        assert_transparent_display(&ShardId::new(raw), &raw);
    }
    for raw in [0, 1, u64::from(u32::MAX) + 1, u64::MAX] {
        assert_transparent_display(&DataSpaceId::new(raw), &raw);
    }
}

#[test]
fn tuple_debug_preserves_escaping_and_pretty_forms() {
    for text in [
        "a",
        "café",
        r#"a"b"#,
        "long-name-that-requires-owned-storage",
    ] {
        let name: Name = text.parse().expect("canonical name");
        assert_debug(
            &name,
            &format!("Name({text:?})"),
            &format!("Name(\n    {text:?},\n)"),
        );
        let path: StatePath = text.parse().expect("canonical state path");
        assert_debug(
            &path,
            &format!("StatePath({text:?})"),
            &format!("StatePath(\n    {text:?},\n)"),
        );
    }
}

#[test]
fn peer_id_constructor_and_formatters_preserve_the_public_key() {
    const fn construct(public_key: PublicKey) -> PeerId {
        PeerId::new(public_key)
    }
    let key: PublicKey = KEY.parse().expect("canonical public key");
    let peer_id = construct(key.clone());
    let expected = PeerId {
        public_key: key.clone(),
    };
    assert_eq!(peer_id, expected);
    let bytes = norito::to_bytes(&peer_id).expect("encode peer identity");
    assert_eq!(
        bytes,
        norito::to_bytes(&expected).expect("encode named field")
    );
    assert_eq!(
        norito::decode_from_bytes::<PeerId>(&bytes).expect("decode peer identity"),
        expected
    );
    assert_eq!(
        norito::json::to_json(&peer_id).expect("encode identity JSON"),
        norito::json::to_json(&expected).expect("encode named-field JSON"),
    );
    assert_transparent_display(&peer_id, &key);
    assert_eq!(format!("{peer_id:?}"), format!("{key}"));
    assert_eq!(format!("{peer_id:#?}"), format!("{key:#}"));
    assert_eq!(format!("{peer_id:🦀^100.3?}"), format!("{key:🦀^100.3}"));
    assert_eq!(format!("{peer_id:+#0100.3?}"), format!("{key:+#0100.3}"));
    assert_eq!(write!(FailingWriter, "{peer_id:?}"), Err(fmt::Error));
}

#[test]
fn compound_and_metadata_display_preserve_text_and_ignore_outer_flags() {
    let domain = DomainId::try_new("Merchant", "Bank").expect("canonical domain");
    assert_fixed_display(&domain, "merchant.bank");
    let mut metadata = Metadata::default();
    assert_fixed_display(&metadata, "Metadata");
    metadata.insert("private".parse().expect("name"), Json::new("hidden value"));
    assert_fixed_display(&metadata, "Metadata");

    let peer_id: PeerId = KEY.parse().expect("canonical peer identity");
    for address in ["127.0.0.1:1337", "[::1]:1337"] {
        let address = address.parse().expect("socket address");
        let peer = Peer::new(address, peer_id.clone());
        assert_fixed_display(&peer, &format!("{}@{}", peer.id(), peer.address()));
        let expected = format!(
            "Peer {{ address: {:?}, id: {:?} }}",
            peer.address(),
            peer.id()
        );
        let pretty = format!(
            "Peer {{\n    address: {},\n    id: {},\n}}",
            format!("{:#?}", peer.address()).replace('\n', "\n    "),
            format!("{:#?}", peer.id()).replace('\n', "\n    "),
        );
        assert_debug(&peer, &expected, &pretty);
    }
}
