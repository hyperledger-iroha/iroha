//! Captured frame identities for the FFI-owned address family.

use crate::addr::{
    IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrHost, SocketAddrV4, SocketAddrV6,
};
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json};
use std::{collections::BTreeMap, fmt::Debug};

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::new();
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("write hexadecimal to a String");
    }
    encoded
}

fn binary_record<T>(label: &str, value: &T) -> json::Value
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug + PartialEq,
{
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        <T as NoritoSerialize>::schema_hash()
    );
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        <T as NoritoDeserialize>::schema_hash()
    );
    let frame = norito::to_bytes(value).expect("encode address frame");
    let decoded: T = norito::decode_from_bytes(&frame).expect("decode address frame");
    assert_eq!(&decoded, value);
    let mut layouts = Vec::new();
    for requested in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let _guard = norito::core::DecodeFlagsGuard::enter(requested);
        let (payload, actual) = norito::codec::encode_with_header_flags(value);
        let framed = norito::core::frame_bare_with_header_flags::<T>(&payload, actual)
            .expect("frame the explicitly advertised layout");
        let decoded: T = norito::decode_from_bytes(&framed).expect("decode advertised layout");
        assert_eq!(&decoded, value, "{label}: layout {actual:#04x}");
        layouts.push(norito::json!({
            "requested_flags": requested,
            "actual_flags": actual,
            "payload_hex": (hex(&payload)),
            "frame_hex": (hex(&framed)),
        }));
    }
    norito::json!({
        "label": label,
        "nominal": (T::nominal_name()),
        "serialize_hash": (hex(&<T as NoritoSerialize>::schema_hash())),
        "deserialize_hash": (hex(&<T as NoritoDeserialize>::schema_hash())),
        "frame_hex": (hex(&frame)),
        "payload_hex": (hex(&norito::codec::Encode::encode(value))),
        "layouts": layouts,
    })
}

#[cfg(feature = "json")]
fn json_record<T>(label: &str, value: &T) -> json::Value
where
    T: json::JsonSerialize + json::JsonDeserialize + Debug + PartialEq,
{
    let encoded = json::to_value(value).expect("address JSON");
    let decoded: T = json::from_value(encoded.clone()).expect("decode address JSON");
    assert_eq!(&decoded, value);
    norito::json!({"label": label, "value": encoded})
}

fn observed_records() -> json::Value {
    let mut frames = Vec::new();
    #[cfg(feature = "json")]
    let mut json_records = Vec::new();
    macro_rules! sample {
        ($label:literal, $value:expr) => {{
            let value = $value;
            frames.push(binary_record($label, &value));
            frames.push(binary_record(
                concat!($label, "/option"),
                &Some(value.clone()),
            ));
            frames.push(binary_record(
                concat!($label, "/vec"),
                &vec![value.clone(), value.clone()],
            ));
            frames.push(binary_record(
                concat!($label, "/map"),
                &BTreeMap::from([(7_u8, value.clone()), (19, value.clone())]),
            ));
            #[cfg(feature = "json")]
            json_records.push(json_record($label, &value));
        }};
    }
    let v4 = Ipv4Addr::new([192, 0, 2, 9]);
    let v6 = Ipv6Addr::from([0x2001, 0x0db8, 0, 0, 0, 0, 0, 0x1234]);
    let socket4 = SocketAddrV4 { ip: v4, port: 0 };
    let socket6 = SocketAddrV6 {
        ip: v6,
        port: u16::MAX,
    };
    let host = SocketAddrHost {
        host: "api.example.test".into(),
        port: 443,
    };
    sample!("ipv4", v4);
    sample!("ipv4-max", Ipv4Addr::new([255; 4]));
    sample!("ipv6", v6);
    sample!("ipv6-max", Ipv6Addr::from([u16::MAX; 8]));
    sample!("ip-v4", IpAddr::V4(v4));
    sample!("ip-v6", IpAddr::V6(v6));
    sample!("socket-v4", socket4);
    sample!(
        "socket-v4-max-port",
        SocketAddrV4 {
            ip: v4,
            port: u16::MAX
        }
    );
    sample!("socket-v6", socket6);
    sample!("socket-v6-zero-port", SocketAddrV6 { ip: v6, port: 0 });
    sample!("socket-host", host.clone());
    sample!("socket-enum-v4", SocketAddr::Ipv4(socket4));
    sample!("socket-enum-v6", SocketAddr::Ipv6(socket6));
    sample!("socket-enum-host", SocketAddr::Host(host));
    let result = norito::json!({"frames": frames});
    #[cfg(feature = "json")]
    let result = {
        let mut result = result;
        result
            .as_object_mut()
            .unwrap()
            .insert("json".into(), json_records.into());
        result
    };
    result
}

#[test]
fn address_schema_identities_preserve_captured_frames_and_layouts() {
    let expected: json::Value = json::from_str(include_str!(
        "../tests/fixtures/address_schema_identity_frames.json"
    ))
    .expect("immutable pre-declaration address fixture");
    #[cfg(not(feature = "json"))]
    let expected = {
        let mut expected = expected;
        expected.as_object_mut().unwrap().remove("json");
        expected
    };
    assert_eq!(observed_records(), expected);
}

#[test]
fn address_frame_identity_rejects_other_roots_and_corruption() {
    let value = Ipv4Addr::new([192, 0, 2, 9]);
    let frame = norito::to_bytes(&value).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<Ipv4Addr>(&frame).unwrap(),
        value
    );
    assert!(norito::decode_from_bytes::<[u8; 4]>(&frame).is_err());
    assert!(norito::decode_from_bytes::<Ipv4Addr>(&frame[..frame.len() - 1]).is_err());
    let mut substituted = frame;
    substituted[6] ^= 1;
    assert!(norito::decode_from_bytes::<Ipv4Addr>(&substituted).is_err());
}
