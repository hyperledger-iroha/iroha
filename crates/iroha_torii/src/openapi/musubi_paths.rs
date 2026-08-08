fn musubi_paths() -> Map {
    let mut paths = Map::new();
    for (path, summary, description, request_type, response_type) in [
        (
            musubi_routes::EXACT_PACKAGE.path(),
            "Fetch an exact Musubi V1 package.",
            "Execute a bounded exact structural package query without namespace or alias normalization.",
            "MusubiExactPackageQueryV1",
            "MusubiPackageRecordV1",
        ),
        (
            musubi_routes::EXACT_RELEASE.path(),
            "Fetch an exact Musubi V1 release.",
            "Execute one bounded exact query that returns coherent home and universal release projections from the same finalized state view.",
            "MusubiExactReleaseQueryV1",
            "MusubiExactReleaseSnapshotV1",
        ),
        (
            musubi_routes::PROVIDER_BUNDLE_ATTESTATION.path(),
            "Audit an exact Musubi V1 provider bundle attestation.",
            "Return one immutable full provider proof by its exact archive, replication-order, and provider key.",
            "MusubiProviderBundleAttestationKeyV1",
            "MusubiProviderBundleAttestationRecordV1",
        ),
        (
            musubi_routes::RESOLVER_INDEX.path(),
            "Read the Musubi V1 resolver index.",
            "Return a finalized cursor-bound page from the universal sparse resolver index.",
            "MusubiResolverIndexQueryV1",
            "MusubiResolverIndexPageV1",
        ),
        (
            musubi_routes::VERSIONS.path(),
            "List structured Musubi V1 versions.",
            "Return a finalized cursor-bound package version page.",
            "MusubiPackagePageQueryV1",
            "MusubiVersionPageV1",
        ),
        (
            musubi_routes::MAINTAINERS.path(),
            "List Musubi V1 package governance entries.",
            "Return a finalized cursor-bound page of accepted owners and maintainers plus pending invitations.",
            "MusubiPackagePageQueryV1",
            "MusubiMaintainerPageV1",
        ),
        (
            musubi_routes::ARCHIVE_LOCATIONS.path(),
            "List Musubi V1 archive locations.",
            "Return a finalized cursor-bound renewable archive-location page.",
            "MusubiArchiveLocationQueryV1",
            "MusubiArchiveLocationPageV1",
        ),
        (
            musubi_routes::ARCHIVE_RETENTION.path(),
            "Classify Musubi V1 archive cache retention.",
            "Return bounded exact point-lookup decisions plus the consensus-committed finalized block time from universal archive reverse references, release governance, and storage state. Unknown archives retain fail-closed.",
            "MusubiArchiveRetentionQueryV1",
            "MusubiArchiveRetentionPageV1",
        ),
        (
            musubi_routes::ALIAS.path(),
            "Fetch a permanent Musubi V1 alias.",
            "Return one exact permanent global alias record.",
            "MusubiAliasQueryV1",
            "MusubiAliasRecordV1",
        ),
        (
            musubi_routes::ALIAS_HISTORY.path(),
            "List permanent Musubi V1 alias history.",
            "Return a finalized cursor-bound alias-history page.",
            "MusubiAliasQueryV1",
            "MusubiAliasHistoryPageV1",
        ),
        (
            musubi_routes::ORDERED_PREFIX.path(),
            "Read the ordered Musubi V1 directory.",
            "Return a finalized cursor-bound byte-ordered package-prefix page; fuzzy search is not a resolver input.",
            "MusubiOrderedPrefixQueryV1",
            "MusubiOrderedPackagePageV1",
        ),
        (
            musubi_routes::SEARCH.path(),
            "Search Musubi V1 packages.",
            "Return a bounded cursor-bound exact-token page from the rebuildable finalized-event description and keyword projection. Dependency resolution never reads this projection.",
            "MusubiSearchQueryV1",
            "MusubiSearchPageV1",
        ),
    ] {
        let request_schema = format!("#/components/schemas/{request_type}");
        let response_schema = format!("#/components/schemas/{response_type}");
        let mut methods = json_post_operation(
            "Musubi",
            summary,
            description,
            &request_schema,
            &response_schema,
            Vec::new(),
        );
        if let Some(operation) = methods.get_mut("post").and_then(Value::as_object_mut) {
            operation.insert(
                "x-iroha-norito-request-type".to_owned(),
                Value::String(request_type.to_owned()),
            );
            operation.insert(
                "x-iroha-norito-response-type".to_owned(),
                Value::String(response_type.to_owned()),
            );
        }
        paths.insert(path.to_owned(), Value::Object(methods));
    }

    for (path, summary, request_type) in [
        (
            musubi_routes::NAMESPACE_BINDING_REGISTER.path(),
            "Build a Musubi V1 namespace-binding registration.",
            "RegisterMusubiNamespaceBindingV1",
        ),
        (
            musubi_routes::ARCHIVE_REGISTER.path(),
            "Build a Musubi V1 archive registration.",
            "RegisterMusubiArchiveV1",
        ),
        (
            musubi_routes::PROVIDER_BUNDLE_ATTESTATION_REGISTER.path(),
            "Build an immutable Musubi V1 provider bundle-attestation registration.",
            "RegisterMusubiProviderBundleAttestationV1",
        ),
        (
            musubi_routes::ARCHIVE_LOCATION_ADD.path(),
            "Build a Musubi V1 archive-location add or renewal.",
            "AddMusubiArchiveLocationV1",
        ),
        (
            musubi_routes::ARCHIVE_LOCATION_RETIRE.path(),
            "Build a Musubi V1 archive-location retirement.",
            "RetireMusubiArchiveLocationV1",
        ),
        (
            musubi_routes::RELEASE_PUBLISH.path(),
            "Build a Musubi V1 release publication.",
            "PublishMusubiReleaseV1",
        ),
        (
            musubi_routes::RELEASE_YANK_SET.path(),
            "Build a reversible Musubi V1 yank transition.",
            "SetMusubiReleaseYankV1",
        ),
        (
            musubi_routes::PACKAGE_METADATA_SET.path(),
            "Build a Musubi V1 package metadata replacement.",
            "SetMusubiPackageMetadataV1",
        ),
        (
            musubi_routes::PACKAGE_MEMBER_INVITE.path(),
            "Build a Musubi V1 package-member invitation.",
            "InviteMusubiPackageMaintainerV1",
        ),
        (
            musubi_routes::PACKAGE_MEMBER_ACCEPT.path(),
            "Build a Musubi V1 package-member invitation acceptance.",
            "AcceptMusubiPackageMaintainerV1",
        ),
        (
            musubi_routes::PACKAGE_MEMBER_INVITATION_REVOKE.path(),
            "Build a Musubi V1 pending package-member invitation revocation.",
            "RevokeMusubiPackageMaintainerInvitationV1",
        ),
        (
            musubi_routes::PACKAGE_MEMBER_SET_ROLE.path(),
            "Build a Musubi V1 package-member role replacement.",
            "SetMusubiPackageMaintainerRoleV1",
        ),
        (
            musubi_routes::PACKAGE_MEMBER_REMOVE.path(),
            "Build a Musubi V1 package-member removal.",
            "RemoveMusubiPackageMaintainerV1",
        ),
        (
            musubi_routes::ALIAS_REGISTER.path(),
            "Build a paid permanent Musubi V1 alias registration.",
            "RegisterMusubiAliasV1",
        ),
        (
            musubi_routes::PACKAGE_RECOVER.path(),
            "Build a Parliament-enacted Musubi V1 package recovery.",
            "RecoverMusubiPackageV1",
        ),
        (
            musubi_routes::ALIAS_RETARGET.path(),
            "Build a Parliament-enacted Musubi V1 alias retarget.",
            "RetargetMusubiAliasV1",
        ),
        (
            musubi_routes::ARTIFACT_TAKEDOWN.path(),
            "Build a Parliament-enacted Musubi V1 artifact takedown.",
            "SetMusubiArtifactTakedownV1",
        ),
        (
            musubi_routes::REGISTRY_POLICY_SET.path(),
            "Build a Parliament-enacted Musubi V1 registry-policy replacement.",
            "SetMusubiRegistryPolicyV1",
        ),
        (
            musubi_routes::RELEASE_DIGEST_ASSERT.path(),
            "Build an exact Musubi V1 release-digest assertion.",
            "AssertMusubiReleaseDigestV1",
        ),
    ] {
        let request_schema = format!("#/components/schemas/{request_type}");
        let mut methods = json_post_operation(
            "Musubi",
            summary,
            "Return one deterministic unsigned, versioned instruction envelope for local signing; Torii never accepts private keys.",
            &request_schema,
            "#/components/schemas/MusubiInstructionEnvelopeV1",
            Vec::new(),
        );
        if let Some(operation) = methods.get_mut("post").and_then(Value::as_object_mut) {
            operation.insert(
                "x-iroha-norito-request-type".to_owned(),
                Value::String(request_type.to_owned()),
            );
            operation.insert(
                "x-iroha-norito-response-type".to_owned(),
                Value::String("MusubiInstructionEnvelopeV1".to_owned()),
            );
        }
        paths.insert(path.to_owned(), Value::Object(methods));
    }
    paths
}
