//! Typed Musubi route, authority, async dispatch and blocking-facade contracts.

use super::{
    Client,
    capability_test_support::AsyncOnlyTransport,
    evidence_http_tests::{base_url, client_with_base_url},
    musubi::{MAX_RESPONSE_BYTES, QueryResult},
};
use crate::{
    Error, blocking,
    http::{Method, Response, TransportRequest},
};
use base64::Engine as _;
use iroha_crypto::{Hash, Signature};
use iroha_data_model::{
    account::address::{ChainDiscriminantGuard, chain_discriminant},
    musubi::{
        ArchiveId, MusubiAliasQueryV1, MusubiArchiveLocationQueryV1,
        MusubiArchiveRetentionDecisionV1, MusubiArchiveRetentionDispositionV1,
        MusubiArchiveRetentionPageV1, MusubiArchiveRetentionQueryV1, MusubiExactPackageQueryV1,
        MusubiExactReleaseQueryV1, MusubiNamespaceBindingDigestV1, MusubiOrderedPrefixQueryV1,
        MusubiOrderedPrefixV1, MusubiPackageIdV1, MusubiPackagePageQueryV1, MusubiPackageRecordV1,
        MusubiPackageRevisionsV1, MusubiPackageScopeV1, MusubiPageRequestV1,
        MusubiProviderBundleAttestationKeyV1, MusubiRegistrySnapshotV1, MusubiReleaseIdV1,
        MusubiResolverIndexQueryV1, MusubiSearchPageRequestV1, MusubiSearchQueryV1,
    },
    sorafs::{capacity::ProviderId, pin_registry::ReplicationOrderId},
};
use iroha_model_base::topology::DataSpaceId;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const OP: &str = "musubi.v1.query.exact_package";
type Requests = Arc<Mutex<Vec<TransportRequest>>>;

fn package_query() -> MusubiExactPackageQueryV1 {
    MusubiExactPackageQueryV1 {
        package: MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().unwrap(),
        ),
    }
}

fn release_query(package: MusubiPackageIdV1) -> MusubiExactReleaseQueryV1 {
    MusubiExactReleaseQueryV1 {
        release: MusubiReleaseIdV1 {
            package,
            version: "1.0.0".parse().unwrap(),
        },
    }
}

fn package_record(client: &Client) -> MusubiPackageRecordV1 {
    MusubiPackageRecordV1 {
        package: package_query().package,
        claimed_namespace: "universal".parse().unwrap(),
        claimed_namespace_binding: MusubiNamespaceBindingDigestV1([1; 32]),
        owners: vec![client.account.clone()],
        member_accounts: vec![client.account.clone()],
        claimed_at_height: 1,
        revisions: MusubiPackageRevisionsV1 {
            governance: 1,
            metadata: 1,
            archive_locations: 1,
        },
    }
}

fn json<T: norito::json::JsonSerialize>(value: &T, discriminant: u16) -> Response<Vec<u8>> {
    let _format = ChainDiscriminantGuard::enter(discriminant);
    Response::builder()
        .status(200)
        .header("content-type", "application/json; charset=utf-8")
        .body(norito::json::to_vec(value).unwrap())
        .unwrap()
}

fn attach(
    responder: impl Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync + 'static,
    delay: Duration,
    timeout: Duration,
) -> (Client, Requests, Arc<AtomicUsize>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let completed = Arc::new(AtomicUsize::new(0));
    let transport = Arc::new(AsyncOnlyTransport {
        responder: Box::new(responder),
        requests: requests.clone(),
        completed: completed.clone(),
        delay,
    });
    let mut builder = client_with_base_url(base_url())
        .to_builder()
        .http_transport(transport);
    builder.torii_request_timeout = timeout;
    builder.account_chain_discriminant = 369;
    (builder.build().unwrap(), requests, completed)
}

fn header<'a>(request: &'a TransportRequest, name: &str) -> &'a str {
    let values: Vec<_> = request
        .headers
        .iter()
        .filter(|(key, _)| key.as_str() == name)
        .collect();
    assert_eq!(values.len(), 1, "one {name}");
    values[0].1.to_str().unwrap()
}

