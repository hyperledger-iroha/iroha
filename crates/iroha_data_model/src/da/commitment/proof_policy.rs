//! Immutable canonical DA policies and exact original decoded allocation custody.
//!
//! The sole wire record remains version, policy hash and ordered policies. Storage
//! state is never serialized. Prepared decoding retains the actual policy array,
//! every UTF-8 alias and their ledger through the last block reader. Transport
//! construction does not grant custody, DA authorization or finality.

use super::DaProofPolicy;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_allocation::{AllocationBudget, ChargedShared, RetainedPayload};
use iroha_crypto::Hash;
use iroha_schema::{Declaration, IntoSchema, MetaMap, Metadata, NamedFieldsMeta, TypeId};
use norito::{
    codec::{Decode, Encode},
    core as ncore,
};
use std::{alloc::Layout, cmp::Ordering, fmt};

/// Versioned immutable proof policies for every configured lane.
///
/// Ordinary construction and decoding are untrusted transport. Prepared decoding
/// shares only the same original charged arrays and UTF-8 leaves; callers cannot
/// extract or grow an admitted allocation independently of its retained ledger.
pub struct DaProofPolicyBundle {
    storage: Storage,
}
#[expect(
    variant_size_differences,
    reason = "untrusted transport is bounded inline; admitted custody retains its original prepaid shared owner without a second heap allocation"
)]
enum Storage {
    Untrusted(CanonicalParts),
    Admitted(ChargedShared<RetainedPayload<CanonicalParts>>),
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields, decode_fields)]
struct CanonicalParts {
    version: u16,
    policy_hash: Hash,
    policies: Vec<DaProofPolicy>,
}
mod prepared;
pub use prepared::{DaProofPolicyCustodyError, PreparedDaProofPolicyBundle};

