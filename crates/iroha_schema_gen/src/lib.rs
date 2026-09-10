//! Iroha schema generation support library. Contains the `build_schemas` `fn`, which is the
//! function which decides which types are included in the schema.
use iroha_data_model::{
    block::stream::{BlockMessage, BlockSubscriptionRequest},
    query::{QueryResponse, SignedQuery},
};
use iroha_schema::prelude::*;
use iroha_torii_shared::status::Status;
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
            // Durable cross-service DA spool envelope.
            iroha_data_model::da::ingest::StoredDaReceipt,
            iroha_data_model::fastpq::TransferTranscript,
            iroha_data_model::fastpq::TransferTranscriptBundle,
            iroha_data_model::fastpq::FastpqTransitionBatch,
            iroha_data_model::fastpq::FastpqStateTransition,
            iroha_data_model::fastpq::FastpqBalanceKeyV1,
            iroha_data_model::fastpq::FastpqOrdinaryCompactArtifactV1,
            iroha_data_model::fastpq::FastpqAxtCompactArtifactV1,
            iroha_data_model::fastpq::FastpqArtifactIdentityDescriptionV1,
            iroha_data_model::fastpq::FastpqOrdinarySourceStatementOpeningV1,
            iroha_data_model::fastpq::FastpqOrdinarySourceStatementArchiveV1,
            iroha_data_model::fastpq::FastpqSourceExecutionEntryV1,
            // Never referenced, but present in type signature. Like `PhantomData<X>`
            MerkleTree<SignedTransaction>,
            // Default permissions
            iroha_executor_data_model::permission::peer::CanManagePeers,
            iroha_executor_data_model::permission::peer::CanManageLaneRelayEmergency,
            iroha_executor_data_model::permission::domain::CanRegisterDomain,
            iroha_executor_data_model::permission::domain::CanUnregisterDomain,
            iroha_executor_data_model::permission::domain::CanModifyDomainMetadata,
            iroha_executor_data_model::permission::account::CanRegisterAccount,
            iroha_executor_data_model::permission::account::CanUnregisterAccount,
            iroha_executor_data_model::permission::account::CanModifyAccountMetadata,
            iroha_executor_data_model::permission::asset_definition::CanUnregisterAssetDefinition,
            iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata,
            iroha_executor_data_model::permission::asset_definition::CanManageAssetDefinitionConfidentialPolicy,
            iroha_executor_data_model::permission::asset_definition::CanManageAssetDefinitionAlias,
            iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition,
            iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition,
            iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition,
            iroha_executor_data_model::permission::asset::CanMintAssetToAccount,
            iroha_executor_data_model::permission::asset::CanBurnAsset,
            iroha_executor_data_model::permission::asset::CanTransferAsset,
            iroha_executor_data_model::permission::nft::CanRegisterNft,
            iroha_executor_data_model::permission::nft::CanUnregisterNft,
            iroha_executor_data_model::permission::nft::CanTransferNft,
            iroha_executor_data_model::permission::nft::CanModifyNftMetadata,
            iroha_executor_data_model::permission::parameter::CanSetParameters,
            iroha_executor_data_model::permission::parameter::CanSetHijiriParameters,
            iroha_executor_data_model::permission::role::CanManageRoles,
            iroha_executor_data_model::permission::trigger::CanRegisterTrigger,
            iroha_executor_data_model::permission::trigger::CanExecuteTrigger,
            iroha_executor_data_model::permission::trigger::CanUnregisterTrigger,
            iroha_executor_data_model::permission::trigger::CanModifyTrigger,
            iroha_executor_data_model::permission::trigger::CanModifyTriggerMetadata,
            iroha_executor_data_model::permission::executor::CanUpgradeExecutor,
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode,
            // Native bounded smart-contract artifact upload protocol
            iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk,
            iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload,
            iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload,
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
/// You should only include the top-level types because other types shall be included recursively.
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
            FastpqBalanceKeyV1, FastpqStateTransition, FastpqTransitionBatch,
            TransferDeltaTranscript, TransferTranscript, TransferTranscriptBundle,
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
    pub use iroha_torii_shared::status::{Status, Uptime};
    pub use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
}
#[cfg(test)]
mod tests {
    use super::{IntoSchema, complete_data_model::*};
    use iroha_schema::{MetaMap, Metadata};
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
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigCancel);
        map
    }
    // Stored metadata edges determine schema closure. Nominal type parameters
    // can be phantom (HashOf<T>) or fully represented by leaf metadata (Compact<T>).
    fn find_missing_schema_references(schemas: &MetaMap) -> BTreeMap<&str, Vec<core::any::TypeId>> {
        let registered: HashSet<_> = schemas.iter().map(|(id, _)| *id).collect();
        let mut missing = BTreeMap::new();
        // Iterating registered nodes once also handles cycles without recursion.
        for (_, entry) in schemas.iter() {
            let references: Vec<_> = match &entry.metadata {
                Metadata::Struct(fields) => {
                    fields.declarations.iter().map(|field| field.ty).collect()
                }
                Metadata::Tuple(fields) => fields.types.clone(),
                Metadata::Enum(variants) => variants
                    .variants
                    .iter()
                    .filter_map(|variant| variant.ty)
                    .collect(),
                Metadata::FixedPoint(fixed) => vec![fixed.base],
                Metadata::Array(array) => vec![array.ty],
                Metadata::Vec(vector) => vec![vector.ty],
                Metadata::Map(map) => vec![map.key, map.value],
                Metadata::Option(inner) => vec![*inner],
                Metadata::Result(result) => vec![result.ok, result.err],
                Metadata::Bitmap(bitmap) => vec![bitmap.repr],
                Metadata::Int(_) | Metadata::Float(_) | Metadata::String | Metadata::Bool => {
                    Vec::new()
                }
            };
            let absent: Vec<_> = references
                .into_iter()
                .filter(|id| !registered.contains(id))
                .collect();
            if !absent.is_empty() {
                missing.insert(entry.type_id.as_str(), absent);
            }
        }
        missing
    }
    #[test]
    fn stored_da_receipt_schema_includes_its_receipt_payload() {
        use iroha_data_model::da::ingest::{DaIngestReceipt, StoredDaReceipt};

        let schemas = super::build_schemas();
        assert!(schemas.contains_key::<StoredDaReceipt>());
        assert!(schemas.contains_key::<DaIngestReceipt>());
    }
    #[test]
    fn no_extra_or_missing_schemas() {
        // NOTE: Skipping Box<str> until schema generation supports unsized string boxes.
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
        let schemas = super::build_schemas();
        let missing_schemas = find_missing_schema_references(&schemas);
        assert!(
            missing_schemas.is_empty(),
            "Missing schemas: \n{missing_schemas:#?}"
        );
    }
    #[derive(IntoSchema)]
    struct ClosureRoot;

    #[derive(IntoSchema)]
    struct ClosurePeer;

    #[test]
    fn metadata_closure_checks_every_stored_reference_slot() {
        use core::any::TypeId;
        use iroha_schema::{
            ArrayMeta, BitmapMeta, Declaration, EnumMeta, EnumVariant, FixedMeta, MapMeta,
            NamedFieldsMeta, ResultMeta, UnnamedFieldsMeta, VecMeta,
        };
        let byte = TypeId::of::<u8>();
        let word = TypeId::of::<u16>();
        let cases = [
            (
                Metadata::Struct(NamedFieldsMeta {
                    declarations: vec![
                        Declaration {
                            name: "first".into(),
                            ty: byte,
                        },
                        Declaration {
                            name: "second".into(),
                            ty: word,
                        },
                    ],
                }),
                vec![byte, word],
            ),
            (
                Metadata::Tuple(UnnamedFieldsMeta {
                    types: vec![byte, word],
                }),
                vec![byte, word],
            ),
            (
                Metadata::Enum(EnumMeta {
                    variants: vec![
                        EnumVariant {
                            tag: "First".into(),
                            discriminant: 0,
                            ty: Some(byte),
                        },
                        EnumVariant {
                            tag: "Unit".into(),
                            discriminant: 1,
                            ty: None,
                        },
                        EnumVariant {
                            tag: "Second".into(),
                            discriminant: 2,
                            ty: Some(word),
                        },
                    ],
                }),
                vec![byte, word],
            ),
            (
                Metadata::FixedPoint(FixedMeta {
                    base: byte,
                    decimal_places: 4,
                }),
                vec![byte],
            ),
            (Metadata::Array(ArrayMeta { ty: byte, len: 32 }), vec![byte]),
            (Metadata::Vec(VecMeta { ty: byte }), vec![byte]),
            (
                Metadata::Map(MapMeta {
                    key: byte,
                    value: word,
                }),
                vec![byte, word],
            ),
            (Metadata::Option(byte), vec![byte]),
            (
                Metadata::Result(ResultMeta {
                    ok: byte,
                    err: word,
                }),
                vec![byte, word],
            ),
            (
                Metadata::Bitmap(BitmapMeta {
                    repr: byte,
                    masks: Vec::new(),
                }),
                vec![byte],
            ),
        ];
        let owner = <ClosureRoot as iroha_schema::TypeId>::id();
        for (metadata, expected) in cases {
            let mut schemas = MetaMap::new();
            schemas.insert::<ClosureRoot>(metadata);
            let missing = find_missing_schema_references(&schemas);
            assert_eq!(missing.len(), 1);
            assert_eq!(missing.get(owner.as_str()), Some(&expected));
            u8::update_schema_map(&mut schemas);
            u16::update_schema_map(&mut schemas);
            assert!(find_missing_schema_references(&schemas).is_empty());
            // Each slot must be checked independently, including map values and errors.
            for missing_id in expected {
                let mut incomplete = schemas.clone();
                if missing_id == byte {
                    assert!(incomplete.remove::<u8>());
                } else {
                    assert_eq!(missing_id, word);
                    assert!(incomplete.remove::<u16>());
                }
                let missing = find_missing_schema_references(&incomplete);
                assert_eq!(missing.len(), 1);
                assert_eq!(missing.get(owner.as_str()), Some(&vec![missing_id]));
            }
        }
    }

    #[test]
    fn metadata_leaves_and_unit_payloads_have_no_stored_edges() {
        use iroha_schema::{EnumMeta, EnumVariant, FloatMode, IntMode, UnnamedFieldsMeta};
        for metadata in [
            Metadata::Int(IntMode::FixedWidth),
            Metadata::Int(IntMode::Compact),
            Metadata::Float(FloatMode::Binary32),
            Metadata::Float(FloatMode::Binary64),
            Metadata::String,
            Metadata::Bool,
            Metadata::Tuple(UnnamedFieldsMeta { types: Vec::new() }),
            Metadata::Enum(EnumMeta {
                variants: vec![EnumVariant {
                    tag: "Unit".into(),
                    discriminant: 0,
                    ty: None,
                }],
            }),
        ] {
            let mut schemas = MetaMap::new();
            schemas.insert::<ClosureRoot>(metadata);
            assert!(find_missing_schema_references(&schemas).is_empty());
        }
        let compact = Compact::<u32>::schema();
        assert!(!compact.contains_key::<u32>());
        assert!(find_missing_schema_references(&compact).is_empty());
    }

    #[test]
    fn metadata_closure_handles_cycles_and_reports_dangling_edges() {
        use core::any::TypeId;
        use iroha_schema::UnnamedFieldsMeta;
        let mut schemas = MetaMap::new();
        schemas.insert::<ClosureRoot>(Metadata::Option(TypeId::of::<ClosurePeer>()));
        schemas.insert::<ClosurePeer>(Metadata::Tuple(UnnamedFieldsMeta {
            types: vec![TypeId::of::<ClosureRoot>()],
        }));
        assert!(find_missing_schema_references(&schemas).is_empty());
        assert!(schemas.remove::<ClosurePeer>());
        let missing = find_missing_schema_references(&schemas);
        assert_eq!(missing.len(), 1);
        assert_eq!(
            missing.get(<ClosureRoot as iroha_schema::TypeId>::id().as_str()),
            Some(&vec![TypeId::of::<ClosurePeer>()])
        );
    }

    #[derive(iroha_schema::TypeId)]
    struct PhantomReferent;

    impl IntoSchema for PhantomReferent {
        fn type_name() -> String {
            "PhantomReferent".to_owned()
        }

        fn update_schema_map(_: &mut MetaMap) {
            panic!("schema closure must not expand a phantom hash referent");
        }
    }

    #[test]
    fn metadata_closure_checks_hash_storage_without_its_phantom_referent() {
        let mut schemas = HashOf::<PhantomReferent>::schema();
        assert!(!schemas.contains_key::<PhantomReferent>());
        assert!(find_missing_schema_references(&schemas).is_empty());
        assert!(schemas.remove::<Hash>());
        let missing = find_missing_schema_references(&schemas);
        assert_eq!(missing.len(), 1);
        assert_eq!(
            missing.get(<HashOf<PhantomReferent> as iroha_schema::TypeId>::id().as_str()),
            Some(&vec![core::any::TypeId::of::<Hash>()])
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
            FastpqBalanceKeyV1, FastpqTransitionBatch, TransferTranscript, TransferTranscriptBundle,
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
        assert!(
            schemas.contains_key::<FastpqBalanceKeyV1>(),
            "FastpqBalanceKeyV1 missing from schema map"
        );
        assert!(
            schemas.contains_key::<iroha_data_model::fastpq::FastpqOrdinaryCompactArtifactV1>()
        );
        assert!(schemas.contains_key::<iroha_data_model::fastpq::FastpqAxtCompactArtifactV1>());
        assert!(
            schemas.contains_key::<iroha_data_model::fastpq::FastpqArtifactIdentityDescriptionV1>()
        );
        assert!(
            schemas.contains_key::<iroha_data_model::fastpq::FastpqPublicTransferStatementV1>()
        );
        assert!(
            schemas
                .contains_key::<iroha_data_model::fastpq::FastpqOrdinarySourceStatementOpeningV1>()
        );
        assert!(
            schemas.contains_key::<iroha_data_model::fastpq::FastpqOrdinarySourceStatementLeafV1>()
        );
        assert!(
            schemas
                .contains_key::<iroha_data_model::fastpq::FastpqOrdinarySourceStatementManifestV1>(
                )
        );
        assert!(
            schemas
                .contains_key::<iroha_data_model::fastpq::FastpqOrdinarySourceStatementArchiveV1>()
        );
        assert!(schemas.contains_key::<iroha_data_model::fastpq::FastpqSourceStatementContextV1>());
        assert!(schemas.contains_key::<iroha_data_model::fastpq::FastpqSourceRouteV1>());
        assert!(schemas.contains_key::<iroha_data_model::fastpq::FastpqSourceLaneV1>());
        assert!(schemas.contains_key::<iroha_data_model::fastpq::FastpqSourceExecutionKindV1>());
        assert!(schemas.contains_key::<iroha_data_model::fastpq::FastpqSourceExecutionEntryV1>());
    }
}
