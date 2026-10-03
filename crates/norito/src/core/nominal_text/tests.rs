//! Original nominal identity, canonical flags and borrowed payload parity.

use super::*;
use crate::core::{
    Archived, DecodeFlagsGuard, DeserializePayload, Encoder, Header, SerializePayload,
};

#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito.tests.NominalText")]
#[repr(align(32))]
struct Text(String);

impl SerializePayload for Text {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.as_str().serialize(writer)
    }
}
impl<'a> DeserializePayload<'a> for Text {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).unwrap()
    }
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        String::try_deserialize(archived.cast()).map(Self)
    }
}
impl NominalText for Text {
    const MAX_TEXT_BYTES: usize = 16 * 1024;
}

#[test]
fn original_nominal_padding_and_every_layout_match_the_canonical_writer() {
    for value in ["", "ascii", "é", "q\u{301}", &"long".repeat(4096)] {
        let typed = Text(value.to_owned());
        let canonical = crate::encode_canonical(&typed).unwrap();
        let text = borrow_canonical_text::<Text>(&canonical).unwrap();
        assert_eq!(text, value);
        let offset = text.as_ptr() as usize - canonical.as_ptr() as usize;
        assert!(offset >= Header::SIZE + crate::core::payload_alignment_padding_for::<Text>());
        assert_eq!(text.as_ptr(), canonical[offset..].as_ptr());
        for flags in 0..=crate::core::supported_header_flags() {
            if crate::core::validate_header_flags(flags).is_err() {
                continue;
            }
            let _flags = DecodeFlagsGuard::enter(flags);
            let alternate = crate::core::to_bytes(&typed).unwrap();
            assert_eq!(
                borrow_canonical_text::<Text>(&alternate).is_ok(),
                alternate == canonical,
                "flags {flags}"
            );
            assert_eq!(
                crate::decode_canonical::<Text>(&alternate).is_ok(),
                alternate == canonical,
            );
        }
        // A byte-aligned view must not adopt or copy an aligned archive owner.
        let mut shifted = vec![0];
        shifted.extend_from_slice(&canonical);
        let text = borrow_canonical_text::<Text>(&shifted[1..]).unwrap();
        assert_eq!(text.as_ptr(), shifted[1 + offset..].as_ptr());
    }
}

#[test]
fn malformed_headers_padding_prefixes_utf8_and_nominal_drift_are_rejected() {
    let canonical = crate::encode_canonical(&Text("valid".to_owned())).unwrap();
    for end in 0..canonical.len() {
        assert!(borrow_canonical_text::<Text>(&canonical[..end]).is_err());
    }
    for offset in [0, 4, 5, 6, 22, 23, 31, 39, 40] {
        let mut changed = canonical.clone();
        changed[offset] ^= 0xff;
        assert!(
            borrow_canonical_text::<Text>(&changed).is_err(),
            "offset {offset}"
        );
    }
    let mut suffixed = canonical.clone();
    suffixed.push(0);
    assert!(borrow_canonical_text::<Text>(&suffixed).is_err());
    let string_frame = crate::encode_canonical(&"valid".to_owned()).unwrap();
    assert!(matches!(
        borrow_canonical_text::<Text>(&string_frame),
        Err(Error::SchemaMismatch)
    ));
    let payload_start = Header::SIZE + crate::core::payload_alignment_padding_for::<Text>();
    let value_start = borrow_canonical_text::<Text>(&canonical).unwrap().as_ptr() as usize
        - canonical.as_ptr() as usize;
    for (offset, byte) in [(payload_start, 127), (value_start, 0xff)] {
        let mut changed = canonical.clone();
        changed[offset] = byte;
        let checksum = crate::core::crc64(&changed[payload_start..]);
        changed[31..39].copy_from_slice(&checksum.to_le_bytes());
        assert!(borrow_canonical_text::<Text>(&changed).is_err());
    }
    let large = crate::encode_canonical(&Text("a".repeat(Text::MAX_TEXT_BYTES + 1))).unwrap();
    assert!(matches!(
        borrow_canonical_text::<Text>(&large),
        Err(Error::FieldLengthExceeded { .. })
    ));
}

#[test]
fn payload_check_precedes_body_access_and_restores_outer_context() {
    let _flags = DecodeFlagsGuard::enter(crate::core::default_encode_flags());
    let mut bytes = Vec::new();
    crate::core::serialize_to_buffer(&"abc", &mut bytes).unwrap();
    let (text, consumed) = borrow_text_payload(&bytes, |length| {
        assert_eq!(length, 3);
        Ok(())
    })
    .unwrap();
    assert_eq!((text, consumed), ("abc", bytes.len()));
    assert!(matches!(
        borrow_text_payload(&bytes[..bytes.len() - 1], |_| Err(Error::SchemaMismatch)),
        Err(Error::SchemaMismatch)
    ));
    assert!(matches!(
        borrow_text_payload(&bytes[..bytes.len() - 1], |_| Ok(())),
        Err(Error::LengthMismatch)
    ));
    let canonical = crate::encode_canonical(&Text("flags".into())).unwrap();
    let before = crate::core::get_decode_flags();
    borrow_canonical_text::<Text>(&canonical).unwrap();
    assert_eq!(crate::core::get_decode_flags(), before);
}
