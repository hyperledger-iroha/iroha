//! Opaque actual source originals and the coordinator's incarnation-bound immutable archive.

use super::*;

/// Native operation sources selected by the coordinator under its exclusive custody owner.
/// There is no public constructor, serialized source or caller-selected replacement.
/// The native proof owner independently verifies every supplied proof before consuming it.
pub struct NativeSourcesV1<'a> {
    predecessor: Option<&'a ReleasedStep>,
    folded: Option<&'a KagemushaWalletFoldRecordV1>,
    held_send: Option<&'a ReleasedStep>,
    receive_credit: Option<&'a KagemushaWalletCreditDigestRecordV1>,
    objects: &'a mut dyn ObjectStore,
}
impl<'a> NativeSourcesV1<'a> {
    pub(super) fn new(
        predecessor: Option<&'a ReleasedStep>,
        folded: Option<&'a KagemushaWalletFoldRecordV1>,
        held_send: Option<&'a ReleasedStep>,
        receive_credit: Option<&'a KagemushaWalletCreditDigestRecordV1>,
        objects: &'a mut dyn ObjectStore,
    ) -> Self {
        Self {
            predecessor,
            folded,
            held_send,
            receive_credit,
            objects,
        }
    }
    /// Exact indexed released predecessor, including its original retained completion.
    pub const fn predecessor(&self) -> Option<&'a ReleasedStep> {
        self.predecessor
    }
    /// Actual retained fold belonging to that predecessor, when it is available.
    pub const fn folded(&self) -> Option<&'a KagemushaWalletFoldRecordV1> {
        self.folded
    }
    /// Actual indexed original Send bound byte-for-byte to an Archive's retained Payment.
    pub const fn held_send(&self) -> Option<&'a ReleasedStep> {
        self.held_send
    }
    /// Genuine present/insert paths derived after the predecessor credit root was verified.
    pub const fn receive_credit(&self) -> Option<&'a KagemushaWalletCreditDigestRecordV1> {
        self.receive_credit
    }
    /// Read/publish immutable source objects under this coordinator's actual custody archive.
    /// These objects cannot select a monetary head, fabricate a fold or authorize Advance.
    pub fn objects(&mut self) -> &mut dyn ObjectStore {
        self.objects
    }
}
