//! Iroha schema generation support library. Contains the
//! `build_schemas` `fn`, which is the function which decides which
//! types are included in the schema.
use iroha_data_model::{
    block::stream::{BlockMessage, BlockSubscriptionRequest},
    query::{QueryResponse, SignedQuery},
};
use iroha_schema::prelude::*;
use iroha_telemetry::metrics::Status;

macro_rules! types {
    ($($t:ty),+ $(,)?) => {
        // use all the types in a type position, so that IDE can resolve them
        const _: () = {
            use complete_data_model::*;
            $(
                let _resolve_my_type_pls: $t;
            )+
        };

        /// Apply `callback` to all types in the schema.
        #[macro_export]
        macro_rules! map_all_schema_types {
            ($callback:ident) => {{
                $( $callback!($t); )+
            }}
        }
    }
}

// Macro containing the list of all top-level schema types. It is used both to
// generate the schema map and to export the `map_all_schema_types!` macro for
// compile-time iteration.
macro_rules! schema_types {
    ($callback:ident) => {
        $callback! {
            Peer,
            SignedTransaction,
            SignedQuery,
            QueryResponse,
            iroha_data_model::query::dsl::CommittedTxPredicate,
            // Event stream
            EventMessage,
            EventSubscriptionRequest,
            // Block stream
            BlockMessage,
            BlockSubscriptionRequest,
            iroha_data_model::fastpq::TransferTranscript,
            iroha_data_model::fastpq::TransferTranscriptBundle,
            iroha_data_model::fastpq::FastpqTransitionBatch,
            iroha_data_model::fastpq::FastpqStateTransition,
            // Never referenced, but present in type signature. Like `PhantomData<X>`
            MerkleTree<SignedTransaction>,
            // Default permissions
            iroha_executor_data_model::permission::peer::CanManagePeers,
            iroha_executor_data_model::permission::domain::CanRegisterDomain,
            iroha_executor_data_model::permission::domain::CanUnregisterDomain,
            iroha_executor_data_model::permission::domain::CanModifyDomainMetadata,
            iroha_executor_data_model::permission::account::CanRegisterAccount,
            iroha_executor_data_model::permission::account::CanUnregisterAccount,
            iroha_executor_data_model::permission::account::CanModifyAccountMetadata,
            iroha_executor_data_model::permission::asset_definition::CanRegisterAssetDefinition,
            iroha_executor_data_model::permission::asset_definition::CanUnregisterAssetDefinition,
            iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata,
            iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition,
            iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition,
            iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition,
            iroha_executor_data_model::permission::asset::CanMintAsset,
            iroha_executor_data_model::permission::asset::CanBurnAsset,
            iroha_executor_data_model::permission::asset::CanTransferAsset,
            iroha_executor_data_model::permission::nft::CanRegisterNft,
            iroha_executor_data_model::permission::nft::CanUnregisterNft,
            iroha_executor_data_model::permission::nft::CanTransferNft,
            iroha_executor_data_model::permission::nft::CanModifyNftMetadata,
            iroha_executor_data_model::permission::parameter::CanSetParameters,
            iroha_executor_data_model::permission::role::CanManageRoles,
            iroha_executor_data_model::permission::trigger::CanRegisterTrigger,
            iroha_executor_data_model::permission::trigger::CanExecuteTrigger,
            iroha_executor_data_model::permission::trigger::CanUnregisterTrigger,
            iroha_executor_data_model::permission::trigger::CanModifyTrigger,
            iroha_executor_data_model::permission::trigger::CanModifyTriggerMetadata,
            iroha_executor_data_model::permission::executor::CanUpgradeExecutor,
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode,
            // Multi-signature operations
            iroha_executor_data_model::isi::multisig::MultisigInstructionBox,
            // Multi-signature account metadata
            iroha_executor_data_model::isi::multisig::MultisigSpec,
            iroha_executor_data_model::isi::multisig::MultisigProposalValue,
            // It is exposed via Torii
            Status
        }
    };
}

/// Builds the schema for the current state of Iroha.
///
/// You should only include the top-level types because other types
/// shall be included recursively.
pub fn build_schemas() -> MetaMap {
    use iroha_data_model::prelude::*;

    macro_rules! schemas {
        ($($t:ty),* $(,)?) => {{
            let mut out = MetaMap::new(); $(
                <$t as IntoSchema>::update_schema_map(&mut out);
            )*
            out
        }};
    }

    schema_types!(schemas)
}

schema_types!(types);

pub mod complete_data_model {
    //! Complete set of types participating in the schema

    pub use core::num::{NonZeroU16, NonZeroU32, NonZeroU64};
    pub use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

