use iroha_torii_shared::connect::{ConnectPayloadV1, EnvelopeV1};

fn decode_hex(input: &str) -> Vec<u8> {
    assert!(input.len().is_multiple_of(2), "hex length must be even");
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16).expect("valid hex hi");
        let lo = (bytes[i + 1] as char).to_digit(16).expect("valid hex lo");
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    out
}

fn decode_envelope_compat(bytes: &[u8]) -> EnvelopeV1 {
    let view = norito::core::from_bytes_view(bytes).expect("framed envelope view");
    match view.decode::<EnvelopeV1>() {
        Ok(envelope) => envelope,
        Err(norito::core::Error::SchemaMismatch) => view
            .decode_unchecked::<EnvelopeV1>()
            .expect("decode envelope with legacy schema hash"),
        Err(err) => panic!("decode envelope: {err:?}"),
    }
}

#[test]
fn decode_live_connect_envelope_hex_fixture() {
    let hex = "4e5254300000f35017c774558f19f35017c774558f19006c00000000000000d9148a06a682358f00080000000000000002000000000000005400000000000000010000001800000000000000100000000000000046495f50524f504f53414c5f5349474e28000000000000002000000000000000086a72f402ddccae0559eacfefdd1403c3eb889eb3b920f0a614ede8888405d1";
    let bytes = decode_hex(hex);
    let env = decode_envelope_compat(&bytes);
    println!("decoded envelope seq={}", env.seq);
    println!("decoded payload={env:?}");
    match env.payload {
        ConnectPayloadV1::SignRequestRaw { domain_tag, bytes } => {
            assert_eq!(domain_tag, "FI_PROPOSAL_SIGN");
            assert_eq!(bytes.len(), 32);
        }
        other => panic!("unexpected payload variant: {other:?}"),
    }
}

#[test]
fn decode_java_sign_result_ok_envelope_fixture() {
    let hex = "4e52543000000b36414bbbba14690b36414bbbba1469008002000000000000e8ae6adadc072f3e00080000000000000002000000000000006802000000000000030000005c020000000000000400000000000000000000004802000000000000400000000000000001000000000000000001000000000000000101000000000000000201000000000000000301000000000000000401000000000000000501000000000000000601000000000000000701000000000000000801000000000000000901000000000000000a01000000000000000b01000000000000000c01000000000000000d01000000000000000e01000000000000000f01000000000000001001000000000000001101000000000000001201000000000000001301000000000000001401000000000000001501000000000000001601000000000000001701000000000000001801000000000000001901000000000000001a01000000000000001b01000000000000001c01000000000000001d01000000000000001e01000000000000001f01000000000000002001000000000000002101000000000000002201000000000000002301000000000000002401000000000000002501000000000000002601000000000000002701000000000000002801000000000000002901000000000000002a01000000000000002b01000000000000002c01000000000000002d01000000000000002e01000000000000002f01000000000000003001000000000000003101000000000000003201000000000000003301000000000000003401000000000000003501000000000000003601000000000000003701000000000000003801000000000000003901000000000000003a01000000000000003b01000000000000003c01000000000000003d01000000000000003e01000000000000003f";
    let bytes = decode_hex(hex);
    let env = decode_envelope_compat(&bytes);
    match env.payload {
        ConnectPayloadV1::SignResultOk { signature } => {
            assert_eq!(signature.bytes().len(), 64);
            assert_eq!(signature.bytes()[0], 0);
            assert_eq!(signature.bytes()[63], 63);
        }
        other => panic!("unexpected payload variant: {other:?}"),
    }
}
