//! Closed value-format selection for the unadmitted compact public relation.
//!
//! Only the validated table's Rust value type selects a format. Proof bytes,
//! caller metadata and parameter strings cannot select or override this domain.
//! The full-domain format binds its nominal quantity frame and scale in a new
//! context while retaining the existing bounded SMT relation geometry.

use iroha_data_model::fastpq::FastpqQuantityUnits;

use crate::{
    ProofSemantics, PublicInputs, Result, StateTransition,
    gadgets::public_transfer_statement::{
        PreparedPublicTransfers, PublicTransferLimits, PublicTransferTranscript,
        prepare_public_transfers, prepare_quantity_public_transfers,
    },
};

mod sealed {
    pub trait Sealed {}
    impl Sealed for u64 {}
    impl Sealed for iroha_data_model::fastpq::FastpqQuantityUnits {}
}

/// A closed public value domain, with separately bound route identities.
pub(super) trait CompactTransferValue: sealed::Sealed + Sized {
    /// Whether the distinct full-domain quantity context is mandatory.
    const QUANTITY_CONTEXT: bool;
    /// Fixed single-transfer relation identity.
    const TRANSFER_IDENTITY: &'static str;
    /// Fixed single AXT relation identity.
    const AXT_IDENTITY: &'static str;
    /// Fixed ordinary bundle segment identity.
    const BATCH_IDENTITY: &'static str;
    /// Fixed AXT bundle segment identity.
    const AXT_BATCH_IDENTITY: &'static str;

    /// Validate complete public facts using this type's fixed value decoder.
    fn prepare<'a>(
        rows: &'a [StateTransition],
        claims: &'a [PublicTransferTranscript],
        inputs: PublicInputs,
        semantics: ProofSemantics,
        limits: PublicTransferLimits,
    ) -> Result<PreparedPublicTransfers<'a, Self>>;
}

impl CompactTransferValue for u64 {
    const QUANTITY_CONTEXT: bool = false;
    const TRANSFER_IDENTITY: &'static str =
        "fastpq:prototype:public-transfer:v1:342cols:923slots:65536rows";
    const AXT_IDENTITY: &'static str =
        "fastpq:prototype:axt-public-transfer:v1:342cols:923slots:65536rows";
    const BATCH_IDENTITY: &'static str =
        "fastpq:prototype:ordinary-transfer-bundle-segment:v1:342cols:923slots:65536rows";
    const AXT_BATCH_IDENTITY: &'static str =
        "fastpq:prototype:axt-transfer-bundle-segment:v1:342cols:923slots:65536rows";

    fn prepare<'a>(
        rows: &'a [StateTransition],
        claims: &'a [PublicTransferTranscript],
        inputs: PublicInputs,
        semantics: ProofSemantics,
        limits: PublicTransferLimits,
    ) -> Result<PreparedPublicTransfers<'a, Self>> {
        prepare_public_transfers(rows, claims, inputs, semantics, limits)
    }
}

impl CompactTransferValue for FastpqQuantityUnits {
    const QUANTITY_CONTEXT: bool = true;
    const TRANSFER_IDENTITY: &'static str =
        "fastpq:prototype:quantity-public-transfer:v1:342cols:923slots:65536rows";
    const AXT_IDENTITY: &'static str =
        "fastpq:prototype:quantity-axt-public-transfer:v1:342cols:923slots:65536rows";
    const BATCH_IDENTITY: &'static str =
        "fastpq:prototype:quantity-ordinary-transfer-bundle-segment:v1:342cols:923slots:65536rows";
    const AXT_BATCH_IDENTITY: &'static str =
        "fastpq:prototype:quantity-axt-transfer-bundle-segment:v1:342cols:923slots:65536rows";

    fn prepare<'a>(
        rows: &'a [StateTransition],
        claims: &'a [PublicTransferTranscript],
        inputs: PublicInputs,
        semantics: ProofSemantics,
        limits: PublicTransferLimits,
    ) -> Result<PreparedPublicTransfers<'a, Self>> {
        prepare_quantity_public_transfers(rows, claims, inputs, semantics, limits)
    }
}
