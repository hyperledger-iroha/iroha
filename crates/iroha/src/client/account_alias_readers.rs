//! Bounded authenticated identity reads across explicitly selected ledger readers.

use super::*;
use std::{io::ErrorKind, time::Instant};

const MAX_IDENTITY_READERS: usize = 16;

fn unavailable_reader(error: &eyre::Report) -> bool {
    let transient_io = |kind| {
        matches!(
            kind,
            ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::NotConnected
                | ErrorKind::BrokenPipe
                | ErrorKind::TimedOut
                | ErrorKind::UnexpectedEof
        )
    };
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<crate::Error>() {
            return match error {
                crate::Error::Timeout { .. } => true,
                crate::Error::Transport {
                    kind: crate::TransportErrorKind::Io(kind),
                    ..
                } => transient_io(*kind),
                _ => matches!(error.http_status(), Some(502..=504)),
            };
        }
    }
    false
}

impl Client {
    /// Resolve an alias through this reader and explicitly selected alternative readers.
    ///
    /// At most 16 readers, including this reader, may be selected. Every reader
    /// must retain the same network, account, signer and address profile.
    /// Each is tried at most once, with its own configured transport and listener
    /// credentials and fresh canonical request authentication. Only connection/deadline
    /// failures and HTTP 502/503/504 advance to another reader. Exact absence,
    /// authorization refusals and invalid or substituted responses never do.
    ///
    /// The shortest configured request timeout and inherited deadline bound the whole
    /// lookup. Each attempt receives a share of the remaining time, so an unresponsive
    /// reader cannot spend the other readers' entire budget.
    ///
    /// # Errors
    /// Returns an error for inconsistent or duplicate reader contexts, an unbounded
    /// reader set, exhausted deadline, signing/transport failure, or a rejected or
    /// malformed canonical response. No cached identity or unsigned read is used.
    pub fn resolve_account_alias_authenticated_with_readers(
        &self,
        alias: &AccountAliasName,
        alternatives: &[&Client],
    ) -> Result<Option<AccountAliasResolutionV1>> {
        self.with_available_identity_reader(alternatives, |reader| {
            reader.resolve_account_alias_authenticated(alias)
        })
    }

    /// Read the exact multisignature authority through explicitly selected readers.
    ///
    /// This uses the same bounded, freshly authenticated reader selection as
    /// [`Self::resolve_account_alias_authenticated_with_readers`]. Only this
    /// read-only operation is repeated; no proposal, approval or transaction is sent.
    /// Missing authority, a different resolved account, and malformed responses stop
    /// immediately rather than consulting another reader.
    ///
    /// # Errors
    /// Returns reader-context, deadline, transport, HTTP, or canonical selector errors.
    pub fn post_multisig_spec_with_readers(
        &self,
        request: &MultisigSpecRequest,
        alternatives: &[&Client],
    ) -> Result<MultisigSpecResponse> {
        self.with_available_identity_reader(alternatives, |reader| {
            reader.post_multisig_spec(request)
        })
    }

