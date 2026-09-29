//! Exact borrowed-view custody for native signer proof blocks.
//!
//! A certified block proves its header/result under authenticated authority, but does
//! not by itself identify the State/Kura source that supplied its local certificate.
//! Only this walk constructs a source-bound block. Borrowing the original view prevents
//! its identity from being reused while any block is retained; identity is never serialized.

use super::{Error, StateView};
use crate::sumeragi::certified_chain::{CertifiedBlock, CertifiedChain};

/// One existing certified reader tied to the exact immutable view used for native proof rows.
pub(crate) struct SignerCertifiedWalkV1<'view, 'state> {
    view: &'view StateView<'state>,
    chain: CertifiedChain<'view, StateView<'state>>,
}
impl<'view, 'state> SignerCertifiedWalkV1<'view, 'state> {
    pub(crate) fn new(view: &'view StateView<'state>) -> Result<Self, Error> {
        Ok(Self {
            view,
            chain: CertifiedChain::new(view).map_err(|_| Error::Finality)?,
        })
    }

    /// Every receipt is produced by this reader's actual checked walk, never supplied by a caller.
    pub(crate) fn walk(
        &self,
        start: u64,
        end: u64,
    ) -> impl Iterator<Item = Result<SignerCertifiedBlockV1<'view, 'state>, Error>> + '_ {
        self.chain.walk(start, end).map(|block| {
            block
                .map_err(|_| Error::Finality)
                .map(|block| SignerCertifiedBlockV1 {
                    view: self.view,
                    block,
                })
        })
    }
}

/// Actual certified receipt with its original borrowed source; no caller-facing constructor.
pub(crate) struct SignerCertifiedBlockV1<'view, 'state> {
    view: &'view StateView<'state>,
    block: CertifiedBlock,
}
impl SignerCertifiedBlockV1<'_, '_> {
    pub(crate) fn height(&self) -> u64 {
        self.block.height()
    }

    /// Reference identity is checked before any native proof predicate may consume the receipt.
    /// A fresh view, even of the same State and bytes, is a different proof source.
    pub(crate) fn in_view(&self, view: &StateView<'_>) -> Result<&CertifiedBlock, Error> {
        // Erase only the type's lifetime parameter for address comparison. Both original
        // references remain borrowed and live; neither address is stored or serialized.
        if core::ptr::eq(
            core::ptr::from_ref(self.view).cast::<()>(),
            core::ptr::from_ref(view).cast::<()>(),
        ) {
            Ok(&self.block)
        } else {
            Err(Error::Finality)
        }
    }
}
