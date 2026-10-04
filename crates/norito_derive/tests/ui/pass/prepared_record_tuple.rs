//! A closed tuple retains its exact positional codec and one prepared field walk.
use norito::core::{
    CanonicalField, DecodeField, DecodeFromSlice, DecodeIntoError, DecodeRecordFields,
    FieldDestination,
};
#[derive(norito::NoritoSerialize, norito::DeserializePayload, norito::NoritoSchema)]
#[norito(decode_fields, decode_from_slice)]
#[norito_schema(name = "ui.PreparedTuple")]
struct Tuple(u32, u64);
struct Destination(u32, u64);
impl FieldDestination for Destination {
    type Error = std::convert::Infallible;
}
impl DecodeField<0, u32> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, u32>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        field.with_payload(|bytes| {
            let (value, used) = u32::decode_from_slice(bytes)?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            self.0 = value;
            Ok(())
        })
    }
}
impl DecodeField<1, u64> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, u64>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        field.with_payload(|bytes| {
            let (value, used) = u64::decode_from_slice(bytes)?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            self.1 = value;
            Ok(())
        })
    }
}
fn main() {
    fn original<T: DecodeRecordFields<Destination>>() {}
    original::<Tuple>();
    let bytes = norito::encode_canonical(&Tuple(3, 7)).unwrap();
    let value = norito::decode_canonical::<Tuple>(&bytes).unwrap();
    assert_eq!((value.0, value.1), (3, 7));
}
