//! Iroha Queries provides declarative API for Iroha Queries.

#![allow(clippy::missing_inline_in_public_items)]

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::String,
    vec::{self, Vec},
};
#[cfg(feature = "std")]
use std::vec;

use derive_more::Constructor;
use iroha_crypto::{MerkleProof, PublicKey, SignatureOf};
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_schema::IntoSchema;
use iroha_version::prelude::*;
use parameters::{ForwardCursor, QueryParams};
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub use self::model::*;
use self::{
    account::*, asset::*, block::*, domain::*, dsl::*, executor::*, nft::*, peer::*, permission::*,
    role::*, transaction::*, trigger::*,
};
use crate::{
    account::{Account, AccountId},
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::{BlockHeader, SignedBlock},
    domain::{Domain, DomainId},
    metadata::Metadata,
    name::Name,
    nft::{Nft, NftId},
    parameter::{Parameter, Parameters},
    peer::PeerId,
    permission::Permission,
    role::{Role, RoleId},
    seal::Sealed,
    transaction::SignedTransaction,
    trigger::{Trigger, TriggerId},
};
#[cfg(feature = "fault_injection")]
use crate::{
    prelude::{
        InstructionBox, TransactionEntrypoint, TransactionRejectionReason, TransactionResult,
    },
    ValidationFail,
};

pub mod builder;
pub mod dsl;
pub mod parameters;

/// A query that either returns a single value or errors out
// NOTE: we are planning to remove this class of queries (https://github.com/hyperledger-iroha/iroha/issues/4933)
pub trait SingularQuery: Sealed {
    /// The type of the output of the query
    type Output;
}

/// A query that returns an iterable collection of values
///
/// Iterable queries logically return a stream of items.
/// In the actual implementation, the items collected into batches and a cursor is used to fetch the next batch.
/// [`builder::QueryIterator`] abstracts over this and allows the query consumer to use a familiar [`Iterator`] interface to iterate over the results.
pub trait Query: Sealed {
    /// The type of single element of the output collection
    type Item: HasProjection<PredicateMarker> + HasProjection<SelectorMarker, AtomType = ()>;
}

#[model]
mod model {
    use derive_where::derive_where;
    use getset::Getters;
    use iroha_crypto::HashOf;
    use iroha_macro::serde_where;

    use super::*;
    use crate::{
        prelude::{TransactionEntrypoint, TransactionResult},
        trigger::action,
    };

