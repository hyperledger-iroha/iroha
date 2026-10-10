//! Immutable committed event collection retaining its original allocation custody.
//!
//! Storage has no wire discriminator: both forms use the canonical event Vec codec.
//! Ordinary decoding is untrusted transport; admitted clones share the original graph.

use std::{alloc::Layout, cmp::Ordering, fmt};

use iroha_allocation::{
    AllocationBudget, AllocationCharge, ChargedShared, RetainedPayload, SharedFromChargeError,
};
use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, VecMeta};
use norito::core as ncore;

use super::ContractEmissionV1;

type Payload = RetainedPayload<Vec<ContractEmissionV1>>;

/// Immutable ordered committed emissions and their optional original physical custody.
pub struct ContractEmissionsV1 {
    storage: Storage,
}

enum Storage {
    Untrusted(Vec<ContractEmissionV1>),
    Admitted(ChargedShared<Payload>),
}

/// Refusal before the original event graph enters a shared carrier owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractEmissionAdmissionErrorV1 {
    /// The original payload or control charge belongs to a different finite pool.
    ForeignPool,
    /// The exact original control charge cannot initialize this shared shell.
    Shared(SharedFromChargeError),
}

impl fmt::Display for ContractEmissionAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignPool => {
                formatter.write_str("event payload and control must retain the original pool")
            }
            Self::Shared(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ContractEmissionAdmissionErrorV1 {}

impl ContractEmissionsV1 {
    /// Construct untrusted transport without granting execution or allocation authority.
    #[must_use]
    pub fn from_untrusted(values: Vec<ContractEmissionV1>) -> Self {
        Self {
            storage: Storage::Untrusted(values),
        }
    }

    /// Exact shared shell layout, separate from original payload and ledger allocations.
    #[must_use]
    pub fn allocation_layout() -> Layout {
        ChargedShared::<Payload>::allocation_layout()
    }

    /// Move the original funded journal into the carrier without copying any payload.
    ///
    /// The caller has already retained every actual payload allocation in `payload`.
    /// No capacity is acquired, refunded or replaced by this transition.
    ///
    /// # Errors
    /// Returns both original inputs unchanged on foreign pool, wrong layout or allocator refusal.
    pub fn try_admit(
        payload: Payload,
        control_charge: AllocationCharge,
        budget: &AllocationBudget,
    ) -> Result<Self, (Payload, AllocationCharge, ContractEmissionAdmissionErrorV1)> {
        if !payload.belongs_to(budget) || !control_charge.belongs_to(budget) {
            return Err((
                payload,
                control_charge,
                ContractEmissionAdmissionErrorV1::ForeignPool,
            ));
        }
        let shell = match ChargedShared::<Payload>::reserve_from_charge(control_charge) {
            Ok(shell) => shell,
            Err((charge, error)) => {
                return Err((
                    payload,
                    charge,
                    ContractEmissionAdmissionErrorV1::Shared(error),
                ));
            }
        };
        Ok(Self {
            storage: Storage::Admitted(shell.initialize(payload)),
        })
    }

    /// Borrow the original ordered emissions without mutable or owned extraction.
    #[must_use]
    pub fn as_slice(&self) -> &[ContractEmissionV1] {
        self.values().as_slice()
    }
    fn values(&self) -> &Vec<ContractEmissionV1> {
        match &self.storage {
            Storage::Untrusted(values) => values,
            Storage::Admitted(owner) => owner.get(),
        }
    }
    /// Iterate the exact original records in execution order.
    pub fn iter(&self) -> std::slice::Iter<'_, ContractEmissionV1> {
        self.as_slice().iter()
    }
    /// Number of committed emissions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }
    /// Whether this invocation emitted no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
    /// Whether all original payload, ledger and shared-control charges retain this pool.
    #[must_use]
    pub fn admitted_to(&self, budget: &AllocationBudget) -> bool {
        match &self.storage {
            Storage::Untrusted(_) => false,
            Storage::Admitted(owner) => {
                owner.belongs_to(budget) && RetainedPayload::belongs_to(owner, budget)
            }
        }
    }
    /// Whether two admitted collections share the same original physical graph.
    #[must_use]
    pub fn ptr_eq(left: &Self, right: &Self) -> bool {
        match (&left.storage, &right.storage) {
            (Storage::Admitted(left), Storage::Admitted(right)) => {
                ChargedShared::ptr_eq(left, right)
            }
            _ => false,
        }
    }
}
impl Default for ContractEmissionsV1 {
    fn default() -> Self {
        Self::from_untrusted(Vec::new())
    }
}
impl Clone for ContractEmissionsV1 {
    fn clone(&self) -> Self {
        Self {
            storage: match &self.storage {
                Storage::Untrusted(values) => Storage::Untrusted(values.clone()),
                Storage::Admitted(owner) => Storage::Admitted(owner.clone()),
            },
        }
    }
}
impl fmt::Debug for ContractEmissionsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(formatter)
    }
}
impl PartialEq for ContractEmissionsV1 {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for ContractEmissionsV1 {}
impl PartialOrd for ContractEmissionsV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ContractEmissionsV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}
impl ncore::SerializePayload for ContractEmissionsV1 {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), norito::Error> {
        ncore::SerializePayload::serialize(self.values(), encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        ncore::SerializePayload::encoded_len_hint(self.values())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        ncore::SerializePayload::encoded_len_exact(self.values())
    }
}
impl<'de> ncore::DeserializePayload<'de> for ContractEmissionsV1 {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical contract emission collection")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, norito::Error> {
        <Vec<ContractEmissionV1> as ncore::DeserializePayload>::try_deserialize(archived.cast())
            .map(Self::from_untrusted)
    }
}
impl<'de> ncore::DecodeFromSlice<'de> for ContractEmissionsV1 {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), norito::Error> {
        let (values, used) = ncore::decode_vec_from_slice_serial::<ContractEmissionV1>(bytes)?;
        Ok((Self::from_untrusted(values), used))
    }
}
impl norito::NoritoSchema for ContractEmissionsV1 {
    fn nominal_name() -> String {
        "iroha_data_model::smart_contract::event::ContractEmissionsV1".into()
    }
    fn frame_name() -> String {
        <Vec<ContractEmissionV1> as norito::NoritoSchema>::frame_name()
    }
}
impl TypeId for ContractEmissionsV1 {
    fn id() -> String {
        "ContractEmissionsV1".into()
    }
}
impl IntoSchema for ContractEmissionsV1 {
    fn type_name() -> String {
        "ContractEmissionsV1".into()
    }
    fn update_schema_map(map: &mut MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(Metadata::Vec(VecMeta {
                ty: std::any::TypeId::of::<ContractEmissionV1>(),
            }));
            ContractEmissionV1::update_schema_map(map);
        }
    }
}
impl norito::json::JsonSerialize for ContractEmissionsV1 {
    fn json_serialize(&self, output: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.values(), output);
    }
    fn json_serialize_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::JsonSerialize::json_serialize_to(self.values(), output)
    }
}
impl norito::json::JsonDeserialize for ContractEmissionsV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        <Vec<ContractEmissionV1> as norito::json::JsonDeserialize>::json_deserialize(parser)
            .map(Self::from_untrusted)
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        <Vec<ContractEmissionV1> as norito::json::JsonDeserialize>::json_from_value(value)
            .map(Self::from_untrusted)
    }
}
impl<'a> norito::json::FastFromJson<'a> for ContractEmissionsV1 {
    fn parse(
        walker: &mut norito::json::TapeWalker<'a>,
        _arena: &mut norito::json::Arena,
    ) -> Result<Self, norito::Error> {
        walker.ensure_document_depth()?;
        let input = walker.input();
        let mut parser = norito::json::Parser::new_at(input, walker.raw_pos());
        let value = <Self as norito::json::JsonDeserialize>::json_deserialize(&mut parser)
            .map_err(norito::Error::from)?;
        walker.sync_to_raw(parser.position());
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_allocation::ChargedBuffer;

    #[allow(unsafe_code)]
    fn funded_empty_with_backing(budget: &AllocationBudget) -> Payload {
        let values = ChargedBuffer::<ContractEmissionV1>::new(1, budget).unwrap();
        let mut ledger = ChargedBuffer::new(1, budget).unwrap();
        // SAFETY: the original empty Vec still owns its exact capacity allocation.
        // Its sole charge follows that same allocation into the retained payload.
        let (values, charge) = unsafe { values.into_allocation_parts() };
        ledger.push_reserved(charge);
        match unsafe { RetainedPayload::try_new(values, ledger, budget) } {
            Ok(payload) => payload,
            Err(_) => panic!("original funded payload belongs to its pool"),
        }
    }
    fn control(budget: &AllocationBudget, layout: Layout) -> AllocationCharge {
        let mut reservation = budget.try_reserve(layout).unwrap();
        reservation.try_split(layout).unwrap()
    }
    fn sample() -> ContractEmissionV1 {
        use crate::smart_contract::{ContractAddress, entrypoint::*};
        use iroha_crypto::{Hash, HashOf, KeyPair};
        let caller =
            crate::account::AccountId::new(KeyPair::try_random().unwrap().public_key().clone());
        let network = crate::id::NetworkId::from_genesis_hash(
            HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::new(
                b"event custody test network",
            )),
        );
        let contract = ContractAddress::derive(
            &network,
            &caller,
            1,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        )
        .unwrap();
        let payload_type = EntrypointValueTypeV1 {
            nodes: vec![EntrypointValueTypeNodeV1::Struct(
                EntrypointStructTypeNodeV1 {
                    name: "Fixture::Accepted".into(),
                    fields: vec![],
                },
            )],
        };
        ContractEmissionV1 {
            contract,
            caller,
            code_hash: Hash::new(b"event custody code"),
            entrypoint: 0,
            event: 0,
            payload: EntrypointReturnRecordV1 {
                schema_hash: entrypoint_return_schema_hash_v1(&norito::codec::Encode::encode(
                    &payload_type,
                )),
                atoms: vec![],
            },
            definition: super::super::ContractEventDescriptorV1 {
                name: "Accepted".parse().unwrap(),
                payload_type,
            },
        }
    }