fn assert_signed(client: &Client, request: &TransportRequest) {
    let signature = Signature::try_from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(header(request, "x-iroha-signature"))
            .unwrap(),
    )
    .unwrap();
    let account = header(request, "x-iroha-account");
    assert_eq!(account, client.account.to_canonical_hex().unwrap());
    assert!(account.is_ascii());
    assert!(account.starts_with("0x"));
    assert!(
        account[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert!(!header(request, "x-iroha-nonce").is_empty());
    assert!(
        !request
            .headers
            .iter()
            .any(|(name, _)| name.as_str() == "x-iroha-witness")
    );
    let message = Client::exact_network_request_message(
        &client.network_id,
        &request.method,
        &request.url,
        &request.body,
        header(request, "x-iroha-timestamp-ms").parse().unwrap(),
        header(request, "x-iroha-nonce"),
    )
    .unwrap();
    signature
        .verify(client.key_pair.public_key(), &message)
        .unwrap();
    let wrong_network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"different-network")),
    );
    let altered = Client::exact_network_request_message(
        &wrong_network,
        &request.method,
        &request.url,
        &request.body,
        header(request, "x-iroha-timestamp-ms").parse().unwrap(),
        header(request, "x-iroha-nonce"),
    )
    .unwrap();
    assert!(
        signature
            .verify(client.key_pair.public_key(), &altered)
            .is_err()
    );
    assert_eq!(header(request, "accept"), "application/json");
    assert_eq!(header(request, "content-type"), "application/json");
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.max_response_bytes, MAX_RESPONSE_BYTES);
}

#[tokio::test]
async fn public_musubi_query_signs_the_exact_fixed_route_and_body() {
    let fixture_client = client_with_base_url(base_url());
    let record = package_record(&fixture_client);
    record.validate().unwrap();
    let expected = record.clone();
    let (client, requests, _) = attach(
        move |_| Ok(json(&record, 369)),
        Duration::ZERO,
        Duration::from_secs(1),
    );
    let query = package_query();
    let output = client
        .account_client()
        .unwrap()
        .musubi()
        .exact_package(&query)
        .await
        .unwrap();
    let QueryResult::Found(actual) = output else {
        panic!("expected finalized package")
    };
    assert_eq!(actual, expected);
    let requests = requests.lock().unwrap();
    let request = &requests[0];
    assert_eq!(request.url.path(), "/v1/musubi/queries/exact-package");
    assert_eq!(request.body, norito::json::to_vec(&query).unwrap());
    assert_signed(&client, request);
}

#[tokio::test]
async fn public_musubi_query_rejects_legacy_witness_injection_before_dispatch() {
    let (client, requests, _) = attach(
        |_| panic!("invalid signer must not dispatch"),
        Duration::ZERO,
        Duration::from_secs(1),
    );
    let mut builder = client.to_builder();
    builder
        .headers
        .insert("x-IROHA-witness".to_owned(), "legacy-witness".to_owned());
    let account = builder.build().unwrap().account_client().unwrap();
    assert!(matches!(
        account.musubi().exact_package(&package_query()).await,
        Err(Error::InvalidRequest { operation: OP, .. })
    ));
    assert!(requests.lock().unwrap().is_empty());
}

fn missing_query_client() -> (Client, Requests) {
    let (client, requests, _) = attach(
        |_| Ok(Response::builder().status(404).body(Vec::new()).unwrap()),
        Duration::ZERO,
        Duration::from_secs(1),
    );
    (client, requests)
}

fn assert_last_signed_query<T: norito::json::JsonSerialize>(
    client: &Client,
    requests: &Requests,
    request: &T,
) {
    let requests = requests.lock().unwrap();
    let sent = requests.last().unwrap();
    assert_eq!(sent.body, norito::json::to_vec(request).unwrap());
    assert_signed(client, sent);
}

fn assert_musubi_route_catalog(requests: &Requests) {
    let requests = requests.lock().unwrap();
    let mut actual: Vec<_> = requests.iter().map(|request| request.url.path()).collect();
    actual.sort_unstable();
    let mut expected: Vec<_> = iroha_torii_shared::route_catalog::CATALOGED_ROUTES
        .iter()
        .filter(|route| route.stable_route_id().starts_with("musubi.v1.query."))
        .map(|route| route.path())
        .collect();
    expected.sort_unstable();
    assert_eq!(actual.len(), 12);
    assert_eq!(actual, expected);
    assert!(actual.contains(&"/v1/musubi/queries/provider-bundle-attestation"));
}

