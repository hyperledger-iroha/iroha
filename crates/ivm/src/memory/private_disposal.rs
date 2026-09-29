//! Final exclusive image disposal precedes backing deallocation and reservation refund.

use super::MemoryImage;

impl Drop for MemoryImage {
    fn drop(&mut self) {
        // Local images are exact boxed slices. Funded images have fully
        // initialized fixed backing before becoming a MemoryImage. Scrub the
        // whole initialized image, including INPUT and bytes with no live tag.
        // Child backing and original charges are released only after this Drop.
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self[..]);
    }
}

#[cfg(test)]
pub(crate) mod tests;
