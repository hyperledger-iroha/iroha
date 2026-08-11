//! Canonical Musubi V1 typed-query and unsigned-instruction routes.

use super::{
    AdmissionPolicy, ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener,
    RouteDescriptor, RouteEffect, RouteProjections,
};

const fn account_post(
    stable_route_id: &'static str,
    path: &'static str,
    effect: RouteEffect,
) -> RouteDescriptor {
    RouteDescriptor::new(
        stable_route_id,
        HttpMethod::Post,
        path,
        ApiSurface::Public,
        Listener::Torii,
        effect,
        AdmissionPolicy::AuthenticatedAccount,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
    .with_projections(RouteProjections::ALL)
    .with_cors_options(true)
}

const fn query_post(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
    account_post(stable_route_id, path, RouteEffect::ReadOnly)
}

const fn instruction_post(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
    account_post(stable_route_id, path, RouteEffect::ExpensiveCompute)
}

/// Fetch one exact structural package record.
pub const EXACT_PACKAGE: RouteDescriptor = query_post(
    "musubi.v1.query.exact_package",
    "/v1/musubi/queries/exact-package",
);
/// Fetch one exact structural release record.
pub const EXACT_RELEASE: RouteDescriptor = query_post(
    "musubi.v1.query.exact_release",
    "/v1/musubi/queries/exact-release",
);
/// Fetch one exact immutable provider bundle-attestation record.
pub const PROVIDER_BUNDLE_ATTESTATION: RouteDescriptor = query_post(
    "musubi.v1.query.provider_bundle_attestation",
    "/v1/musubi/queries/provider-bundle-attestation",
);
/// Fetch a finalized resolver-index page.
pub const RESOLVER_INDEX: RouteDescriptor = query_post(
    "musubi.v1.query.resolver_index",
    "/v1/musubi/queries/resolver-index",
);
/// Fetch a finalized structured-version page.
pub const VERSIONS: RouteDescriptor =
    query_post("musubi.v1.query.versions", "/v1/musubi/queries/versions");
/// Fetch finalized accepted members and pending maintainer invitations.
pub const MAINTAINERS: RouteDescriptor = query_post(
    "musubi.v1.query.maintainers",
    "/v1/musubi/queries/maintainers",
);
/// Fetch a finalized archive-location page.
pub const ARCHIVE_LOCATIONS: RouteDescriptor = query_post(
    "musubi.v1.query.archive_locations",
    "/v1/musubi/queries/archive-locations",
);
/// Fetch bounded exact finalized cache-retention decisions.
pub const ARCHIVE_RETENTION: RouteDescriptor = query_post(
    "musubi.v1.query.archive_retention",
    "/v1/musubi/queries/archive-retention",
);
/// Fetch one exact permanent global alias.
pub const ALIAS: RouteDescriptor = query_post("musubi.v1.query.alias", "/v1/musubi/queries/alias");
/// Fetch a finalized permanent-alias history page.
pub const ALIAS_HISTORY: RouteDescriptor = query_post(
    "musubi.v1.query.alias_history",
    "/v1/musubi/queries/alias-history",
);
/// Fetch a finalized byte-ordered package-prefix page.
pub const ORDERED_PREFIX: RouteDescriptor = query_post(
    "musubi.v1.query.ordered_prefix",
    "/v1/musubi/queries/ordered-prefix",
);
/// Search package names, namespaces, descriptions, and keywords by exact normalized terms.
pub const SEARCH: RouteDescriptor =
    query_post("musubi.v1.query.search", "/v1/musubi/queries/search");
/// Build an unsigned namespace-binding registration.
pub const NAMESPACE_BINDING_REGISTER: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.namespace_binding_register",
    "/v1/musubi/instructions/namespace-binding-register",
);
/// Build an unsigned archive registration.
pub const ARCHIVE_REGISTER: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.archive_register",
    "/v1/musubi/instructions/archive-register",
);
/// Build an unsigned immutable provider bundle-attestation registration.
pub const PROVIDER_BUNDLE_ATTESTATION_REGISTER: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.provider_bundle_attestation_register",
    "/v1/musubi/instructions/provider-bundle-attestation-register",
);
/// Build an unsigned archive-location add or renewal.
pub const ARCHIVE_LOCATION_ADD: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.archive_location_add",
    "/v1/musubi/instructions/archive-location-add",
);
/// Build an unsigned archive-location retirement.
pub const ARCHIVE_LOCATION_RETIRE: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.archive_location_retire",
    "/v1/musubi/instructions/archive-location-retire",
);
/// Build an unsigned release publication.
pub const RELEASE_PUBLISH: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.release_publish",
    "/v1/musubi/instructions/release-publish",
);
/// Build an unsigned reversible yank transition.
pub const RELEASE_YANK_SET: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.release_yank_set",
    "/v1/musubi/instructions/release-yank-set",
);
/// Build an unsigned package metadata replacement.
pub const PACKAGE_METADATA_SET: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.package_metadata_set",
    "/v1/musubi/instructions/package-metadata-set",
);
/// Build an unsigned package-member invitation.
pub const PACKAGE_MEMBER_INVITE: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.package_member_invite",
    "/v1/musubi/instructions/package-member-invite",
);
/// Build an unsigned package-member invitation acceptance.
pub const PACKAGE_MEMBER_ACCEPT: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.package_member_accept",
    "/v1/musubi/instructions/package-member-accept",
);
/// Build an unsigned pending package-member invitation revocation.
pub const PACKAGE_MEMBER_INVITATION_REVOKE: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.package_member_invitation_revoke",
    "/v1/musubi/instructions/package-member-invitation-revoke",
);
/// Build an unsigned package-member role replacement.
pub const PACKAGE_MEMBER_SET_ROLE: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.package_member_set_role",
    "/v1/musubi/instructions/package-member-set-role",
);
/// Build an unsigned package-member removal.
pub const PACKAGE_MEMBER_REMOVE: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.package_member_remove",
    "/v1/musubi/instructions/package-member-remove",
);
/// Build an unsigned paid permanent-alias registration.
pub const ALIAS_REGISTER: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.alias_register",
    "/v1/musubi/instructions/alias-register",
);
/// Build an unsigned Parliament-enacted package recovery.
pub const PACKAGE_RECOVER: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.package_recover",
    "/v1/musubi/instructions/package-recover",
);
/// Build an unsigned Parliament-enacted alias retarget.
pub const ALIAS_RETARGET: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.alias_retarget",
    "/v1/musubi/instructions/alias-retarget",
);
/// Build an unsigned Parliament-enacted artifact takedown.
pub const ARTIFACT_TAKEDOWN: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.artifact_takedown",
    "/v1/musubi/instructions/artifact-takedown",
);
/// Build an unsigned Parliament-enacted registry-policy replacement.
pub const REGISTRY_POLICY_SET: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.registry_policy_set",
    "/v1/musubi/instructions/registry-policy-set",
);
/// Build an unsigned exact release-digest assertion.
pub const RELEASE_DIGEST_ASSERT: RouteDescriptor = instruction_post(
    "musubi.v1.instruction.release_digest_assert",
    "/v1/musubi/instructions/release-digest-assert",
);

