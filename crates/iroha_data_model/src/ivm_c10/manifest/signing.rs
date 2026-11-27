//! Canonical C10 manifest signing and projection from the verified compiler interface.

use super::*;
use crate::events::{
    EventFilterBox,
    pipeline::{PipelineEventFilterBox, TransactionStatus},
};
use crate::ivm_c10::{
    PipelineEventFilterBoxC10V1, TransactionEventFilterC10V1, TransactionStatusC10V1,
};
use crate::smart_contract::manifest::{
    BorrowedManifestValue, ContractManifest, EntrypointDescriptor, ManifestSigningError,
    TriggerDescriptor,
};
use norito::core::{BoundedEncodeError, DecodeBudgetContext};

/// The canonical C10 signing frame borrows every manifest field except the signature itself.
/// Its nominal identity separates C10 provenance from baseline signatures.
#[derive(norito::derive::SerializePayload, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::ContractManifestSignaturePayloadC10V1")]
pub struct ManifestSignaturePayloadC10V1<'a> {
    seiyaku_name: BorrowedManifestValue<'a, Option<String>>,
    code_hash: BorrowedManifestValue<'a, Option<Hash>>,
    abi_hash: BorrowedManifestValue<'a, Option<Hash>>,
    compiler_fingerprint: BorrowedManifestValue<'a, Option<String>>,
    features_bitmap: BorrowedManifestValue<'a, Option<u64>>,
    access_set_hints: BorrowedManifestValue<'a, Option<AccessSetHints>>,
    permissions: BorrowedManifestValue<'a, Vec<ContractPermissionDescriptorV1>>,
    events: BorrowedManifestValue<'a, Vec<ContractEventDescriptorV1>>,
    entrypoints: BorrowedManifestValue<'a, Option<Vec<EntrypointDescriptorC10V1>>>,
    states: BorrowedManifestValue<'a, Option<Vec<StateDescriptor>>>,
    error_types: BorrowedManifestValue<'a, Option<Vec<ContractErrorTypeDescriptor>>>,
    enum_types: BorrowedManifestValue<'a, Vec<ContractEnumTypeDescriptorV1>>,
    error_messages: BorrowedManifestValue<'a, Option<Vec<ContractErrorMessage>>>,
    kotoba: BorrowedManifestValue<'a, Option<Vec<KotobaTranslationEntry>>>,
}

impl ContractManifestC10V1 {
    /// Borrow all authenticated fields in their canonical order.
    #[must_use]
    pub fn signature_payload(&self) -> ManifestSignaturePayloadC10V1<'_> {
        let Self {
            seiyaku_name,
            code_hash,
            abi_hash,
            compiler_fingerprint,
            features_bitmap,
            access_set_hints,
            permissions,
            events,
            entrypoints,
            states,
            error_types,
            enum_types,
            error_messages,
            kotoba,
            provenance: _,
        } = self;
        ManifestSignaturePayloadC10V1 {
            seiyaku_name: BorrowedManifestValue(seiyaku_name),
            code_hash: BorrowedManifestValue(code_hash),
            abi_hash: BorrowedManifestValue(abi_hash),
            compiler_fingerprint: BorrowedManifestValue(compiler_fingerprint),
            features_bitmap: BorrowedManifestValue(features_bitmap),
            access_set_hints: BorrowedManifestValue(access_set_hints),
            permissions: BorrowedManifestValue(permissions),
            events: BorrowedManifestValue(events),
            entrypoints: BorrowedManifestValue(entrypoints),
            states: BorrowedManifestValue(states),
            error_types: BorrowedManifestValue(error_types),
            enum_types: BorrowedManifestValue(enum_types),
            error_messages: BorrowedManifestValue(error_messages),
            kotoba: BorrowedManifestValue(kotoba),
        }
    }

    /// Encode the canonical signing frame under the caller's original budget.
    ///
    /// # Errors
    /// Returns the original allocation, size or serialization refusal.
    pub fn signature_payload_bytes(
        &self,
        context: &DecodeBudgetContext,
        max_frame_bytes: usize,
    ) -> Result<Vec<u8>, BoundedEncodeError> {
        context.with(|| {
            let _canonical =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::to_bytes_bounded(&self.signature_payload(), max_frame_bytes)
        })
    }

    /// Sign the C10 payload with its own schema identity.
    ///
    /// # Errors
    /// Returns the original signer or bounded encoder error.
    pub fn try_signed(
        mut self,
        context: &DecodeBudgetContext,
        max_frame_bytes: usize,
        key_pair: &iroha_crypto::KeyPair,
    ) -> Result<Self, ManifestSigningError> {
        let bytes = self.signature_payload_bytes(context, max_frame_bytes)?;
        let signature = iroha_crypto::Signature::try_new(key_pair.private_key(), &bytes)?;
        let signer = context
            .with(|| key_pair.public_key().try_clone_for_admission())
            .map_err(BoundedEncodeError::from)?;
        self.provenance = Some(ManifestProvenance { signer, signature });
        Ok(self)
    }

    /// Whether two manifests authenticate the same content irrespective of their publisher.
    #[must_use]
    pub fn same_signed_content(&self, other: &Self) -> bool {
        let Self {
            seiyaku_name,
            code_hash,
            abi_hash,
            compiler_fingerprint,
            features_bitmap,
            access_set_hints,
            permissions,
            events,
            entrypoints,
            states,
            error_types,
            enum_types,
            error_messages,
            kotoba,
            provenance: _,
        } = self;
        seiyaku_name == &other.seiyaku_name
            && code_hash == &other.code_hash
            && abi_hash == &other.abi_hash
            && compiler_fingerprint == &other.compiler_fingerprint
            && features_bitmap == &other.features_bitmap
            && access_set_hints == &other.access_set_hints
            && permissions == &other.permissions
            && events == &other.events
            && entrypoints == &other.entrypoints
            && states == &other.states
            && error_types == &other.error_types
            && enum_types == &other.enum_types
            && error_messages == &other.error_messages
            && kotoba == &other.kotoba
    }
}