impl DaProofPolicyBundle {
    /// Initial and sole first-release proof-policy layout.
    pub const VERSION_V1: u16 = 1;
    /// Construct current untrusted transport with its exact ordered policy hash.
    #[must_use]
    pub fn new(policies: Vec<DaProofPolicy>) -> Self {
        let policy_hash = super::hash_policies(&policies);
        Self::from_untrusted_parts(Self::VERSION_V1, policy_hash, policies)
    }
    /// Construct exact untrusted fields without correcting their claims.
    /// Semantic validation must verify version, ordering and the policy hash.
    #[must_use]
    pub fn from_untrusted_parts(
        version: u16,
        policy_hash: Hash,
        policies: Vec<DaProofPolicy>,
    ) -> Self {
        Self {
            storage: Storage::Untrusted(CanonicalParts {
                version,
                policy_hash,
                policies,
            }),
        }
    }
    fn parts(&self) -> &CanonicalParts {
        match &self.storage {
            Storage::Untrusted(parts) => parts,
            Storage::Admitted(parts) => parts.get(),
        }
    }
    /// Exact advertised bundle version; transport decoding does not validate it.
    #[must_use]
    pub fn version(&self) -> u16 {
        self.parts().version
    }
    /// Exact committed hash of the original ordered policies.
    #[must_use]
    pub fn policy_hash(&self) -> Hash {
        self.parts().policy_hash
    }
    /// Borrow every original policy and alias without cloning or mutable escape.
    /// An explicit clone of a borrowed policy allocates a separate untrusted graph;
    /// this owner never funds that clone or scratch used for semantic validation.
    #[must_use]
    pub fn policies(&self) -> &[DaProofPolicy] {
        &self.parts().policies
    }
    /// Whether the control, ledger and all actual children retain this original pool.
    /// This is allocation custody only, never DA or consensus authorization.
    #[must_use]
    pub fn admitted_to(&self, budget: &AllocationBudget) -> bool {
        match &self.storage {
            Storage::Untrusted(_) => false,
            Storage::Admitted(owner) => {
                owner.belongs_to(budget) && RetainedPayload::belongs_to(owner, budget)
            }
        }
    }
    /// Whether two admitted readers keep the identical original immutable owner.
    #[must_use]
    pub fn ptr_eq(left: &Self, right: &Self) -> bool {
        match (&left.storage, &right.storage) {
            (Storage::Admitted(left), Storage::Admitted(right)) => {
                ChargedShared::ptr_eq(left, right)
            }
            _ => false,
        }
    }
    /// Exact physical immutable control, separate from its array, leaves and ledger.
    #[must_use]
    pub fn allocation_layout() -> Layout {
        ChargedShared::<RetainedPayload<CanonicalParts>>::allocation_layout()
    }
}
impl Clone for DaProofPolicyBundle {
    fn clone(&self) -> Self {
        Self {
            storage: match &self.storage {
                Storage::Untrusted(parts) => Storage::Untrusted(parts.clone()),
                Storage::Admitted(parts) => Storage::Admitted(parts.clone()),
            },
        }
    }
}
impl fmt::Debug for DaProofPolicyBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaProofPolicyBundle")
            .field("version", &self.version())
            .field("policy_hash", &self.policy_hash())
            .field("policies", &self.policies())
            .finish()
    }
}
impl PartialEq for DaProofPolicyBundle {
    fn eq(&self, other: &Self) -> bool {
        self.parts() == other.parts()
    }
}
impl Eq for DaProofPolicyBundle {}
impl PartialOrd for DaProofPolicyBundle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DaProofPolicyBundle {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parts().cmp(other.parts())
    }
}
impl ncore::SerializePayload for DaProofPolicyBundle {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::SerializePayload::serialize(self.parts(), encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        ncore::SerializePayload::encoded_len_hint(self.parts())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        ncore::SerializePayload::encoded_len_exact(self.parts())
    }
}
impl<'de> ncore::DeserializePayload<'de> for DaProofPolicyBundle {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical DA policy archive")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let parts =
            <CanonicalParts as ncore::DeserializePayload>::try_deserialize(archived.cast())?;
        Ok(Self {
            storage: Storage::Untrusted(parts),
        })
    }
}
impl<D> ncore::DecodeRecordFields<D> for DaProofPolicyBundle
where
    D: ncore::FieldDestination
        + ncore::DecodeField<0, u16>
        + ncore::DecodeField<1, Hash>
        + ncore::DecodeField<2, Vec<DaProofPolicy>>,
{
    type Values = (
        <D as ncore::DecodeField<0, u16>>::Value,
        <D as ncore::DecodeField<1, Hash>>::Value,
        <D as ncore::DecodeField<2, Vec<DaProofPolicy>>>::Value,
    );
    fn decode_fields(
        bytes: &[u8],
        destination: &mut D,
    ) -> Result<(Self::Values, usize), ncore::DecodeIntoError<D::Error>> {
        <CanonicalParts as ncore::DecodeRecordFields<D>>::decode_fields(bytes, destination)
    }
}
impl norito::NoritoSchema for DaProofPolicyBundle {
    fn nominal_name() -> String {
        Self::static_frame_name()
            .expect("fixed DA policy identity")
            .to_owned()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_data_model::da::commitment::DaProofPolicyBundle")
    }
}
impl TypeId for DaProofPolicyBundle {
    fn id() -> String {
        "DaProofPolicyBundle".to_owned()
    }
}
impl IntoSchema for DaProofPolicyBundle {
    fn type_name() -> String {
        "DaProofPolicyBundle".to_owned()
    }
    fn update_schema_map(map: &mut MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "version".to_owned(),
                        ty: std::any::TypeId::of::<u16>(),
                    },
                    Declaration {
                        name: "policy_hash".to_owned(),
                        ty: std::any::TypeId::of::<Hash>(),
                    },
                    Declaration {
                        name: "policies".to_owned(),
                        ty: std::any::TypeId::of::<Vec<DaProofPolicy>>(),
                    },
                ],
            }));
            u16::update_schema_map(map);
            Hash::update_schema_map(map);
            Vec::<DaProofPolicy>::update_schema_map(map);
        }
    }
}
impl norito::json::FastJsonWrite for DaProofPolicyBundle {
    fn json_object_field_order() -> Option<&'static [&'static str]> {
        Some(&["version", "policy_hash", "policies"])
    }
    fn write_json(&self, out: &mut String) {
        norito::json::FastJsonWrite::write_json(self.parts(), out)
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::FastJsonWrite::write_json_to(self.parts(), out)
    }
}
impl norito::json::JsonDeserialize for DaProofPolicyBundle {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        Ok(Self {
            storage: Storage::Untrusted(
                <CanonicalParts as norito::json::JsonDeserialize>::json_deserialize(parser)?,
            ),
        })
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        Ok(Self {
            storage: Storage::Untrusted(
                <CanonicalParts as norito::json::JsonDeserialize>::json_from_value(value)?,
            ),
        })
    }
}
