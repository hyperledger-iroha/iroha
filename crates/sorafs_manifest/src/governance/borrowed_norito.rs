//! Borrowed governance field encodings with payload-only obligations.

use norito::core::SerializePayload;
/// Borrowed value that delegates canonical Norito serialization.
pub(super) struct Value<'a, T>(pub(super) &'a T);
impl<T: SerializePayload> SerializePayload for Value<'_, T> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> std::option::Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> std::option::Option<usize> {
        self.0.encoded_len_exact()
    }
}
/// Borrowed byte slice with the exact owned `Vec<u8>` wire representation.
pub(super) struct Vec<'a>(pub(super) &'a [u8]);
impl SerializePayload for Vec<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::write_seq_len(
            writer,
            u64::try_from(self.0.len()).map_err(|_| norito::core::Error::LengthMismatch)?,
        )?;
        writer.write_all(self.0)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> std::option::Option<usize> {
        self.0
            .len()
            .checked_add(norito::core::seq_len_prefix_len(self.0.len()))
    }
    fn encoded_len_exact(&self) -> std::option::Option<usize> {
        self.encoded_len_hint()
    }
}
/// Borrowed optional byte slice with the exact `Option<Vec<u8>>` wire representation.
pub(super) struct Option<'a>(pub(super) std::option::Option<&'a [u8]>);
impl SerializePayload for Option<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        match self.0 {
            Some(bytes) => {
                writer.write_all(&[1])?;
                let value = Vec(bytes);
                norito::core::write_len_prefixed(writer, &value)?;
            }
            None => writer.write_all(&[0])?,
        }
        Ok(())
    }
    fn encoded_len_hint(&self) -> std::option::Option<usize> {
        self.encoded_len_exact()
    }
    fn encoded_len_exact(&self) -> std::option::Option<usize> {
        match self.0 {
            Some(bytes) => {
                let payload = Vec(bytes).encoded_len_exact()?;
                1_usize
                    .checked_add(norito::core::len_prefix_len(payload))?
                    .checked_add(payload)
            }
            None => Some(1),
        }
    }
}
