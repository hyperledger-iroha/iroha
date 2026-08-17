//! Type-safe native `SoraFS` reputation-journal transaction and query helpers.
use super::{Client, QueryError, QueryResult};
use eyre::{Result, WrapErr as _, bail, eyre};
use iroha_data_model::{
    isi::sorafs::{
        AppendSorafsPorReputationJournalEntry, AppendSorafsStreamTokenReputationJournalEntry,
        SetSorafsReputationJournalAuthorityPolicy,
    },
    metadata::Metadata,
    query::sorafs::prelude::{
        FindSorafsReputationJournalAuthorityPolicy, FindSorafsReputationJournalEventBySourceId,
        FindSorafsReputationJournalEvents,
    },
    sorafs::reputation::{
        REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1, ReputationJournalAuthorityPolicyRecordV1,
        ReputationJournalAuthorityPolicyV1, ReputationJournalEntryV1,
        ReputationJournalFinalizedCursorV1, ReputationJournalFinalizedEventCursorV1,
        ReputationJournalFinalizedEventPageV1, ReputationJournalFinalizedEventV1,
        ReputationJournalSourceIdV1, ReputationJournalSourceKindV1,
    },
    transaction::{FeePaymentIntent, SignedTransaction},
};
fn validate_entry_for_transaction(
    client: &Client,
    entry: &ReputationJournalEntryV1,
    expected_kind: ReputationJournalSourceKindV1,
) -> Result<()> {
    entry
        .validate()
        .wrap_err("invalid canonical SoraFS reputation-journal entry")?;
    if entry.source_kind() != expected_kind {
        bail!("SoraFS reputation-journal transaction accepts only {expected_kind:?} entries");
    }
    if entry.recorded_by != client.account {
        bail!(
            "SoraFS reputation-journal entry recorded_by must equal the client transaction authority"
        );
    }
    Ok(())
}
fn query_validation_error(context: &'static str, error: impl std::fmt::Display) -> QueryError {
    QueryError::Other(eyre!("{context}: {error}"))
}
impl Client {
    /// Build and sign a native transaction that activates a reputation-journal authority policy.
    ///
    /// Submission remains caller-controlled through [`Self::submit_transaction`] or
    /// [`Self::submit_transaction_blocking`]. The transaction authority needs
    /// `CanManageSorafsReputationJournalPolicy` at execution time.
    ///
    /// # Errors
    /// Returns an error for an invalid V1 policy, nonce entropy failure, or signing failure.
    pub fn try_build_sorafs_reputation_journal_authority_policy_transaction(
        &self,
        policy: ReputationJournalAuthorityPolicyV1,
        fee_payment: FeePaymentIntent,
        metadata: Metadata,
    ) -> Result<SignedTransaction> {
        policy
            .validate()
            .wrap_err("invalid SoraFS reputation-journal authority policy")?;
        self.try_build_transaction_from_items(
            [SetSorafsReputationJournalAuthorityPolicy::new(policy)],
            fee_payment,
            metadata,
        )
    }
    /// Build and sign a native transaction that appends one canonical `PoR` journal entry.
    ///
    /// Submission remains caller-controlled. The client account must equal `entry.recorded_by`
    /// and needs `CanRecordSorafsReputationJournal` at execution time.
    ///
    /// # Errors
    /// Returns an error for an invalid or wrong-family entry, authority mismatch, nonce entropy
    /// failure, or signing failure.
    pub fn try_build_sorafs_reputation_journal_por_entry_transaction(
        &self,
        entry: ReputationJournalEntryV1,
        fee_payment: FeePaymentIntent,
        metadata: Metadata,
    ) -> Result<SignedTransaction> {
        validate_entry_for_transaction(self, &entry, ReputationJournalSourceKindV1::Por)?;
        self.try_build_transaction_from_items(
            [AppendSorafsPorReputationJournalEntry::new(entry)],
            fee_payment,
            metadata,
        )
    }
    /// Build and sign a native transaction that appends one canonical stream-token journal entry.
    ///
    /// Submission remains caller-controlled. The client account must equal `entry.recorded_by`
    /// and needs `CanRecordSorafsReputationJournal` at execution time.
    ///
    /// # Errors
    /// Returns an error for an invalid or wrong-family entry, authority mismatch, nonce entropy
    /// failure, or signing failure.
    pub fn try_build_sorafs_reputation_journal_stream_token_entry_transaction(
        &self,
        entry: ReputationJournalEntryV1,
        fee_payment: FeePaymentIntent,
        metadata: Metadata,
    ) -> Result<SignedTransaction> {
        validate_entry_for_transaction(self, &entry, ReputationJournalSourceKindV1::StreamToken)?;
        self.try_build_transaction_from_items(
            [AppendSorafsStreamTokenReputationJournalEntry::new(entry)],
            fee_payment,
            metadata,
        )
    }
    /// Query the active chain-authoritative reputation-journal authority policy.
    ///
    /// The request is account-authenticated. The authority needs any one of
    /// `CanManageSorafsReputationJournalPolicy`, `CanRecordSorafsReputationJournal`, or
    /// `CanResolveSorafsCapacityDispute`.
    ///
    /// # Errors
    /// Returns an error if request signing, transport, authorization, decoding, or validation
    /// fails.
    pub fn query_sorafs_reputation_journal_authority_policy(
        &self,
    ) -> QueryResult<ReputationJournalAuthorityPolicyRecordV1> {
        let record = self.query_single(FindSorafsReputationJournalAuthorityPolicy)?;
        record.validate().map_err(|error| {
            query_validation_error(
                "invalid SoraFS reputation-journal authority-policy response",
                error,
            )
        })?;
        Ok(record)
    }
    /// Query one finalized reputation-journal event by its native source identifier.
    ///
    /// The request is account-authenticated even though the journal event is public transparency
    /// state. Supply `expected_finalized_cursor` to pin the exact immutable view.
    ///
    /// # Errors
    /// Returns an error for an inert cursor/source identifier, request signing, transport,
    /// decoding, or a response that violates the requested source/finality binding.
    pub fn query_sorafs_reputation_journal_event_by_source_id(
        &self,
        source_id: ReputationJournalSourceIdV1,
        expected_finalized_cursor: Option<ReputationJournalFinalizedCursorV1>,
    ) -> QueryResult<ReputationJournalFinalizedEventV1> {
        if source_id == ReputationJournalSourceIdV1::ZERO {
            return Err(QueryError::Other(eyre!(
                "SoraFS reputation-journal source identifier must be non-zero"
            )));
        }
        if let Some(cursor) = expected_finalized_cursor {
            cursor.validate().map_err(|error| {
                query_validation_error(
                    "invalid expected SoraFS reputation-journal finalized cursor",
                    error,
                )
            })?;
        }
        let event = self.query_single(FindSorafsReputationJournalEventBySourceId::new(
            source_id,
            expected_finalized_cursor,
        ))?;
        if event.entry.source_id != source_id {
            return Err(QueryError::Other(eyre!(
                "SoraFS reputation-journal source response does not match the requested source"
            )));
        }
        let response_cursor = ReputationJournalFinalizedCursorV1 {
            height: event.block_height,
            block_hash: event.block_hash,
            finalized_at_unix_ms: event.recorded_at_unix_ms,
        };
        event.validate(response_cursor).map_err(|error| {
            query_validation_error("invalid SoraFS reputation-journal source response", error)
        })?;
        if let Some(cursor) = expected_finalized_cursor {
            event.validate(cursor).map_err(|error| {
                query_validation_error(
                    "invalid finalized SoraFS reputation-journal source response",
                    error,
                )
            })?;
        }
        Ok(event)
    }
    /// Query a bounded exclusive-cursor page from the global finalized reputation journal.
    ///
    /// The request is account-authenticated even though the event page is public transparency
    /// state. Reuse the returned page's finalized cursor for every continuation.
    ///
    /// # Errors
    /// Returns an error for invalid cursors, a limit outside the V1 bound, request signing,
    /// transport, decoding, or malformed response pagination/finality.
    pub fn query_sorafs_reputation_journal_events(
        &self,
        expected_finalized_cursor: Option<ReputationJournalFinalizedCursorV1>,
        after: Option<ReputationJournalFinalizedEventCursorV1>,
        limit: u32,
    ) -> QueryResult<ReputationJournalFinalizedEventPageV1> {
        if let Some(cursor) = expected_finalized_cursor {
            cursor.validate().map_err(|error| {
                query_validation_error(
                    "invalid expected SoraFS reputation-journal finalized cursor",
                    error,
                )
            })?;
        }
        if let Some(cursor) = after {
            cursor.validate().map_err(|error| {
                query_validation_error("invalid SoraFS reputation-journal event cursor", error)
            })?;
        }
        if let (Some(expected), Some(after)) = (expected_finalized_cursor, after) {
            let outside_expected_view = match after.block_height.cmp(&expected.height) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => after.block_hash != expected.block_hash,
                std::cmp::Ordering::Less => false,
            };
            if outside_expected_view {
                return Err(QueryError::Other(eyre!(
                    "SoraFS reputation-journal event cursor does not belong to the expected finalized view"
                )));
            }
        }
        if limit == 0
            || usize::try_from(limit)
                .ok()
                .is_none_or(|limit| limit > REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
        {
            return Err(QueryError::Other(eyre!(
                "SoraFS reputation-journal query limit must be within 1..={REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1}"
            )));
        }
        let page = self.query_single(FindSorafsReputationJournalEvents::new(
            expected_finalized_cursor,
            after,
            limit,
        ))?;
        if expected_finalized_cursor.is_some_and(|expected| expected != page.finalized_cursor) {
            return Err(QueryError::Other(eyre!(
                "SoraFS reputation-journal page response changed the requested finalized cursor"
            )));
        }
        if page.events.len() > usize::try_from(limit).expect("u32 always fits supported usize") {
            return Err(QueryError::Other(eyre!(
                "SoraFS reputation-journal page response exceeds the requested item limit"
            )));
        }
        if after.is_none() && page.events.first().is_some_and(|event| event.sequence != 1) {
            return Err(QueryError::Other(eyre!(
                "initial SoraFS reputation-journal page response must begin at sequence one"
            )));
        }
        page.validate_after(after).map_err(|error| {
            query_validation_error("invalid SoraFS reputation-journal event page", error)
        })?;
        Ok(page)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        client::evidence_http_tests::{
            SnapshotStore, base_url, client_with_base_url, mark_data_model_compatible,
            with_mock_http,
        },
        http::{Response as HttpResponse, StatusCode},
        http_default::RequestSnapshot,
    };
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        isi::sorafs::{
            AppendSorafsPorReputationJournalEntry, AppendSorafsStreamTokenReputationJournalEntry,
            SetSorafsReputationJournalAuthorityPolicy,
        },
        query::{
            QueryRequest, QueryResponse, SignedQuery, SingularQueryBox, SingularQueryOutputBox,
        },
        sorafs::{
            capacity::ProviderId,
            reputation::{
                PorTerminalOutcomeV1, PorTerminalStatusV1,
                REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
                ReputationJournalAuthorityPolicyRecordV1, ReputationJournalEntryV1,
                ReputationJournalEventIdV1, ReputationJournalFinalizedCursorV1,
                ReputationJournalFinalizedEventCursorV1, ReputationJournalFinalizedEventPageV1,
                ReputationJournalPayloadV1, StreamTokenValidationBindingV1,
                StreamTokenValidationOutcomeV1, StreamTokenValidationStatusV1,
            },
        },
        transaction::{Executable, FeePaymentIntent, SignedTransaction},
    };
    use iroha_version::codec::DecodeVersioned as _;
    use std::sync::{Arc, Mutex};
    const SOURCE_TIME_MS: u64 = 1_700_000_000_000;
    fn policy(authority: &AccountId) -> ReputationJournalAuthorityPolicyV1 {
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: authority.clone(),
            dispute_recorder_authority: authority.clone(),
            token_recorder_authority: authority.clone(),
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        }
    }
    fn por_entry(
        authority: &AccountId,
        policy: &ReputationJournalAuthorityPolicyV1,
    ) -> ReputationJournalEntryV1 {
        ReputationJournalEntryV1::try_new(
            ProviderId::new([0x21; 32]),
            policy.canonical_digest().expect("policy digest"),
            authority.clone(),
            SOURCE_TIME_MS,
            None,
            ReputationJournalPayloadV1::PorTerminal(PorTerminalOutcomeV1 {
                challenge_id: [0x31; 32],
                manifest_digest: [0x32; 32],
                epoch_id: 1,
                drand_round: 2,
                forced: false,
                sample_count: 4,
                failed_samples: 0,
                issued_at_unix_ms: SOURCE_TIME_MS - 2_000,
                deadline_at_unix_ms: SOURCE_TIME_MS,
                responded_at_unix_ms: Some(SOURCE_TIME_MS - 1_000),
                decided_at_unix_ms: SOURCE_TIME_MS,
                proof_digest: Some([0x33; 32]),
                repair_task_id: None,
                verifier_latency_ms: Some(7),
                status: PorTerminalStatusV1::Verified,
            }),
        )
        .expect("canonical PoR entry")
    }
    fn token_entry(
        authority: &AccountId,
        policy: &ReputationJournalAuthorityPolicyV1,
    ) -> ReputationJournalEntryV1 {
        ReputationJournalEntryV1::try_new(
            ProviderId::new([0x41; 32]),
            policy.canonical_digest().expect("policy digest"),
            authority.clone(),
            SOURCE_TIME_MS,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(StreamTokenValidationOutcomeV1 {
                binding: StreamTokenValidationBindingV1 {
                    gateway_id: [0x42; 32],
                    gateway_sequence: 1,
                    request_context_digest: [0x43; 32],
                },
                token_body_digest: Some([0x44; 32]),
                token_key_version: Some(1),
                validated_at_unix_ms: SOURCE_TIME_MS,
                status: StreamTokenValidationStatusV1::Accepted,
            }),
        )
        .expect("canonical stream-token entry")
    }
    fn assert_exact_instruction<I: 'static + PartialEq + std::fmt::Debug>(
        client: &Client,
        transaction: &SignedTransaction,
        fee_payment: &FeePaymentIntent,
        metadata: &Metadata,
        expected: &I,
    ) {
        transaction
            .verify_signature()
            .expect("transaction signature");
        assert_eq!(transaction.payload().network_id(), Some(&client.network_id));
        assert_eq!(transaction.authority(), &client.account);
        assert_eq!(transaction.fee_payment_intent(), fee_payment);
        assert_eq!(transaction.metadata(), metadata);
        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("reputation transaction must contain instructions");
        };
        assert_eq!(instructions.len(), 1);
        let actual = instructions[0]
            .as_any()
            .downcast_ref::<I>()
            .expect("exact reputation instruction type");
        assert_eq!(actual, expected);
    }
    #[test]
    fn transaction_builders_sign_exact_typed_instruction() {
        let client = client_with_base_url(base_url());
        let policy = policy(&client.account);
        let canonical_por = por_entry(&client.account, &policy);
        let canonical_token = token_entry(&client.account, &policy);
        let fee_payment = FeePaymentIntent::authority(Vec::new(), None);
        let metadata = Metadata::default();
        let transaction = client
            .try_build_sorafs_reputation_journal_authority_policy_transaction(
                policy.clone(),
                fee_payment.clone(),
                metadata.clone(),
            )
            .expect("policy transaction");
        assert_exact_instruction(
            &client,
            &transaction,
            &fee_payment,
            &metadata,
            &SetSorafsReputationJournalAuthorityPolicy::new(policy.clone()),
        );
        let transaction = client
            .try_build_sorafs_reputation_journal_por_entry_transaction(
                canonical_por.clone(),
                fee_payment.clone(),
                metadata.clone(),
            )
            .expect("PoR transaction");
        assert_exact_instruction(
            &client,
            &transaction,
            &fee_payment,
            &metadata,
            &AppendSorafsPorReputationJournalEntry::new(canonical_por),
        );
        let transaction = client
            .try_build_sorafs_reputation_journal_stream_token_entry_transaction(
                canonical_token.clone(),
                fee_payment.clone(),
                metadata.clone(),
            )
            .expect("stream-token transaction");
        assert_exact_instruction(
            &client,
            &transaction,
            &fee_payment,
            &metadata,
            &AppendSorafsStreamTokenReputationJournalEntry::new(canonical_token),
        );
    }
    #[test]
    fn transaction_builders_reject_invalid_family_and_authority_before_signing() {
        let client = client_with_base_url(base_url());
        let policy = policy(&client.account);
        let canonical_por = por_entry(&client.account, &policy);
        let canonical_token = token_entry(&client.account, &policy);
        let fee = || FeePaymentIntent::authority(Vec::new(), None);
        let mut invalid_policy = policy.clone();
        invalid_policy.revision = 0;
        let other = AccountId::new(
            KeyPair::try_random()
                .expect("key pair")
                .public_key()
                .clone(),
        );
        let wrong_authority = por_entry(&other, &policy);
        let mut malformed = canonical_por.clone();
        malformed.event_id = ReputationJournalEventIdV1::default();
        for result in [
            client
                .try_build_sorafs_reputation_journal_authority_policy_transaction(
                    invalid_policy,
                    fee(),
                    Metadata::default(),
                )
                .map(drop),
            client
                .try_build_sorafs_reputation_journal_por_entry_transaction(
                    canonical_token,
                    fee(),
                    Metadata::default(),
                )
                .map(drop),
            client
                .try_build_sorafs_reputation_journal_stream_token_entry_transaction(
                    canonical_por.clone(),
                    fee(),
                    Metadata::default(),
                )
                .map(drop),
            client
                .try_build_sorafs_reputation_journal_por_entry_transaction(
                    wrong_authority,
                    fee(),
                    Metadata::default(),
                )
                .map(drop),
            client
                .try_build_sorafs_reputation_journal_por_entry_transaction(
                    malformed,
                    fee(),
                    Metadata::default(),
                )
                .map(drop),
        ] {
            assert!(result.is_err());
        }
    }
    fn norito_response(response: &QueryResponse) -> HttpResponse<Vec<u8>> {
        HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", super::super::APPLICATION_NORITO)
            .body(norito::to_bytes(response).expect("encode query response"))
            .expect("response")
    }
    fn assert_signed_singular_query(
        snapshot: &RequestSnapshot,
        client: &Client,
        check: impl FnOnce(&SingularQueryBox),
    ) {
        assert_eq!(snapshot.url.path(), iroha_torii_shared::uri::QUERY);
        let signed = SignedQuery::decode_all_versioned(&snapshot.body).expect("signed query");
        signed.verify_signature().expect("query signature");
        assert_eq!(signed.payload.network_id, client.network_id);
        assert_eq!(signed.authority(), &client.account);
        assert!(signed.payload.creation_time_ms > 0);
        assert!(signed.payload.time_to_live_ms.get() > 0);
        assert_ne!(signed.payload.nonce, [0; 32]);
        let QueryRequest::Singular(query) = signed.request() else {
            panic!("expected singular reputation query");
        };
        check(query);
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the authenticated journal-query fixture audits all exact request and response bindings together"
    )]
    fn typed_queries_are_authenticated_and_preserve_exact_fields() {
        let client = client_with_base_url(base_url());
        mark_data_model_compatible(&client);
        let policy = policy(&client.account);
        let policy_record = ReputationJournalAuthorityPolicyRecordV1::try_new(
            policy,
            client.account.clone(),
            SOURCE_TIME_MS,
        )
        .expect("policy record");
        let finalized_cursor = ReputationJournalFinalizedCursorV1 {
            height: 7,
            block_hash: [0x51; 32],
            finalized_at_unix_ms: SOURCE_TIME_MS + 1,
        };
        let after = ReputationJournalFinalizedEventCursorV1 {
            sequence: 3,
            block_height: 6,
            block_hash: [0x52; 32],
            event_index: 0,
        };
        let source_entry = por_entry(&client.account, &policy_record.policy);
        let source_id = source_entry.source_id;
        let source_event = ReputationJournalFinalizedEventV1 {
            sequence: 2,
            block_height: 6,
            block_hash: [0x53; 32],
            event_index: 0,
            recorded_at_unix_ms: SOURCE_TIME_MS,
            entry: source_entry,
        };
        let page = ReputationJournalFinalizedEventPageV1 {
            finalized_cursor,
            events: Vec::new(),
            has_more: false,
            next_after: None,
        };
        let responses: Arc<Mutex<Vec<QueryResponse>>> = Arc::new(Mutex::new(vec![
            QueryResponse::Singular(SingularQueryOutputBox::from(page.clone())),
            QueryResponse::Singular(SingularQueryOutputBox::from(source_event.clone())),
            QueryResponse::Singular(SingularQueryOutputBox::from(policy_record.clone())),
        ]));
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = {
            let responses = Arc::clone(&responses);
            let snapshots = Arc::clone(&snapshots);
            move |snapshot: RequestSnapshot| {
                snapshots.lock().expect("snapshots").push(snapshot);
                let response = responses
                    .lock()
                    .expect("responses")
                    .pop()
                    .expect("response");
                Ok(norito_response(&response))
            }
        };
        with_mock_http(responder, || {
            assert_eq!(
                client
                    .query_sorafs_reputation_journal_authority_policy()
                    .expect("policy query"),
                policy_record
            );
            assert_eq!(
                client
                    .query_sorafs_reputation_journal_event_by_source_id(
                        source_id,
                        Some(finalized_cursor),
                    )
                    .expect("source query"),
                source_event
            );
            assert_eq!(
                client
                    .query_sorafs_reputation_journal_events(Some(finalized_cursor), Some(after), 4,)
                    .expect("events query"),
                page
            );
        });
        let snapshots = snapshots.lock().expect("snapshots");
        assert_eq!(snapshots.len(), 3);
        assert_signed_singular_query(&snapshots[0], &client, |query| {
            assert!(matches!(
                query,
                SingularQueryBox::FindSorafsReputationJournalAuthorityPolicy(_)
            ));
        });
        assert_signed_singular_query(&snapshots[1], &client, |query| {
            let SingularQueryBox::FindSorafsReputationJournalEventBySourceId(query) = query else {
                panic!("source query variant");
            };
            assert_eq!(query.source_id(), source_id);
            assert_eq!(query.expected_finalized_cursor(), Some(finalized_cursor));
        });
        assert_signed_singular_query(&snapshots[2], &client, |query| {
            let SingularQueryBox::FindSorafsReputationJournalEvents(query) = query else {
                panic!("events query variant");
            };
            assert_eq!(query.expected_finalized_cursor, Some(finalized_cursor));
            assert_eq!(query.after, Some(after));
            assert_eq!(query.limit, 4);
        });
        assert_ne!(
            SignedQuery::decode_all_versioned(&snapshots[0].body)
                .expect("first query")
                .payload
                .nonce,
            SignedQuery::decode_all_versioned(&snapshots[1].body)
                .expect("second query")
                .payload
                .nonce
        );
        assert_ne!(
            SignedQuery::decode_all_versioned(&snapshots[1].body)
                .expect("second query")
                .payload
                .nonce,
            SignedQuery::decode_all_versioned(&snapshots[2].body)
                .expect("third query")
                .payload
                .nonce
        );
    }
    #[test]
    fn unpinned_source_query_rejects_malformed_event_response() {
        let client = client_with_base_url(base_url());
        mark_data_model_compatible(&client);
        let policy = policy(&client.account);
        let entry = por_entry(&client.account, &policy);
        let source_id = entry.source_id;
        let malformed = ReputationJournalFinalizedEventV1 {
            sequence: 1,
            block_height: 1,
            block_hash: [0x61; 32],
            event_index: 0,
            recorded_at_unix_ms: 0,
            entry,
        };
        let response = QueryResponse::Singular(SingularQueryOutputBox::from(malformed));
        let sends: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = {
            let sends = Arc::clone(&sends);
            move |snapshot| {
                sends.lock().expect("sends").push(snapshot);
                Ok(norito_response(&response))
            }
        };
        let result = with_mock_http(responder, || {
            client.query_sorafs_reputation_journal_event_by_source_id(source_id, None)
        });
        assert!(result.is_err());
        assert_eq!(sends.lock().expect("sends").len(), 1);
    }
    fn finalized_event(
        sequence: u64,
        block_height: u64,
        block_hash: [u8; 32],
        event_index: u32,
        entry: ReputationJournalEntryV1,
    ) -> ReputationJournalFinalizedEventV1 {
        ReputationJournalFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index,
            recorded_at_unix_ms: SOURCE_TIME_MS,
            entry,
        }
    }
    #[test]
    fn event_page_query_rejects_responses_outside_request_bounds() {
        let client = client_with_base_url(base_url());
        mark_data_model_compatible(&client);
        let policy = policy(&client.account);
        let first = por_entry(&client.account, &policy);
        let second = token_entry(&client.account, &policy);
        let finalized_cursor = ReputationJournalFinalizedCursorV1 {
            height: 3,
            block_hash: [0x66; 32],
            finalized_at_unix_ms: SOURCE_TIME_MS + 1,
        };
        let oversized = ReputationJournalFinalizedEventPageV1 {
            finalized_cursor,
            events: vec![
                finalized_event(1, 1, [0x64; 32], 0, first.clone()),
                finalized_event(2, 2, [0x65; 32], 0, second),
            ],
            has_more: false,
            next_after: None,
        };
        oversized.validate().expect("protocol-valid oversized page");
        let shifted = ReputationJournalFinalizedEventPageV1 {
            finalized_cursor,
            events: vec![finalized_event(2, 2, [0x65; 32], 0, first)],
            has_more: false,
            next_after: None,
        };
        shifted.validate().expect("protocol-valid shifted page");
        let responses: Arc<Mutex<Vec<QueryResponse>>> = Arc::new(Mutex::new(vec![
            QueryResponse::Singular(SingularQueryOutputBox::from(shifted)),
            QueryResponse::Singular(SingularQueryOutputBox::from(oversized)),
        ]));
        let responder = {
            let responses = Arc::clone(&responses);
            move |_| {
                let response = responses
                    .lock()
                    .expect("responses")
                    .pop()
                    .expect("response");
                Ok(norito_response(&response))
            }
        };
        with_mock_http(responder, || {
            assert!(
                client
                    .query_sorafs_reputation_journal_events(Some(finalized_cursor), None, 1,)
                    .is_err()
            );
            assert!(
                client
                    .query_sorafs_reputation_journal_events(Some(finalized_cursor), None, 1,)
                    .is_err()
            );
        });
    }
    #[test]
    fn query_validation_rejects_bad_inputs_without_http() {
        let client = client_with_base_url(base_url());
        mark_data_model_compatible(&client);
        let sends: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = {
            let sends = Arc::clone(&sends);
            move |snapshot| {
                sends.lock().expect("sends").push(snapshot);
                panic!("invalid query must not send HTTP")
            }
        };
        with_mock_http(responder, || {
            let invalid_finalized = ReputationJournalFinalizedCursorV1 {
                height: 0,
                block_hash: [0; 32],
                finalized_at_unix_ms: 0,
            };
            let invalid_after = ReputationJournalFinalizedEventCursorV1 {
                sequence: 0,
                block_height: 0,
                block_hash: [0; 32],
                event_index: 0,
            };
            let finalized = ReputationJournalFinalizedCursorV1 {
                height: 7,
                block_hash: [0x71; 32],
                finalized_at_unix_ms: SOURCE_TIME_MS,
            };
            let cursor_beyond_finality = ReputationJournalFinalizedEventCursorV1 {
                sequence: 8,
                block_height: 8,
                block_hash: [0x72; 32],
                event_index: 0,
            };
            for result in [
                client
                    .query_sorafs_reputation_journal_event_by_source_id(
                        ReputationJournalSourceIdV1::ZERO,
                        None,
                    )
                    .map(drop),
                client
                    .query_sorafs_reputation_journal_events(Some(invalid_finalized), None, 1)
                    .map(drop),
                client
                    .query_sorafs_reputation_journal_events(None, Some(invalid_after), 1)
                    .map(drop),
                client
                    .query_sorafs_reputation_journal_events(
                        Some(finalized),
                        Some(cursor_beyond_finality),
                        1,
                    )
                    .map(drop),
                client
                    .query_sorafs_reputation_journal_events(None, None, 0)
                    .map(drop),
                client
                    .query_sorafs_reputation_journal_events(
                        None,
                        None,
                        u32::try_from(REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
                            .expect("limit fits u32")
                            + 1,
                    )
                    .map(drop),
            ] {
                assert!(result.is_err());
            }
        });
        assert!(sends.lock().expect("sends").is_empty());
    }
}