impl TryFrom<ContractManifest> for ContractManifestC10V1 {
    type Error = &'static str;

    /// Project a verified, unsigned compiler manifest into its C10 registration family.
    /// A baseline signature cannot authorize the new frame.
    fn try_from(manifest: ContractManifest) -> Result<Self, Self::Error> {
        let ContractManifest {
            seiyaku_name,
            code_hash,
            abi_hash,
            compiler_fingerprint,
            features_bitmap,
            access_set_hints,
            permissions,
            events,
            entrypoints,
            states,
            error_types,
            enum_types,
            error_messages,
            kotoba,
            provenance,
        } = manifest;
        if provenance.is_some() {
            return Err("C10 registration requires signing the C10 manifest frame");
        }
        let entrypoints = entrypoints
            .map(|entries| {
                entries
                    .into_iter()
                    .map(EntrypointDescriptorC10V1::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(Self {
            seiyaku_name,
            code_hash,
            abi_hash,
            compiler_fingerprint,
            features_bitmap,
            access_set_hints,
            permissions,
            events,
            entrypoints,
            states,
            error_types,
            enum_types,
            error_messages,
            kotoba,
            provenance: None,
        })
    }
}

impl TryFrom<EntrypointDescriptor> for EntrypointDescriptorC10V1 {
    type Error = &'static str;
    fn try_from(entry: EntrypointDescriptor) -> Result<Self, Self::Error> {
        let EntrypointDescriptor {
            name,
            kind,
            params,
            argument_schema,
            return_type,
            return_schema,
            authorization,
            read_keys,
            write_keys,
            access_hints_complete,
            access_hints_skipped,
            triggers,
        } = entry;
        let triggers = triggers
            .into_iter()
            .map(TriggerDescriptorC10V1::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            name,
            kind,
            params,
            argument_schema,
            return_type,
            return_schema,
            authorization,
            read_keys,
            write_keys,
            access_hints_complete,
            access_hints_skipped,
            triggers,
        })
    }
}

impl TryFrom<TriggerDescriptor> for TriggerDescriptorC10V1 {
    type Error = &'static str;
    fn try_from(trigger: TriggerDescriptor) -> Result<Self, Self::Error> {
        let TriggerDescriptor {
            id,
            repeats,
            filter,
            authority,
            metadata,
            callback,
        } = trigger;
        let filter = match filter {
            EventFilterBox::Data(f) => EventFilterBoxC10V1::Data(f),
            EventFilterBox::Time(f) => EventFilterBoxC10V1::Time(f),
            EventFilterBox::TriggerCompleted(f) => EventFilterBoxC10V1::TriggerCompleted(f),
            EventFilterBox::ExecuteTrigger(f) => EventFilterBoxC10V1::ExecuteTrigger(f),
            EventFilterBox::Pipeline(f) => EventFilterBoxC10V1::Pipeline(match f {
                PipelineEventFilterBox::Block(f) => PipelineEventFilterBoxC10V1::Block(f),
                PipelineEventFilterBox::Witness(f) => PipelineEventFilterBoxC10V1::Witness(f),
                PipelineEventFilterBox::Transaction(f) => {
                    let status = match f.status {
                        None => None,
                        Some(TransactionStatus::Queued) => Some(TransactionStatusC10V1::Queued),
                        Some(TransactionStatus::Expired) => Some(TransactionStatusC10V1::Expired),
                        Some(TransactionStatus::Approved) => Some(TransactionStatusC10V1::Approved),
                        // An exact rejection is owned by its execution profile. The compiler currently
                        // emits only common statuses; never reinterpret a baseline rejection's tags.
                        Some(TransactionStatus::Rejected(_)) => {
                            return Err(
                                "baseline rejection cannot select an exact C10 transaction rejection",
                            );
                        }
                    };
                    PipelineEventFilterBoxC10V1::Transaction(TransactionEventFilterC10V1 {
                        hash: f.hash,
                        block_height: f.block_height,
                        lane_id: f.lane_id,
                        dataspace_id: f.dataspace_id,
                        status,
                    })
                }
            }),
        };
        Ok(Self {
            id,
            repeats,
            filter,
            authority,
            metadata,
            callback,
        })
    }
}
