// Included by `da::ingest::tests`; these regressions keep DA attribution tied
// to the account authenticated by the exact-network canonical HTTP envelope.

fn verified_principal_for_request(
    request: &DaIngestRequest,
) -> crate::app_auth::VerifiedCanonicalRequest {
    let signer = request.signatures[0].signer.clone();
    crate::app_auth::VerifiedCanonicalRequest {
        account: ALICE_ID.clone(),
        signer: signer.clone(),
        verified_signers: vec![signer],
    }
}

#[test]
fn authenticated_da_principal_becomes_pin_owner() {
    let request = sample_request();
    let principal = verified_principal_for_request(&request);

    let owner = authenticate_da_ingest_request(
        &request,
        &principal,
        &crate::signed_query_test_network_id(),
    )
    .expect("matching authenticated signer should admit DA ingest");

    assert_eq!(owner, ALICE_ID.clone());
}

#[test]
fn authenticated_da_principal_rejects_unrelated_submitter() {
    let request = sample_request();
    let unrelated = checked_fixture_ed25519_keypair(0x24);
    let principal = crate::app_auth::VerifiedCanonicalRequest {
        account: ALICE_ID.clone(),
        signer: unrelated.public_key().clone(),
        verified_signers: vec![unrelated.public_key().clone()],
    };

    let err = authenticate_da_ingest_request(
        &request,
        &principal,
        &crate::signed_query_test_network_id(),
    )
    .expect_err("self-declared submitter must belong to the authenticated signer set");

    assert_eq!(err.0, StatusCode::FORBIDDEN);
    assert_eq!(
        err.1,
        "DA ingest authorization includes a signer outside the authenticated account witness"
    );
}

#[test]
fn authenticated_da_principal_rejects_metadata_owner_override() {
    let mut request = sample_request();
    request.metadata.items.push(MetadataEntry::new(
        META_DA_REGISTRY_OWNER,
        BOB_ID.to_string().into_bytes(),
        MetadataVisibility::Public,
    ));
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let digest = request.signing_digest();
    request.signatures[0].signature = checked_signature(keypair.private_key(), &digest);
    let principal = verified_principal_for_request(&request);

    let err = authenticate_da_ingest_request(
        &request,
        &principal,
        &crate::signed_query_test_network_id(),
    )
    .expect_err("caller-controlled DA pin ownership must be rejected");

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        err.1,
        "metadata entry `da.registry.owner` is retired; pin ownership comes from the authenticated account"
    );
}

#[test]
fn authenticated_da_principal_rejects_invalid_self_signature() {
    let mut request = sample_request();
    let principal = verified_principal_for_request(&request);
    request.sequence = request.sequence.saturating_add(1);

    let err = authenticate_da_ingest_request(
        &request,
        &principal,
        &crate::signed_query_test_network_id(),
    )
    .expect_err("the DA intent must remain bound to its declared submitter");

    assert_eq!(err.0, StatusCode::UNAUTHORIZED);
    assert_eq!(err.1, "DA ingest request signatures are invalid");
}

#[test]
fn authenticated_da_principal_rejects_same_label_different_genesis() {
    let mut request = sample_request();
    let expected_network_id = crate::signed_query_test_network_id();
    request.network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        Hash::prehashed([0xD1; 32]),
    ));
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let digest = request.signing_digest();
    request.signatures[0].signature = checked_signature(keypair.private_key(), &digest);
    let principal = verified_principal_for_request(&request);

    let err = authenticate_da_ingest_request(&request, &principal, &expected_network_id)
        .expect_err("a valid request for a same-label foreign genesis must fail closed");

    assert_eq!(err.0, StatusCode::FORBIDDEN);
    assert_eq!(err.1, "DA ingest request targets a different network");
}