    #[test]
    fn admitted_clones_keep_original_graph_and_credit_through_last_reader() {
        let budget = AllocationBudget::new(64 * 1024);
        let payload = funded_empty_with_backing(&budget);
        let pointer = payload.get().as_ptr();
        let charge = control(&budget, ContractEmissionsV1::allocation_layout());
        let before = budget.reserved_bytes();
        let admitted = match ContractEmissionsV1::try_admit(payload, charge, &budget) {
            Ok(value) => value,
            Err(_) => panic!("original event source must admit"),
        };
        assert!(admitted.admitted_to(&budget));
        let reader = admitted.clone();
        assert!(ContractEmissionsV1::ptr_eq(&admitted, &reader));
        assert_eq!(reader.as_slice().as_ptr(), pointer);
        assert_eq!(budget.reserved_bytes(), before);
        assert!(reader.is_empty());
        assert_eq!(reader.len(), 0);
        assert_eq!(reader.iter().count(), 0);
        drop(admitted);
        assert_eq!(budget.reserved_bytes(), before);
        drop(reader);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn rejected_admission_returns_unchanged_original_owners_without_refund() {
        let original = AllocationBudget::new(64 * 1024);
        let foreign = AllocationBudget::new(64 * 1024);
        let payload = funded_empty_with_backing(&original);
        let pointer = payload.get().as_ptr();
        let charge = control(&original, ContractEmissionsV1::allocation_layout());
        let before = original.reserved_bytes();
        let (payload, charge, error) =
            match ContractEmissionsV1::try_admit(payload, charge, &foreign) {
                Err(inputs) => inputs,
                Ok(_) => panic!("foreign pool must reject"),
            };
        assert_eq!(error, ContractEmissionAdmissionErrorV1::ForeignPool);
        assert_eq!(payload.get().as_ptr(), pointer);
        assert_eq!(original.reserved_bytes(), before);
        let admitted = match ContractEmissionsV1::try_admit(payload, charge, &original) {
            Ok(value) => value,
            Err(_) => panic!("retry preserves original source"),
        };
        drop(admitted);
        assert_eq!(original.reserved_bytes(), 0);

        let payload = funded_empty_with_backing(&original);
        let pointer = payload.get().as_ptr();
        let charge = control(&original, Layout::new::<u8>());
        let before = original.reserved_bytes();
        let (payload, charge, error) =
            match ContractEmissionsV1::try_admit(payload, charge, &original) {
                Err(inputs) => inputs,
                Ok(_) => panic!("wrong control layout must reject"),
            };
        assert!(matches!(
            error,
            ContractEmissionAdmissionErrorV1::Shared(SharedFromChargeError::LayoutMismatch { .. })
        ));
        assert_eq!(payload.get().as_ptr(), pointer);
        assert_eq!(original.reserved_bytes(), before);
        drop(payload);
        drop(charge);
        assert_eq!(original.reserved_bytes(), 0);
    }

    #[test]
    fn collection_has_one_vector_wire_and_json_layout_and_transport_stays_untrusted() {
        let values = vec![sample()];
        let collection = ContractEmissionsV1::from_untrusted(values.clone());
        assert_eq!(collection.len(), 1);
        assert_eq!(collection.iter().next(), values.first());
        let bytes = norito::encode_canonical(&values).unwrap();
        assert_eq!(norito::encode_canonical(&collection).unwrap(), bytes);
        let decoded = norito::decode_canonical::<ContractEmissionsV1>(&bytes).unwrap();
        assert_eq!(decoded, collection);
        let json = norito::json::to_json(&values).unwrap();
        assert_eq!(norito::json::to_json(&collection).unwrap(), json);
        assert_eq!(
            norito::json::from_str::<ContractEmissionsV1>(&json).unwrap(),
            collection
        );
        let budget = AllocationBudget::new(64 * 1024);
        assert!(!decoded.admitted_to(&budget));
        assert!(!ContractEmissionsV1::ptr_eq(
            &collection,
            &collection.clone()
        ));
        assert_eq!(collection.clone(), collection);
        assert!(ContractEmissionsV1::default() < collection);
        assert!(format!("{collection:?}").contains("Accepted"));
    }
}
