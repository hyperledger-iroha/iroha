//! Destination of escaped record keys for the scalar policy field kernels.
//!
//! The pipeline validity policy and the records declared with
//! `scalar_policy_record!` decode their keys through these destinations.

use super::PipelineValidityPolicyV1;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use norito::json;

/// Original JSON or allocation refusal from direct pipeline policy parsing.
#[derive(Debug, thiserror::Error)]
pub enum PipelinePolicyJsonAdmissionError {
    /// Native syntax/scalar error or original decoder observer refusal.
    #[error(transparent)]
    Json(#[from] json::Error),
    /// Original-pool or physical allocator refusal for escaped key scratch.
    #[error(transparent)]
    Allocation(#[from] ChargedBufferError),
}
impl PipelineValidityPolicyV1 {
    /// Decode this scalar record directly with original-pool escaped-key scratch.
    ///
    /// Ordinary and admitted reads share the one field kernel generated with the
    /// type. The result is Copy and retains no allocation. The enclosing owner
    /// funds its record/control and keeps source bytes and the original decoder
    /// context. Call [`Self::validate`] before installation; parsing does not grant
    /// policy validity or committed State authority.
    /// # Errors
    /// Preserves native JSON observer, finite-pool and physical allocator errors.
    pub fn try_decode_json(
        parser: &mut json::Parser<'_>,
        budget: &AllocationBudget,
    ) -> Result<Self, PipelinePolicyJsonAdmissionError> {
        super::decode_record(parser, &Admitted { budget })
    }
}
pub(crate) fn invalid(message: &'static str) -> json::Error {
    json::Error::WithPos {
        msg: message,
        byte: 0,
        line: 1,
        col: 1,
    }
}
pub(crate) fn key_text<'a, T: AsRef<str>>(key: &'a json::KeyRef<'_, T>) -> &'a str {
    match key {
        json::KeyRef::Borrowed(text) => text,
        json::KeyRef::Owned(text) => text.as_ref(),
    }
}
pub(crate) trait Destination {
    type Error: From<json::Error>;
    type Text: AsRef<str>;
    fn key<'a>(
        &self,
        object: &mut json::MapVisitor<'a, '_>,
    ) -> Result<Option<json::KeyRef<'a, Self::Text>>, Self::Error>;
}
pub(crate) struct Ordinary;
impl Destination for Ordinary {
    type Error = json::Error;
    type Text = String;
    fn key<'a>(
        &self,
        object: &mut json::MapVisitor<'a, '_>,
    ) -> Result<Option<json::KeyRef<'a>>, Self::Error> {
        object.next_key()
    }
}
pub(crate) struct Text(ChargedBuffer<u8>);
impl AsMut<[u8]> for Text {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}
impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        std::str::from_utf8(self.0.as_slice()).expect("the original string kernel writes UTF-8")
    }
}
pub(crate) struct Admitted<'a> {
    pub(crate) budget: &'a AllocationBudget,
}
impl Destination for Admitted<'_> {
    type Error = PipelinePolicyJsonAdmissionError;
    type Text = Text;
    fn key<'a>(
        &self,
        object: &mut json::MapVisitor<'a, '_>,
    ) -> Result<Option<json::KeyRef<'a, Text>>, Self::Error> {
        object.next_key_with_buffer(|length| {
            let mut buffer = ChargedBuffer::new(length, self.budget)?;
            for _ in 0..length {
                buffer.push_reserved(0);
            }
            Ok::<_, Self::Error>(Text(buffer))
        })
    }
}
#[cfg(test)]
mod tests;
