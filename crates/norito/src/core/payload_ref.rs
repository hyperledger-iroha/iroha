//! Serialization-only borrows of an existing canonical field payload.

use super::{Encoder, Error, SerializePayload};
use crate::json::{BoundedJsonError, JsonSerialize, JsonWriteSink};

/// Borrow a field without adding framing, copying its graph, or providing a decoder.
///
/// The containing record owns its schema and field layout. This adapter forwards binary
/// and JSON serialization to the original value, including its length hints and errors.
pub struct PayloadRef<'a, T: ?Sized>(pub &'a T);

impl<T: ?Sized> std::ops::Deref for PayloadRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
    }
}

impl<T: SerializePayload + ?Sized> SerializePayload for PayloadRef<'_, T> {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(encoder)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl<T: JsonSerialize + ?Sized> JsonSerialize for PayloadRef<'_, T> {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out)
    }

    fn json_serialize_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        self.0.json_serialize_to(out)
    }
}

#[cfg(test)]
mod tests;