    /// An iterable query bundled with a filter
    #[serde_where(Q, CompoundPredicate<Q::Item>, SelectorTuple<Q::Item>)]
    #[derive_where(
        Debug, Clone, PartialEq, Eq; Q, CompoundPredicate<Q::Item>, SelectorTuple<Q::Item>
    )]
    #[derive(Decode, Encode, Constructor, IntoSchema, Deserialize, Serialize)]
    pub struct QueryWithFilter<Q>
    where
        Q: Query,
    {
        pub query: Q,
        #[serde(default = "predicate_default")]
        pub predicate: CompoundPredicate<Q::Item>,
        #[serde(default)]
        pub selector: SelectorTuple<Q::Item>,
    }

    fn predicate_default<T>() -> CompoundPredicate<T>
    where
        T: HasProjection<PredicateMarker>,
    {
        CompoundPredicate::PASS
    }

    /// An enum of all possible iterable queries
    #[derive(
        Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema, FromVariant,
    )]
    pub enum QueryBox {
        FindDomains(QueryWithFilter<FindDomains>),
        FindAccounts(QueryWithFilter<FindAccounts>),
        FindAssets(QueryWithFilter<FindAssets>),
        FindAssetsDefinitions(QueryWithFilter<FindAssetsDefinitions>),
        FindNfts(QueryWithFilter<FindNfts>),
        FindRoles(QueryWithFilter<FindRoles>),

        FindRoleIds(QueryWithFilter<FindRoleIds>),
        FindPermissionsByAccountId(QueryWithFilter<FindPermissionsByAccountId>),
        FindRolesByAccountId(QueryWithFilter<FindRolesByAccountId>),
        FindAccountsWithAsset(QueryWithFilter<FindAccountsWithAsset>),

        FindPeers(QueryWithFilter<FindPeers>),
        FindActiveTriggerIds(QueryWithFilter<FindActiveTriggerIds>),
        FindTriggers(QueryWithFilter<FindTriggers>),
        FindTransactions(QueryWithFilter<FindTransactions>),
        FindBlocks(QueryWithFilter<FindBlocks>),
        FindBlockHeaders(QueryWithFilter<FindBlockHeaders>),
    }

    /// An enum of all possible iterable query batches.
    ///
    /// We have an enum of batches instead of individual elements, because it makes it easier to check that the batches have elements of the same type and reduces serialization overhead.
    #[derive(
        Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema, FromVariant,
    )]
    pub enum QueryOutputBatchBox {
        PublicKey(Vec<PublicKey>),
        String(Vec<String>),
        Metadata(Vec<Metadata>),
        Json(Vec<Json>),
        Numeric(Vec<Numeric>),
        Name(Vec<Name>),
        DomainId(Vec<DomainId>),
        Domain(Vec<Domain>),
        AccountId(Vec<AccountId>),
        Account(Vec<Account>),
        AssetId(Vec<AssetId>),
        Asset(Vec<Asset>),
        AssetDefinitionId(Vec<AssetDefinitionId>),
        AssetDefinition(Vec<AssetDefinition>),
        NftId(Vec<NftId>),
        Nft(Vec<Nft>),
        Role(Vec<Role>),
        Parameter(Vec<Parameter>),
        Permission(Vec<Permission>),
        CommittedTransaction(Vec<CommittedTransaction>),
        TransactionResult(Vec<TransactionResult>),
        TransactionResultHash(Vec<HashOf<TransactionResult>>),
        TransactionEntrypoint(Vec<TransactionEntrypoint>),
        TransactionEntrypointHash(Vec<HashOf<TransactionEntrypoint>>),
        Peer(Vec<PeerId>),
        RoleId(Vec<RoleId>),
        TriggerId(Vec<TriggerId>),
        Trigger(Vec<Trigger>),
        Action(Vec<action::Action>),
        Block(Vec<SignedBlock>),
        BlockHeader(Vec<BlockHeader>),
        BlockHeaderHash(Vec<HashOf<BlockHeader>>),
    }

    #[derive(
        Debug, Clone, PartialEq, Eq, Decode, Encode, Constructor, Deserialize, Serialize, IntoSchema,
    )]
    pub struct QueryOutputBatchBoxTuple {
        pub tuple: Vec<QueryOutputBatchBox>,
    }

    /// An enum of all possible singular queries
    #[derive(
        Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema, FromVariant,
    )]
    pub enum SingularQueryBox {
        FindExecutorDataModel(FindExecutorDataModel),
        FindParameters(FindParameters),
    }

    /// An enum of all possible singular query outputs
    #[derive(
        Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema, FromVariant,
    )]
    pub enum SingularQueryOutputBox {
        ExecutorDataModel(crate::executor::ExecutorDataModel),
        Parameters(Parameters),
    }

    /// The results of a single iterable query request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema)]
    pub struct QueryOutput {
        /// A single batch of results
        pub batch: QueryOutputBatchBoxTuple,
        /// The number of items in the query remaining to be fetched after this batch
        pub remaining_items: u64,
        /// If not `None`, contains a cursor that can be used to fetch the next batch of results. Otherwise the current batch is the last one.
        pub continue_cursor: Option<ForwardCursor>,
    }

    /// A type-erased iterable query, along with all the parameters needed to execute it
    #[derive(
        Debug, Clone, PartialEq, Eq, Constructor, Decode, Encode, Deserialize, Serialize, IntoSchema,
    )]
    pub struct QueryWithParams {
        pub query: QueryBox,
        #[serde(default)]
        pub params: QueryParams,
    }

    /// A query request that can be sent to an Iroha peer.
    ///
    /// In case of HTTP API, the query request must also be signed (see [`QueryRequestWithAuthority`] and [`SignedQuery`]).
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema)]
    pub enum QueryRequest {
        Singular(SingularQueryBox),
        Start(QueryWithParams),
        Continue(ForwardCursor),
    }

    /// An enum containing either a singular or an iterable query
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema)]
    pub enum AnyQueryBox {
        Singular(SingularQueryBox),
        Iterable(QueryWithParams),
    }

    /// A response to a [`QueryRequest`] from an Iroha peer
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema)]
    pub enum QueryResponse {
        Singular(SingularQueryOutputBox),
        Iterable(QueryOutput),
    }

    /// A [`QueryRequest`], combined with an authority that wants to execute the query
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema)]
    pub struct QueryRequestWithAuthority {
        pub authority: AccountId,
        pub request: QueryRequest,
    }

    /// A signature of [`QueryRequestWithAuthority`] to be used in [`SignedQueryV1`]
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema)]
    pub struct QuerySignature(pub SignatureOf<QueryRequestWithAuthority>);

    declare_versioned!(SignedQuery 1..2, Debug, Clone, FromVariant, IntoSchema);

    /// A signed and authorized query request
    #[derive(Debug, Clone, Encode, Serialize, IntoSchema)]
    #[version_with_scale(version = 1, versioned_alias = "SignedQuery")]
    pub struct SignedQueryV1 {
        pub signature: QuerySignature,
        pub payload: QueryRequestWithAuthority,
    }

    /// Response returned by [`FindTransactions`] query.
    #[derive(
        Debug,
        Clone,
        PartialOrd,
        Ord,
        PartialEq,
        Eq,
        Getters,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[getset(get = "pub")]
    #[ffi_type]
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

