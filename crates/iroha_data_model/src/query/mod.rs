//! Iroha Queries provides declarative API for Iroha queries.
//!
//! Queries implement the [`crate::query::Query`] trait and can be stored as trait objects for
//! dynamic dispatch. The [`crate::query::QueryBox`] alias wraps a `Box<dyn ErasedQuery + Send + Sync>` and is used
//! throughout the data model whenever heterogeneous queries need to be passed
//! around.

#![allow(clippy::missing_inline_in_public_items)]

use std::{
    any::Any,
    boxed::Box,
    format,
    string::String,
    sync::OnceLock,
    vec::{self, Vec},
};

use derive_more::Constructor;
use iroha_crypto::{HashOf, MerkleProof, PublicKey, SignatureOf};
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_schema::IntoSchema;
use iroha_version::Version;
use norito::codec::{Decode, Encode};
use parameters::{ForwardCursor, QueryParams};

// Ensure `QueryWithFilter` is publicly re-exported in fast mode as well.
#[cfg(feature = "fast_dsl")]
pub use self::model::QueryWithFilter;
pub use self::model::*;
use self::{
    account::*, asset::*, block::*, domain::*, dsl::*, executor::*, nft::*, peer::*, permission::*,
    role::*, rwa::*, transaction::*, trigger::*,
};
#[cfg(feature = "fault_injection")]
use crate::transaction::ExecutionStep;
use crate::{
    account::{Account, AccountId},
    asset::{
        definition::AssetDefinition,
        id::{AssetDefinitionId, AssetId},
        value::Asset,
    },
    block::{BlockHeader, SignedBlock},
    domain::{Domain, DomainId},
    metadata::Metadata,
    name::Name,
    nft::{Nft, NftId},
    parameter::{Parameter, Parameters},
    peer::PeerId,
    permission::Permission,
    repo::RepoAgreement,
    role::{Role, RoleId},
    rwa::{Rwa, RwaId},
    seal,
    trigger::{Trigger, TriggerId},
};

impl iroha_version::Version for SignedQuery {
    fn version(&self) -> u8 {
        1
    }

    fn supported_versions() -> core::ops::Range<u8> {
        1..2
    }
}

#[cfg(test)]
mod signature_tests {
    use std::num::NonZeroU64;

    use super::*;

    #[test]
    fn query_signature_decode_from_slice_roundtrip() {
        let authority = AccountId::new(
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key"),
        );
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let cursor = ForwardCursor {
            query: "cursor-1".to_owned(),
            cursor: NonZeroU64::new(1).expect("nonzero"),
            gas_budget: Some(5),
        };
        let payload = QueryRequestWithAuthority {
            authority: authority.clone(),
            request: QueryRequest::Continue(cursor),
        };

        let signature = iroha_crypto::SignatureOf::new(&private_key, &payload);
        let query_signature = QuerySignature(signature.clone());

        let encoded = norito::to_bytes(&query_signature).expect("encode query signature");
        let decoded: QuerySignature =
            norito::core::decode_from_bytes(&encoded).expect("decode query signature");
        assert_eq!(decoded, query_signature);

        let inner_encoded = norito::to_bytes(&signature).expect("encode inner signature");
        let inner_decoded: iroha_crypto::SignatureOf<QueryRequestWithAuthority> =
            norito::core::decode_from_bytes(&inner_encoded).expect("decode inner signature");
        assert_eq!(inner_decoded, signature);
    }
}

impl iroha_version::codec::EncodeVersioned for SignedQuery {
    fn encode_versioned(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1);
        bytes.push(self.version());
        bytes.extend(norito::codec::encode_adaptive(self));
        bytes
    }
}

impl iroha_version::codec::DecodeVersioned for SignedQuery {
    fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
        use iroha_version::error::Error;
        if let Some(version) = input.first() {
            if Self::supported_versions().contains(version) {
                let mut cursor = &input[1..];
                let decoded = <Self as norito::codec::DecodeAll>::decode_all(&mut cursor)
                    .map_err(Error::from)?;
                if cursor.is_empty() {
                    Ok(decoded)
                } else {
                    Err(Error::NoritoCodec(
                        "SignedQuery payload contains trailing bytes".into(),
                    ))
                }
            } else {
                Err(Error::UnsupportedVersion(Box::new(
                    iroha_version::UnsupportedVersion::new(
                        *version,
                        iroha_version::RawVersioned::NoritoBytes(input.to_vec()),
                    ),
                )))
            }
        } else {
            Err(Error::NotVersioned)
        }
    }
}

/// Norito-compatible JSON representations for query payloads.
#[cfg(all(feature = "json", not(doc)))]
pub mod json_wrappers {
    use super::*;

    /// JSON wrapper for iterable query parameters (roundtrip-capable).
    ///
    /// Two encodings are supported:
    /// - Non-`fast_dsl`: include `wire` (type key) and `payload_b64` for the erased query.
    /// - `fast_dsl`: include `item_kind` and the fast-DSL payload parts as base64.
    ///
    /// Carries query parameters alongside the request.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct QueryWithParamsJson {
        /// Parameters controlling pagination, sorting, and projections.
        pub params: parameters::QueryParams,
        #[norito(default)]
        /// Optional identifier of the erased query type, provided for non-`fast_dsl` payloads.
        pub wire: Option<String>,
        #[norito(default)]
        /// Base64-encoded erased query payload when `wire` is present.
        pub payload_b64: Option<String>,
        #[cfg(feature = "fast_dsl")]
        #[norito(default)]
        /// Query item discriminator for `fast_dsl` envelopes.
        pub item_kind: Option<QueryItemKind>,
        #[cfg(feature = "fast_dsl")]
        #[norito(default)]
        /// Base64-encoded fast DSL query payload.
        pub query_payload_b64: Option<String>,
        #[cfg(feature = "fast_dsl")]
        #[norito(default)]
        /// Base64-encoded predicate fragment associated with the query, if any.
        pub predicate_b64: Option<String>,
        #[cfg(feature = "fast_dsl")]
        #[norito(default)]
        /// Base64-encoded selector fragment narrowing the result set.
        pub selector_b64: Option<String>,
    }

    /// JSON wrapper for `QueryRequest` enum.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "content")]
    pub enum QueryRequestJson {
        /// Singular (non-iterable) query request.
        Singular(SingularQueryBox),
        /// Iterable query request together with its parameters.
        Start(QueryWithParamsJson),
        /// Continuation token for paginated iterable queries.
        Continue(parameters::ForwardCursor),
    }

    /// JSON wrapper for `QueryRequestWithAuthority`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct QueryRequestWithAuthorityJson {
        /// Account that authorised the query.
        pub authority: crate::account::AccountId,
        /// Request being authorised.
        pub request: QueryRequestJson,
    }

    /// JSON wrapper for the canonical `SignedQuery` form.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SignedQueryCanonicalJson {
        /// Signature authenticating the query payload.
        pub signature: super::model::QuerySignature,
        /// Canonical payload describing the authorised query.
        pub payload: QueryRequestWithAuthorityJson,
    }

    /// JSON wrapper for versioned `SignedQuery`.
    ///
    /// Version is a string per common versioned JSON conventions elsewhere.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(tag = "version", content = "content"))]
    pub enum SignedQueryJson {
        /// Canonical JSON representation of a signed query.
        #[norito(rename = "canonical")]
        Canonical(SignedQueryCanonicalJson),
    }

    /// Convert a JSON-wrapped query request into the native query request.
    ///
    /// # Errors
    /// Returns an error string when required fields are missing or the payload
    /// cannot be decoded into a query request.
    pub fn query_request_from_json(req: QueryRequestJson) -> Result<QueryRequest, &'static str> {
        match req {
            QueryRequestJson::Singular(q) => Ok(QueryRequest::Singular(q)),
            QueryRequestJson::Continue(c) => Ok(QueryRequest::Continue(c)),
            QueryRequestJson::Start(s) => {
                #[cfg(not(feature = "fast_dsl"))]
                {
                    let wire = s.wire.ok_or("missing wire id")?;
                    let payload_b64 = s.payload_b64.ok_or("missing payload")?;
                    let bytes = base64_decode(&payload_b64).map_err(|()| "invalid payload_b64")?;
                    let qb = super::query_registry()
                        .decode(&wire, &bytes)
                        .ok_or("unknown query wire id")?
                        .map_err(|_| "failed to decode query payload")?;
                    Ok(QueryRequest::Start(QueryWithParams {
                        query: qb,
                        params: s.params,
                    }))
                }
                #[cfg(feature = "fast_dsl")]
                {
                    let item = s.item_kind.ok_or("missing item_kind")?;
                    let query_payload = s.query_payload_b64.ok_or("missing query_payload_b64")?;
                    let predicate_b64 = s.predicate_b64.ok_or("missing predicate_b64")?;
                    let selector_b64 = s.selector_b64.ok_or("missing selector_b64")?;
                    let qp = base64_decode(&query_payload).map_err(|()| "bad query_payload_b64")?;
                    let pr = base64_decode(&predicate_b64).map_err(|()| "bad predicate_b64")?;
                    let se = base64_decode(&selector_b64).map_err(|()| "bad selector_b64")?;
                    Ok(QueryRequest::Start(QueryWithParams {
                        query: (),
                        query_payload: qp,
                        item,
                        predicate_bytes: pr,
                        selector_bytes: se,
                        params: s.params,
                    }))
                }
            }
        }
    }

    /// Convert a query request into its JSON wrapper form.
    pub fn query_request_to_json(req: &QueryRequest) -> QueryRequestJson {
        match req {
            QueryRequest::Singular(q) => QueryRequestJson::Singular(q.clone()),
            QueryRequest::Continue(c) => QueryRequestJson::Continue(c.clone()),
            QueryRequest::Start(qwp) => {
                #[cfg(not(feature = "fast_dsl"))]
                {
                    qwp.query_box().map_or_else(
                        || {
                            // Fallback: emit params only
                            QueryRequestJson::Start(QueryWithParamsJson {
                                params: qwp.params.clone(),
                                wire: None,
                                payload_b64: None,
                            })
                        },
                        |qb| {
                            let wire = (**qb).type_name_key().to_string();
                            let payload = (**qb).encode_bytes();
                            QueryRequestJson::Start(QueryWithParamsJson {
                                params: qwp.params.clone(),
                                wire: Some(wire),
                                payload_b64: Some(base64_encode(&payload)),
                                #[cfg(feature = "fast_dsl")]
                                item_kind: None,
                                #[cfg(feature = "fast_dsl")]
                                query_payload_b64: None,
                                #[cfg(feature = "fast_dsl")]
                                predicate_b64: None,
                                #[cfg(feature = "fast_dsl")]
                                selector_b64: None,
                            })
                        },
                    )
                }
                #[cfg(feature = "fast_dsl")]
                {
                    QueryRequestJson::Start(QueryWithParamsJson {
                        params: qwp.params.clone(),
                        wire: None,
                        payload_b64: None,
                        item_kind: Some(qwp.item),
                        query_payload_b64: Some(base64_encode(&qwp.query_payload)),
                        predicate_b64: Some(base64_encode(&qwp.predicate_bytes)),
                        selector_b64: Some(base64_encode(&qwp.selector_bytes)),
                    })
                }
            }
        }
    }

    impl TryFrom<SignedQueryJson> for SignedQuery {
        type Error = &'static str;
        fn try_from(v: SignedQueryJson) -> Result<Self, Self::Error> {
            match v {
                SignedQueryJson::Canonical(v1) => {
                    // Validate signature against the reconstructed payload
                    let request_for_verify = query_request_from_json(v1.payload.request.clone())?;
                    let QuerySignature(sig) = &v1.signature;
                    sig.verify(
                        v1.payload.authority.signatory(),
                        &QueryRequestWithAuthority {
                            authority: v1.payload.authority.clone(),
                            request: request_for_verify,
                        },
                    )
                    .map_err(|_| "invalid SignedQuery signature")?;

                    let request = query_request_from_json(v1.payload.request)?;

                    // Build canonical SignedQuery
                    Ok(SignedQuery {
                        signature: v1.signature,
                        payload: QueryRequestWithAuthority {
                            authority: v1.payload.authority,
                            request,
                        },
                    })
                }
            }
        }
    }

    impl From<&SignedQuery> for SignedQueryJson {
        fn from(sq: &SignedQuery) -> Self {
            let req_json = query_request_to_json(&sq.payload.request);
            SignedQueryJson::Canonical(SignedQueryCanonicalJson {
                signature: sq.signature.clone(),
                payload: QueryRequestWithAuthorityJson {
                    authority: sq.payload.authority.clone(),
                    request: req_json,
                },
            })
        }
    }

    pub(super) fn base64_encode(bytes: &[u8]) -> String {
        use base64::engine::{Engine, general_purpose::STANDARD};
        STANDARD.encode(bytes)
    }

    pub(super) fn base64_decode(s: &str) -> Result<Vec<u8>, ()> {
        use base64::engine::{Engine, general_purpose::STANDARD};
        STANDARD.decode(s.as_bytes()).map_err(|_| ())
    }
}

/// JSON utilities for assembling and parsing queries.
#[cfg(feature = "json")]
#[doc = "JSON conversion helpers used by query APIs."]
pub mod json;

// NOTE: Additional encode instrumentation for queries lives in iroha_crypto (SignatureOf::new, HashOf::new).
#[cfg(feature = "fault_injection")]
use crate::{
    ValidationFail,
    prelude::{
        InstructionBox, TransactionEntrypoint, TransactionRejectionReason, TransactionResult,
    },
};

/// Builder helpers for constructing query instances.
#[doc = "Builder utilities for composing typed queries."]
pub mod builder;
// Use the full DSL by default; swap for a lightweight version under `fast_dsl`.
/// Ergonomic DSL for building queries.
#[cfg(not(feature = "fast_dsl"))]
#[doc = "Declarative query DSL."]
pub mod dsl;
/// Optimised DSL variant that avoids intermediate allocations.
#[cfg(feature = "fast_dsl")]
#[doc = "Optimized query DSL for fast evaluation."]
pub mod dsl_fast;
#[cfg(feature = "fast_dsl")]
pub use dsl_fast as dsl;
/// Query parameter types and helpers.
#[doc = "Query parameter storage and cursor types."]
pub mod parameters;
pub(crate) mod tx_predicate;

/// A query that either returns a single value or errors out
// NOTE: we are planning to remove this class of queries (https://github.com/hyperledger-iroha/iroha/issues/4933)
/// Trait implemented by query types participating in the Iroha API.
pub trait SingularQuery: seal::SingularQuery {
    /// The type of the output of the query
    type Output;

    /// Execute the query. No-op by default
    fn execute(&self) {}

    /// Encode the query into bytes using Norito binary serialization
    fn dyn_encode(&self) -> Vec<u8>;

    /// Downcast to concrete type
    fn as_any(&self) -> &dyn Any;
}

/// A query that returns an iterable collection of values.
///
/// Implementations are typically used through the [`QueryBox`] type alias
/// (`Box<dyn Query + Send + Sync>`), which allows storing different query types behind a
/// single interface.
///
/// Iterable queries logically return a stream of items.
/// In the actual implementation, the items collected into batches and a cursor is used to fetch the next batch.
/// [`builder::QueryIterator`] abstracts over this and allows the query consumer to use a familiar [`Iterator`] interface to iterate over the results.
pub trait Query: seal::Query + Send + Sync + 'static {
    /// The type of single element of the output collection
    type Item: HasProjection<PredicateMarker> + HasProjection<SelectorMarker, AtomType = ()>;

    /// Execute the query. No-op by default
    fn execute(&self) {}

    /// Encode the query into bytes using Norito binary serialization
    fn dyn_encode(&self) -> Vec<u8>
    where
        Self: Encode,
    {
        self.encode()
    }

    /// Downcast to concrete type
    fn as_any(&self) -> &dyn Any
    where
        Self: Sized,
    {
        self
    }
}

/// Function signature used to construct a query from raw bytes.
pub type QueryConstructor = fn(&[u8]) -> Result<QueryBox<QueryOutputBatchBox>, norito::Error>;