    pub use iroha_crypto::*;
    pub use iroha_data_model::{
        Level,
        account::NewAccount,
        asset::NewAssetDefinition,
        block::{
            BlockHeader, BlockPayload, BlockResult, BlockSignature, SignedBlock,
            error::BlockRejectionReason,
            stream::{BlockMessage, BlockSubscriptionRequest},
        },
        domain::NewDomain,
        events::pipeline::{BlockEventFilter, TransactionEventFilter},
        executor::{Executor, ExecutorDataModel},
        fastpq::{
            FastpqStateTransition, FastpqTransitionBatch, TransferDeltaTranscript,
            TransferTranscript, TransferTranscriptBundle,
        },
        ipfs::IpfsPath,
        isi::{
            InstructionType,
            error::{
                InstructionEvaluationError, InstructionExecutionError, InvalidParameterError,
                MathError, MintabilityError, Mismatch, RepetitionError, TypeError,
            },
        },
        parameter::{
            BlockParameter, BlockParameters, CustomParameter, CustomParameterId, Parameter,
            Parameters, SmartContractParameter, SmartContractParameters, SumeragiParameter,
            SumeragiParameters, TransactionParameter, TransactionParameters,
        },
        prelude::*,
        query::{
            CommittedTransaction, QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple,
            QueryRequestWithAuthority, QueryResponse, QuerySignature, QueryWithFilter,
            QueryWithParams, SignedQuery, SingularQueryOutputBox,
            dsl::{CompoundPredicate, PredicateMarker, SelectorMarker},
            error::{FindError, QueryExecutionFail},
            parameters::{ForwardCursor, QueryParams},
        },
        transaction::{
            TransactionSignature,
            error::TransactionLimitError,
            signed::{SignedTransaction, TransactionPayload},
        },
    };
    pub use iroha_genesis::{GenesisIvmAction, GenesisIvmTrigger, IvmPath};
    pub use iroha_primitives::{
        addr::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrHost, SocketAddrV4, SocketAddrV6},
        const_vec::ConstVec,
        conststr::ConstString,
        json::Json,
    };
    pub use iroha_schema::Compact;
    pub use iroha_telemetry::metrics::{Status, Uptime};
}

#[cfg(test)]
mod tests {
    use iroha_schema::MetaMapEntry;

    use super::{IntoSchema, complete_data_model::*};

    fn is_const_generic(generic: &str) -> bool {
        generic.parse::<usize>().is_ok()
    }

    fn generate_test_map() -> BTreeMap<core::any::TypeId, String> {
        let mut map = BTreeMap::new();

        macro_rules! insert_into_test_map {
            ($t:ty) => {{
                let type_id = <$t as iroha_schema::TypeId>::id();

                if let Some(type_id) = map.insert(core::any::TypeId::of::<$t>(), type_id) {
                    panic!(
                        "{}: Duplicate type id. Make sure that type ids are unique",
                        type_id
                    );
                }
            }};
        }

        map_all_schema_types!(insert_into_test_map);

        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigRegister);
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigPropose);
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigApprove);

        map
    }

    // For `PhantomData` wrapped types schemas aren't expanded recursively.
    // This test ensures that schemas for those types are present as well.
    fn find_missing_type_params(type_names: &HashSet<String>) -> HashMap<&str, Vec<&str>> {
        let mut missing_schemas = HashMap::<&str, _>::new();

        for type_name in type_names {
            let (Some(start), Some(end)) = (type_name.find('<'), type_name.rfind('>')) else {
                continue;
            };

            assert!(start < end, "Invalid type name: {type_name}");

            for generic in type_name.split(", ") {
                if !is_const_generic(generic) {
                    continue;
                }

                if !type_names.contains(generic) {
                    missing_schemas
                        .entry(type_name)
                        .or_insert_with(Vec::new)
                        .push(generic);
                }
            }
        }

        missing_schemas
    }

    #[test]
    fn no_extra_or_missing_schemas() {
        // NOTE: Skipping Box<str> until [this PR](https://github.com/paritytech/parity-scale-codec/pull/565) is merged
        let exceptions: [core::any::TypeId; 1] = [core::any::TypeId::of::<Box<str>>()];

        let schemas_types = super::build_schemas()
            .into_iter()
            .collect::<HashMap<_, _>>();
        let map_types = generate_test_map();

        let mut missing_types = HashSet::new();
        for (type_id, type_name) in &map_types {
            if !schemas_types.contains_key(type_id) && !exceptions.contains(type_id) {
                missing_types.insert(type_name);
            }
        }
        assert!(
            missing_types.is_empty(),
            "Missing types: {missing_types:#?}",
        );
    }

    #[test]
    fn no_missing_referenced_types() {
        let type_names = super::build_schemas()
            .into_iter()
            .map(|(_, MetaMapEntry { type_id, .. })| type_id)
            .collect();
        let missing_schemas = find_missing_type_params(&type_names);

        assert!(
            missing_schemas.is_empty(),
            "Missing schemas: \n{missing_schemas:#?}"
        );
    }

    #[test]
    // NOTE: This test guards from incorrect implementation where
    // `SortedVec<T>` and `Vec<T>` start stepping over each other
    fn no_schema_type_overlap() {
        let mut schemas = super::build_schemas();
        <Vec<PublicKey>>::update_schema_map(&mut schemas);
        <BTreeSet<SignedTransaction>>::update_schema_map(&mut schemas);
    }

    #[test]
    fn fastpq_types_have_schema_entries() {
        use iroha_data_model::fastpq::{
            FastpqTransitionBatch, TransferTranscript, TransferTranscriptBundle,
        };

        let schemas = super::build_schemas();
        let has_transcript = schemas.contains_key::<TransferTranscript>();
        let has_bundle = schemas.contains_key::<TransferTranscriptBundle>();
        let has_batch = schemas.contains_key::<FastpqTransitionBatch>();
        assert!(has_transcript, "TransferTranscript missing from schema map");
        assert!(
            has_bundle,
            "TransferTranscriptBundle missing from schema map"
        );
        assert!(has_batch, "FastpqTransitionBatch missing from schema map");
    }
}
