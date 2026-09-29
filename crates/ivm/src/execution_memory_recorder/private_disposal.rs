//! Erase exclusive fixed diagnostic storage only after its final shared owner leaves.

use super::{
    DiagnosticMemoryAccess, DiagnosticMemoryAccessKind, DiagnosticMemoryPrivacyTag, RecorderInner,
};
use iroha_crypto::zeroize_value_for_confidential_discard;

impl DiagnosticMemoryAccess {
    pub(super) fn scrub(&mut self) {
        if let Some(ordinal) = &mut self.step_ordinal {
            zeroize_value_for_confidential_discard(ordinal);
        }
        self.step_ordinal = None;
        zeroize_value_for_confidential_discard(&mut self.access_ordinal);
        zeroize_value_for_confidential_discard(&mut self.byte_offset);
        zeroize_value_for_confidential_discard(&mut self.address);
        zeroize_value_for_confidential_discard(&mut self.before);
        zeroize_value_for_confidential_discard(&mut self.after);
        // Assign valid variants. Byte-zeroing the struct could create invalid
        // enum discriminants or touch padding and is intentionally not used.
        self.kind = DiagnosticMemoryAccessKind::Read;
        self.privacy_tag = DiagnosticMemoryPrivacyTag::Unknown;
    }
}

impl Drop for RecorderInner {
    fn drop(&mut self) {
        // Arc destroys this payload in place after its final borrower drops.
        // Neither fixed backing grows or truncates during recorder ownership.
        // Buffer field destruction then frees backing before its original credit.
        if let Some(image) = &mut self.initial_image {
            zeroize_value_for_confidential_discard(image.as_mut_slice());
        }
        for row in self.rows.as_mut_slice() {
            row.scrub();
        }
    }
}

#[cfg(test)]
mod tests;