/// Registry storing constructors for query types keyed by their type names.
#[derive(Default)]
pub struct QueryRegistry {
    entries: Vec<(&'static str, QueryConstructor)>,
}

impl QueryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a query type.
    #[must_use]
    pub fn register<T>(mut self) -> Self
    where
        T: Query<Item = QueryOutputBatchBox> + Decode + Encode + 'static,
    {
        fn ctor<T>(input: &[u8]) -> Result<QueryBox<QueryOutputBatchBox>, norito::Error>
        where
            T: Query<Item = QueryOutputBatchBox> + Decode + Encode + 'static,
        {
            let query = T::decode(&mut &*input)?;
            Ok(Box::new(query))
        }

        let name = std::any::type_name::<T>();
        self.entries.push((name, ctor::<T>));
        self
    }

    /// Decode a query using the constructor registered for the given type name.
    pub fn decode(
        &self,
        name: &str,
        bytes: &[u8],
    ) -> Option<Result<QueryBox<QueryOutputBatchBox>, norito::Error>> {
        self.entries
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, ctor)| ctor(bytes))
    }
}

/// Build a [`QueryRegistry`] populated with the provided query types.
#[macro_export]
macro_rules! query_registry {
    ($($ty:ty),* $(,)?) => {
        $crate::query::QueryRegistry::new()
            $(.register::<$ty>())*
    };
}

static QUERY_REGISTRY: OnceLock<QueryRegistry> = OnceLock::new();

static DEFAULT_QUERY_REGISTRY: OnceLock<QueryRegistry> = OnceLock::new();

/// Set the global query registry used to decode queries by type name.
///
/// This should be called exactly once during application start-up. Subsequent
/// calls are ignored.
///
/// If this function is never invoked, the data model falls back to a built-in
/// registry covering the standard iterable query set, allowing JSON and Norito
/// decoding to work out of the box in tests and utilities.
pub fn set_query_registry(registry: QueryRegistry) {
    let _ = QUERY_REGISTRY.set(registry);
}

fn query_registry() -> &'static QueryRegistry {
    QUERY_REGISTRY
        .get()
        .unwrap_or_else(|| builtin_query_registry())
}

fn builtin_query_registry() -> &'static QueryRegistry {
    DEFAULT_QUERY_REGISTRY.get_or_init(|| {
        crate::query_registry![
            ErasedIterQuery<crate::domain::Domain>,
            ErasedIterQuery<crate::account::Account>,
            ErasedIterQuery<crate::asset::value::Asset>,
            ErasedIterQuery<crate::asset::definition::AssetDefinition>,
            ErasedIterQuery<crate::repo::RepoAgreement>,
            ErasedIterQuery<crate::nft::Nft>,
            ErasedIterQuery<crate::role::Role>,
            ErasedIterQuery<crate::role::RoleId>,
            ErasedIterQuery<crate::peer::PeerId>,
            ErasedIterQuery<crate::trigger::TriggerId>,
            ErasedIterQuery<crate::trigger::Trigger>,
            ErasedIterQuery<CommittedTransaction>,
            ErasedIterQuery<crate::block::SignedBlock>,
            ErasedIterQuery<crate::block::BlockHeader>,
            ErasedIterQuery<crate::proof::ProofRecord>,
        ]
    })
}

#[model]
mod model {
    use getset::Getters;
    use iroha_crypto::HashOf;

    use super::*;
    use crate::{
        prelude::{TransactionEntrypoint, TransactionResult},
        trigger::action,
    };

    /// An iterable query bundled with a filter.
    ///
    /// This structure stores the query as a boxed trait object so heterogeneous
    /// registry entries can be handled uniformly while predicates/selectors are
    /// still being migrated to their final shape.
    #[derive(Decode, Encode, Constructor, IntoSchema)]
    pub struct QueryWithFilter<T>
    where
        T: HasProjection<PredicateMarker>
            + HasProjection<SelectorMarker, AtomType = ()>
            + Send
            + Sync,
    {
        /// Inner query wrapped in a trait object for dynamic dispatch.
        #[cfg(not(feature = "fast_dsl"))]
        #[cfg_attr(feature = "json", norito(skip))]
        pub query: Box<dyn Query<Item = T> + Send + Sync>,
        /// Placeholder in fast mode to avoid heavy trait object bounds/derives.
        #[cfg(feature = "fast_dsl")]
        pub query: (),
        pub predicate: CompoundPredicate<T>,
        pub selector: SelectorTuple<T>,
    }

    impl<T> QueryWithFilter<T>
    where
        T: HasProjection<PredicateMarker>
            + HasProjection<SelectorMarker, AtomType = ()>
            + Send
            + Sync,
    {
        /// Constructor that accepts the concrete query in non-`fast_dsl` builds and ignores it
        /// in `fast_dsl` builds where the query payload is carried elsewhere.
        #[inline]
        pub fn new_with_query(
            #[cfg(not(feature = "fast_dsl"))] query: Box<dyn super::Query<Item = T> + Send + Sync>,
            #[cfg(feature = "fast_dsl")] (): (),
            predicate: CompoundPredicate<T>,
            selector: SelectorTuple<T>,
        ) -> Self {
            #[cfg(not(feature = "fast_dsl"))]
            {
                Self::new(query, predicate, selector)
            }
            #[cfg(feature = "fast_dsl")]
            {
                Self::new((), predicate, selector)
            }
        }
    }

    #[allow(dead_code)]
    fn predicate_default<T>() -> CompoundPredicate<T>
    where
        T: HasProjection<PredicateMarker>,
    {
        CompoundPredicate::PASS
    }

    /// Type-erased iterable query used throughout the data model.
    ///
    /// This is an alias for `Box<dyn ErasedQuery<T> + Send + Sync>` enabling heterogeneous
    /// query collections.
    ///
    /// # Examples
    /// ```rust
    /// use iroha_data_model::prelude::*;
    ///
    /// let query: QueryBox<Account> = Box::new(FindAccounts);
    /// ```
    pub trait ErasedEncode {
        /// Encode the erased query into Norito bytes.
        fn erased_encode(&self) -> Vec<u8>;
    }

    impl<T: Encode> ErasedEncode for T {
        fn erased_encode(&self) -> Vec<u8> {
            self.encode()
        }
    }

    use std::any::Any;

    /// Trait implemented by query types participating in the Iroha API.
    pub trait ErasedQuery<T>: Query<Item = T> + ErasedEncode + Any + Send + Sync {
        /// Expose the concrete query as `Any` for downcasting.
        fn erased_as_any(&self) -> &dyn Any;
        /// Encode the concrete query behind the erased trait object without
        /// re-encoding the `QueryBox` wrapper (avoids recursion).
        fn encode_bytes(&self) -> Vec<u8>;
        /// Return a stable registry key for this concrete query type.
        ///
        /// This must match what is used during registration (i.e.,
        /// `std::any::type_name::<ConcreteQuery>()`). Using a dedicated
        /// method avoids relying on `type_name_of_val` for trait objects,
        /// which returns the trait object type rather than the concrete type.
        fn type_name_key(&self) -> &'static str;
    }