    pub(crate) fn with_available_identity_reader<T>(
        &self,
        alternatives: &[&Client],
        read: impl Fn(&Client) -> Result<T>,
    ) -> Result<T> {
        eyre::ensure!(
            alternatives.len() < MAX_IDENTITY_READERS,
            "identity reads select at most {MAX_IDENTITY_READERS} readers"
        );
        let readers: Vec<_> = std::iter::once(self)
            .chain(alternatives.iter().copied())
            .collect();
        let mut endpoints = std::collections::HashSet::new();
        for reader in &readers {
            eyre::ensure!(
                reader.chain == self.chain
                    && reader.network_id == self.network_id
                    && reader.account == self.account
                    && reader.key_pair.public_key() == self.key_pair.public_key()
                    && reader.account_chain_discriminant == self.account_chain_discriminant,
                "identity readers must retain one network and signing authority"
            );
            eyre::ensure!(
                endpoints.insert(&reader.torii_url),
                "identity reader endpoints must be distinct"
            );
        }
        let now = Instant::now();
        let configured_deadline = readers
            .iter()
            .map(|reader| reader.torii_request_timeout)
            .filter(|timeout| !timeout.is_zero())
            .min()
            .map(|timeout| {
                now.checked_add(timeout)
                    .ok_or_else(|| eyre!("identity read timeout exceeds the monotonic clock"))
            })
            .transpose()?;
        let deadline = readers
            .iter()
            .filter_map(|reader| reader.http_transport.deadline())
            .chain(configured_deadline)
            .min()
            .ok_or_else(|| eyre!("selected identity readers require a finite deadline"))?;
        let mut last_error = None;
        for (index, reader) in readers.iter().enumerate() {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or_else(crate::http_default::request_deadline_elapsed)?;
            let attempts_left = u32::try_from(readers.len() - index)
                .map_err(|_| eyre!("identity reader count exceeds its finite bound"))?;
            let share = remaining / attempts_left;
            if share.is_zero() {
                return Err(crate::http_default::request_deadline_elapsed());
            }
            let attempt_deadline = Instant::now()
                .checked_add(share)
                .ok_or_else(crate::http_default::request_deadline_elapsed)?
                .min(deadline);
            let bounded = reader.with_request_deadline(attempt_deadline);
            let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                self.account_chain_discriminant,
            );
            let result = read(&bounded);
            if Instant::now() >= deadline {
                return Err(crate::http_default::request_deadline_elapsed());
            }
            match result {
                Ok(value) => return Ok(value),
                Err(error) if unavailable_reader(&error) => last_error = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(last_error.expect("nonempty readers each returned an unavailable error"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::evidence_http_tests::{
        SnapshotStore, base_url, client_with_base_url, json_response, with_mock_http,
    };
    use crate::http_default::RequestSnapshot;
    use std::sync::{Arc, Mutex};

    fn alias() -> AccountAliasName {
        "staff@bank.paynet".parse().unwrap()
    }

    fn success(client: &Client) -> Response<Vec<u8>> {
        json_response(
            StatusCode::OK,
            &norito::json::to_json(&norito::json!({
                "alias": (alias().to_string()),
                "account_id": (client.account.to_i105_for_discriminant(client.account_chain_discriminant).unwrap())
            }))
            .unwrap(),
        )
    }

    fn exact_absence() -> String {
        use iroha_torii_shared::{ErrorDetails, ErrorEnvelope, aliases::*};
        let envelope =
            ErrorEnvelope::new(ACCOUNT_ALIAS_NOT_FOUND_CODE, "absent").with_details(ErrorDetails {
                account_alias_not_found: Some(AccountAliasNotFoundV1 {
                    alias: alias().to_string(),
                }),
                ..Default::default()
            });
        norito::json::to_json(&envelope).unwrap()
    }

    fn readers(transport: DefaultHttpTransport) -> [Client; 2] {
        let mut first = client_with_base_url(base_url()).with_test_http_transport(transport);
        first.torii_request_timeout = Duration::from_millis(200);
        let mut second = first.clone();
        first.torii_url = "http://mock.local/seat/1/".parse().unwrap();
        second.torii_url = "http://mock.local/seat/2/".parse().unwrap();
        [first, second].map(|client| {
            let token = format!("fixture-token-{}", client.torii_url.path());
            client
                .to_builder()
                .api_token(Some(crate::secrecy::SecretString::new(token)))
                .build()
                .unwrap()
        })
    }

    fn exercise(
        first: impl Fn(&RequestSnapshot) -> Result<Response<Vec<u8>>> + Send + Sync + 'static,
    ) -> (
        Result<Option<AccountAliasResolutionV1>>,
        Vec<RequestSnapshot>,
    ) {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let captured = snapshots.clone();
        let answer = Arc::new(Mutex::new(None::<Response<Vec<u8>>>));
        let shared_answer = answer.clone();
        let result = with_mock_http(
            move |request| {
                let initial = request.url.path().starts_with("/seat/1/");
                captured.lock().unwrap().push(request.clone());
                if initial {
                    first(&request)
                } else {
                    Ok(shared_answer.lock().unwrap().as_ref().unwrap().clone())
                }
            },
            |transport| {
                let readers = readers(transport);
                *answer.lock().unwrap() = Some(success(&readers[0]));
                let result = readers[0]
                    .resolve_account_alias_authenticated_with_readers(&alias(), &[&readers[1]]);
                for (index, request) in snapshots.lock().unwrap().iter().enumerate() {
                    crate::client::tests::assert_canonical_account_signed_request(
                        &readers[index],
                        request,
                    );
                }
                result
            },
        );
        let requests = snapshots.lock().unwrap().clone();
        (result, requests)
    }

    #[test]
    fn unavailable_reader_advances_with_exact_endpoint_credentials_and_fresh_signature() {
        for status in [
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            let (result, requests) = exercise(move |_| Ok(json_response(status, "{}")));
            assert!(result.unwrap().is_some());
            assert_eq!(requests.len(), 2);
            let mut nonces = std::collections::HashSet::new();
            for (index, request) in requests.iter().enumerate() {
                assert_eq!(request.method, HttpMethod::POST);
                assert_eq!(
                    request.url.path(),
                    format!("/seat/{}/v1/aliases/resolve", index + 1)
                );
                let headers: HashMap<_, _> = request.headers.iter().cloned().collect();
                assert_eq!(
                    headers["x-api-token"],
                    format!("fixture-token-/seat/{}/", index + 1)
                );
                assert!(nonces.insert(headers[HEADER_NONCE].clone()));
                assert!(!headers[HEADER_SIGNATURE].is_empty());
                assert_eq!(request.body, requests[0].body);
                assert!(request.timeout.unwrap() <= Duration::from_millis(200));
            }
        }
    }

    #[test]
    fn unavailable_connection_advances_but_route_refusal_does_not() {
        for (kind, succeeds, count) in [
            (ErrorKind::ConnectionRefused, true, 2),
            (ErrorKind::ConnectionReset, true, 2),
            (ErrorKind::PermissionDenied, false, 1),
            (ErrorKind::InvalidData, false, 1),
            (ErrorKind::Other, false, 1),
        ] {
            let (result, requests) = exercise(move |_| Err(std::io::Error::from(kind).into()));
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(requests.len(), count);
        }
    }

    #[test]
    fn unresponsive_reader_only_spends_its_share_then_healthy_reader_answers() {
        let started = Instant::now();
        let (result, requests) = exercise(|request| {
            let budget = request.timeout.unwrap();
            assert!(budget <= Duration::from_millis(100));
            std::thread::sleep(budget);
            Err(std::io::Error::from(ErrorKind::TimedOut).into())
        });
        assert!(result.unwrap().is_some());
        assert_eq!(requests.len(), 2);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(requests[1].timeout.unwrap() <= Duration::from_millis(110));
    }

    #[test]
    fn authority_refusals_and_invalid_payloads_stop_without_another_reader() {
        for (status, body) in [
            (StatusCode::BAD_REQUEST, "{}"),
            (StatusCode::UNAUTHORIZED, "{}"),
            (StatusCode::FORBIDDEN, "{}"),
            (StatusCode::NOT_FOUND, "{}"),
            (StatusCode::TOO_MANY_REQUESTS, "{}"),
            (StatusCode::OK, "not json"),
            (StatusCode::OK, "{\"alias\":"),
            (
                StatusCode::OK,
                "{\"alias\":\"different@bank.paynet\",\"account_id\":\"invalid\"}",
            ),
        ] {
            let (result, requests) = exercise(move |_| Ok(json_response(status, body)));
            assert!(result.is_err());
            assert_eq!(requests.len(), 1);
        }
    }

    #[test]
    fn late_refusal_is_not_replaced_by_a_retryable_deadline() {
        for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            let (result, requests) = exercise(move |request| {
                // The refusal arrives after this reader's share of the deadline.
                std::thread::sleep(request.timeout.unwrap() + Duration::from_millis(20));
                Ok(json_response(status, "{}"))
            });
            let error = result.unwrap_err();
            assert_eq!(
                error.downcast_ref::<crate::Error>().unwrap().http_status(),
                Some(status.as_u16())
            );
            assert_eq!(requests.len(), 1);
        }
    }

    #[test]
    fn late_exact_absence_is_not_replaced_by_a_retryable_deadline() {
        let body = exact_absence();
        let (result, requests) = exercise(move |request| {
            std::thread::sleep(request.timeout.unwrap() + Duration::from_millis(20));
            Ok(json_response(StatusCode::NOT_FOUND, &body))
        });
        assert!(result.unwrap().is_none());
        assert_eq!(requests.len(), 1);
    }

    #[test]
    fn exact_absence_stops_without_another_reader() {
        let body = exact_absence();
        let (result, requests) = exercise(move |_| Ok(json_response(StatusCode::NOT_FOUND, &body)));
        assert!(result.unwrap().is_none());
        assert_eq!(requests.len(), 1);
    }

    #[test]
    fn decoder_and_untyped_io_errors_never_select_another_reader() {
        let raw: eyre::Report = std::io::Error::from(ErrorKind::UnexpectedEof).into();
        assert!(!unavailable_reader(&raw));
        assert!(!unavailable_reader(
            &raw.wrap_err("completed response decoder")
        ));
        assert!(!unavailable_reader(
            &crate::Error::Decode {
                operation: "multisig.spec",
                details: "truncated completed response".into(),
            }
            .into()
        ));
        assert!(unavailable_reader(
            &crate::Error::Transport {
                operation: "multisig.spec",
                kind: crate::TransportErrorKind::Io(ErrorKind::UnexpectedEof),
                details: "incomplete HTTP response".into(),
            }
            .into()
        ));
    }

    #[test]
    fn inconsistent_reader_contexts_and_expired_deadlines_fail_before_dispatch() {
        for variant in 0..8 {
            with_mock_http(
                |_| panic!("invalid context must not dispatch"),
                |transport| {
                    let [mut first, mut second] = readers(transport);
                    match variant {
                        0 => {
                            second.network_id = NetworkId::from_genesis_hash(HashOf::<
                                iroha_data_model::block::BlockHeader,
                            >::from_untyped_unchecked(
                                Hash::prehashed([0x77; Hash::LENGTH]),
                            ))
                        }
                        1 => {
                            second.account =
                                AccountId::new(checked_random_keypair().public_key().clone())
                        }
                        2 => second.account_chain_discriminant ^= 1,
                        3 => second.torii_url = first.torii_url.clone(),
                        4 => first = first.with_request_deadline(Instant::now()),
                        6 => second.key_pair = checked_random_keypair(),
                        7 => second.chain = ChainId::from("other-chain"),
                        _ => {
                            first.torii_request_timeout = Duration::ZERO;
                            second.torii_request_timeout = Duration::ZERO;
                        }
                    }
                    assert!(
                        first
                            .resolve_account_alias_authenticated_with_readers(&alias(), &[&second])
                            .is_err()
                    );
                },
            );
        }
    }

    #[test]
    fn exhausted_readers_return_last_typed_failure_and_each_is_tried_once() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let captured = snapshots.clone();
        let error = with_mock_http(
            move |request| {
                captured.lock().unwrap().push(request);
                Ok(json_response(StatusCode::SERVICE_UNAVAILABLE, "{}"))
            },
            |transport| {
                let readers = readers(transport);
                readers[0]
                    .resolve_account_alias_authenticated_with_readers(&alias(), &[&readers[1]])
            },
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<crate::Error>().unwrap().http_status(),
            Some(503)
        );
        assert_eq!(snapshots.lock().unwrap().len(), 2);
    }

    #[test]
    fn selected_reader_limit_is_checked_before_dispatch() {
        for count in [MAX_IDENTITY_READERS, MAX_IDENTITY_READERS + 1] {
            let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
            let captured = snapshots.clone();
            let error = with_mock_http(
                move |request| {
                    captured.lock().unwrap().push(request);
                    Ok(json_response(StatusCode::SERVICE_UNAVAILABLE, "{}"))
                },
                |transport| {
                    let mut selected = readers(transport)[0].clone();
                    selected.torii_request_timeout = Duration::from_secs(2);
                    let alternatives: Vec<_> = (1..count)
                        .map(|index| {
                            let mut reader = selected.clone();
                            reader.torii_url = format!("http://mock.local/reader/{index}/")
                                .parse()
                                .unwrap();
                            reader
                        })
                        .collect();
                    selected.resolve_account_alias_authenticated_with_readers(
                        &alias(),
                        &alternatives.iter().collect::<Vec<_>>(),
                    )
                },
            )
            .unwrap_err();
            let calls = snapshots.lock().unwrap().len();
            if count == MAX_IDENTITY_READERS {
                assert_eq!(calls, MAX_IDENTITY_READERS);
                assert_eq!(
                    error.downcast_ref::<crate::Error>().unwrap().http_status(),
                    Some(503)
                );
            } else {
                assert_eq!(calls, 0);
            }
        }
    }

    #[test]
    fn multisig_spec_uses_selected_readers_without_retrying_missing_authority() {
        for first_status in [StatusCode::BAD_GATEWAY, StatusCode::NOT_FOUND] {
            let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
            let captured = snapshots.clone();
            let result = with_mock_http(
                move |request| {
                    let first = request.url.path().starts_with("/seat/1/");
                    captured.lock().unwrap().push(request);
                    Ok(json_response(
                        if first {
                            first_status
                        } else {
                            StatusCode::NOT_FOUND
                        },
                        "{}",
                    ))
                },
                |transport| {
                    let readers = readers(transport);
                    readers[0].post_multisig_spec_with_readers(
                        &MultisigSpecRequest {
                            multisig_account_id: Some(readers[0].account.clone()),
                            multisig_account_alias: None,
                        },
                        &[&readers[1]],
                    )
                },
            );
            assert_eq!(
                result
                    .unwrap_err()
                    .downcast_ref::<crate::Error>()
                    .unwrap()
                    .http_status(),
                Some(404)
            );
            let requests = snapshots.lock().unwrap();
            assert_eq!(
                requests.len(),
                if first_status == StatusCode::BAD_GATEWAY {
                    2
                } else {
                    1
                }
            );
            assert!(
                requests
                    .iter()
                    .all(|request| request.url.path().ends_with("/v1/multisig/spec"))
            );
        }
    }

    #[test]
    fn truncated_completed_multisig_response_stops_without_another_reader() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let captured = snapshots.clone();
        let result = with_mock_http(
            move |request| {
                captured.lock().unwrap().push(request);
                Ok(json_response(
                    StatusCode::OK,
                    "{\"resolved_multisig_account_id\":",
                ))
            },
            |transport| {
                let readers = readers(transport);
                readers[0].post_multisig_spec_with_readers(
                    &MultisigSpecRequest {
                        multisig_account_id: Some(readers[0].account.clone()),
                        multisig_account_alias: None,
                    },
                    &[&readers[1]],
                )
            },
        );
        assert!(result.is_err());
        assert_eq!(snapshots.lock().unwrap().len(), 1);
    }

    #[test]
    fn multisig_spec_decodes_healthy_reader_and_rejects_changed_account() {
        use iroha_executor_data_model::isi::multisig::MultisigSpec;
        for changed_account in [false, true] {
            let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
            let captured = snapshots.clone();
            let answer = Arc::new(Mutex::new(None::<Response<Vec<u8>>>));
            let shared_answer = answer.clone();
            let result = with_mock_http(
                move |request| {
                    let initial = request.url.path().starts_with("/seat/1/");
                    captured.lock().unwrap().push(request);
                    if initial {
                        Ok(json_response(StatusCode::SERVICE_UNAVAILABLE, "{}"))
                    } else {
                        Ok(shared_answer.lock().unwrap().as_ref().unwrap().clone())
                    }
                },
                |transport| {
                    let readers = readers(transport);
                    let target = readers[0].account.clone();
                    let response = MultisigSpecResponse {
                        resolved_multisig_account_id: if changed_account {
                            AccountId::new(checked_random_keypair().public_key().clone())
                        } else {
                            target.clone()
                        },
                        spec: MultisigSpec {
                            signatories: [(target.clone(), 1)].into_iter().collect(),
                            quorum: std::num::NonZeroU16::new(1).unwrap(),
                            transaction_ttl_ms: std::num::NonZeroU64::new(1_000).unwrap(),
                        },
                    };
                    let _profile =
                        iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                            readers[0].account_chain_discriminant,
                        );
                    *answer.lock().unwrap() = Some(json_response(
                        StatusCode::OK,
                        &norito::json::to_json(&response).unwrap(),
                    ));
                    readers[0].post_multisig_spec_with_readers(
                        &MultisigSpecRequest {
                            multisig_account_id: Some(target),
                            multisig_account_alias: None,
                        },
                        &[&readers[1]],
                    )
                },
            );
            assert_eq!(result.is_err(), changed_account);
            assert_eq!(snapshots.lock().unwrap().len(), 2);
        }
    }
}
