use norito::core::{Archived, Error as NoritoCodecError};

use crate::error::ParseError;

/// Split a string into two non-empty parts separated by `delimiter`.
///
/// Returns the left and right parts or [`ParseError`] with the provided messages.
/// The returned slices borrow from the input [`&str`] and thus share its lifetime.
///
/// # Errors
///
/// Returns an error when the delimiter is missing, or when either the
/// left or right side of the split is empty. The error messages are set
/// to `no_delim_err`, `empty_left_err`, or `empty_right_err` respectively.
pub fn split_nonempty<'a>(
    s: &'a str,
    delimiter: char,
    no_delim_err: &'static str,
    empty_left_err: &'static str,
    empty_right_err: &'static str,
) -> Result<(&'a str, &'a str), ParseError> {
    match s.rsplit_once(delimiter) {
        None => Err(ParseError {
            reason: no_delim_err,
        }),
        Some(("", _)) => Err(ParseError {
            reason: empty_left_err,
        }),
        Some((_, "")) => Err(ParseError {
            reason: empty_right_err,
        }),
        Some((left, right)) => Ok((left, right)),
    }
}

/// An owning wrapper around a value used in persistent storage.
///
/// This type is primarily used together with [`Ref`] to avoid
/// repetition of simple "value" structs throughout the codebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Owned<T>(pub T);

impl<T> core::ops::Deref for Owned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for Owned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> AsRef<T> for Owned<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> Owned<T> {
    /// Construct a new [`Owned`].
    pub const fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Consume the wrapper and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

#[cfg(feature = "json")]
impl<T> norito::json::FastJsonWrite for Owned<T>
where
    T: norito::json::JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl<T> norito::json::JsonDeserialize for Owned<T>
where
    T: norito::json::JsonDeserialize,
{
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        T::json_deserialize(parser).map(Owned)
    }
}

impl<T> norito::core::NoritoSerialize for Owned<T>
where
    T: norito::core::NoritoSerialize,
{
    fn schema_hash() -> [u8; 16]
    where
        Self: Sized,
    {
        <T as norito::core::NoritoSerialize>::schema_hash()
    }

    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::Error> {
        <T as norito::core::NoritoSerialize>::serialize(&self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        <T as norito::core::NoritoSerialize>::encoded_len_hint(&self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        <T as norito::core::NoritoSerialize>::encoded_len_exact(&self.0)
    }
}

impl<'de, T> norito::core::NoritoDeserialize<'de> for Owned<T>
where
    T: norito::core::NoritoDeserialize<'de>,
{
    fn schema_hash() -> [u8; 16] {
        <T as norito::core::NoritoDeserialize>::schema_hash()
    }

    fn deserialize(archived: &'de Archived<Self>) -> Self {
        let inner = <T as norito::core::NoritoDeserialize>::deserialize(archived.cast());
        Self(inner)
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, NoritoCodecError> {
        <T as norito::core::NoritoDeserialize>::try_deserialize(archived.cast()).map(Owned)
    }
}

/// Reference to an entity identified by `Id` with associated `Value`.
///
/// This is a lightweight alternative to storing the full entity when the
/// identifier is already known. It is commonly produced from key-value
/// storages where the key acts as the identifier and the value stores the
/// rest of the data.
#[derive(Debug, Copy, Clone)]
pub struct Ref<'a, Id, Value> {
    /// Identification of the entity.
    pub id: &'a Id,
    /// Associated value for the entity.
    pub value: &'a Value,
}

impl<'a, Id, Value> Ref<'a, Id, Value> {
    /// Construct a new [`Ref`].
    pub const fn new(id: &'a Id, value: &'a Value) -> Self {
        Self { id, value }
    }

    /// Get identifier of the referenced entity.
    pub const fn id(&self) -> &Id {
        self.id
    }

    /// Get the associated value of the referenced entity.
    pub const fn value(&self) -> &Value {
        self.value
    }
}

impl<Id, Value> core::ops::Deref for Ref<'_, Id, Value> {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_nonempty_succeeds() {
        let (l, r) = split_nonempty("left#right", '#', "missing", "empty_left", "empty_right")
            .expect("split succeeds");
        assert_eq!(l, "left");
        assert_eq!(r, "right");
    }

    #[test]
    fn owned_wraps_and_unwraps() {
        let wrapped = Owned::new(5u32);
        assert_eq!(*wrapped, 5);
        assert_eq!(wrapped.into_inner(), 5);
    }

    #[test]
    fn ref_provides_access() {
        let value = Owned::new(10u32);
        let key = 1u32;
        let r = Ref::new(&key, &value);
        assert_eq!(**r, 10);
        assert_eq!(r.id(), &1u32);
        assert_eq!(r.value(), &value);
    }
}