    impl<T, Q> ErasedQuery<T> for Q
    where
        Q: Query<Item = T> + ErasedEncode + Any + Send + Sync + norito::core::NoritoSerialize,
    {
        fn erased_as_any(&self) -> &dyn Any {
            self
        }
        fn encode_bytes(&self) -> Vec<u8> {
            // Delegate to dynamic encoder; concrete types may override it.
            self.dyn_encode()
        }
        fn type_name_key(&self) -> &'static str {
            std::any::type_name::<Self>()
        }
    }

    /// Type alias used for ergonomic query handling.
    pub type QueryBox<T> = Box<dyn ErasedQuery<T> + Send + Sync>;

    impl norito::core::NoritoSerialize for QueryBox<QueryOutputBatchBox> {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
            let name = (**self).type_name_key().to_string();
            let payload = (**self).encode_bytes();
            norito::core::NoritoSerialize::serialize(&(name, payload), writer)
        }
    }

    impl<'a> norito::core::NoritoDeserialize<'a> for QueryBox<QueryOutputBatchBox> {
        fn deserialize(
            archived: &'a norito::core::Archived<QueryBox<QueryOutputBatchBox>>,
        ) -> Self {
            let (name, bytes): (String, Vec<u8>) =
                norito::core::NoritoDeserialize::deserialize(archived.cast());
            query_registry()
                .decode(&name, &bytes)
                .expect("query is not registered")
                .expect("failed to decode query")
        }
    }

    /// An enum of all possible iterable query batches.
    ///
    /// We have an enum of batches instead of individual elements, because it makes it easier to check that the batches have elements of the same type and reduces serialization overhead.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, FromVariant)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Boxed batch of query output items.
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    pub enum QueryOutputBatchBox {
        /// Batch of public keys.
        PublicKey(Vec<PublicKey>),
        /// Batch of string values.
        String(Vec<String>),
        /// Batch of metadata entries.
        Metadata(Vec<Metadata>),
        /// Batch of JSON values.
        Json(Vec<Json>),
        /// Batch of numeric values.
        Numeric(Vec<Numeric>),
        /// Batch of names.
        Name(Vec<Name>),
        /// Batch of domain identifiers.
        DomainId(Vec<DomainId>),
        /// Batch of domain definitions.
        Domain(Vec<Domain>),
        /// Batch of account identifiers.
        AccountId(Vec<AccountId>),
        /// Batch of accounts.
        Account(Vec<Account>),
        /// Batch of asset identifiers.
        AssetId(Vec<AssetId>),
        /// Batch of assets.
        Asset(Vec<Asset>),
        /// Batch of asset definition identifiers.
        AssetDefinitionId(Vec<AssetDefinitionId>),
        /// Batch of asset definitions.
        AssetDefinition(Vec<AssetDefinition>),
        /// Batch of repository agreements.
        RepoAgreement(Vec<RepoAgreement>),
        /// Batch of NFT identifiers.
        NftId(Vec<NftId>),
        /// Batch of NFTs.
        Nft(Vec<Nft>),
        /// Batch of RWA identifiers.
        RwaId(Vec<RwaId>),
        /// Batch of RWAs.
        Rwa(Vec<Rwa>),
        /// Batch of roles.
        Role(Vec<Role>),
        /// Batch of parameters.
        Parameter(Vec<Parameter>),
        /// Batch of permissions.
        Permission(Vec<Permission>),
        /// Batch of committed transactions.
        CommittedTransaction(Vec<CommittedTransaction>),
        /// Batch of transaction results.
        TransactionResult(Vec<TransactionResult>),
        /// Batch of transaction result hashes.
        TransactionResultHash(Vec<HashOf<TransactionResult>>),
        /// Batch of transaction entrypoints.
        TransactionEntrypoint(Vec<TransactionEntrypoint>),
        /// Batch of transaction entrypoint hashes.
        TransactionEntrypointHash(Vec<HashOf<TransactionEntrypoint>>),
        /// Batch of peer identifiers.
        Peer(Vec<PeerId>),
        /// Batch of role identifiers.
        RoleId(Vec<RoleId>),
        /// Batch of trigger identifiers.
        TriggerId(Vec<TriggerId>),
        /// Batch of triggers.
        Trigger(Vec<Trigger>),
        /// Batch of actions.
        Action(Vec<action::Action>),
        /// Batch of signed blocks.
        Block(Vec<SignedBlock>),
        /// Batch of block headers.
        BlockHeader(Vec<BlockHeader>),
        /// Batch of block header hashes.
        BlockHeaderHash(Vec<HashOf<BlockHeader>>),
        /// Batch of proof records.
        ProofRecord(Vec<crate::proof::ProofRecord>),
        /// Batch of offline allowance records.
        OfflineAllowanceRecord(Vec<crate::offline::OfflineAllowanceRecord>),
        /// Batch of offline-to-online transfer records.
        OfflineToOnlineTransfer(Vec<crate::offline::OfflineTransferRecord>),
        /// Batch of offline counter summaries.
        OfflineCounterSummary(Vec<crate::offline::OfflineCounterSummary>),
        /// Batch of offline verdict revocations.
        OfflineVerdictRevocation(Vec<crate::offline::OfflineVerdictRevocation>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Constructor, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Helper tuple to materialise batches into Norito collections.
    pub struct QueryOutputBatchBoxTuple {
        /// Sequence of batches produced by an iterable query.
        pub tuple: Vec<QueryOutputBatchBox>,
    }

    /// An enum of all possible singular queries
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, FromVariant)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Boxed trait-object for singular queries.
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    pub enum SingularQueryBox {
        /// Fetch the current executor data model definition.
        FindExecutorDataModel(FindExecutorDataModel),
        /// Fetch current global parameters.
        FindParameters(FindParameters),
        /// Fetch an account by identifier.
        FindAccountById(account::prelude::FindAccountById),
        /// Fetch aliases bound to an account subject.
        FindAliasesByAccountId(account::prelude::FindAliasesByAccountId),
        /// Fetch the recovery policy keyed by a stable account alias.
        FindAccountRecoveryPolicyByAlias(account::prelude::FindAccountRecoveryPolicyByAlias),
        /// Fetch the recovery request keyed by a stable account alias.
        FindAccountRecoveryRequestByAlias(account::prelude::FindAccountRecoveryRequestByAlias),
        /// Fetch a proof record by its identifier.
        FindProofRecordById(proof::prelude::FindProofRecordById),
        /// Fetch a contract manifest by its code hash.
        FindContractManifestByCodeHash(smart_contract::prelude::FindContractManifestByCodeHash),
        /// Fetch the active ABI version.
        FindAbiVersion(runtime::prelude::FindAbiVersion),
        /// Fetch an asset by identifier.
        FindAssetById(asset::prelude::FindAssetById),
        /// Fetch an asset definition by identifier.
        FindAssetDefinitionById(asset::prelude::FindAssetDefinitionById),
        /// Fetch a trigger by identifier.
        FindTriggerById(trigger::prelude::FindTriggerById),
        /// Fetch a Twitter binding record by hash.
        FindTwitterBindingByHash(oracle::prelude::FindTwitterBindingByHash),
        /// Fetch domain endorsement records.
        FindDomainEndorsements(endorsement::prelude::FindDomainEndorsements),
        /// Fetch the domain endorsement policy.
        FindDomainEndorsementPolicy(endorsement::prelude::FindDomainEndorsementPolicy),
        /// Fetch the committee for a domain.
        FindDomainCommittee(endorsement::prelude::FindDomainCommittee),
        /// Fetch a DA pin intent by storage ticket.
        FindDaPinIntentByTicket(self::da::prelude::FindDaPinIntentByTicket),
        /// Fetch a DA pin intent by manifest digest.
        FindDaPinIntentByManifest(self::da::prelude::FindDaPinIntentByManifest),
        /// Fetch a DA pin intent by alias.
        FindDaPinIntentByAlias(self::da::prelude::FindDaPinIntentByAlias),
        /// Fetch a DA pin intent by lane/epoch/sequence tuple.
        FindDaPinIntentByLaneEpochSequence(self::da::prelude::FindDaPinIntentByLaneEpochSequence),
        /// Fetch a verified lane relay record by its canonical relay reference.
        FindLaneRelayEnvelopeByRef(self::nexus::prelude::FindLaneRelayEnvelopeByRef),
        /// Fetch the registered owner for a `SoraFS` provider.
        FindSorafsProviderOwner(sorafs::prelude::FindSorafsProviderOwner),
        /// Fetch the active SNS owner for a dataspace alias.
        FindDataspaceNameOwnerById(sns::prelude::FindDataspaceNameOwnerById),
        #[cfg(test)]
        #[doc(hidden)]
        __TestFallback,
    }

    /// An enum of all possible singular query outputs
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, FromVariant)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Boxed output of a singular query.
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    pub enum SingularQueryOutputBox {
        /// Executor data model payload.
        ExecutorDataModel(crate::executor::ExecutorDataModel),
        /// Parameter set payload.
        Parameters(Parameters),
        /// Linked domain identifier list.
        DomainIds(Vec<DomainId>),
        /// Account payload.
        Account(Account),
        /// Bound account alias records.
        AccountAliasBindingRecords(Vec<account::AccountAliasBindingRecord>),
        /// Account recovery policy payload.
        AccountRecoveryPolicy(crate::account::AccountRecoveryPolicy),
        /// Account recovery request payload.
        AccountRecoveryRequest(crate::account::AccountRecoveryRequest),
        /// Linked account identifier list.
        AccountIds(Vec<AccountId>),
        /// Proof record payload.
        ProofRecord(crate::proof::ProofRecord),
        /// Smart contract manifest payload.
        ContractManifest(crate::smart_contract::manifest::ContractManifest),
        /// Active ABI version payload.
        AbiVersion(runtime::AbiVersion),
        /// Asset payload.
        Asset(crate::asset::value::Asset),
        /// Asset definition payload.
        AssetDefinition(crate::asset::definition::AssetDefinition),
        /// Trigger payload.
        Trigger(crate::trigger::Trigger),
        /// Twitter binding payload.
        TwitterBindingRecord(crate::oracle::TwitterBindingRecord),
        /// Domain endorsements payload.
        DomainEndorsements(Vec<crate::nexus::DomainEndorsementRecord>),
        /// Domain endorsement policy payload.
        DomainEndorsementPolicy(crate::nexus::DomainEndorsementPolicy),
        /// Domain committee payload.
        DomainCommittee(crate::nexus::DomainCommittee),
        /// DA pin intent payload.
        DaPinIntent(crate::da::pin_intent::DaPinIntentWithLocation),
        /// Verified lane relay payload.
        VerifiedLaneRelayRecord(crate::nexus::VerifiedLaneRelayRecord),
        /// Account identifier payload.
        AccountId(AccountId),
    }

    /// The results of a single iterable query request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Materialised batch of query results with pagination metadata.
    pub struct QueryOutput {
        /// A single batch of results
        pub batch: QueryOutputBatchBoxTuple,
        /// The number of items in the query remaining to be fetched after this batch
        pub remaining_items: u64,
        /// If not `None`, contains a cursor that can be used to fetch the next batch of results. Otherwise the current batch is the last one.
        pub continue_cursor: Option<ForwardCursor>,
    }

    /// A type-erased iterable query, along with all the parameters needed to execute it
    #[derive(Decode, Encode)]
    pub struct QueryWithParams {
        /// Concrete query payload (absent when `fast_dsl` is enabled).
        #[cfg(not(feature = "fast_dsl"))]
        pub query: QueryBox<QueryOutputBatchBox>,
        /// Placeholder to keep layout stable when `fast_dsl` is enabled.
        #[cfg(feature = "fast_dsl")]
        pub query: (),
        /// Encoded query payload for `fast_dsl` runs.
        #[cfg(feature = "fast_dsl")]
        pub query_payload: Vec<u8>,
        /// Item kind for `fast_dsl` queries.
        #[cfg(feature = "fast_dsl")]
        pub item: QueryItemKind,
        /// Encoded predicate used by `fast_dsl`.
        #[cfg(feature = "fast_dsl")]
        pub predicate_bytes: Vec<u8>,
        /// Encoded selector used by `fast_dsl`.
        #[cfg(feature = "fast_dsl")]
        pub selector_bytes: Vec<u8>,
        /// Cursor and pagination parameters.
        pub params: QueryParams,
    }

    impl Clone for QueryWithParams {
        fn clone(&self) -> Self {
            use norito::codec::{Decode, Encode};

            let encoded = self.encode();
            let mut cursor: &[u8] = &encoded;
            Self::decode(&mut cursor).expect("QueryWithParams::clone: decode failed")
        }
    }

    type FastDslParts<'a> = (QueryItemKind, &'a [u8], &'a [u8], &'a [u8]);

    impl QueryWithParams {
        /// Convenience constructor from a boxed erased query and params.
        ///
        /// - In non-`fast_dsl` builds, this stores the provided `query` directly.
        /// - In `fast_dsl` builds, this extracts the predicate and selector from the
        ///   type-erased iterable query and records its item kind and payload.
        pub fn new(
            #[cfg(not(feature = "fast_dsl"))] query: QueryBox<QueryOutputBatchBox>,
            #[cfg(feature = "fast_dsl")] query: &QueryBox<QueryOutputBatchBox>,
            params: QueryParams,
        ) -> Self {
            #[cfg(not(feature = "fast_dsl"))]
            {
                Self { query, params }
            }
            #[cfg(feature = "fast_dsl")]
            {
                macro_rules! try_build {
                    ($item:ty, $kind:ident) => {
                        if let Some(erased) = super::iter_query_inner::<$item>(query) {
                            return Self {
                                query: (),
                                query_payload: erased.payload().to_vec(),
                                item: QueryItemKind::$kind,
                                predicate_bytes: norito::codec::Encode::encode(erased.predicate()),
                                selector_bytes: norito::codec::Encode::encode(erased.selector()),
                                params,
                            };
                        }
                    };
                }
                // Attempt for all supported item kinds
                try_build!(crate::domain::Domain, Domain);
                try_build!(crate::account::Account, Account);
                try_build!(crate::account::AccountId, AccountId);
                try_build!(crate::asset::value::Asset, Asset);
                try_build!(crate::asset::definition::AssetDefinition, AssetDefinition);
                try_build!(crate::nft::Nft, Nft);
                try_build!(crate::rwa::Rwa, Rwa);
                try_build!(crate::role::Role, Role);
                try_build!(crate::role::RoleId, RoleId);
                try_build!(crate::peer::PeerId, PeerId);
                try_build!(crate::trigger::TriggerId, TriggerId);
                try_build!(crate::trigger::Trigger, Trigger);
                try_build!(crate::query::CommittedTransaction, CommittedTransaction);
                try_build!(crate::block::SignedBlock, SignedBlock);
                try_build!(crate::block::BlockHeader, BlockHeader);
                try_build!(crate::proof::ProofRecord, ProofRecord);
                try_build!(crate::permission::Permission, Permission);
                try_build!(
                    crate::offline::OfflineAllowanceRecord,
                    OfflineAllowanceRecord
                );
                try_build!(
                    crate::offline::OfflineTransferRecord,
                    OfflineToOnlineTransfer
                );

                // Fallback: if unknown, leave everything empty/default and rely on server-side validation
                Self {
                    query: (),
                    query_payload: Vec::new(),
                    item: QueryItemKind::Domain, // default; will be rejected if used
                    predicate_bytes: Vec::new(),
                    selector_bytes: Vec::new(),
                    params,
                }
            }
        }
        /// Borrow the inner type-erased query box when available.
        ///
        /// In `fast_dsl` builds, the query payload is omitted and this returns `None`.
        pub fn query_box(&self) -> Option<&QueryBox<QueryOutputBatchBox>> {
            #[cfg(not(feature = "fast_dsl"))]
            {
                Some(&self.query)
            }
            #[cfg(feature = "fast_dsl")]
            {
                None
            }
        }

        /// Access the fast-DSL payload components when available.
        #[cfg(feature = "fast_dsl")]
        pub fn fast_dsl_parts(&self) -> Option<FastDslParts<'_>> {
            Some((
                self.item,
                &self.predicate_bytes,
                &self.selector_bytes,
                &self.query_payload,
            ))
        }

        /// Access the fast-DSL payload components when unavailable.
        #[cfg(not(feature = "fast_dsl"))]
        pub fn fast_dsl_parts(&self) -> Option<FastDslParts<'_>> {
            let _ = self;
            None
        }
    }

    /// Item kind tag used in `fast_dsl` builds to indicate the target iterable type.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(feature = "fast_dsl", derive(Decode, Encode, IntoSchema))]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Categories of query items used for pagination and filtering.
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    pub enum QueryItemKind {
        /// Domain items.
        Domain,
        /// Account items.
        Account,
        /// Account identifier items.
        AccountId,
        /// Asset items.
        Asset,
        /// Asset definition items.
        AssetDefinition,
        /// Repository agreement items.
        RepoAgreement,
        /// NFT items.
        Nft,
        /// RWA lot items.
        Rwa,
        /// Role items.
        Role,
        /// Role identifier items.
        RoleId,
        /// Peer identifier items.
        PeerId,
        /// Trigger identifier items.
        TriggerId,
        /// Trigger items.
        Trigger,
        /// Committed transaction items.
        CommittedTransaction,
        /// Signed block items.
        SignedBlock,
        /// Block header items.
        BlockHeader,
        /// Proof record items.
        ProofRecord,
        /// Permission items.
        Permission,
        /// Offline allowance records.
        OfflineAllowanceRecord,
        /// Offline-to-online transfer records.
        OfflineToOnlineTransfer,
        /// Offline counter summary records.
        OfflineCounterSummary,
        /// Offline verdict revocation records.
        OfflineVerdictRevocation,
    }

    /// Trait mapping item types to a `QueryItemKind` marker.
    ///
    /// In `fast_dsl` builds this is implemented for each supported item type
    /// and used to populate the item kind in requests. In non-`fast_dsl`
    /// builds this trait is a no-op blanket impl so that code can be generic
    /// over it without additional feature gating in bounds.
    pub trait ItemKindTag {
        #[cfg(feature = "fast_dsl")]
        /// Return the [`QueryItemKind`] discriminator for the implementing type.
        fn kind() -> QueryItemKind;
    }

    #[cfg(not(feature = "fast_dsl"))]
    impl<T> ItemKindTag for T {}

    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for Domain {
        fn kind() -> QueryItemKind {
            QueryItemKind::Domain
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for Account {
        fn kind() -> QueryItemKind {
            QueryItemKind::Account
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for AccountId {
        fn kind() -> QueryItemKind {
            QueryItemKind::AccountId
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for Asset {
        fn kind() -> QueryItemKind {
            QueryItemKind::Asset
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for AssetDefinition {
        fn kind() -> QueryItemKind {
            QueryItemKind::AssetDefinition
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for RepoAgreement {
        fn kind() -> QueryItemKind {
            QueryItemKind::RepoAgreement
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for Nft {
        fn kind() -> QueryItemKind {
            QueryItemKind::Nft
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for Rwa {
        fn kind() -> QueryItemKind {
            QueryItemKind::Rwa
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for Role {
        fn kind() -> QueryItemKind {
            QueryItemKind::Role
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for RoleId {
        fn kind() -> QueryItemKind {
            QueryItemKind::RoleId
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for PeerId {
        fn kind() -> QueryItemKind {
            QueryItemKind::PeerId
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for TriggerId {
        fn kind() -> QueryItemKind {
            QueryItemKind::TriggerId
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for Trigger {
        fn kind() -> QueryItemKind {
            QueryItemKind::Trigger
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for CommittedTransaction {
        fn kind() -> QueryItemKind {
            QueryItemKind::CommittedTransaction
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for SignedBlock {
        fn kind() -> QueryItemKind {
            QueryItemKind::SignedBlock
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for BlockHeader {
        fn kind() -> QueryItemKind {
            QueryItemKind::BlockHeader
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for crate::proof::ProofRecord {
        fn kind() -> QueryItemKind {
            QueryItemKind::ProofRecord
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for crate::permission::Permission {
        fn kind() -> QueryItemKind {
            QueryItemKind::Permission
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for crate::offline::OfflineAllowanceRecord {
        fn kind() -> QueryItemKind {
            QueryItemKind::OfflineAllowanceRecord
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for crate::offline::OfflineTransferRecord {
        fn kind() -> QueryItemKind {
            QueryItemKind::OfflineToOnlineTransfer
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for crate::offline::OfflineCounterSummary {
        fn kind() -> QueryItemKind {
            QueryItemKind::OfflineCounterSummary
        }
    }
    #[cfg(feature = "fast_dsl")]
    impl ItemKindTag for crate::offline::OfflineVerdictRevocation {
        fn kind() -> QueryItemKind {
            QueryItemKind::OfflineVerdictRevocation
        }
    }

    // Manual schema for QueryWithParams: represent only `params` field.
    impl iroha_schema::TypeId for QueryWithParams {
        fn id() -> String {
            "QueryWithParams".to_owned()
        }
    }

    impl iroha_schema::IntoSchema for QueryWithParams {
        fn type_name() -> String {
            "QueryWithParams".to_owned()
        }

        fn update_schema_map(map: &mut iroha_schema::MetaMap) {
            use iroha_schema::{Declaration, Metadata, NamedFieldsMeta};
            if !map.contains_key::<Self>() {
                map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                    declarations: vec![Declaration {
                        name: "params".to_owned(),
                        ty: core::any::TypeId::of::<QueryParams>(),
                    }],
                }));
                <QueryParams as iroha_schema::IntoSchema>::update_schema_map(map);
            }
        }
    }

    /// A query request that can be sent to an Iroha peer.
    ///
    /// In case of HTTP API, the query request must also be signed (see [`QueryRequestWithAuthority`] and [`SignedQuery`]).
    #[derive(Decode, Encode, IntoSchema)]
    pub enum QueryRequest {
        /// Singular query (non-iterable) request.
        Singular(SingularQueryBox),
        /// Start an iterable query with parameters.
        Start(QueryWithParams),
        /// Continue an iterable query from a cursor.
        Continue(ForwardCursor),
    }

    /// An enum containing either a singular or an iterable query
    #[derive(Decode, Encode, IntoSchema)]
    pub enum AnyQueryBox {
        /// Wrapped singular query.
        Singular(SingularQueryBox),
        /// Wrapped iterable query.
        Iterable(QueryWithParams),
    }

    /// A response to a [`QueryRequest`] from an Iroha peer
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Result returned by Torii in response to a query.
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    pub enum QueryResponse {
        /// Singular query output.
        Singular(SingularQueryOutputBox),
        /// Iterable query output.
        Iterable(QueryOutput),
    }

    /// A [`QueryRequest`], combined with an authority that wants to execute the query
    #[derive(Decode, Encode, IntoSchema)]
    pub struct QueryRequestWithAuthority {
        /// Account executing the query.
        pub authority: AccountId,
        /// Query payload.
        pub request: QueryRequest,
    }

    /// A signature of [`QueryRequestWithAuthority`] to be used in [`SignedQuery`]
    #[derive(Debug, Clone, PartialEq, Eq, IntoSchema)]
    /// Container type for `QuerySignature(pub` query data.
    pub struct QuerySignature(pub SignatureOf<QueryRequestWithAuthority>);

    #[cfg(not(feature = "ffi_import"))]
    impl<'a> norito::core::DecodeFromSlice<'a> for QuerySignature {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            let (signature, used) =
                <SignatureOf<QueryRequestWithAuthority> as norito::core::DecodeFromSlice>::decode_from_slice(
                    bytes,
                )?;
            Ok((QuerySignature(signature), used))
        }
    }

    #[cfg(not(feature = "ffi_import"))]
    impl norito::core::NoritoSerialize for QuerySignature {
        fn schema_hash() -> [u8; 16] {
            <SignatureOf<QueryRequestWithAuthority> as norito::core::NoritoSerialize>::schema_hash()
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            norito::core::NoritoSerialize::encoded_len_hint(&self.0)
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            norito::core::NoritoSerialize::encoded_len_exact(&self.0)
        }

        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
            norito::core::NoritoSerialize::serialize(&self.0, writer)
        }
    }

    #[cfg(not(feature = "ffi_import"))]
    impl<'de> norito::core::NoritoDeserialize<'de> for QuerySignature {
        fn schema_hash() -> [u8; 16] {
            <SignatureOf<QueryRequestWithAuthority> as norito::core::NoritoDeserialize>::schema_hash(
            )
        }

        fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
            let as_sig = archived.cast::<SignatureOf<QueryRequestWithAuthority>>();
            let sig = <SignatureOf<QueryRequestWithAuthority> as norito::core::NoritoDeserialize>::deserialize(as_sig);
            QuerySignature(sig)
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::FastJsonWrite for QuerySignature {
        fn write_json(&self, out: &mut String) {
            let encoded = super::json_wrappers::base64_encode(self.0.payload());
            norito::json::write_json_string(&encoded, out);
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonDeserialize for QuerySignature {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            let encoded = parser.parse_string()?;
            let bytes = super::json_wrappers::base64_decode(&encoded).map_err(|()| {
                norito::json::Error::InvalidField {
                    field: String::from("QuerySignature"),
                    message: String::from("invalid base64 signature payload"),
                }
            })?;
            let signature = iroha_crypto::Signature::from_bytes(&bytes);
            Ok(QuerySignature(SignatureOf::from_signature(signature)))
        }
    }

    /// A signed and authorized query request
    #[derive(Encode, IntoSchema)]
    pub struct SignedQuery {
        pub signature: QuerySignature,
        pub payload: QueryRequestWithAuthority,
    }

    /// Response returned by [`FindTransactions`] query.
    #[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    /// Snapshot representing a transaction committed to the ledger.
    pub struct CommittedTransaction {
        /// Hash of the block containing this transaction.
        pub block_hash: HashOf<BlockHeader>,
        /// Hash of the transaction entrypoint.
        pub entrypoint_hash: HashOf<TransactionEntrypoint>,
        /// Merkle inclusion proof for the transaction entrypoint.
        pub entrypoint_proof: MerkleProof<TransactionEntrypoint>,
        /// The initial execution step of the transaction.
        pub entrypoint: TransactionEntrypoint,
        /// Hash of the transaction result.
        pub result_hash: HashOf<TransactionResult>,
        /// Merkle inclusion proof for the transaction result.
        pub result_proof: MerkleProof<TransactionResult>,
        /// The result of executing the transaction (trigger sequence or rejection).
        pub result: TransactionResult,
    }
}

// Server-side predicate support for CommittedTransaction (feature-gated on std)
/// Filters applied when matching committed transactions.
#[derive(Clone, Default, Encode, Decode)]
pub struct CommittedTxFilters {
    /// Require the authority to equal the provided account.
    pub authority_eq: Option<crate::account::AccountId>,
    /// Require the authority to differ from the provided account.
    pub authority_ne: Option<crate::account::AccountId>,
    /// Require the authority to be one of the listed accounts.
    pub authority_in: std::vec::Vec<crate::account::AccountId>,
    /// Require the authority to be absent from the listed accounts.
    pub authority_nin: std::vec::Vec<crate::account::AccountId>,
    /// Require the presence (`true`) or absence (`false`) of an authority.
    pub authority_exists: Option<bool>,
    /// Require the transaction timestamp (ms) to be greater than or equal to this bound.
    pub ts_ge: Option<u64>,
    /// Require the transaction timestamp (ms) to be less than or equal to this bound.
    pub ts_le: Option<u64>,
    /// Require the entrypoint hash to equal the provided hash.
    pub entry_eq: Option<HashOf<crate::transaction::signed::TransactionEntrypoint>>,
    /// Require the entrypoint hash to be one of the listed hashes.
    pub entry_in: std::vec::Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>>,
    /// Require the entrypoint hash to differ from the provided hash.
    pub entry_ne: Option<HashOf<crate::transaction::signed::TransactionEntrypoint>>,
    /// Require the entrypoint hash to be absent from the listed hashes.
    pub entry_nin: std::vec::Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>>,
    /// Require the presence (`true`) or absence (`false`) of an entrypoint hash.
    pub entry_exists: Option<bool>,
    /// Require the execution result to be `Ok` (`true`) or `Err` (`false`).
    pub result_ok: Option<bool>,
    /// Require the execution result to differ from the provided outcome.
    pub result_ok_ne: Option<bool>,
    /// Require the execution result to be one of the listed boolean outcomes.
    pub result_ok_in: std::vec::Vec<bool>,
    /// Require the execution result to be absent from the listed boolean outcomes.
    pub result_ok_nin: std::vec::Vec<bool>,
    /// Require whether a result is present (`true`) or absent (`false`).
    pub result_exists: Option<bool>,
}

impl CommittedTxFilters {
    /// Helper associated with query processing.
    #[allow(clippy::too_many_lines)]
    pub fn applies(&self, tx: &CommittedTransaction) -> bool {
        let authority_val = tx.entrypoint.authority_opt().cloned();
        if let Some(required) = self.authority_exists
            && (required != authority_val.is_some())
        {
            return false;
        }
        if self
            .authority_eq
            .as_ref()
            .is_some_and(|eq| authority_val.as_ref() != Some(eq))
        {
            return false;
        }
        if self
            .authority_ne
            .as_ref()
            .is_some_and(|ne| authority_val.as_ref() == Some(ne))
        {
            return false;
        }
        if !self.authority_in.is_empty()
            && !authority_val
                .as_ref()
                .is_some_and(|a| self.authority_in.iter().any(|x| x == a))
        {
            return false;
        }
        if !self.authority_nin.is_empty()
            && authority_val
                .as_ref()
                .is_some_and(|a| self.authority_nin.iter().any(|x| x == a))
        {
            return false;
        }
        // timestamp lower bound
        if let Some(ge) = self.ts_ge {
            let created_ms = tx.entrypoint.creation_time_ms().unwrap_or(0);
            if created_ms < ge {
                return false;
            }
        }
        // timestamp upper bound
        if let Some(le) = self.ts_le {
            let created_ms = tx.entrypoint.creation_time_ms().unwrap_or(u64::MAX);
            if created_ms > le {
                return false;
            }
        }
        // entrypoint hash
        if self
            .entry_eq
            .as_ref()
            .is_some_and(|eq| &tx.entrypoint_hash != eq)
        {
            return false;
        }
        if self
            .entry_ne
            .as_ref()
            .is_some_and(|ne| &tx.entrypoint_hash == ne)
        {
            return false;
        }
        if let Some(required) = self.entry_exists {
            // Entrypoint hash always exists for committed transactions.
            // If the predicate requires non-existence, this cannot match.
            if !required {
                return false;
            }
        }
        if !self.entry_in.is_empty() && !self.entry_in.iter().any(|h| h == &tx.entrypoint_hash) {
            return false;
        }
        if !self.entry_nin.is_empty() && self.entry_nin.iter().any(|h| h == &tx.entrypoint_hash) {
            return false;
        }
        // result_ok
        if let Some(required) = self.result_exists {
            // Result is always present for committed transactions; require true.
            if !required {
                return false;
            }
        }
        if let Some(ok) = self.result_ok {
            let actual = tx.result.as_ref().is_ok();
            if actual != ok {
                return false;
            }
        }
        if let Some(ne) = self.result_ok_ne {
            let actual = tx.result.as_ref().is_ok();
            if actual == ne {
                return false;
            }
        }
        if !self.result_ok_in.is_empty() {
            let actual = tx.result.as_ref().is_ok();
            if !self.result_ok_in.contains(&actual) {
                return false;
            }
        }
        if !self.result_ok_nin.is_empty() {
            let actual = tx.result.as_ref().is_ok();
            if self.result_ok_nin.contains(&actual) {
                return false;
            }
        }
        true
    }
}

impl crate::seal::SingularQuery for SingularQueryBox {}

/// A type-erased iterable query retaining its predicate and selector.
///
/// `ErasedIterQuery` allows storing queries with different concrete types in a
/// uniform container. Consumers can later attempt to recover the underlying
/// `QueryWithFilter` using [`iter_query_inner`].
#[derive(Debug, Clone, Decode, Encode, IntoSchema)]
pub struct ErasedIterQuery<T>
where
    T: HasProjection<PredicateMarker> + HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
{
    predicate: CompoundPredicate<T>,
    selector: SelectorTuple<T>,
    /// Opaque bytes of the original concrete query (e.g., `FindAccounts`, `FindAccountsWithAsset`).
    ///
    /// The server uses this payload to reconstruct the concrete query when
    /// executing on the node, so variant-specific parameters (like
    /// `asset_definition` for `FindAccountsWithAsset`) are preserved.
    payload: Vec<u8>,
}

/// Attempt to extract a concrete `&QueryWithFilter<T>` from a type-erased iterable query.
///
/// This enables downstream crates (e.g., visitor utilities) to dispatch on the
/// concrete query type without depending on private wrapper internals.
// Helper: downcast a type-erased iterable query to a concrete erased form
/// Helper associated with query processing.
pub fn iter_query_inner<T>(q: &QueryBox<QueryOutputBatchBox>) -> Option<&ErasedIterQuery<T>>
where
    T: HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + Send
        + Sync
        + 'static,
{
    let any: &dyn Any = &**q;
    any.downcast_ref::<ErasedIterQuery<T>>()
}

impl<T> seal::Query for ErasedIterQuery<T> where
    T: HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + Send
        + Sync
        + 'static
{
}

impl<T> Query for ErasedIterQuery<T>
where
    T: HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + Send
        + Sync
        + 'static,
{
    type Item = QueryOutputBatchBox;

    fn dyn_encode(&self) -> Vec<u8> {
        // Encode the erased iterable query wrapper itself so that decoding via
        // the registry receives the full structure (predicate, selector, payload).
        norito::codec::Encode::encode(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T> ErasedIterQuery<T>
where
    T: HasProjection<PredicateMarker> + HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
{
    /// Construct from parts.
    pub fn new(
        predicate: CompoundPredicate<T>,
        selector: SelectorTuple<T>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            predicate,
            selector,
            payload,
        }
    }
    /// Borrow the stored predicate
    pub fn predicate(&self) -> &CompoundPredicate<T> {
        &self.predicate
    }
    /// Borrow the stored selector
    pub fn selector(&self) -> &SelectorTuple<T> {
        &self.selector
    }
    /// Cloned predicate value
    pub fn predicate_cloned(&self) -> CompoundPredicate<T> {
        self.predicate.clone()
    }
    /// Cloned selector value
    pub fn selector_cloned(&self) -> SelectorTuple<T> {
        self.selector.clone()
    }

    /// Borrow the encoded payload of the original concrete query.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

// NOTE: Projection traits for QueryOutputBatchBox are provided generically in the DSL.

impl<T> From<QueryWithFilter<T>> for QueryBox<QueryOutputBatchBox>
where
    T: HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + Send
        + Sync
        + 'static,
{
    fn from(query: QueryWithFilter<T>) -> Self {
        // Best-effort: downcast the erased query to known concrete types to
        // capture its encoded payload. If downcast fails (e.g., in fast_dsl
        // builds), fall back to an empty payload.
        #[allow(unused_mut)]
        let mut payload: Vec<u8> = Vec::new();
        // NOTE: We cannot introspect the actual concrete type from the trait
        // object here due to object-safety constraints (`as_any` requires Sized).
        // Leave the payload empty in this construction path; the primary
        // builder path (`QueryBuilder::execute`) preserves the payload.
        Box::new(ErasedIterQuery::new(
            query.predicate,
            query.selector,
            payload,
        ))
    }
}

#[cfg(feature = "fault_injection")]
impl CommittedTransaction {
    /// Injects a set of fictitious instructions into the transaction payload to simulate tampering.
    ///
    /// Only available when the `fault_injection` feature is enabled.
    pub fn inject_instructions(
        &mut self,
        extra_instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
    ) {
        let additions: Vec<InstructionBox> =
            extra_instructions.into_iter().map(Into::into).collect();

        if additions.is_empty() {
            return;
        }

        match &mut self.entrypoint {
            TransactionEntrypoint::External(entrypoint) => {
                entrypoint.inject_instructions(additions.clone());
            }
            TransactionEntrypoint::PrivateKaigi(entrypoint) => {
                entrypoint.inject_instructions(additions.clone());
            }
            TransactionEntrypoint::Time(entrypoint) => {
                let mut modified = entrypoint.instructions.0.clone().into_vec();
                modified.extend(additions);
                entrypoint.instructions = ExecutionStep(modified.into());
            }
        }
        // Update the leaf hash to match the tampered entrypoint.
        self.entrypoint_hash = self.entrypoint.hash();
    }

    /// Swaps the transaction result between `Ok` and `Err` to simulate tampering.
    ///
    /// Only available when the `fault_injection` feature is enabled.
    pub fn swap_result(&mut self) {
        let TransactionResult(result) = &mut self.result;
        *result = if result.is_ok() {
            Err(TransactionRejectionReason::Validation(
                ValidationFail::InternalError("result swapped".into()),
            ))
        } else {
            Ok(Vec::new())
        };
        // Update the leaf hash to match the tampered result.
        self.result_hash = self.result.hash();
    }
}

impl QueryOutputBatchBox {
    // this is used in client cli to do type-erased iterable queries
    /// Extends this batch with another batch of the same type
    ///
    /// # Panics
    ///
    /// Panics if the types of the two batches do not match
    pub fn extend(&mut self, other: QueryOutputBatchBox) {
        match (self, other) {
            (Self::PublicKey(v1), Self::PublicKey(v2)) => v1.extend(v2),
            (Self::String(v1), Self::String(v2)) => v1.extend(v2),
            (Self::Metadata(v1), Self::Metadata(v2)) => v1.extend(v2),
            (Self::Numeric(v1), Self::Numeric(v2)) => v1.extend(v2),
            (Self::Name(v1), Self::Name(v2)) => v1.extend(v2),
            (Self::DomainId(v1), Self::DomainId(v2)) => v1.extend(v2),
            (Self::Domain(v1), Self::Domain(v2)) => v1.extend(v2),
            (Self::AccountId(v1), Self::AccountId(v2)) => v1.extend(v2),
            (Self::Account(v1), Self::Account(v2)) => v1.extend(v2),
            (Self::AssetId(v1), Self::AssetId(v2)) => v1.extend(v2),
            (Self::Asset(v1), Self::Asset(v2)) => v1.extend(v2),
            (Self::AssetDefinitionId(v1), Self::AssetDefinitionId(v2)) => v1.extend(v2),
            (Self::AssetDefinition(v1), Self::AssetDefinition(v2)) => v1.extend(v2),
            (Self::NftId(v1), Self::NftId(v2)) => v1.extend(v2),
            (Self::Nft(v1), Self::Nft(v2)) => v1.extend(v2),
            (Self::RwaId(v1), Self::RwaId(v2)) => v1.extend(v2),
            (Self::Rwa(v1), Self::Rwa(v2)) => v1.extend(v2),
            (Self::Role(v1), Self::Role(v2)) => v1.extend(v2),
            (Self::Parameter(v1), Self::Parameter(v2)) => v1.extend(v2),
            (Self::Permission(v1), Self::Permission(v2)) => v1.extend(v2),
            (Self::CommittedTransaction(v1), Self::CommittedTransaction(v2)) => v1.extend(v2),
            (Self::TransactionResult(v1), Self::TransactionResult(v2)) => v1.extend(v2),
            (Self::TransactionResultHash(v1), Self::TransactionResultHash(v2)) => v1.extend(v2),
            (Self::TransactionEntrypoint(v1), Self::TransactionEntrypoint(v2)) => v1.extend(v2),
            (Self::TransactionEntrypointHash(v1), Self::TransactionEntrypointHash(v2)) => {
                v1.extend(v2)
            }
            (Self::Peer(v1), Self::Peer(v2)) => v1.extend(v2),
            (Self::RoleId(v1), Self::RoleId(v2)) => v1.extend(v2),
            (Self::TriggerId(v1), Self::TriggerId(v2)) => v1.extend(v2),
            (Self::Trigger(v1), Self::Trigger(v2)) => v1.extend(v2),
            (Self::Action(v1), Self::Action(v2)) => v1.extend(v2),
            (Self::Block(v1), Self::Block(v2)) => v1.extend(v2),
            (Self::BlockHeader(v1), Self::BlockHeader(v2)) => v1.extend(v2),
            (Self::BlockHeaderHash(v1), Self::BlockHeaderHash(v2)) => v1.extend(v2),
            (Self::RepoAgreement(v1), Self::RepoAgreement(v2)) => v1.extend(v2),
            (Self::OfflineAllowanceRecord(v1), Self::OfflineAllowanceRecord(v2)) => v1.extend(v2),
            (Self::OfflineToOnlineTransfer(v1), Self::OfflineToOnlineTransfer(v2)) => v1.extend(v2),
            (Self::OfflineCounterSummary(v1), Self::OfflineCounterSummary(v2)) => v1.extend(v2),
            (Self::OfflineVerdictRevocation(v1), Self::OfflineVerdictRevocation(v2)) => {
                v1.extend(v2)
            }
            _ => panic!("Cannot extend different types of IterableQueryOutputBatchBox"),
        }
    }

    /// Returns length of this batch
    #[allow(clippy::len_without_is_empty)] // having a len without `is_empty` is fine, we don't return empty batches
    pub fn len(&self) -> usize {
        match self {
            Self::PublicKey(v) => v.len(),
            Self::String(v) => v.len(),
            Self::Metadata(v) => v.len(),
            Self::Json(v) => v.len(),
            Self::Numeric(v) => v.len(),
            Self::Name(v) => v.len(),
            Self::DomainId(v) => v.len(),
            Self::Domain(v) => v.len(),
            Self::AccountId(v) => v.len(),
            Self::Account(v) => v.len(),
            Self::AssetId(v) => v.len(),
            Self::Asset(v) => v.len(),
            Self::AssetDefinitionId(v) => v.len(),
            Self::AssetDefinition(v) => v.len(),
            Self::NftId(v) => v.len(),
            Self::Nft(v) => v.len(),
            Self::RwaId(v) => v.len(),
            Self::Rwa(v) => v.len(),
            Self::Role(v) => v.len(),
            Self::Parameter(v) => v.len(),
            Self::Permission(v) => v.len(),
            Self::CommittedTransaction(v) => v.len(),
            Self::TransactionResult(v) => v.len(),
            Self::TransactionResultHash(v) => v.len(),
            Self::TransactionEntrypoint(v) => v.len(),
            Self::TransactionEntrypointHash(v) => v.len(),
            Self::Peer(v) => v.len(),
            Self::RoleId(v) => v.len(),
            Self::TriggerId(v) => v.len(),
            Self::Trigger(v) => v.len(),
            Self::Action(v) => v.len(),
            Self::Block(v) => v.len(),
            Self::BlockHeader(v) => v.len(),
            Self::BlockHeaderHash(v) => v.len(),
            Self::ProofRecord(v) => v.len(),
            Self::RepoAgreement(v) => v.len(),
            Self::OfflineAllowanceRecord(v) => v.len(),
            Self::OfflineToOnlineTransfer(v) => v.len(),
            Self::OfflineCounterSummary(v) => v.len(),
            Self::OfflineVerdictRevocation(v) => v.len(),
        }
    }
}

impl QueryOutputBatchBoxTuple {
    /// Extends this batch tuple with another batch tuple of the same type
    ///
    /// # Panics
    ///
    /// Panics if the types or lengths of the two batche tuples do not match
    pub fn extend(&mut self, other: Self) {
        assert_eq!(
            self.tuple.len(),
            other.tuple.len(),
            "Cannot extend QueryOutputBatchBoxTuple with different number of elements"
        );

        self.tuple
            .iter_mut()
            .zip(other)
            .for_each(|(self_batch, other_batch)| self_batch.extend(other_batch));
    }

    /// Returns length of this batch tuple
    // This works under assumption that all batches in the tuples have the same length, which should be true for iroha
    /// Helper associated with query processing.
    pub fn len(&self) -> usize {
        self.tuple[0].len()
    }

    /// Returns `true` if this batch tuple is empty
    pub fn is_empty(&self) -> bool {
        self.tuple[0].len() == 0
    }

    /// Returns an iterator over the batches in this tuple
    pub fn iter(&self) -> impl Iterator<Item = &QueryOutputBatchBox> {
        self.tuple.iter()
    }
}

impl IntoIterator for QueryOutputBatchBoxTuple {
    type Item = QueryOutputBatchBox;
    type IntoIter = QueryOutputBatchBoxIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        QueryOutputBatchBoxIntoIter(self.tuple.into_iter())
    }
}

/// An iterator over the batches in a [`QueryOutputBatchBoxTuple`]
pub struct QueryOutputBatchBoxIntoIter(vec::IntoIter<QueryOutputBatchBox>);

impl Iterator for QueryOutputBatchBoxIntoIter {
    type Item = QueryOutputBatchBox;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl SingularQuery for SingularQueryBox {
    type Output = SingularQueryOutputBox;

    fn dyn_encode(&self) -> Vec<u8> {
        self.encode()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl QueryOutput {
    /// Create a new [`QueryOutput`] from the iroha response parts.
    pub fn new(
        batch: QueryOutputBatchBoxTuple,
        remaining_items: u64,
        continue_cursor: Option<ForwardCursor>,
    ) -> Self {
        Self {
            batch,
            remaining_items,
            continue_cursor,
        }
    }

    /// Split this [`QueryOutput`] into its constituent parts.
    pub fn into_parts(self) -> (QueryOutputBatchBoxTuple, u64, Option<ForwardCursor>) {
        (self.batch, self.remaining_items, self.continue_cursor)
    }
}

impl QueryRequest {
    /// Construct a [`QueryRequestWithAuthority`] from this [`QueryRequest`] and an authority
    pub fn with_authority(self, authority: AccountId) -> QueryRequestWithAuthority {
        QueryRequestWithAuthority {
            authority,
            request: self,
        }
    }
}

impl QueryWithParams {
    /// Borrow the parameters attached to this iterable query request.
    #[must_use]
    pub fn params(&self) -> &QueryParams {
        &self.params
    }
}

impl QueryRequestWithAuthority {
    /// Return the authority that issued this request.
    #[must_use]
    pub fn authority(&self) -> &AccountId {
        &self.authority
    }

    /// Return the underlying query payload.
    #[must_use]
    pub fn request(&self) -> &QueryRequest {
        &self.request
    }

    /// Consume `self`, returning its components.
    #[must_use]
    pub fn into_parts(self) -> (AccountId, QueryRequest) {
        (self.authority, self.request)
    }

    /// Sign this [`QueryRequestWithAuthority`], creating a [`SignedQuery`]
    #[inline]
    #[must_use]
    pub fn sign(self, key_pair: &iroha_crypto::KeyPair) -> SignedQuery {
        let signature = SignatureOf::new(key_pair.private_key(), &self);

        SignedQuery {
            signature: QuerySignature(signature),
            payload: self,
        }
    }
}

impl SignedQuery {
    /// Get authority that has signed this query
    pub fn authority(&self) -> &AccountId {
        &self.payload.authority
    }

    /// Get the request that was signed
    pub fn request(&self) -> &QueryRequest {
        &self.payload.request
    }
}

mod candidate {
    use super::*;

    #[derive(Encode, Decode)]
    struct SignedQueryCandidate {
        signature: QuerySignature,
        payload: QueryRequestWithAuthority,
    }

    impl SignedQueryCandidate {
        fn validate(self) -> Result<SignedQuery, &'static str> {
            let QuerySignature(signature) = &self.signature;
            signature
                .verify(self.payload.authority.signatory(), &self.payload)
                .map_err(|_| "Query request signature is not valid")?;

            Ok(SignedQuery {
                payload: self.payload,
                signature: self.signature,
            })
        }
    }

    impl<'de> norito::core::NoritoDeserialize<'de> for SignedQuery {
        fn deserialize(archived: &'de norito::core::Archived<SignedQuery>) -> Self {
            let candidate = <SignedQueryCandidate as norito::core::NoritoDeserialize>::deserialize(
                archived.cast(),
            );
            candidate.validate().expect("invalid SignedQuery")
        }
    }

    // JSON deserialization for SignedQuery is disabled in non-json builds.

    #[cfg(test)]
    mod tests {
        use std::sync::LazyLock;

        use iroha_crypto::KeyPair;
        #[cfg(feature = "json")]
        use norito::json;

        use crate::{
            account::AccountId,
            query::{
                FindExecutorDataModel, QueryRequest, SingularQueryBox,
                candidate::SignedQueryCandidate, parameters,
            },
        };

        #[cfg(feature = "json")]
        #[test]
        fn query_with_params_json_defaults_optional_fields() {
            let params = parameters::QueryParams::default();
            let mut map = json::Map::new();
            map.insert(
                "params".to_owned(),
                json::to_value(&params).expect("params to JSON"),
            );
            let value = json::Value::Object(map);

            let parsed: super::json_wrappers::QueryWithParamsJson =
                json::from_value(value).expect("missing optional fields must default");

            assert!(parsed.wire.is_none());
            assert!(parsed.payload_b64.is_none());
            #[cfg(feature = "fast_dsl")]
            {
                assert!(parsed.item_kind.is_none());
                assert!(parsed.query_payload_b64.is_none());
                assert!(parsed.predicate_b64.is_none());
                assert!(parsed.selector_b64.is_none());
            }
        }

        static ALICE_ID: LazyLock<AccountId> =
            LazyLock::new(|| AccountId::new(ALICE_KEYPAIR.public_key().clone()));
        static ALICE_KEYPAIR: LazyLock<KeyPair> = LazyLock::new(|| {
            KeyPair::new(
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                    .parse()
                    .unwrap(),
                "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                    .parse()
                    .unwrap(),
            )
            .unwrap()
        });

        static BOB_KEYPAIR: LazyLock<KeyPair> = LazyLock::new(|| {
            KeyPair::new(
                "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
                    .parse()
                    .unwrap(),
                "802620AF3F96DEEF44348FEB516C057558972CEC4C75C4DB9C5B3AAC843668854BF828"
                    .parse()
                    .unwrap(),
            )
            .unwrap()
        });

        #[test]
        fn valid() {
            let signed_query = QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            ))
            .with_authority(ALICE_ID.clone())
            .sign(&ALICE_KEYPAIR);

            let candidate = SignedQueryCandidate {
                signature: signed_query.signature,
                payload: signed_query.payload,
            };

            candidate.validate().unwrap();
        }

        #[test]
        fn invalid_signature() {
            let signed_query = QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            ))
            .with_authority(ALICE_ID.clone())
            .sign(&ALICE_KEYPAIR);

            let mut candidate = SignedQueryCandidate {
                signature: signed_query.signature,
                payload: signed_query.payload,
            };

            // Corrupt the raw signature payload and rebuild the signature
            let mut sig_bytes = candidate.signature.0.payload().to_vec();
            let idx = sig_bytes.len() - 1;
            sig_bytes[idx] = sig_bytes[idx].wrapping_add(1);
            *candidate.signature.0 = iroha_crypto::Signature::from_bytes(&sig_bytes);

            let err = candidate
                .validate()
                .err()
                .expect("expected signature validation to fail");
            assert_eq!(err, "Query request signature is not valid");
        }

        #[test]
        fn mismatching_authority() {
            let signed_query = QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            ))
            // signing with a wrong key here
            .with_authority(ALICE_ID.clone())
            .sign(&BOB_KEYPAIR);

            let candidate = SignedQueryCandidate {
                signature: signed_query.signature,
                payload: signed_query.payload,
            };

            let err = candidate
                .validate()
                .err()
                .expect("expected signature validation to fail");
            assert_eq!(err, "Query request signature is not valid");
        }
    }
}

#[cfg(test)]
mod json_roundtrip_tests {
    use std::sync::LazyLock;

    use iroha_crypto::KeyPair;

    use super::*;
    #[cfg(not(feature = "fast_dsl"))]
    use crate::query::domain::prelude::FindDomains;
    use crate::{
        account::AccountId,
        domain::Domain,
        query::{
            executor::prelude::FindParameters,
            json_wrappers::{SignedQueryJson, query_request_from_json, query_request_to_json},
        },
    };

    static ALICE_ID: LazyLock<AccountId> =
        LazyLock::new(|| AccountId::new(ALICE_KEYPAIR.public_key().clone()));
    static ALICE_KEYPAIR: LazyLock<KeyPair> = LazyLock::new(|| {
        KeyPair::new(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap(),
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap(),
        )
        .unwrap()
    });

    #[test]
    fn query_request_json_roundtrip_singular() {
        let req = QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters));
        let json = query_request_to_json(&req);
        let back = query_request_from_json(json).expect("json->request");
        match back {
            QueryRequest::Singular(SingularQueryBox::FindParameters(_)) => {}
            _ => panic!("expected FindParameters singular query"),
        }
    }

    #[cfg(not(feature = "fast_dsl"))]
    #[test]
    fn signed_query_json_roundtrip_start_non_fastdsl() {
        // Initialize a minimal registry for erased domain iterable queries
        set_query_registry(crate::query_registry![ErasedIterQuery<Domain>]);

        // Build a simple iterable query box for domains
        let qwf: QueryWithFilter<Domain> = QueryWithFilter::new_with_query(
            Box::new(FindDomains),
            CompoundPredicate::PASS,
            SelectorTuple::default(),
        );
        let qb: QueryBox<QueryOutputBatchBox> = qwf.into();
        let query_with_params = QueryWithParams {
            query: qb,
            params: parameters::QueryParams::default(),
        };

        let signed = QueryRequest::Start(query_with_params)
            .with_authority(ALICE_ID.clone())
            .sign(&ALICE_KEYPAIR);

        // Wrap to JSON and back
        let json = SignedQueryJson::from(&signed);
        let back = SignedQuery::try_from(json).expect("json->native");

        // Check shape
        match back.request() {
            QueryRequest::Start(q) => {
                // Should reconstruct a boxed query in non-fast_dsl build
                let qb = q.query_box().expect("query box present");
                let ty = (**qb).type_name_key();
                assert!(ty.contains("ErasedIterQuery"));
            }
            _ => panic!("expected Start request"),
        }
    }

    #[cfg(feature = "fast_dsl")]
    #[test]
    fn signed_query_json_roundtrip_start_fastdsl() {
        // Construct QueryWithParams directly with payload components
        use crate::query::QueryItemKind;
        let pred = norito::codec::Encode::encode(&CompoundPredicate::<Domain>::PASS);
        let sel = norito::codec::Encode::encode(&SelectorTuple::<Domain>::default());
        let qwp = QueryWithParams {
            query: (),
            query_payload: vec![1, 2, 3],
            item: QueryItemKind::Domain,
            predicate_bytes: pred,
            selector_bytes: sel,
            params: parameters::QueryParams::default(),
        };
        let signed = QueryRequest::Start(qwp)
            .with_authority(ALICE_ID.clone())
            .sign(&ALICE_KEYPAIR);

        let json = SignedQueryJson::from(&signed);
        let back = SignedQuery::try_from(json).expect("json->native");
        match back.request() {
            QueryRequest::Start(q) => {
                // fast_dsl carries () for query and separate payloads
                let _ = (&q.predicate_bytes, &q.selector_bytes, q.item);
            }
            _ => panic!("expected Start request"),
        }
    }
}
/// Use a custom syntax to implement [`Query`] for applicable types
macro_rules! impl_iter_queries {
    ($ty:ty => $item:ty $(, $($rest:tt)*)?) => {
        impl seal::Query for $ty {}
        impl Query for $ty {
            type Item = $item;

            fn dyn_encode(&self) -> Vec<u8> {
                self.encode()
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        $(
            impl_iter_queries!($($rest)*);
        )?
    };
    // allow for a trailing comma
    () => {}
}

/// Use a custom syntax to implement [`SingularQueries`] for applicable types
macro_rules! impl_singular_queries {
    ($ty:ty => $output:ty $(, $($rest:tt)*)?) => {
        impl seal::SingularQuery for $ty {}
        impl SingularQuery for $ty {
            type Output = $output;

            fn dyn_encode(&self) -> Vec<u8> {
                self.encode()
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        $(
            impl_singular_queries!($($rest)*);
        )?
    };
    // allow for a trailing comma
    () => {}
}

impl_iter_queries! {
    FindRoles => crate::role::Role,
    FindRoleIds => crate::role::RoleId,
    FindRolesByAccountId => crate::role::RoleId,
    FindPermissionsByAccountId => crate::permission::Permission,
    FindAccounts => crate::account::Account,
    FindAccountIds => crate::account::AccountId,
    FindAssets => crate::asset::value::Asset,
    FindAssetsDefinitions => crate::asset::definition::AssetDefinition,
    repo::FindRepoAgreements => crate::repo::RepoAgreement,
    FindNfts => crate::nft::Nft,
    FindRwas => crate::rwa::Rwa,
    FindDomains => crate::domain::Domain,
    FindPeers => crate::peer::PeerId,
    FindActiveTriggerIds => crate::trigger::TriggerId,
    FindTriggers => crate::trigger::Trigger,
    offline::FindOfflineAllowances => crate::offline::OfflineAllowanceRecord,
    offline::FindOfflineAllowanceByCertificateId => crate::offline::OfflineAllowanceRecord,
    offline::FindOfflineCounterSummaries => crate::offline::OfflineCounterSummary,
    offline::FindOfflineToOnlineTransfers => crate::offline::OfflineTransferRecord,
    offline::FindOfflineToOnlineTransfersByController => crate::offline::OfflineTransferRecord,
    offline::FindOfflineToOnlineTransfersByReceiver => crate::offline::OfflineTransferRecord,
    offline::FindOfflineToOnlineTransfersByStatus => crate::offline::OfflineTransferRecord,
    offline::FindOfflineToOnlineTransfersByPolicy => crate::offline::OfflineTransferRecord,
    offline::FindOfflineToOnlineTransferById => crate::offline::OfflineTransferRecord,
    offline::FindOfflineVerdictRevocations => crate::offline::OfflineVerdictRevocation,
    FindTransactions => CommittedTransaction,
    FindAccountsWithAsset => crate::account::Account,
    FindBlockHeaders => crate::block::BlockHeader,
    FindBlocks => SignedBlock,
    proof::prelude::FindProofRecords => crate::proof::ProofRecord,
    proof::prelude::FindProofRecordsByBackend => crate::proof::ProofRecord,
    proof::prelude::FindProofRecordsByStatus => crate::proof::ProofRecord,
}

impl_singular_queries! {
    FindParameters => crate::parameter::Parameters,
    FindExecutorDataModel => crate::executor::ExecutorDataModel,
    account::prelude::FindAccountById => crate::account::Account,
    account::prelude::FindAliasesByAccountId => Vec<account::AccountAliasBindingRecord>,
    account::prelude::FindAccountRecoveryPolicyByAlias => crate::account::AccountRecoveryPolicy,
    account::prelude::FindAccountRecoveryRequestByAlias => crate::account::AccountRecoveryRequest,
    proof::prelude::FindProofRecordById => crate::proof::ProofRecord,
    smart_contract::prelude::FindContractManifestByCodeHash => crate::smart_contract::manifest::ContractManifest,
    runtime::prelude::FindAbiVersion => crate::query::runtime::AbiVersion,
    asset::prelude::FindAssetById => crate::asset::value::Asset,
    asset::prelude::FindAssetDefinitionById => crate::asset::definition::AssetDefinition,
    trigger::prelude::FindTriggerById => crate::trigger::Trigger,
    oracle::FindTwitterBindingByHash => crate::oracle::TwitterBindingRecord,
    endorsement::prelude::FindDomainEndorsements => Vec<crate::nexus::DomainEndorsementRecord>,
    endorsement::prelude::FindDomainEndorsementPolicy => crate::nexus::DomainEndorsementPolicy,
    endorsement::prelude::FindDomainCommittee => crate::nexus::DomainCommittee,
    da::prelude::FindDaPinIntentByTicket => crate::da::pin_intent::DaPinIntentWithLocation,
    da::prelude::FindDaPinIntentByManifest => crate::da::pin_intent::DaPinIntentWithLocation,
    da::prelude::FindDaPinIntentByAlias => crate::da::pin_intent::DaPinIntentWithLocation,
    da::prelude::FindDaPinIntentByLaneEpochSequence => crate::da::pin_intent::DaPinIntentWithLocation,
    nexus::prelude::FindLaneRelayEnvelopeByRef => crate::nexus::VerifiedLaneRelayRecord,
    sns::prelude::FindDataspaceNameOwnerById => crate::account::AccountId,
}

// NOTE: Query DSL projection traits are provided generically in dsl module now.

#[cfg(test)]
mod trait_object_tests {
    use norito::codec::Encode;

    use super::*;
    use crate::query::dsl::{HasProjection, PredicateMarker, SelectorMarker};

    #[test]
    fn query_dyn_encode_matches_encode() {
        let q = domain::FindDomains;
        let expected = q.encode();
        let actual = Query::dyn_encode(&q);
        assert_eq!(actual, expected);
    }

    #[test]
    fn query_as_any_downcasts() {
        let q = domain::FindDomains;
        // Call the method in a way that avoids trait-object dispatch
        let any = <domain::FindDomains as Query>::as_any(&q);
        assert!(any.downcast_ref::<domain::FindDomains>().is_some());
    }

    #[test]
    fn query_execute_does_not_panic() {
        let q = domain::FindDomains;
        Query::execute(&q);
    }

    #[test]
    fn singular_query_dyn_encode_matches_encode() {
        let q = FindExecutorDataModel;
        let expected = q.encode();
        let actual = SingularQuery::dyn_encode(&q);
        assert_eq!(actual, expected);
    }

    #[test]
    fn singular_query_as_any_downcasts() {
        let q = FindExecutorDataModel;
        let trait_obj: &dyn SingularQuery<Output = crate::executor::ExecutorDataModel> = &q;
        assert!(
            trait_obj
                .as_any()
                .downcast_ref::<FindExecutorDataModel>()
                .is_some()
        );
    }

    #[test]
    fn singular_query_execute_does_not_panic() {
        let q = FindExecutorDataModel;
        SingularQuery::execute(&q);
    }

    #[test]
    fn find_block_headers_has_selector_projection() {
        <block::FindBlockHeaders as HasProjection<SelectorMarker>>::atom(());
    }

    fn assert_predicate<T: HasProjection<PredicateMarker>>() {}
    fn assert_selector<T: HasProjection<SelectorMarker>>() {}

    #[test]
    fn committed_transaction_has_projection_impls() {
        assert_predicate::<CommittedTransaction>();
        assert_selector::<CommittedTransaction>();
    }

    #[test]
    fn iter_queries_have_projection_impls() {
        assert_predicate::<trigger::FindTriggers>();
        assert_selector::<trigger::FindTriggers>();

        assert_predicate::<asset::FindAssetsDefinitions>();
        assert_selector::<asset::FindAssetsDefinitions>();

        assert_predicate::<nft::FindNfts>();
        assert_selector::<nft::FindNfts>();

        assert_predicate::<rwa::FindRwas>();
        assert_selector::<rwa::FindRwas>();

        assert_predicate::<role::FindRoles>();
        assert_selector::<role::FindRoles>();

        assert_predicate::<peer::FindPeers>();
        assert_selector::<peer::FindPeers>();

        assert_predicate::<trigger::FindActiveTriggerIds>();
        assert_selector::<trigger::FindActiveTriggerIds>();
    }

    #[test]
    fn query_with_filter_converts() {
        use crate::query::dsl::{CompoundPredicate, SelectorTuple};

        #[allow(clippy::unit_arg)]
        let q = QueryWithFilter::new(
            {
                #[cfg(not(feature = "fast_dsl"))]
                {
                    Box::new(domain::FindDomains)
                }
                #[cfg(feature = "fast_dsl")]
                {}
            },
            CompoundPredicate::<crate::domain::Domain>::PASS,
            SelectorTuple::<crate::domain::Domain>::default(),
        );

        let _: QueryBox<QueryOutputBatchBox> = q.into();
    }
}

/// A macro reducing boilerplate when defining query types.
macro_rules! queries {
    ($($($meta:meta)* $item:item)+) => {
        pub use self::model::*;

        #[iroha_data_model_derive::model]
        mod model{
            use super::*;
            use norito::codec::{Decode, Encode}; $(

            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
            #[derive(Decode, Encode)]
            #[cfg_attr(
                feature = "json",
                derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
            )]
            #[derive(derive_more::Constructor)]
            #[derive(iroha_schema::IntoSchema)]
            $($meta)*
            $item )+
        }
    };
}

pub mod role {
    //! Role-related query definitions.
    //!
    //! Queries related to [`crate::role`].

    use std::{format, string::String, vec::Vec};

    // prelude not needed here; keep imports minimal
    use derive_more::Display;

    // Bring required IDs into scope for queries! items
    use crate::AccountId;

    queries! {
            /// [`FindRoles`] Iroha Query finds all `Role`s presented.
            #[derive(Copy, Display)]
            #[display("Find all roles")]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
            pub struct FindRoles;

            /// [`FindRoleIds`] Iroha Query finds `RoleId`s of
            /// all `Role`s presented.
            #[derive(Copy, Display)]
            #[display("Find all role ids")]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
            pub struct FindRoleIds;

            /// [`FindRolesByAccountId`] Iroha Query finds all `Role`s for a specified account.
            #[derive(Display)]
            #[display("Find all roles for `{id}` account")]
            #[repr(transparent)]
            // SAFETY: `FindRolesByAccountId` has no trap representation in `AccountId`
    /// Query for roles associated with a given account.
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
            pub struct FindRolesByAccountId {
                /// `Id` of an account to find.
                pub id: AccountId,
            }
        }

    impl FindRolesByAccountId {
        /// Return the queried account identifier.
        pub fn account_id(&self) -> &AccountId {
            &self.id
        }
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this module.
        pub use super::{FindRoleIds, FindRoles, FindRolesByAccountId};
    }
}

pub mod permission {
    //! Permission-related query definitions.
    //!
    //! Queries related to [`crate::permission`].

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    // Bring required IDs into scope for queries! items
    use crate::AccountId;

    queries! {
            /// [`FindPermissionsByAccountId`] Iroha Query finds all [`crate::permission::Permission`] values
            /// for a specified account.
            #[derive(Display)]
            #[display("Find permission tokens specified for `{id}` account")]
            #[repr(transparent)]
            // SAFETY: `FindPermissionsByAccountId` has no trap representation in `AccountId`
    /// Query for permissions associated with a given account.
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
            pub struct FindPermissionsByAccountId {
                /// `Id` of an account to find.
                pub id: AccountId,
            }
        }

    impl FindPermissionsByAccountId {
        /// Return the queried account identifier.
        pub fn account_id(&self) -> &AccountId {
            &self.id
        }
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this module.
        pub use super::FindPermissionsByAccountId;
    }
}

pub mod account {
    //! Account-related query definitions.
    //!
    //! Queries related to [`crate::account`].

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;
    use norito::codec::{Decode, Encode};

    // Bring required IDs into scope for queries! items
    use crate::prelude::AssetDefinitionId;

    /// API-facing record describing one alias bound to an account.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, iroha_schema::IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    pub struct AccountAliasBindingRecord {
        /// Canonical account identifier that owns the binding.
        pub account_id: crate::account::AccountId,
        /// Canonical alias literal such as `merchant@banka.centralbank`.
        pub alias: String,
        /// Dataspace alias such as `centralbank`.
        pub dataspace: String,
        /// Optional domain qualifier such as `banka`.
        #[norito(default)]
        pub domain: Option<String>,
        /// Whether this alias is the account's primary label.
        #[norito(default)]
        pub is_primary: bool,
        /// Effective SNS lifecycle status for the alias.
        pub status: crate::sns::NameStatus,
        /// Lease expiry timestamp (unix ms) when the alias ceases to be active.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
        /// End of the grace period (unix ms) after expiry.
        #[norito(default)]
        pub grace_until_ms: Option<u64>,
        /// Timestamp (unix ms) when the current lease term started.
        #[norito(default)]
        pub bound_at_ms: u64,
    }

    queries! {
            /// [`FindAccountById`] Iroha Query finds an `Account` by its identifier.
            #[derive(Display)]
            #[display("Find account `{id}`")]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
            pub struct FindAccountById {
                /// Domainless account identifier to resolve.
                pub id: crate::account::AccountId,
            }

            /// [`FindAccounts`] Iroha Query finds all `Account`s presented.
            #[derive(Copy, Display)]
            #[display("Find all accounts")]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
            pub struct FindAccounts;

            /// [`FindAccountIds`] Iroha Query finds identifiers of all `Account`s presented.
            #[derive(Copy, Display)]
            #[display("Find all account ids")]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
            pub struct FindAccountIds;

            /// [`FindAccountsWithAsset`] Iroha Query gets [`crate::asset::definition::AssetDefinition`] ids as input and
            /// finds all [`crate::account::Account`]s storing [`crate::asset::value::Asset`] with such definition.
            #[derive(Display)]
            #[display("Find accounts with `{asset_definition}` asset")]
            #[repr(transparent)]
            // SAFETY: `FindAccountsWithAsset` has no trap representation in `AssetDefinitionId`
    /// Query for accounts that hold a specific asset.
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
            pub struct FindAccountsWithAsset {
                /// `Id` of the definition of the asset which should be stored in founded accounts.
                pub asset_definition: AssetDefinitionId,
            }

            /// [`FindAliasesByAccountId`] query lists aliases bound to the account subject.
            #[derive(Display)]
            #[display("Find aliases bound to account `{id}`")]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
            pub struct FindAliasesByAccountId {
                /// Domainless account identifier whose alias bindings should be resolved.
                pub id: crate::account::AccountId,
                /// Optional dataspace alias filter such as `centralbank`.
                #[norito(default)]
                pub dataspace: Option<String>,
                /// Optional exact domain filter such as `banka`.
                #[norito(default)]
                pub domain: Option<String>,
            }

            /// [`FindAccountRecoveryPolicyByAlias`] query resolves the alias-keyed recovery policy.
            #[derive(Display)]
            #[display("Find recovery policy for alias `{alias:?}`")]
            #[repr(transparent)]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
            pub struct FindAccountRecoveryPolicyByAlias {
                /// Stable account alias whose recovery policy should be loaded.
                pub alias: crate::account::AccountAlias,
            }

            /// [`FindAccountRecoveryRequestByAlias`] query resolves the alias-keyed recovery request.
            #[derive(Display)]
            #[display("Find recovery request for alias `{alias:?}`")]
            #[repr(transparent)]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
            pub struct FindAccountRecoveryRequestByAlias {
                /// Stable account alias whose recovery request should be loaded.
                pub alias: crate::account::AccountAlias,
            }
        }

    impl FindAccountsWithAsset {
        /// Return the queried asset definition identifier.
        pub fn asset_definition_id(&self) -> &AssetDefinitionId {
            &self.asset_definition
        }
    }

    impl FindAccountById {
        /// Return the queried account identifier.
        pub fn account_id(&self) -> &crate::account::AccountId {
            &self.id
        }
    }

    impl FindAliasesByAccountId {
        /// Return the queried account identifier.
        pub fn account_id(&self) -> &crate::account::AccountId {
            &self.id
        }

        /// Return the optional dataspace alias filter.
        pub fn dataspace(&self) -> Option<&str> {
            self.dataspace.as_deref()
        }

        /// Return the optional domain filter.
        pub fn domain(&self) -> Option<&str> {
            self.domain.as_deref()
        }
    }

    impl FindAccountRecoveryPolicyByAlias {
        /// Return the queried stable alias.
        pub fn alias(&self) -> &crate::account::AccountAlias {
            &self.alias
        }
    }

    impl FindAccountRecoveryRequestByAlias {
        /// Return the queried stable alias.
        pub fn alias(&self) -> &crate::account::AccountAlias {
            &self.alias
        }
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::{
            AccountAliasBindingRecord, FindAccountById, FindAccountIds,
            FindAccountRecoveryPolicyByAlias, FindAccountRecoveryRequestByAlias, FindAccounts,
            FindAccountsWithAsset, FindAliasesByAccountId,
        };
    }
}

pub mod asset {
    //! Asset-related query definitions.
    //!
    //! Queries related to [`crate::asset`].

    #![allow(clippy::missing_inline_in_public_items)]

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    // Bring required IDs into scope for queries! items
    use crate::{AssetId, asset::AssetDefinitionId};

    queries! {
        /// [`FindAssets`] Iroha Query finds all `Asset`s presented.
        #[derive(Copy, Display)]
        #[display("Find all assets")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindAssets;

        /// [`FindAssetsDefinitions`] Iroha Query finds all `AssetDefinition`s presented.
        #[derive(Copy, Display)]
        #[display("Find all asset definitions")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindAssetsDefinitions;

        /// [`FindAssetById`] Iroha Query finds a specific `Asset` by identifier.
        #[derive(Display)]
        #[display("Find asset `{id}`")]
        #[repr(transparent)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
        pub struct FindAssetById {
            /// Identifier of the asset to look up.
            pub id: AssetId,
        }

        /// [`FindAssetDefinitionById`] Iroha Query finds a specific `AssetDefinition` by identifier.
        #[derive(Display)]
        #[display("Find asset definition `{id}`")]
        #[repr(transparent)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
        pub struct FindAssetDefinitionById {
            /// Identifier of the asset definition to look up.
            pub id: AssetDefinitionId,
        }
    }

    impl FindAssetById {
        /// Return the queried asset identifier.
        pub fn asset_id(&self) -> &AssetId {
            &self.id
        }
    }

    impl FindAssetDefinitionById {
        /// Return the queried asset definition identifier.
        pub fn asset_definition_id(&self) -> &AssetDefinitionId {
            &self.id
        }
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::{
            FindAssetById, FindAssetDefinitionById, FindAssets, FindAssetsDefinitions,
        };
    }
}

pub mod repo {
    //! Repository-related query definitions.
    //!
    //! Queries related to [`crate::repo`].

    use derive_more::Display;

    queries! {
        /// [`FindRepoAgreements`] Iroha Query finds all repo agreements stored on-chain.
        #[derive(Copy, Display)]
        #[display("Find all repo agreements")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindRepoAgreements;
    }

    pub mod prelude {
        //! Prelude re-export for repo queries.
        pub use super::FindRepoAgreements;
    }
}

pub mod offline {
    //! Offline allowance and settlement query definitions.
    use derive_more::Display;
    use iroha_crypto::Hash;

    use crate::{account::AccountId, offline::OfflineTransferStatus};

    queries! {
        /// Find all registered offline allowances.
        #[derive(Copy, Display)]
        #[display("Find all offline allowances")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindOfflineAllowances;

        /// Find a specific offline allowance by its certificate id.
        #[derive(Display)]
        #[display("Find offline allowance `{certificate_id}`")]
        #[repr(transparent)]
        pub struct FindOfflineAllowanceByCertificateId {
            /// Deterministic certificate identifier.
            pub certificate_id: Hash,
        }

        /// Find all pending offline-to-online transfer bundles.
        #[derive(Copy, Display)]
        #[display("Find all offline-to-online transfers")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindOfflineToOnlineTransfers;

        /// Find a specific offline-to-online transfer bundle by id.
        #[derive(Display)]
        #[display("Find offline-to-online transfer `{bundle_id}`")]
        #[repr(transparent)]
        pub struct FindOfflineToOnlineTransferById {
            /// Deterministic bundle identifier.
            pub bundle_id: Hash,
        }

        /// Find offline-to-online transfers submitted by a specific controller account.
        #[derive(Display)]
        #[display("Find offline-to-online transfers by controller `{controller}`")]
        #[repr(transparent)]
        pub struct FindOfflineToOnlineTransfersByController {
            /// Controller account identifier.
            pub controller: AccountId,
        }

        /// Find offline-to-online transfers targeting a specific receiver account.
        #[derive(Display)]
        #[display("Find offline-to-online transfers by receiver `{receiver}`")]
        #[repr(transparent)]
        pub struct FindOfflineToOnlineTransfersByReceiver {
            /// Receiver account identifier.
            pub receiver: AccountId,
        }

        /// Find offline-to-online transfers by lifecycle status.
        #[derive(Display)]
        #[display("Find offline-to-online transfers by status `{status:?}`")]
        #[repr(transparent)]
        pub struct FindOfflineToOnlineTransfersByStatus {
            /// Lifecycle status filter.
            pub status: OfflineTransferStatus,
        }

        /// Find offline-to-online transfers by Android attestation policy.
        #[derive(Display)]
        #[display("Find offline-to-online transfers by policy `{policy:?}`")]
        #[repr(transparent)]
        pub struct FindOfflineToOnlineTransfersByPolicy {
            /// Attestation policy filter.
            pub policy: crate::offline::AndroidIntegrityPolicy,
        }

        /// Retrieve counter summaries derived from offline allowance records.
        #[derive(Copy, Display)]
        #[display("Find offline counter summaries")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindOfflineCounterSummaries;

        /// Retrieve recorded offline verdict revocations.
        #[derive(Copy, Display)]
        #[display("Find offline verdict revocations")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindOfflineVerdictRevocations;
    }

    pub mod prelude {
        //! Prelude re-exports for offline queries.
        pub use super::{
            FindOfflineAllowanceByCertificateId, FindOfflineAllowances,
            FindOfflineCounterSummaries, FindOfflineToOnlineTransferById,
            FindOfflineToOnlineTransfers, FindOfflineToOnlineTransfersByController,
            FindOfflineToOnlineTransfersByPolicy, FindOfflineToOnlineTransfersByReceiver,
            FindOfflineToOnlineTransfersByStatus, FindOfflineVerdictRevocations,
        };
    }
}

pub mod oracle {
    //! Oracle-specific query definitions.
    use derive_more::Display;

    use crate::oracle::KeyedHash;

    queries! {
        /// Find a twitter binding by keyed hash.
        #[derive(Display)]
        #[display("Find twitter binding `{binding_hash:?}`")]
        #[repr(transparent)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
        pub struct FindTwitterBindingByHash {
            /// Pseudonymous keyed hash used to look up the binding.
            pub binding_hash: KeyedHash,
        }

    }

    impl FindTwitterBindingByHash {
        /// Return the keyed hash identifying the binding.
        pub fn binding_hash(&self) -> &KeyedHash {
            &self.binding_hash
        }
    }

    pub mod prelude {
        //! Prelude re-exports for oracle queries.
        pub use super::FindTwitterBindingByHash;
    }
}

pub mod da {
    //! Data availability pin intent query definitions.
    //!
    //! Queries for retrieving DA pin intents stored in the `SoraFS` registry surface.

    use crate::{da::types::StorageTicketId, nexus::LaneId, sorafs::pin_registry::ManifestDigest};

    queries! {
        /// Fetch a DA pin intent by its storage ticket.
        #[repr(transparent)]
        pub struct FindDaPinIntentByTicket {
            /// Storage ticket to look up.
            pub storage_ticket: StorageTicketId,
        }

        /// Fetch a DA pin intent by its manifest digest.
        #[repr(transparent)]
        pub struct FindDaPinIntentByManifest {
            /// Manifest digest to look up.
            pub manifest_hash: ManifestDigest,
        }

        /// Fetch a DA pin intent by its alias.
        #[repr(transparent)]
        pub struct FindDaPinIntentByAlias {
            /// Alias to look up.
            pub alias: String,
        }

        /// Fetch a DA pin intent by lane/epoch/sequence tuple.
        pub struct FindDaPinIntentByLaneEpochSequence {
            /// Lane identifier associated with the intent.
            pub lane_id: LaneId,
            /// Epoch containing the intent.
            pub epoch: u64,
            /// Sequence number within the lane/epoch.
            pub sequence: u64,
        }
    }

    pub mod prelude {
        //! Prelude re-exports for DA pin intent queries.
        pub use super::{
            FindDaPinIntentByAlias, FindDaPinIntentByLaneEpochSequence, FindDaPinIntentByManifest,
            FindDaPinIntentByTicket,
        };
    }
}

pub mod nexus {
    //! Nexus relay query definitions.

    use crate::nexus::LaneRelayEnvelopeRef;

    queries! {
        /// Fetch a verified lane relay by its canonical reference.
        #[repr(transparent)]
        pub struct FindLaneRelayEnvelopeByRef {
            /// Canonical relay reference to look up.
            pub relay_ref: LaneRelayEnvelopeRef,
        }
    }

    pub mod prelude {
        //! Prelude re-exports for Nexus relay queries.
        pub use super::FindLaneRelayEnvelopeByRef;
    }
}

pub mod nft {
    //! NFT-related query definitions.
    //!
    //! Queries related to [`crate::nft`].

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindNfts`] Iroha Query finds all `Nft`s presented.
        #[derive(Copy, Display)]
        #[display("Find all NFTs")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindNfts;
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::FindNfts;
    }
}

pub mod rwa {
    //! RWA-related query definitions.
    //!
    //! Queries related to [`crate::rwa`].

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindRwas`] finds all registered RWA lots.
        #[derive(Copy, Display)]
        #[display("Find all RWAs")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindRwas;
    }

    pub mod prelude {
        //! Prelude re-exports for RWA queries.
        pub use super::FindRwas;
    }
}

pub mod domain {
    //! Domain-related query definitions.
    //!
    //! Queries related to [`crate::domain`].

    #![allow(clippy::missing_inline_in_public_items)]

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindDomains`] Iroha Query finds all `Domain`s presented.
        #[derive(Copy, Display)]
        #[display("Find all domains")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindDomains;

    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::FindDomains;
    }
}

pub mod endorsement {
    //! Domain endorsement-related query definitions.
    //!
    //! Queries related to domain endorsement committees and policies.

    use derive_more::Display;

    use crate::domain::DomainId;

    queries! {
        /// Fetch all recorded endorsements for a given domain.
        #[derive(Display)]
        #[display("Find endorsements for domain `{domain_id}`")]
        #[repr(transparent)]
        pub struct FindDomainEndorsements {
            /// Domain identifier to filter by.
            pub domain_id: DomainId,
        }

        /// Fetch the configured endorsement policy for a domain.
        #[derive(Display)]
        #[display("Find endorsement policy for domain `{domain_id}`")]
        #[repr(transparent)]
        pub struct FindDomainEndorsementPolicy {
            /// Domain identifier to fetch the policy for.
            pub domain_id: DomainId,
        }

        /// Fetch a domain endorsement committee by identifier.
        #[derive(Display)]
        #[display("Find domain committee `{committee_id}`")]
        #[repr(transparent)]
        pub struct FindDomainCommittee {
            /// Committee identifier.
            pub committee_id: String,
        }
    }

    /// Prelude re-exports for endorsement queries.
    pub mod prelude {
        pub use super::{FindDomainCommittee, FindDomainEndorsementPolicy, FindDomainEndorsements};
    }
}

pub mod peer {
    //! Peer-related query definitions.
    //!
    //! Queries related to [`crate::peer`].

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindPeers`] Iroha Query finds all trusted peers presented.
        #[derive(Copy, Display)]
        #[display("Find all peers")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindPeers;
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::FindPeers;
    }
}

pub mod executor {
    //! Executor-related query definitions.
    //!
    //! Queries related to [`crate::executor`].

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindExecutorDataModel`] Iroha Query finds the data model of the current executor.
        #[derive(Copy, Display)]
        #[display("Find executor data model")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindExecutorDataModel;

        /// [`FindParameters`] Iroha Query finds all defined executor configuration parameters.
        #[derive(Copy, Display)]
        #[display("Find all peers parameters")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindParameters;
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::{FindExecutorDataModel, FindParameters};
    }
}

pub mod runtime {
    //! Runtime inspector query definitions.
    //!
    //! Queries related to runtime/ABI.

    use derive_more::Display;

    queries! {
        /// Find the active ABI version.
        #[derive(Copy, Display)]
        #[display("Find active ABI version")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindAbiVersion;
    }

    /// Response type for `FindAbiVersion` query.
    ///
    /// Query for the ABI version currently active on the chain.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        norito::codec::Decode,
        norito::codec::Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AbiVersion {
        /// The ABI version currently active on the node.
        pub abi_version: u16,
    }

    pub mod prelude {
        //! Prelude re-exports.
        pub use super::FindAbiVersion;
    }
}

pub mod proof {
    //! Proof-related query definitions.
    //!
    //! Queries related to zero-knowledge proofs and records.

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// Find a proof verification record by its identifier.
        #[derive(Display)]
        #[display("Find proof record by `{id}`")]
        #[repr(transparent)]
        pub struct FindProofRecordById {
            /// Proof identifier (backend + proof hash).
            pub id: crate::proof::ProofId,
        }

        /// Find all proof verification records.
        #[derive(Copy, Display)]
        #[display("Find all proof records")]
        pub struct FindProofRecords;

        /// Find all proof verification records for a given backend identifier.
        #[derive(Display)]
        #[display("Find proof records for backend `{backend}`")]
        #[repr(transparent)]
        pub struct FindProofRecordsByBackend {
            /// Backend identifier (e.g., "halo2/ipa").
            pub backend: iroha_schema::Ident,
        }

        /// Find all proof verification records for a given status.
        #[derive(Display)]
        #[display("Find proof records with status `{status:?}`")]
        #[repr(transparent)]
        pub struct FindProofRecordsByStatus {
            /// Proof verification status to filter by.
            pub status: crate::proof::ProofStatus,
        }
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this module.
    pub mod prelude {
        pub use super::{
            FindProofRecordById, FindProofRecords, FindProofRecordsByBackend,
            FindProofRecordsByStatus,
        };
    }
}

pub mod sorafs {
    //! `SoraFS` query definitions.
    //!
    //! Queries related to `SoraFS` provider metadata.

    use std::fmt;

    use hex;

    use crate::sorafs::capacity::ProviderId;

    queries! {
        /// Fetch the registered owner for a `SoraFS` provider.
        #[repr(transparent)]
        pub struct FindSorafsProviderOwner {
            /// Provider identifier to resolve.
            pub provider_id: ProviderId,
        }
    }

    impl fmt::Display for FindSorafsProviderOwner {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "Find SoraFS provider owner for `{}`",
                hex::encode(self.provider_id.as_bytes())
            )
        }
    }

    /// Prelude re-exports for `SoraFS` queries.
    pub mod prelude {
        pub use super::FindSorafsProviderOwner;
    }
}

impl seal::SingularQuery for sorafs::prelude::FindSorafsProviderOwner {}
impl SingularQuery for sorafs::prelude::FindSorafsProviderOwner {
    type Output = crate::account::AccountId;

    fn dyn_encode(&self) -> Vec<u8> {
        self.encode()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub mod sns {
    //! SNS-related query definitions.
    //!
    //! Queries related to authoritative SNS-backed ownership.

    use derive_more::Display;

    use crate::nexus::DataSpaceId;

    queries! {
        /// Fetch the active SNS owner for a dataspace alias resolved from the current catalog.
        #[derive(Display)]
        #[display("Find SNS dataspace owner for `{dataspace_id}`")]
        #[repr(transparent)]
        pub struct FindDataspaceNameOwnerById {
            /// Dataspace identifier whose leased alias owner should be resolved.
            pub dataspace_id: DataSpaceId,
        }
    }

    impl FindDataspaceNameOwnerById {
        /// Return the queried dataspace identifier.
        pub fn dataspace_id(&self) -> DataSpaceId {
            self.dataspace_id
        }
    }

    /// Prelude re-exports for SNS queries.
    pub mod prelude {
        pub use super::FindDataspaceNameOwnerById;
    }
}

pub mod trigger {
    //! Trigger-related query definitions.
    //!
    //! Trigger-related queries.
    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// Find all currently active (as in not disabled and/or expired)
        /// trigger IDs.
        #[derive(Copy, Display)]
        #[display("Find all trigger ids")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindActiveTriggerIds;

        /// Find all currently active (as in not disabled and/or expired) triggers.
        #[derive(Copy, Display)]
        #[display("Find all triggers")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindTriggers;

        /// Find a trigger by identifier.
        #[derive(Display)]
        #[display("Find trigger `{id}`")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindTriggerById {
            /// Trigger identifier to resolve.
            pub id: crate::trigger::TriggerId,
        }
    }

    impl FindTriggerById {
        /// Return the queried trigger identifier.
        pub fn trigger_id(&self) -> &crate::trigger::TriggerId {
            &self.id
        }
    }

    pub mod prelude {
        //! Convenient re-exports for common query types.
        pub use super::{FindActiveTriggerIds, FindTriggerById, FindTriggers};
    }
}

pub mod smart_contract {
    //! Smart-contract query definitions.
    //!
    //! Smart contract code/manifest related queries.

    use derive_more::Display;

    queries! {
        /// Find a smart contract manifest by its content-addressed code hash.
        #[derive(Display)]
        #[display("Find contract manifest by `{code_hash}`")]
        #[repr(transparent)]
        pub struct FindContractManifestByCodeHash {
            /// Content-addressed code hash of the compiled `.to` bytecode.
            pub code_hash: iroha_crypto::Hash,
        }
    }

    pub mod prelude {
        //! Prelude re-exports for smart contract queries.
        pub use super::FindContractManifestByCodeHash;
    }
}

pub mod transaction {
    //! Transaction query definitions.
    //!
    //! Queries related to transactions.

    #![allow(clippy::missing_inline_in_public_items)]

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindTransactions`] Iroha Query lists all transactions included in a blockchain
        #[derive(Copy, Display)]
        #[display("Find all transactions")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindTransactions;
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::FindTransactions;
    }
}

pub mod block {
    //! Block query definitions.
    //!
    //! Queries related to blocks.

    #![allow(clippy::missing_inline_in_public_items)]

    use std::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindBlocks`] Iroha Query lists all blocks sorted by
        /// height in descending order
        #[derive(Copy, Display)]
        #[display("Find all blocks")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindBlocks;

        /// [`FindBlockHeaders`] Iroha Query lists all block headers
        /// sorted by height in descending order
        #[derive(Copy, Display)]
        #[display("Find all block headers")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct FindBlockHeaders;
    }

    pub mod prelude {
        //! The prelude re-exports most commonly used traits, structs and macros from this crate.
        pub use super::{FindBlockHeaders, FindBlocks};
    }
}

pub mod error {
    //! Error types produced by query execution.
    //!
    //! Module containing errors that can occur during query execution.

    #[cfg(feature = "json")]
    use iroha_crypto::HashOf;
    use iroha_data_model_derive::model;
    use iroha_macro::FromVariant;
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    pub use self::model::*;
    use super::*;
    use crate::prelude::*;

    #[model]
    mod model {
        use super::*;
        /// Query errors.
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            FromVariant,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        /// High-level failure reasons for query execution.
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[derive(thiserror::Error)]
        pub enum QueryExecutionFail {
            /// {0}
            #[error(transparent)]
            Find(FindError),
            /// Query found wrong type of asset: {0}
            Conversion(
                #[skip_from]
                #[skip_try_from]
                String,
            ),
            /// Query not found in the live query store.
            NotFound,
            /// The server's cursor does not match the provided cursor.
            CursorMismatch,
            /// There aren't enough items for the cursor to proceed.
            CursorDone,
            /// `fetch_size` must not exceed [`MAX_FETCH_SIZE`](crate::query::parameters::MAX_FETCH_SIZE).
            FetchSizeTooBig,
            /// Query execution exceeded the configured gas/materialization budget.
            GasBudgetExceeded,
            /// Some of the specified parameters (`filter/pagination/fetch_size/sorting`) are not applicable to singular queries
            InvalidSingularParameters,
            /// Reached the limit of parallel queries. Either wait for previous queries to complete, or increase the limit in the config.
            CapacityLimit,
            /// The stored cursor has expired and was removed from the server.
            Expired,
            /// The authority reached the per-tenant limit of stored cursors.
            AuthorityQuotaExceeded,
        }

        /// Type assertion error
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
        /// Item-level errors returned when resolving query inputs.
        pub enum FindError {
            /// Failed to find asset: `{0}`
            Asset(Box<AssetId>),
            /// Failed to find asset definition: `{0}`
            AssetDefinition(AssetDefinitionId),
            /// Failed to find NFT: `{0}`
            Nft(NftId),
            /// Failed to find RWA: `{0}`
            Rwa(RwaId),
            /// Failed to find account: `{0}`
            Account(AccountId),
            /// Failed to find domain: `{0}`
            Domain(DomainId),
            /// Failed to find metadata key: `{0}`
            MetadataKey(Name),
            /// Block with hash `{0}` not found
            Block(HashOf<BlockHeader>),
            /// Transaction with hash `{0}` not found
            Transaction(HashOf<SignedTransaction>),
            /// Peer with id `{0}` not found
            Peer(PeerId),
            /// Trigger with id `{0}` not found
            Trigger(TriggerId),
            /// Role with id `{0}` not found
            Role(RoleId),
            /// Failed to find [`Permission`] by id.
            Permission(Box<Permission>),
            /// Failed to find public key: `{0}`
            PublicKey(PublicKey),
            /// Failed to find twitter binding for keyed hash `{0:?}`
            TwitterBinding(crate::oracle::KeyedHash),
        }
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
#[allow(ambiguous_glob_reexports)]
pub mod prelude {
    pub use super::{
        CommittedTransaction, QueryBox, QueryRequest, SingularQueryBox, account::prelude::*,
        asset::prelude::*, block::prelude::*, builder::prelude::*, da::prelude::*,
        domain::prelude::*, dsl::prelude::*, endorsement::prelude::*, executor::prelude::*,
        nft::prelude::*, oracle::prelude::*, parameters::prelude::*, peer::prelude::*,
        permission::prelude::*, role::prelude::*, rwa::prelude::*, transaction::prelude::*,
        trigger::prelude::*,
    };
}

#[cfg(all(test, feature = "fault_injection"))]
mod fault_injection_tests {
    use std::str::FromStr;

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use iroha_crypto::{Hash, HashOf, MerkleProof};

    use super::*;
    use crate::{
        AssetDefinitionId, Level,
        isi::{InstructionBox, Log},
        kaigi::{
            KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
            KaigiRoomPolicy,
        },
        prelude::{DataTriggerSequence, TimeTriggerEntrypoint},
        transaction::{
            PrivateCreateKaigi, PrivateKaigiAction, PrivateKaigiArtifacts, PrivateKaigiFeeSpend,
            PrivateKaigiTemplate, PrivateKaigiTransaction,
        },
        trigger::TriggerId,
    };

    fn zero_hash<T>() -> HashOf<T> {
        let zero = [0u8; 32];
        HashOf::from_untyped_unchecked(Hash::prehashed(zero))
    }

    fn make_time_committed_tx() -> CommittedTransaction {
        let entry = TransactionEntrypoint::Time(TimeTriggerEntrypoint {
            id: TriggerId::from_str("fault_trigger").expect("valid trigger id"),
            instructions: ExecutionStep(Vec::<InstructionBox>::new().into()),
            authority: AccountId::parse_encoded(
                "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
            )
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect("valid authority"),
        });

        let result = TransactionResult(Ok(DataTriggerSequence::default()));
        CommittedTransaction {
            block_hash: zero_hash(),
            entrypoint_hash: entry.hash(),
            entrypoint_proof: MerkleProof::from_audit_path(0, vec![]),
            entrypoint: entry,
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, vec![]),
            result,
        }
    }

    fn make_private_committed_tx() -> CommittedTransaction {
        let mut metadata = Metadata::default();
        metadata.insert(Name::from_str("topic").expect("metadata key"), "private");
        let entry = TransactionEntrypoint::PrivateKaigi(PrivateKaigiTransaction {
            chain: "test-chain".parse().expect("chain"),
            creation_time_ms: 42,
            nonce: None,
            metadata,
            action: PrivateKaigiAction::Create(PrivateCreateKaigi {
                call: PrivateKaigiTemplate {
                    id: KaigiId::new(
                        DomainId::from_str("kaigi").expect("domain"),
                        Name::from_str("private-room").expect("call"),
                    ),
                    title: Some("Private".to_owned()),
                    description: None,
                    max_participants: Some(2),
                    gas_rate_per_minute: 5,
                    metadata: Metadata::default(),
                    scheduled_start_ms: None,
                    privacy_mode: KaigiPrivacyMode::ZkRosterV1,
                    room_policy: KaigiRoomPolicy::Authenticated,
                    relay_manifest: None,
                },
            }),
            artifacts: PrivateKaigiArtifacts {
                commitment: KaigiParticipantCommitment {
                    commitment: Hash::new(b"commitment"),
                    alias_tag: None,
                },
                nullifier: KaigiParticipantNullifier {
                    digest: Hash::new(b"nullifier"),
                    issued_at_ms: 42,
                },
                roster_root: Hash::new(b"root"),
                proof: vec![1, 2, 3],
            },
            fee_spend: PrivateKaigiFeeSpend {
                asset_definition_id: AssetDefinitionId::new(
                    DomainId::from_str("wonderland").expect("domain"),
                    Name::from_str("xor").expect("name"),
                ),
                anchor_root: Hash::new(b"anchor"),
                nullifiers: vec![[0x11; 32]],
                output_commitments: vec![[0x22; 32]],
                encrypted_change_payloads: vec![vec![0x33]],
                proof: vec![0x44],
            },
        });

        let result = TransactionResult(Ok(DataTriggerSequence::default()));
        CommittedTransaction {
            block_hash: zero_hash(),
            entrypoint_hash: entry.hash(),
            entrypoint_proof: MerkleProof::from_audit_path(0, vec![]),
            entrypoint: entry,
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, vec![]),
            result,
        }
    }

    #[test]
    fn time_entrypoint_injection_appends_instructions() {
        let mut tx = make_time_committed_tx();
        let original_hash = tx.entrypoint_hash;

        let injected: InstructionBox = Log {
            level: Level::WARN,
            msg: "timer tamper".into(),
        }
        .into();

        tx.inject_instructions([injected.clone()]);

        assert_ne!(
            tx.entrypoint_hash, original_hash,
            "entrypoint hash must reflect injected instructions"
        );

        let instructions = match &tx.entrypoint {
            TransactionEntrypoint::Time(entry) => entry.instructions.0.clone().into_vec(),
            _ => panic!("expected time entrypoint"),
        };
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0], injected);
    }

    #[test]
    fn private_kaigi_entrypoint_injection_records_overlay() {
        let mut tx = make_private_committed_tx();
        let original_hash = tx.entrypoint_hash;
        let injected: InstructionBox = Log {
            level: Level::WARN,
            msg: "private tamper".into(),
        }
        .into();

        tx.inject_instructions([injected.clone()]);

        assert_ne!(
            tx.entrypoint_hash, original_hash,
            "entrypoint hash must reflect injected instructions"
        );

        let overlay = match &tx.entrypoint {
            TransactionEntrypoint::PrivateKaigi(entry) => {
                crate::transaction::signed::SignedTransaction::fault_injection_overlay(
                    &entry.metadata,
                )
                .unwrap_or_default()
            }
            _ => panic!("expected private Kaigi entrypoint"),
        };
        assert_eq!(overlay.len(), 1);
        assert_eq!(
            overlay[0],
            BASE64_STANDARD.encode(norito::to_bytes(&injected).expect("encode overlay payload"))
        );
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use std::{num::NonZeroU64, str::FromStr};

    use iroha_crypto::{Hash, HashOf, MerkleProof};
    use norito::json;

    use super::*;
    use crate::{
        AssetDefinitionId,
        domain::DomainId,
        kaigi::{
            KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
            KaigiRoomPolicy,
        },
        name::Name,
        transaction::{
            PrivateCreateKaigi, PrivateKaigiAction, PrivateKaigiArtifacts, PrivateKaigiFeeSpend,
            PrivateKaigiTemplate, PrivateKaigiTransaction, TransactionEntrypoint,
            TransactionResult,
        },
    };

    fn zero_hash<T>() -> HashOf<T> {
        let zero = [0u8; 32];
        HashOf::from_untyped_unchecked(Hash::prehashed(zero))
    }

    fn private_committed_tx() -> CommittedTransaction {
        let mut metadata = Metadata::default();
        metadata.insert(Name::from_str("topic").expect("metadata key"), "private");
        let entrypoint = TransactionEntrypoint::PrivateKaigi(PrivateKaigiTransaction {
            chain: "test-chain".parse().expect("chain"),
            creation_time_ms: 42,
            nonce: None,
            metadata,
            action: PrivateKaigiAction::Create(PrivateCreateKaigi {
                call: PrivateKaigiTemplate {
                    id: KaigiId::new(
                        DomainId::from_str("kaigi").expect("domain"),
                        Name::from_str("private-room").expect("call"),
                    ),
                    title: Some("Private".to_owned()),
                    description: None,
                    max_participants: Some(2),
                    gas_rate_per_minute: 5,
                    metadata: Metadata::default(),
                    scheduled_start_ms: None,
                    privacy_mode: KaigiPrivacyMode::ZkRosterV1,
                    room_policy: KaigiRoomPolicy::Authenticated,
                    relay_manifest: None,
                },
            }),
            artifacts: PrivateKaigiArtifacts {
                commitment: KaigiParticipantCommitment {
                    commitment: Hash::new(b"commitment"),
                    alias_tag: None,
                },
                nullifier: KaigiParticipantNullifier {
                    digest: Hash::new(b"nullifier"),
                    issued_at_ms: 42,
                },
                roster_root: Hash::new(b"root"),
                proof: vec![1, 2, 3],
            },
            fee_spend: PrivateKaigiFeeSpend {
                asset_definition_id: AssetDefinitionId::new(
                    DomainId::from_str("wonderland").expect("domain"),
                    Name::from_str("xor").expect("name"),
                ),
                anchor_root: Hash::new(b"anchor"),
                nullifiers: vec![[0x11; 32]],
                output_commitments: vec![[0x22; 32]],
                encrypted_change_payloads: vec![vec![0x33]],
                proof: vec![0x44],
            },
        });
        let result = TransactionResult(Ok(crate::trigger::DataTriggerSequence::default()));
        CommittedTransaction {
            block_hash: zero_hash(),
            entrypoint_hash: entrypoint.hash(),
            entrypoint_proof: MerkleProof::from_audit_path(0, vec![]),
            entrypoint: entrypoint.clone(),
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, vec![]),
            result,
        }
    }

    #[test]
    fn query_output_batch_box_json_roundtrip() {
        let batch = QueryOutputBatchBox::String(vec!["hello".to_owned()]);

        let as_value = json::to_value(&batch).expect("serialize batch");
        assert_eq!(
            as_value,
            norito::json!({ "kind": "String", "content": ["hello"] })
        );

        let decoded: QueryOutputBatchBox = json::from_value(as_value).expect("deserialize batch");
        assert_eq!(decoded, batch);
    }

    #[test]
    fn query_response_iterable_json_roundtrip() {
        let cursor = parameters::ForwardCursor {
            query: "query-id".to_owned(),
            cursor: NonZeroU64::new(1).expect("nonzero"),
            gas_budget: None,
        };
        let output = QueryOutput {
            batch: QueryOutputBatchBoxTuple {
                tuple: vec![QueryOutputBatchBox::Numeric(vec![Numeric::from(42_u32)])],
            },
            remaining_items: 0,
            continue_cursor: Some(cursor),
        };
        let response = QueryResponse::Iterable(output.clone());

        let as_value = json::to_value(&response).expect("serialize response");
        let decoded: QueryResponse =
            json::from_value(as_value.clone()).expect("deserialize response");
        assert_eq!(decoded, response);

        // Ensure JSON structure exposes iterable wrapper with batch payload.
        match as_value {
            json::Value::Object(map) => {
                assert_eq!(map.get("kind"), Some(&norito::json!("Iterable")));
                assert!(map.contains_key("content"));
            }
            other => panic!("expected object for iterable response, got {other:?}"),
        }
    }

    #[test]
    fn committed_tx_filters_treat_private_kaigi_authority_as_absent() {
        let tx = private_committed_tx();

        let filters = CommittedTxFilters {
            authority_exists: Some(false),
            ts_ge: Some(40),
            ts_le: Some(50),
            ..CommittedTxFilters::default()
        };
        assert!(filters.applies(&tx));

        let authority_required = CommittedTxFilters {
            authority_exists: Some(true),
            ..CommittedTxFilters::default()
        };
        assert!(!authority_required.applies(&tx));
    }
}