#[cfg(feature = "fault_injection")]
impl CommittedTransaction {
    /// Injects a set of fictitious instructions into the transaction payload to simulate tampering.
    ///
    /// Only available when the `fault_injection` feature is enabled.
    pub fn inject_instructions(
        &mut self,
        extra_instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
    ) {
        match &mut self.entrypoint {
            TransactionEntrypoint::External(entrypoint) => {
                entrypoint.inject_instructions(extra_instructions);
            }
            TransactionEntrypoint::Time(_) => {
                unimplemented!("time-triggered entrypoints are not subject to fault injection")
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

impl QueryRequestWithAuthority {
    /// Sign this [`QueryRequestWithAuthority`], creating a [`SignedQuery`]
    #[inline]
    #[must_use]
    pub fn sign(self, key_pair: &iroha_crypto::KeyPair) -> SignedQuery {
        let signature = SignatureOf::new(key_pair.private_key(), &self);

        SignedQueryV1 {
            signature: QuerySignature(signature),
            payload: self,
        }
        .into()
    }
}

impl SignedQuery {
    /// Get authority that has signed this query
    pub fn authority(&self) -> &AccountId {
        let SignedQuery::V1(query) = self;
        &query.payload.authority
    }

    /// Get the request that was signed
    pub fn request(&self) -> &QueryRequest {
        let SignedQuery::V1(query) = self;
        &query.payload.request
    }
}

mod candidate {
    use parity_scale_codec::Input;

    use super::*;

    #[derive(Decode, Deserialize)]
    struct SignedQueryCandidate {
        signature: QuerySignature,
        payload: QueryRequestWithAuthority,
    }

    impl SignedQueryCandidate {
        fn validate(self) -> Result<SignedQueryV1, &'static str> {
            #[cfg(not(target_family = "wasm"))]
            {
                let QuerySignature(signature) = &self.signature;
                signature
                    .verify(&self.payload.authority.signatory, &self.payload)
                    .map_err(|_| "Query request signature is not valid")?;
            }

            Ok(SignedQueryV1 {
                payload: self.payload,
                signature: self.signature,
            })
        }
    }

    impl Decode for SignedQueryV1 {
        fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
            SignedQueryCandidate::decode(input)?
                .validate()
                .map_err(Into::into)
        }
    }

    impl<'de> Deserialize<'de> for SignedQueryV1 {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::Error as _;

            SignedQueryCandidate::deserialize(deserializer)?
                .validate()
                .map_err(D::Error::custom)
        }
    }

    #[cfg(test)]
    mod tests {
        use std::sync::LazyLock;

        use iroha_crypto::KeyPair;
        use parity_scale_codec::{DecodeAll, Encode};

        use crate::{
            account::AccountId,
            query::{
                candidate::SignedQueryCandidate, FindExecutorDataModel, QueryRequest,
                QuerySignature, SignedQuery, SingularQueryBox,
            },
        };

        static ALICE_ID: LazyLock<AccountId> = LazyLock::new(|| {
            format!("{}@{}", ALICE_KEYPAIR.public_key(), "wonderland")
                .parse()
                .unwrap()
        });
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
            let SignedQuery::V1(signed_query) = QueryRequest::Singular(
                SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel),
            )
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
            let SignedQuery::V1(signed_query) = QueryRequest::Singular(
                SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel),
            )
            .with_authority(ALICE_ID.clone())
            .sign(&ALICE_KEYPAIR);