#[tokio::test]
async fn all_public_musubi_routes_have_one_typed_request_and_exact_signature() {
    let (client, requests) = missing_query_client();
    let account = client.account_client().unwrap();
    let musubi = account.musubi();
    let package = package_query().package;
    let page = MusubiPageRequestV1 {
        limit: 1,
        cursor: None,
    };
    macro_rules! query {
        ($method:ident, $request:expr) => {{
            let request = $request;
            assert!(matches!(
                musubi.$method(&request).await.unwrap(),
                QueryResult::NotFound
            ));
            assert_last_signed_query(&client, &requests, &request);
        }};
    }
    query!(exact_package, package_query());
    query!(exact_release, release_query(package.clone()));
    query!(
        provider_bundle_attestation,
        MusubiProviderBundleAttestationKeyV1 {
            archive_id: ArchiveId([1; 32]),
            replication_order: ReplicationOrderId([2; 32]),
            provider_id: ProviderId([3; 32])
        }
    );
    query!(
        resolver_index,
        MusubiResolverIndexQueryV1 {
            package: package.clone(),
            requirement: None,
            page: page.clone()
        }
    );
    query!(
        versions,
        MusubiPackagePageQueryV1 {
            package: package.clone(),
            page: page.clone()
        }
    );
    query!(
        maintainers,
        MusubiPackagePageQueryV1 {
            package,
            page: page.clone()
        }
    );
    query!(
        archive_locations,
        MusubiArchiveLocationQueryV1 {
            archive_id: ArchiveId([1; 32]),
            page: page.clone()
        }
    );
    query!(
        archive_retention,
        MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![ArchiveId([1; 32])],
            expected_snapshot: None
        }
    );
    query!(
        alias,
        MusubiAliasQueryV1 {
            alias: "demo".parse().unwrap(),
            page: page.clone()
        }
    );
    query!(
        alias_history,
        MusubiAliasQueryV1 {
            alias: "demo".parse().unwrap(),
            page: page.clone()
        }
    );
    query!(
        ordered_prefix,
        MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new("universal/demo").unwrap(),
            page
        }
    );
    query!(
        search,
        MusubiSearchQueryV1 {
            query: "demo".to_owned(),
            page: MusubiSearchPageRequestV1 {
                limit: 1,
                cursor: None
            }
        }
    );
    assert_musubi_route_catalog(&requests);
}

