//! Independent immutable provider ancestry composed into one four-peer launch revision.

use super::{
    MAX_CUSTODY_BYTES, MAX_MANIFEST_BYTES, RetainedCustodyEnrollment, Retention,
    carriers::RequiredTransaction, custody_name, encode, invalid, read_optional,
    retain_receipt_custody, retain_revision_file,
};
use crate::managed::{
    ManagedTransactionFinality, Result, native_operation::require_retained_material,
};
use iroha_crypto::Hash;
use iroha_data_model::{NetworkId, sorafs::capacity::ProviderId};
use iroha_fs::PrivateDirectory;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::generated_service_runtime::ComponentIdentity")]
pub(super) struct ComponentIdentity {
    pub(super) network: NetworkId,
    pub(super) genesis: [u8; 32],
    pub(super) profile: [u8; 32],
    pub(super) policies: [u8; 32],
    pub(super) compliance: [u8; 32],
    pub(super) provider: ProviderId,
    pub(super) slot: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::generated_service_runtime::EnrollmentSelection")]
pub(super) struct EnrollmentSelection {
    pub(super) sequence: u64,
    pub(super) record_digest: [u8; 32],
    pub(super) bytes_digest: [u8; 32],
    pub(super) issued_at_unix_ms: u64,
    pub(super) expires_at_unix_ms: u64,
    transaction: RequiredTransaction,
}

impl EnrollmentSelection {
    fn from_retained(enrollment: &RetainedCustodyEnrollment) -> Self {
        Self {
            sequence: enrollment.statement().sequence,
            record_digest: enrollment.record_digest(),
            bytes_digest: *Hash::new(enrollment.bytes()).as_ref(),
            issued_at_unix_ms: enrollment.statement().issued_at_unix_ms,
            expires_at_unix_ms: enrollment.statement().expires_at_unix_ms,
            transaction: RequiredTransaction::from_original(enrollment.finalized()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::generated_service_runtime::ProviderComponentIntent")]
struct ComponentIntent {
    identity: ComponentIdentity,
    selected: EnrollmentSelection,
    previous: Option<[u8; 32]>,
}

/// The sole renderer constructs this after recovering an opaque native custody original.
/// Persisted component bytes alone never construct a selected enrollment.
pub(super) struct ProviderComponent {
    intent: ComponentIntent,
    digest: [u8; 32],
    enrollment: RetainedCustodyEnrollment,
    directory: PrivateDirectory,
}

/// A derived local publication plan; it cannot construct or authorize native enrollment.
/// Preparing all three plans lets recovery check the exact aggregate before retaining any body.
pub(super) struct PreparedProviderComponent {
    intent: ComponentIntent,
    digest: [u8; 32],
    enrollment: RetainedCustodyEnrollment,
}

impl ProviderComponent {
    pub(super) fn retain(
        runtime: &PrivateDirectory,
        identity: ComponentIdentity,
        enrollment: RetainedCustodyEnrollment,
        previous: Option<&ProviderComponent>,
        retention: Retention,
    ) -> Result<Arc<Self>> {
        Self::prepare(identity, enrollment, previous)?.retain(runtime, retention)
    }

    pub(super) fn prepare(
        identity: ComponentIdentity,
        enrollment: RetainedCustodyEnrollment,
        previous: Option<&ProviderComponent>,
    ) -> Result<PreparedProviderComponent> {
        let selected = EnrollmentSelection::from_retained(&enrollment);
        if identity.slot >= 3
            || identity.genesis == [0; 32]
            || identity.profile == [0; 32]
            || identity.policies == [0; 32]
            || identity.compliance == [0; 32]
            || !(1..=64).contains(&selected.sequence)
            || selected.issued_at_unix_ms >= selected.expires_at_unix_ms
            || selected.expires_at_unix_ms == u64::MAX
            || enrollment.bytes().is_empty()
            || enrollment.bytes().len() > MAX_CUSTODY_BYTES
            || enrollment.statement().binding.network_id != *identity.network.as_bytes()
            || enrollment.statement().binding.purpose
                != (sorafs_manifest::signer::protocol::SignerPurposeBindingV1::StreamToken {
                    provider_id: *identity.provider.as_bytes(),
                })
        {
            return Err(invalid("generated provider component selection is invalid"));
        }
        selected.transaction.validate()?;
        match previous {
            Some(previous) => {
                previous.validate()?;
                require_successor(previous, identity, &enrollment)?;
            }
            None if selected.sequence == 1 => {}
            None => return Err(invalid("original provider component ancestry is absent")),
        }
        let intent = ComponentIntent {
            identity,
            selected,
            previous: previous.map(|value| value.digest),
        };
        let bytes = encode(&intent, MAX_MANIFEST_BYTES)?;
        let digest = *Hash::new(&bytes).as_ref();
        Ok(PreparedProviderComponent {
            intent,
            digest,
            enrollment,
        })
    }

    pub(super) fn validate(&self) -> Result<()> {
        let expected = encode(&self.intent, MAX_MANIFEST_BYTES)?;
        if self
            .directory
            .read(component_name(self.digest), MAX_MANIFEST_BYTES)?
            .as_slice()
            != expected.as_slice()
            || *Hash::new(&expected).as_ref() != self.digest
            || EnrollmentSelection::from_retained(&self.enrollment) != self.intent.selected
            || self
                .directory
                .read(
                    custody_name(self.intent.selected.bytes_digest),
                    MAX_CUSTODY_BYTES,
                )?
                .as_slice()
                != self.enrollment.bytes()
        {
            return Err(crate::managed::ManagedBootstrapFailure::RetainedMaterial.into());
        }
        retain_receipt_custody(&self.directory, true)?;
        self.directory.revalidate()?;
        Ok(())
    }

    pub(super) fn identity(&self) -> ComponentIdentity {
        self.intent.identity
    }

    pub(super) fn selection(&self) -> EnrollmentSelection {
        self.intent.selected
    }

    pub(super) fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub(super) fn enrollment(&self) -> &RetainedCustodyEnrollment {
        &self.enrollment
    }

    pub(super) fn finalized(&self) -> ManagedTransactionFinality {
        *self.enrollment.finalized()
    }

    pub(super) fn directory(&self) -> &PrivateDirectory {
        &self.directory
    }
}

impl PreparedProviderComponent {
    pub(super) fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub(super) fn finalized(&self) -> ManagedTransactionFinality {
        *self.enrollment.finalized()
    }

    pub(super) fn retain(
        self,
        runtime: &PrivateDirectory,
        retention: Retention,
    ) -> Result<Arc<ProviderComponent>> {
        let Self {
            intent,
            digest,
            enrollment,
        } = self;
        let identity = intent.identity;
        let selected = intent.selected;
        let bytes = encode(&intent, MAX_MANIFEST_BYTES)?;
        let name = component_name(digest);
        // Ancestors and renewals already transferred receipt custody to a real runtime. Missing
        // provider roots or receipts must fail before any attempted replacement publication.
        let existing = retention == Retention::ExistingMaterial || selected.sequence > 1;
        let providers = if existing {
            require_retained_material(runtime.open_child("providers").map_err(Into::into))?
        } else {
            runtime.ensure_child("providers")?
        };
        let directory = if existing {
            require_retained_material(
                providers
                    .open_child(identity.slot.to_string())
                    .map_err(Into::into),
            )?
        } else {
            providers.ensure_child(identity.slot.to_string())?
        };
        let committed = read_optional(&directory, &name, MAX_MANIFEST_BYTES)?;
        if committed.as_ref().is_some_and(|value| value != &bytes) {
            return Err(invalid("retained generated provider component changed"));
        }
        // Once the component is committed, its body is original custody material. Recovery
        // may finish an unpublished component, but must never repair a committed one.
        let retention = if committed.is_some() {
            Retention::ExistingMaterial
        } else {
            retention
        };
        let receipt_owned = existing || committed.is_some();
        if receipt_owned {
            require_retained_material(retain_receipt_custody(&directory, true))?;
        }
        let result = (|| {
            retain_revision_file(
                &directory,
                &custody_name(selected.bytes_digest),
                enrollment.bytes(),
                MAX_CUSTODY_BYTES,
                retention,
            )?;
            retain_receipt_custody(&directory, receipt_owned)?;
            retain_revision_file(&directory, &name, &bytes, MAX_MANIFEST_BYTES, retention)?;
            let value = Arc::new(ProviderComponent {
                intent,
                digest,
                enrollment,
                directory,
            });
            value.validate()?;
            Ok(value)
        })();
        if retention == Retention::ExistingMaterial {
            require_retained_material(result)
        } else {
            result
        }
    }
}

fn require_successor(
    previous: &ProviderComponent,
    identity: ComponentIdentity,
    enrollment: &RetainedCustodyEnrollment,
) -> Result<()> {
    let old = previous.selection();
    let next = EnrollmentSelection::from_retained(enrollment);
    if identity != previous.identity()
        || old.sequence.checked_add(1) != Some(next.sequence)
        || enrollment.statement().predecessor_digest != old.record_digest
        || enrollment.finalized().height <= previous.finalized().height
        || next.expires_at_unix_ms <= old.expires_at_unix_ms
    {
        return Err(invalid(
            "provider component does not advance its exact original",
        ));
    }
    Ok(())
}

fn component_name(digest: [u8; 32]) -> String {
    format!("component-{}.nrt", hex::encode(digest))
}
