//! Golden fixtures for Norito serialization of nested enums/options/tuples.

use iroha_schema::IntoSchema;
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};

#[derive(Debug, PartialEq, Eq, IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct NestedSample {
    tag: SampleEnum,
    payload: Option<SamplePayload>,
    marker: bool,
}

#[derive(Debug, PartialEq, Eq, IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct SamplePayload {
    count: u32,
    labels: Vec<String>,
}

impl<'a> norito::core::DecodeFromSlice<'a> for SamplePayload {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        use std::{
            alloc::{Layout, alloc, dealloc},
            ptr, slice,
        };

        let align = std::mem::align_of::<norito::core::Archived<SamplePayload>>();
        let layout = Layout::from_size_align(bytes.len(), align)
            .map_err(|_| norito::core::Error::LengthMismatch)?;

        unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                return Err(norito::core::Error::Message("aligned alloc failed".into()));
            }

            ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            let slice = slice::from_raw_parts(ptr, bytes.len());
            let _guard = norito::core::PayloadCtxGuard::enter(slice);
            let archived = &*(ptr as *const norito::core::Archived<SamplePayload>);
            let value = <SamplePayload as norito::NoritoDeserialize>::deserialize(archived);
            dealloc(ptr, layout);
            Ok((value, bytes.len()))
        }
    }
}

#[derive(Debug, PartialEq, Eq, IntoSchema, NoritoSerialize, NoritoDeserialize)]
enum SampleEnum {
    Unit,
    Tuple(SampleTuple),
    Struct(SampleStruct),
}

#[derive(Debug, PartialEq, Eq, IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct SampleTuple {
    first: u8,
    second: u16,
}

#[derive(Debug, PartialEq, Eq, IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct SampleStruct {
    id: u32,
    name: String,
}

#[test]
fn nested_sample_golden_hex() {
    let value = NestedSample {
        tag: SampleEnum::Struct(SampleStruct {
            id: 0xA1B2C3D4,
            name: "delta".to_owned(),
        }),
        payload: Some(SamplePayload {
            count: 7u32,
            labels: vec!["alpha".to_owned(), "beta".to_owned()],
        }),
        marker: true,
    };

    let bytes = to_bytes(&value).expect("serialize");
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let expected_sequential = "4e52543000009c12f30b5d6b26c19c12f30b5d6b26c1009400000000000000332d84461e4846fa002d000000000000000200000021000000000000000400000000000000d4c3b2a10d00000000000000050000000000000064656c74614e00000000000000014500000000000000040000000000000007000000310000000000000002000000000000000d000000000000000500000000000000616c7068610c00000000000000040000000000000062657461010000000000000001";
    let expected_packed = "4e52543000279c12f30b5d6b26c19c12f30b5d6b26c1004200000000000000f5c1a3e241d78c343f03102e020000000b00d4c3b2a10564656c7461012c02260700000002060505616c7068610462657461000000000000000006000000000000000b0000000000000001";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_sequential
    } else {
        expected_packed
    };
    assert_eq!(hex, expected);

    let decoded: NestedSample = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(decoded, value);

    let bytes_again = to_bytes(&decoded).expect("re-encode");
    assert_eq!(bytes_again, bytes);
}
