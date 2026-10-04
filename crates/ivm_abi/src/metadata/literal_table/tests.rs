//! Descriptor, envelope and borrowed-index parity at the single literal boundary.

use super::*;
use crate::metadata::{LITERAL_SECTION_MAGIC, ProgramMetadata, encode_literal_descriptor};

fn pointer(ty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut bytes = (ty as u16).to_be_bytes().to_vec();
    bytes.push(1);
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(iroha_crypto::Hash::new(payload).as_ref());
    bytes
}

fn program(values: &[(LiteralKindV1, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = ProgramMetadata::default().encode();
    let data_len = values.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
    let prefix = 16 + 8 * values.len();
    let padding = (4 - (prefix + data_len) % 4) % 4;
    bytes.extend_from_slice(&LITERAL_SECTION_MAGIC);
    bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(padding as u32).to_le_bytes());
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    let mut offset = prefix;
    for (kind, value) in values {
        bytes.extend_from_slice(
            &encode_literal_descriptor(*kind, offset as u64)
                .unwrap()
                .to_le_bytes(),
        );
        offset += value.len();
    }
    for (_, value) in values {
        bytes.extend_from_slice(value);
    }
    bytes.extend(std::iter::repeat_n(0, padding));
    bytes
}

fn directory(program: &[u8]) -> Result<LiteralDirectory<'_>, VMError> {
    let parsed = ProgramMetadata::parse(program)?;
    LiteralDirectory::validate(
        program,
        parsed.header_len,
        parsed.literal_section,
        SyscallPolicy::AbiV1,
    )
}

#[test]
fn mixed_directory_borrows_original_payload_and_exact_pointer_order() {
    let bytes = program(&[
        (
            LiteralKindV1::PointerTlv,
            pointer(PointerType::Blob, b"first"),
        ),
        (LiteralKindV1::I64, i64::MIN.to_le_bytes().to_vec()),
        (
            LiteralKindV1::PointerTlv,
            pointer(PointerType::NoritoBytes, b"second"),
        ),
    ]);
    let values = directory(&bytes).unwrap();
    assert_eq!(values.len(), 3);
    assert!(!values.is_empty());
    assert_eq!(values.get(1), Some(ValidatedLiteral::I64(i64::MIN as u64)));
    assert_eq!(values.get(3), None);
    let Some(ValidatedLiteral::Pointer {
        address,
        payload,
        type_id,
    }) = values.get(0)
    else {
        panic!("pointer");
    };
    assert_eq!(type_id, PointerType::Blob);
    assert_eq!(payload, b"first");
    assert_eq!(
        payload.as_ptr(),
        bytes[crate::metadata::HEADER_SIZE + address as usize + 7..].as_ptr()
    );
    let mut pointers = values.pointer_addresses();
    assert_eq!(pointers.len(), 2);
    assert_eq!(pointers.next(), Some(address));
    assert_eq!(pointers.len(), 1);
    assert!(pointers.next().unwrap() > address);
    assert_eq!(pointers.len(), 0);
    assert_eq!(pointers.next(), None);
    assert_eq!(values.iter().len(), 3);
}

#[test]
fn malformed_directory_and_payloads_never_expose_an_index() {
    let original = program(&[
        (LiteralKindV1::PointerTlv, pointer(PointerType::Blob, b"")),
        (LiteralKindV1::I64, 1_i64.to_le_bytes().to_vec()),
    ]);
    let parsed = ProgramMetadata::parse(&original).unwrap();
    let section = parsed.literal_section.unwrap();
    for mutation in 0..7 {
        let mut bytes = original.clone();
        match mutation {
            0 => bytes[section.entries_start + 7] = 0xff,
            1 => bytes[section.entries_start..section.entries_start + 8]
                .copy_from_slice(&0_u64.to_le_bytes()),
            2 => {
                let first = bytes[section.entries_start..section.entries_start + 8].to_vec();
                bytes[section.entries_start + 8..section.entries_start + 16]
                    .copy_from_slice(&first);
            }
            3 => bytes[section.entries_start] += 1,
            4 => bytes[section.data_start + 7] ^= 1,
            5 => bytes[section.data_start + 2] = 2,
            6 => bytes[section.entries_start + 8] -= 1,
            _ => unreachable!(),
        }
        assert!(
            matches!(directory(&bytes), Err(VMError::InvalidMetadata)),
            "mutation {mutation}"
        );
    }
    for width in [0, 7, 9] {
        assert!(matches!(
            directory(&program(&[(LiteralKindV1::I64, vec![0; width])])),
            Err(VMError::InvalidMetadata)
        ));
    }
}

#[test]
fn absent_and_empty_directories_have_no_values_and_policy_errors_stay_typed() {
    for bytes in [ProgramMetadata::default().encode(), program(&[])] {
        let values = directory(&bytes).unwrap();
        assert!(values.is_empty());
        assert_eq!(values.iter().len(), 0);
        assert_eq!(values.pointer_addresses().len(), 0);
    }
    assert!(matches!(
        directory(&program(&[(
            LiteralKindV1::PointerTlv,
            pointer(PointerType::TestOnly, b"")
        )])),
        Err(VMError::AbiTypeNotAllowed {
            abi: 1,
            type_id: 0x0ffe
        })
    ));
    let bytes = program(&[]);
    let mut section = ProgramMetadata::parse(&bytes)
        .unwrap()
        .literal_section
        .unwrap();
    section.count = usize::from(u16::MAX) + 2;
    assert!(matches!(
        LiteralDirectory::validate(
            &bytes,
            crate::metadata::HEADER_SIZE,
            Some(section),
            SyscallPolicy::AbiV1
        ),
        Err(VMError::InvalidMetadata)
    ));
}
