//! Unchecked borrowed-format JSON value storage.
use super::{Error, JsonDeserialize, JsonSerialize, Parser, try_decode_string_copy};
#[derive(Debug, Clone)]
pub struct RawValue {
    inner: Box<str>,
}
impl RawValue {
    #[inline]
    pub fn from_boxed(inner: Box<str>) -> Self {
        Self { inner }
    }
    #[inline]
    pub fn from_string(s: String) -> Self {
        Self {
            inner: s.into_boxed_str(),
        }
    }
    #[inline]
    pub fn get(&self) -> &str {
        &self.inner
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.inner
    }
    #[inline]
    pub fn into_boxed_str(self) -> Box<str> {
        self.inner
    }
    #[inline]
    pub fn into_string(self) -> String {
        self.inner.into()
    }
}
impl JsonSerialize for RawValue {
    fn json_serialize(&self, out: &mut String) {
        out.push_str(self.get());
    }
    // TODO: Add a bounded override only after an allocation-free semantic
    // validator can compare decoded object keys without allocating. Lexical
    // preflight alone cannot preserve duplicate-key rejection for unchecked
    // text accepted by `RawValue`'s public constructors.
}
impl JsonDeserialize for RawValue {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
        let slice = p.raw_value_slice()?;
        Ok(RawValue::from_string(try_decode_string_copy(slice)?))
    }
}