/// Complete Musubi route family registered when `app_api` is compiled.
pub const ROUTES: &[RouteDescriptor] = &[
    EXACT_PACKAGE,
    EXACT_RELEASE,
    PROVIDER_BUNDLE_ATTESTATION,
    RESOLVER_INDEX,
    VERSIONS,
    MAINTAINERS,
    ARCHIVE_LOCATIONS,
    ARCHIVE_RETENTION,
    ALIAS,
    ALIAS_HISTORY,
    ORDERED_PREFIX,
    SEARCH,
    NAMESPACE_BINDING_REGISTER,
    ARCHIVE_REGISTER,
    PROVIDER_BUNDLE_ATTESTATION_REGISTER,
    ARCHIVE_LOCATION_ADD,
    ARCHIVE_LOCATION_RETIRE,
    RELEASE_PUBLISH,
    RELEASE_YANK_SET,
    PACKAGE_METADATA_SET,
    PACKAGE_MEMBER_INVITE,
    PACKAGE_MEMBER_ACCEPT,
    PACKAGE_MEMBER_INVITATION_REVOKE,
    PACKAGE_MEMBER_SET_ROLE,
    PACKAGE_MEMBER_REMOVE,
    ALIAS_REGISTER,
    PACKAGE_RECOVER,
    ALIAS_RETARGET,
    ARTIFACT_TAKEDOWN,
    REGISTRY_POLICY_SET,
    RELEASE_DIGEST_ASSERT,
];
