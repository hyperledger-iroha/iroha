//! Canonical borrowed nominal text has no hidden codec allocation lifetime.

use super::*;
use norito::core::{NominalText, borrow_canonical_text};

#[derive(norito::NoritoSchema)]
#[norito_schema(name = "norito.tests.allocation.NominalText")]
#[repr(align(32))]
struct Text(String);
impl SerializePayload for Text {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.as_str().serialize(writer)
    }
}
impl NominalText for Text {
    const MAX_TEXT_BYTES: usize = 16384;
}

#[test]
fn nominal_text_borrows_cold_and_repeated_frames_without_decode_scope_storage() {
    let bytes = encode_canonical(&Text("q\u{301}".repeat(4096))).unwrap();
    let mut result = None;
    assert_eq!(
        allocations_during(|| result = Some(borrow_canonical_text::<Text>(&bytes))),
        0
    );
    let text = result.take().unwrap().unwrap();
    let offset = text.as_ptr() as usize - bytes.as_ptr() as usize;
    assert_eq!(text.as_ptr(), bytes[offset..].as_ptr());
    assert_eq!(
        allocations_during(|| {
            for _ in 0..8 {
                assert_eq!(
                    borrow_canonical_text::<Text>(&bytes).unwrap().as_ptr(),
                    text.as_ptr()
                );
            }
        }),
        0
    );
    for end in 0..bytes.len() {
        let mut error = None;
        assert_eq!(
            allocations_during(|| error = Some(borrow_canonical_text::<Text>(&bytes[..end]))),
            0
        );
        assert!(error.unwrap().is_err());
    }
    for byte in 0..norito::core::Header::SIZE + 24 {
        let mut changed = bytes.clone();
        changed[byte] ^= 0xff;
        let mut result = None;
        assert_eq!(
            allocations_during(|| result = Some(borrow_canonical_text::<Text>(&changed))),
            0
        );
        assert!(result.unwrap().is_err());
    }
}