#[tokio::test]
async fn public_musubi_query_surfaces_missing_and_stale_cursor() {
    for (status, stale) in [(404, false), (410, true)] {
        let (client, requests, _) = attach(
            move |_| Ok(Response::builder().status(status).body(Vec::new()).unwrap()),
            Duration::ZERO,
            Duration::from_secs(1),
        );
        let output = client
            .account_client()
            .unwrap()
            .musubi()
            .exact_package(&package_query())
            .await
            .unwrap();
        assert_eq!(matches!(output, QueryResult::StaleCursor), stale);
        assert_eq!(matches!(output, QueryResult::NotFound), !stale);
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn musubi_dispatch_is_responsive_cancellable_and_deadline_bounded() {
    fn send_future<F: std::future::Future + Send>(_: &F) {}

    let outer_discriminant = chain_discriminant();
    let (client, requests, completed) = attach(
        |_| Ok(Response::builder().status(404).body(Vec::new()).unwrap()),
        Duration::from_millis(100),
        Duration::from_millis(25),
    );
    let account = client.account_client().unwrap();
    let query = package_query();
    let capability = account.musubi();
    let operation = capability.exact_package(&query);
    send_future(&operation);
    let observer = async {
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(chain_discriminant(), outer_discriminant);
    };
    let (output, ()) = tokio::join!(operation, observer);
    assert!(matches!(output, Err(Error::Timeout { operation: OP })));
    assert_eq!(completed.load(Ordering::SeqCst), 0);
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(chain_discriminant(), outer_discriminant);
}

#[tokio::test]
async fn musubi_errors_preserve_status_bounds_and_transport_kind_without_retry() {
    for (status, content_type, body, expected) in [
        (
            503,
            "text/plain",
            b"temporarily unavailable".to_vec(),
            "http",
        ),
        (307, "text/plain", Vec::new(), "http"),
        (200, "text/plain", b"{}".to_vec(), "decode"),
        (200, "application/json", b"{}".to_vec(), "decode"),
        (
            200,
            "application/json",
            vec![b' '; MAX_RESPONSE_BYTES + 1],
            "bound",
        ),
    ] {
        let (client, requests, _) = attach(
            move |_| {
                Ok(Response::builder()
                    .status(status)
                    .header("content-type", content_type)
                    .header("retry-after", "9")
                    .body(body.clone())
                    .unwrap())
            },
            Duration::ZERO,
            Duration::from_secs(1),
        );
        let error = client
            .account_client()
            .unwrap()
            .musubi()
            .exact_package(&package_query())
            .await
            .unwrap_err();
        match (expected, error) {
            (
                "http",
                Error::Http {
                    operation: OP,
                    status: actual,
                    retry_after,
                    ..
                },
            ) => {
                assert_eq!(actual, status);
                assert_eq!(retry_after, Some(Duration::from_secs(9)));
            }
            ("decode", Error::Decode { operation: OP, .. })
            | ("bound", Error::ResponseTooLarge { .. }) => {}
            (_, error) => panic!("unexpected {error:?}"),
        }
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
    let (client, requests, _) = attach(
        |_| Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset).into()),
        Duration::ZERO,
        Duration::from_secs(1),
    );
    assert!(matches!(
        client
            .account_client()
            .unwrap()
            .musubi()
            .exact_package(&package_query())
            .await,
        Err(Error::Transport {
            operation: OP,
            kind: crate::TransportErrorKind::Io(std::io::ErrorKind::ConnectionReset),
            ..
        })
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn musubi_blocking_account_clones_share_runtime_and_reject_async_entry() {
    let (client, requests, _) = attach(
        |_| Ok(Response::builder().status(404).body(Vec::new()).unwrap()),
        Duration::ZERO,
        Duration::from_secs(1),
    );
    let account = blocking::AccountClient::from_client(client.account_client().unwrap()).unwrap();
    let clone = account.clone();
    for _ in 0..2 {
        assert!(matches!(
            account.musubi().exact_package(&package_query()).unwrap(),
            QueryResult::NotFound
        ));
        assert!(matches!(
            clone.musubi().exact_package(&package_query()).unwrap(),
            QueryResult::NotFound
        ));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(
            account.musubi().exact_package(&package_query()),
            Err(Error::Blocking(_))
        ));
        assert!(matches!(
            clone.musubi().exact_package(&package_query()),
            Err(Error::Blocking(_))
        ));
        drop(account);
        drop(clone);
    });
    assert_eq!(requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn musubi_contexts_isolate_endpoint_network_authority_and_address_decoding() {
    fn replace_response(client: &Client, requests: Requests) -> Client {
        let record = package_record(client);
        let discriminant = client.account_chain_discriminant;
        client
            .to_builder()
            .http_transport(Arc::new(AsyncOnlyTransport {
                responder: Box::new(move |_| Ok(json(&record, discriminant))),
                requests,
                completed: Arc::new(AtomicUsize::new(0)),
                delay: Duration::from_millis(2),
            }))
            .build()
            .unwrap()
    }

    let (first, first_requests, _) = attach(
        |_| panic!("replaced before dispatch"),
        Duration::ZERO,
        Duration::from_secs(1),
    );
    let second_key =
        iroha_crypto::KeyPair::try_from_seed(vec![42; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    let mut second_builder = first.to_builder();
    second_builder.torii_url = "http://different.example:8081/prefix/".parse().unwrap();
    second_builder.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"second network")),
    );
    second_builder.key_pair = second_key.clone();
    second_builder.account =
        iroha_data_model::account::AccountId::new(second_key.public_key().clone());
    second_builder.account_chain_discriminant = 753;
    let second = second_builder.build().unwrap();
    let second_requests = Arc::new(Mutex::new(Vec::new()));

    let first = replace_response(&first, first_requests.clone());
    let second = replace_response(&second, second_requests.clone());
    let first_account = first.account_client().unwrap();
    let second_account = second.account_client().unwrap();
    let first_capability = first_account.musubi();
    let second_capability = second_account.musubi();
    let query = package_query();
    let original = chain_discriminant();
    let (a, b) = tokio::join!(
        first_capability.exact_package(&query),
        second_capability.exact_package(&query)
    );
    assert_eq!(a.unwrap(), QueryResult::Found(package_record(&first)));
    assert_eq!(b.unwrap(), QueryResult::Found(package_record(&second)));
    assert_eq!(chain_discriminant(), original);
    let first_requests = first_requests.lock().unwrap();
    let a = &first_requests[0];
    let second_requests = second_requests.lock().unwrap();
    let b = &second_requests[0];
    assert_ne!(a.url.origin(), b.url.origin());
    assert_eq!(b.url.path(), "/v1/musubi/queries/exact-package");
    assert_ne!(header(a, "x-iroha-account"), header(b, "x-iroha-account"));
    assert_signed(&first, a);
    assert_signed(&second, b);
}

#[tokio::test]
async fn musubi_rejects_invalid_typed_requests_before_network_dispatch() {
    let (client, requests, _) = attach(
        |_| panic!("invalid query must not dispatch"),
        Duration::ZERO,
        Duration::from_secs(1),
    );
    let account = client.account_client().unwrap();
    let capability = account.musubi();
    let package = package_query().package;
    let invalid_page = MusubiPageRequestV1 {
        limit: u32::MAX,
        cursor: None,
    };
    assert!(matches!(
        capability
            .versions(&MusubiPackagePageQueryV1 {
                package,
                page: invalid_page,
            })
            .await,
        Err(Error::InvalidRequest {
            operation: "musubi.v1.query.versions",
            ..
        })
    ));
    assert!(matches!(
        capability
            .archive_locations(&MusubiArchiveLocationQueryV1 {
                archive_id: ArchiveId([0; 32]),
                page: MusubiPageRequestV1 {
                    limit: 1,
                    cursor: None
                },
            })
            .await,
        Err(Error::InvalidRequest {
            operation: "musubi.v1.query.archive_locations",
            ..
        })
    ));
    assert!(matches!(
        capability
            .archive_retention(&MusubiArchiveRetentionQueryV1 {
                archive_ids: Vec::new(),
                expected_snapshot: None,
            })
            .await,
        Err(Error::InvalidRequest {
            operation: "musubi.v1.query.archive_retention",
            ..
        })
    ));
    assert!(matches!(
        capability
            .search(&MusubiSearchQueryV1 {
                query: " ".to_owned(),
                page: MusubiSearchPageRequestV1 {
                    limit: 1,
                    cursor: None
                },
            })
            .await,
        Err(Error::InvalidRequest {
            operation: "musubi.v1.query.search",
            ..
        })
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn musubi_rejects_different_package_and_invalid_decoded_ownership() {
    for wrong_package in [true, false] {
        let fixture = client_with_base_url(base_url());
        let mut record = package_record(&fixture);
        if wrong_package {
            record.package.name = "different".parse().unwrap();
            record.validate().unwrap();
        } else {
            record.owners.clear();
            assert!(record.validate().is_err());
        }
        let (client, requests, _) = attach(
            move |_| Ok(json(&record, 369)),
            Duration::ZERO,
            Duration::from_secs(1),
        );
        assert!(matches!(
            client
                .account_client()
                .unwrap()
                .musubi()
                .exact_package(&package_query())
                .await,
            Err(Error::ResponseBinding { operation: OP, .. })
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn musubi_checks_retention_network_snapshot_and_exact_archive_order() {
    let fixture_client = client_with_base_url(base_url());
    let page = MusubiArchiveRetentionPageV1 {
        network_id: fixture_client.network_id,
        items: vec![MusubiArchiveRetentionDecisionV1 {
            archive_id: ArchiveId([1; 32]),
            disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
            active_releases: 0,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: None,
        }],
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 1,
            finalized_block_hash: [3; 32],
            index_revision: 1,
        },
        finalized_time_ms: 1,
    };
    page.validate().unwrap();
    let request = MusubiArchiveRetentionQueryV1 {
        archive_ids: vec![ArchiveId([1; 32])],
        expected_snapshot: Some(page.snapshot),
    };
    for change in ["none", "network", "snapshot", "archive"] {
        let mut response = page.clone();
        match change {
            "network" => {
                response.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"another network")),
                )
            }
            "snapshot" => response.snapshot.finalized_height += 1,
            "archive" => response.items[0].archive_id = ArchiveId([2; 32]),
            _ => {}
        }
        response.validate().unwrap();
        let (client, requests, _) = attach(
            move |_| Ok(json(&response, 369)),
            Duration::ZERO,
            Duration::from_secs(1),
        );
        let result = client
            .account_client()
            .unwrap()
            .musubi()
            .archive_retention(&request)
            .await;
        if change == "none" {
            assert_eq!(result.unwrap(), QueryResult::Found(page.clone()));
        } else {
            assert!(matches!(
                result,
                Err(Error::ResponseBinding {
                    operation: "musubi.v1.query.archive_retention",
                    ..
                })
            ));
        }
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}