            let mut candidate = SignedQueryCandidate {
                signature: signed_query.signature,
                payload: signed_query.payload,
            };

            // corrupt the signature by changing a single byte in an encoded signature
            let mut signature_bytes = candidate.signature.encode();
            let idx = signature_bytes.len() - 1;
            signature_bytes[idx] = signature_bytes[idx].wrapping_add(1);
            candidate.signature = QuerySignature::decode_all(&mut &signature_bytes[..]).unwrap();

            assert_eq!(
                candidate.validate().unwrap_err(),
                "Query request signature is not valid"
            );
        }

        #[test]
        fn mismatching_authority() {
            let SignedQuery::V1(signed_query) = QueryRequest::Singular(
                SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel),
            )
            // signing with a wrong key here
            .with_authority(ALICE_ID.clone())
            .sign(&BOB_KEYPAIR);

            let candidate = SignedQueryCandidate {
                signature: signed_query.signature,
                payload: signed_query.payload,
            };

            assert_eq!(
                candidate.validate().unwrap_err(),
                "Query request signature is not valid"
            );
        }
    }
}

/// Use a custom syntax to implement [`Query`] for applicable types
macro_rules! impl_iter_queries {
    ($ty:ty => $item:ty $(, $($rest:tt)*)?) => {
        impl Query for $ty {
            type Item = $item;
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
        impl SingularQuery for $ty {
            type Output = $output;
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
    FindAssets => crate::asset::Asset,
    FindAssetsDefinitions => crate::asset::AssetDefinition,
    FindNfts => crate::nft::Nft,
    FindDomains => crate::domain::Domain,
    FindPeers => crate::peer::PeerId,
    FindActiveTriggerIds => crate::trigger::TriggerId,
    FindTriggers => crate::trigger::Trigger,
    FindTransactions => CommittedTransaction,
    FindAccountsWithAsset => crate::account::Account,
    FindBlockHeaders => crate::block::BlockHeader,
    FindBlocks => SignedBlock,
}

impl_singular_queries! {
    FindParameters => crate::parameter::Parameters,
    FindExecutorDataModel => crate::executor::ExecutorDataModel,
}

/// A macro reducing boilerplate when defining query types.
macro_rules! queries {
    ($($($meta:meta)* $item:item)+) => {
        pub use self::model::*;

        #[iroha_data_model_derive::model]
        mod model{
            use super::*; $(

            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
            #[derive(parity_scale_codec::Decode, parity_scale_codec::Encode)]
            #[derive(serde::Deserialize, serde::Serialize)]
            #[derive(derive_more::Constructor)]
            #[derive(iroha_schema::IntoSchema)]
            $($meta)*
            $item )+
        }
    };
}

pub mod role {
    //! Queries related to [`crate::role`].

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    use crate::prelude::*;

    queries! {
        /// [`FindRoles`] Iroha Query finds all `Role`s presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all roles")]
        #[ffi_type]
        pub struct FindRoles;

        /// [`FindRoleIds`] Iroha Query finds `RoleId`s of
        /// all `Role`s presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all role ids")]
        #[ffi_type]
        pub struct FindRoleIds;

        /// [`FindRolesByAccountId`] Iroha Query finds all `Role`s for a specified account.
        #[derive(Display)]
        #[display(fmt = "Find all roles for `{id}` account")]
        #[repr(transparent)]
        // SAFETY: `FindRolesByAccountId` has no trap representation in `AccountId`
        #[ffi_type(unsafe {robust})]
        pub struct FindRolesByAccountId {
            /// `Id` of an account to find.
            pub id: AccountId,
        }
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this module.
    pub mod prelude {
        pub use super::{FindRoleIds, FindRoles, FindRolesByAccountId};
    }
}

pub mod permission {
    //! Queries related to [`crate::permission`].

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    use crate::prelude::*;

    queries! {
        /// [`FindPermissionsByAccountId`] Iroha Query finds all [`Permission`]s
        /// for a specified account.
        #[derive(Display)]
        #[display(fmt = "Find permission tokens specified for `{id}` account")]
        #[repr(transparent)]
        // SAFETY: `FindPermissionsByAccountId` has no trap representation in `AccountId`
        #[ffi_type(unsafe {robust})]
        pub struct FindPermissionsByAccountId {
            /// `Id` of an account to find.
            pub id: AccountId,
        }
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this module.
    pub mod prelude {
        pub use super::FindPermissionsByAccountId;
    }
}

pub mod account {
    //! Queries related to [`crate::account`].

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    use crate::prelude::*;

    queries! {
        // TODO: Better to have find all account ids query instead.
        /// [`FindAccounts`] Iroha Query finds all `Account`s presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all accounts")]
        #[ffi_type]
        pub struct FindAccounts;

        /// [`FindAccountsWithAsset`] Iroha Query gets [`AssetDefinition`]s id as input and
        /// finds all [`Account`]s storing [`Asset`] with such definition.
        #[derive(Display)]
        #[display(fmt = "Find accounts with `{asset_definition}` asset")]
        #[repr(transparent)]
        // SAFETY: `FindAccountsWithAsset` has no trap representation in `AssetDefinitionId`
        #[ffi_type(unsafe {robust})]
        pub struct FindAccountsWithAsset {
            /// `Id` of the definition of the asset which should be stored in founded accounts.
            pub asset_definition: AssetDefinitionId,
        }
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::{FindAccounts, FindAccountsWithAsset};
    }
}

pub mod asset {
    //! Queries related to [`crate::asset`].

    #![allow(clippy::missing_inline_in_public_items)]

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindAssets`] Iroha Query finds all `Asset`s presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all assets")]
        #[ffi_type]
        pub struct FindAssets;

        /// [`FindAssetsDefinitions`] Iroha Query finds all `AssetDefinition`s presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all asset definitions")]
        #[ffi_type]
        pub struct FindAssetsDefinitions;
    }
    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::{FindAssets, FindAssetsDefinitions};
    }
}

pub mod nft {
    //! Queries related to [`crate::nft`].

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindNfts`] Iroha Query finds all `Nft`s presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all NFTs")]
        #[ffi_type]
        pub struct FindNfts;
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::FindNfts;
    }
}

pub mod domain {
    //! Queries related to [`crate::domain`].

    #![allow(clippy::missing_inline_in_public_items)]

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindDomains`] Iroha Query finds all `Domain`s presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all domains")]
        #[ffi_type]
        pub struct FindDomains;
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::FindDomains;
    }
}

pub mod peer {
    //! Queries related to [`crate::peer`].

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindPeers`] Iroha Query finds all trusted peers presented.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all peers")]
        #[ffi_type]
        pub struct FindPeers;
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::FindPeers;
    }
}

pub mod executor {
    //! Queries related to [`crate::executor`].

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindExecutorDataModel`] Iroha Query finds the data model of the current executor.
        #[derive(Copy, Display)]
        #[display(fmt = "Find executor data model")]
        #[ffi_type]
        pub struct FindExecutorDataModel;

        /// [`FindParameters`] Iroha Query finds all defined executor configuration parameters.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all peers parameters")]
        #[ffi_type]
        pub struct FindParameters;
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::{FindExecutorDataModel, FindParameters};
    }
}

pub mod trigger {
    //! Trigger-related queries.
    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// Find all currently active (as in not disabled and/or expired)
        /// trigger IDs.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all trigger ids")]
        #[ffi_type]
        pub struct FindActiveTriggerIds;

        /// Find all currently active (as in not disabled and/or expired) triggers.
        #[derive(Copy, Display)]
        #[display(fmt = "Find all triggers")]
        #[ffi_type]
        pub struct FindTriggers;
    }

    pub mod prelude {
        //! Prelude Re-exports most commonly used traits, structs and macros from this crate.
        pub use super::{FindActiveTriggerIds, FindTriggers};
    }
}

pub mod transaction {
    //! Queries related to transactions.

    #![allow(clippy::missing_inline_in_public_items)]

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindTransactions`] Iroha Query lists all transactions included in a blockchain
        #[derive(Copy, Display)]
        #[display(fmt = "Find all transactions")]
        #[ffi_type]
        pub struct FindTransactions;
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::FindTransactions;
    }
}

