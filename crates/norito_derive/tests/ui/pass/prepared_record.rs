//! One derived record walk supports both ordinary and caller-owned field custody.
use norito::core::{
    CanonicalField, DecodeField, DecodeIntoError, DecodeRecordFields, FieldDestination,
};
use std::convert::Infallible;
#[derive(norito::NoritoSerialize, norito::DeserializePayload, norito::NoritoSchema)]
#[norito(decode_fields, decode_from_slice)]
#[norito_schema(name = "ui.PreparedRecord")]
struct Record {
    value: u8,
    fixed: [u8; 4],
}
struct Destination([u8; 5]);
impl FieldDestination for Destination {
    type Error = Infallible;
}
impl DecodeField<0, u8> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, u8>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        self.0[0] = field.decode_owned()?;
        Ok(())
    }
}
impl DecodeField<1, [u8; 4]> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, [u8; 4]>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        self.0[1..].copy_from_slice(&field.decode_owned()?);
        Ok(())
    }
}
fn main() {
    let value = Record {
        value: 9,
        fixed: [1, 2, 3, 4],
    };
    let bytes = norito::encode_canonical(&value).unwrap();
    let result = norito::decode_canonical::<Record>(&bytes).unwrap();
    assert_eq!(result.value, 9);
    fn requires_one_generated_walk<T: DecodeRecordFields<Destination>>() {}
    requires_one_generated_walk::<Record>();
}
