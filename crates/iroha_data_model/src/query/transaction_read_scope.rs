//! Narrow committed-transaction recovery reads. No new wire format is introduced.
use super::{
    CommittedTransaction, CommittedTxFilters, QueryItemKind, QueryWithParams,
    dsl::{CompoundPredicate, SelectorTuple},
    parameters::QueryParams,
    transaction::prelude::FindTransactions,
};
use crate::account::AccountId;
use norito::codec::Encode;

const MAX_EXACT_TRANSACTION_PREDICATE_BYTES: usize = 16 * 1024;

impl QueryWithParams {
    /// Recognize only one canonical full-row recovery query: an exact native
    /// transaction authority AND an exact entrypoint hash, with default params.
    ///
    /// This is a classification helper, never an authorization decision. The
    /// caller must still authenticate its query authority and enforce self or
    /// exact `CanReadAccountData` access to the returned account. The caller's
    /// admitted decode budget is retained; no cursor or hash-only query is
    /// downscoped. Unknown predicate forms retain ledger-wide authorization.
    ///
    /// # Errors
    /// Returns a native codec error for malformed or resource-exceeding input.
    pub fn exact_transaction_read_authority_with_limits(
        &self,
        limits: norito::DecodeLimits,
    ) -> Result<Option<AccountId>, norito::core::Error> {
        if self.item != QueryItemKind::CommittedTransaction
            || self.query_payload != FindTransactions::new().encode()
            || self.selector_bytes != SelectorTuple::<CommittedTransaction>::default().encode()
            || self.params != QueryParams::default()
            || self.predicate_bytes.len() > MAX_EXACT_TRANSACTION_PREDICATE_BYTES
        {
            return Ok(None);
        }
        norito::with_decode_limits(limits, || {
            let predicate = norito::codec::decode_adaptive::<
                CompoundPredicate<CommittedTransaction>,
            >(&self.predicate_bytes)?;
            let Some(filters) = predicate.committed_tx_filters() else {
                return Ok(None);
            };
            let (Some(authority), Some(entrypoint_hash)) = (filters.authority_eq, filters.entry_eq)
            else {
                return Ok(None);
            };
            // The flat filter view is an index hint, not an authorization
            // proof. Reconstruct only the two permitted typed predicates and
            // require byte equality with the original. This rejects lossy
            // flattening, duplicates, nesting, OR/NOT, extra constraints,
            // noncanonical JSON carriers, and trailing bytes.
            let exact = CompoundPredicate::from_filters(CommittedTxFilters {
                authority_eq: Some(authority.clone()),
                entry_eq: Some(entrypoint_hash),
                ..CommittedTxFilters::default()
            });
            Ok((exact.encode() == self.predicate_bytes).then_some(authority))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{Query, dsl::CommittedTxPredicate as P};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    fn authority(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    }
    fn hash() -> HashOf<crate::transaction::TransactionEntrypoint> {
        HashOf::from_untyped_unchecked(Hash::new(b"exact-transaction-read-fixture"))
    }
    fn query(predicate: CompoundPredicate<CommittedTransaction>) -> QueryWithParams {
        let query = FindTransactions::new();
        QueryWithParams {
            query: (),
            query_payload: query.dyn_encode(),
            item: query.query_item_kind(),
            predicate_bytes: predicate.encode(),
            selector_bytes: SelectorTuple::<CommittedTransaction>::default().encode(),
            params: QueryParams::default(),
        }
    }
    fn exact() -> QueryWithParams {
        query(CompoundPredicate::from_filters(CommittedTxFilters {
            authority_eq: Some(authority(7)),
            entry_eq: Some(hash()),
            ..CommittedTxFilters::default()
        }))
    }
    fn classify(query: &QueryWithParams) -> Option<AccountId> {
        query
            .exact_transaction_read_authority_with_limits(norito::canonical_decode_limits(
                query.predicate_bytes.len(),
            ))
            .ok()
            .flatten()
    }

    #[test]
    fn exact_transaction_read_scope_accepts_only_canonical_authority_and_hash() {
        assert_eq!(classify(&exact()), Some(authority(7)));
        let account = authority(7);
        for tree in [
            P::EntryEq(hash()),
            P::AuthorityEq(account.clone()),
            P::And(vec![
                P::AuthorityEq(account.clone()),
                P::EntryEq(hash()),
                P::ResultEq(true),
            ]),
            P::And(vec![
                P::AuthorityEq(account.clone()),
                P::AuthorityEq(account.clone()),
                P::EntryEq(hash()),
            ]),
            P::And(vec![
                P::AuthorityEq(account.clone()),
                P::AuthorityEq(authority(9)),
                P::EntryEq(hash()),
            ]),
            P::And(vec![P::EntryEq(hash()), P::AuthorityEq(account.clone())]),
            P::And(vec![P::And(vec![
                P::AuthorityEq(account.clone()),
                P::EntryEq(hash()),
            ])]),
            P::Or(vec![P::AuthorityEq(account.clone()), P::EntryEq(hash())]),
            P::Not(Box::new(P::And(vec![
                P::AuthorityEq(account.clone()),
                P::EntryEq(hash()),
            ]))),
            P::AuthorityIn(vec![account]),
            P::Const(true),
            P::Const(false),
        ] {
            let query = query(CompoundPredicate::from_committed_tx_predicate(tree));
            assert_eq!(classify(&query), None);
        }
    }

    #[test]
    fn exact_transaction_read_scope_rejects_envelope_substitution_and_resource_exhaustion() {
        for mutation in [
            "kind",
            "query",
            "selector",
            "params",
            "trailing",
            "oversized",
            "malformed",
        ] {
            let mut query = exact();
            match mutation {
                "kind" => query.item = QueryItemKind::Account,
                "query" => query.query_payload.push(1),
                "selector" => query.selector_bytes.push(1),
                "params" => query.params.pagination.offset = 1,
                "trailing" => query.predicate_bytes.push(1),
                "oversized" => {
                    query.predicate_bytes = vec![0; MAX_EXACT_TRANSACTION_PREDICATE_BYTES + 1]
                }
                _ => query.predicate_bytes = vec![0xff; 32],
            }
            assert_eq!(classify(&query), None, "accepted {mutation}");
        }
        assert!(
            exact()
                .exact_transaction_read_authority_with_limits(norito::DecodeLimits::new(
                    1, 1, 1, 1, 1
                ))
                .is_err()
        );
    }

    #[test]
    fn exact_transaction_read_scope_uses_strict_current_json_predicate_names() {
        let canonical = norito::json::to_json(&P::And(vec![
            P::AuthorityEq(authority(7)),
            P::EntryEq(hash()),
        ]))
        .unwrap();
        let decoded = norito::json::from_json::<P>(&canonical).unwrap();
        assert_eq!(
            classify(&query(CompoundPredicate::from_committed_tx_predicate(
                decoded
            ))),
            Some(authority(7))
        );
        for (old, replacement) in [
            ("\"authority\"", "\"account_id\""),
            ("\"entrypoint_hash\"", "\"transaction_hash\""),
            ("\"eq\"", "\"equals\""),
            ("\"and\"", "\"AND\""),
        ] {
            assert!(canonical.contains(old));
            assert!(norito::json::from_json::<P>(&canonical.replace(old, replacement)).is_err());
        }
        let unknown = format!(
            "{},\"unapproved\":true}}",
            &canonical[..canonical.len() - 1]
        );
        assert!(norito::json::from_json::<P>(&unknown).is_err());
        let duplicate = format!("{{\"op\":\"and\",{}", &canonical[1..]);
        assert!(norito::json::from_json::<P>(&duplicate).is_err());
    }
}