pub mod block {
    //! Queries related to blocks.

    #![allow(clippy::missing_inline_in_public_items)]

    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};

    use derive_more::Display;

    queries! {
        /// [`FindBlocks`] Iroha Query lists all blocks sorted by
        /// height in descending order
        #[derive(Copy, Display)]
        #[display(fmt = "Find all blocks")]
        #[ffi_type]
        pub struct FindBlocks;

        /// [`FindBlockHeaders`] Iroha Query lists all block headers
        /// sorted by height in descending order
        #[derive(Copy, Display)]
        #[display(fmt = "Find all block headers")]
        #[ffi_type]
        pub struct FindBlockHeaders;
    }

    /// The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub mod prelude {
        pub use super::{FindBlockHeaders, FindBlocks};
    }
}

pub mod error {
    //! Module containing errors that can occur during query execution

    use iroha_crypto::HashOf;
    use iroha_data_model_derive::model;
    use iroha_macro::FromVariant;
    use iroha_schema::IntoSchema;
    use parity_scale_codec::{Decode, Encode};

    pub use self::model::*;
    use super::*;
    use crate::prelude::*;

    #[model]
    mod model {
        use super::*;
        use crate::query::parameters::MAX_FETCH_SIZE;

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
            Deserialize,
            Serialize,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(feature = "std", derive(thiserror::Error))]
        pub enum QueryExecutionFail {
            /// {0}
            #[cfg_attr(feature = "std", error(transparent))]
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
            /// fetch_size could not be greater than {MAX_FETCH_SIZE:?}
            FetchSizeTooBig,
            /// Some of the specified parameters (filter/pagination/fetch_size/sorting) are not applicable to singular queries
            InvalidSingularParameters,
            /// Reached the limit of parallel queries. Either wait for previous queries to complete, or increase the limit in the config.
            CapacityLimit,
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
            Deserialize,
            Serialize,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(feature = "std", derive(thiserror::Error))]
        // TODO: Only temporary
        #[ffi_type(opaque)]
        pub enum FindError {
            /// Failed to find asset: `{0}`
            Asset(Box<AssetId>),
            /// Failed to find asset definition: `{0}`
            AssetDefinition(AssetDefinitionId),
            /// Failed to find NFT: `{0}`
            Nft(NftId),
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
        }
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
#[allow(ambiguous_glob_reexports)]
pub mod prelude {
    pub use super::{
        account::prelude::*, asset::prelude::*, block::prelude::*, builder::prelude::*,
        domain::prelude::*, dsl::prelude::*, executor::prelude::*, nft::prelude::*,
        parameters::prelude::*, peer::prelude::*, permission::prelude::*, role::prelude::*,
        transaction::prelude::*, trigger::prelude::*, CommittedTransaction, QueryBox, QueryRequest,
        SingularQueryBox,
    };
}
