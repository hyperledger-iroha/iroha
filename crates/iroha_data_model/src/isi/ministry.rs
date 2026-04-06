//! Ministry instructions.
//!
//! These instructions anchor Ministry transparency and agenda workflows in the
//! canonical ISI registry so Torii and SDKs can build signed transactions
//! without introducing ad-hoc payload formats.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::ministry::AgendaProposalV1;

/// Submit a citizen agenda proposal to the Ministry intake ledger.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SubmitAgendaProposal {
    /// Canonical agenda proposal payload.
    pub proposal: AgendaProposalV1,
}

impl crate::seal::Instruction for SubmitAgendaProposal {}
