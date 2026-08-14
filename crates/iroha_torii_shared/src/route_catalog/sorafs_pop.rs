//! Closed route descriptors for the authenticated SoraFS proof-of-personhood service.
use super::{
    AdmissionPolicy, ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener,
    RouteDescriptor, RouteEffect, RouteProjections,
};
const fn authenticated_post(
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
        AdmissionPolicy::AuthenticatedProtocolPrincipal,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true)
}
/// Submit one canonical encrypted enrollment.
pub const POP_ENROLLMENT: RouteDescriptor = authenticated_post(
    "sorafs.pop.enrollment.submit",
    "/v1/sorafs/pop/enrollments",
    RouteEffect::Mutation,
);
/// Read payload-free enrollment status.
pub const POP_ENROLLMENT_STATUS: RouteDescriptor = authenticated_post(
    "sorafs.pop.enrollment.status",
    "/v1/sorafs/pop/enrollments/status",
    RouteEffect::ReadOnly,
);
/// Record one governed dual-control approval.
pub const POP_APPROVAL: RouteDescriptor = authenticated_post(
    "sorafs.pop.approval.record",
    "/v1/sorafs/pop/approvals",
    RouteEffect::Mutation,
);
/// Trigger runtime-resolved HSM-backed issuance.
pub const POP_ISSUE: RouteDescriptor = authenticated_post(
    "sorafs.pop.credential.issue",
    "/v1/sorafs/pop/issue",
    RouteEffect::Mutation,
);
/// Enqueue a governed revocation successor.
pub const POP_REVOCATION: RouteDescriptor = authenticated_post(
    "sorafs.pop.revocation.enqueue",
    "/v1/sorafs/pop/revocations",
    RouteEffect::Mutation,
);
/// Submit the next durable registry outbox entry.
pub const POP_REGISTRY_SUBMIT: RouteDescriptor = authenticated_post(
    "sorafs.pop.registry.submit",
    "/v1/sorafs/pop/registry/submit-next",
    RouteEffect::Mutation,
);
/// Reconcile the next finalized registry projection.
pub const POP_REGISTRY_RECONCILE: RouteDescriptor = authenticated_post(
    "sorafs.pop.registry.reconcile",
    "/v1/sorafs/pop/registry/reconcile-next",
    RouteEffect::Mutation,
);
/// Read the current finalized registry projection.
pub const POP_REGISTRY_PROJECTION: RouteDescriptor = authenticated_post(
    "sorafs.pop.registry.projection",
    "/v1/sorafs/pop/registry/projection",
    RouteEffect::ReadOnly,
);
/// Fetch finalized encrypted wallet delivery.
pub const POP_WALLET_DELIVERY: RouteDescriptor = authenticated_post(
    "sorafs.pop.wallet.delivery",
    "/v1/sorafs/pop/wallet/delivery",
    RouteEffect::ReadOnly,
);
/// Import finalized encrypted wallet delivery.
pub const POP_WALLET_IMPORT: RouteDescriptor = authenticated_post(
    "sorafs.pop.wallet.import",
    "/v1/sorafs/pop/wallet/import",
    RouteEffect::Mutation,
);
/// Acknowledge durable wallet delivery.
pub const POP_WALLET_ACKNOWLEDGE: RouteDescriptor = authenticated_post(
    "sorafs.pop.wallet.acknowledge",
    "/v1/sorafs/pop/wallet/acknowledge",
    RouteEffect::Mutation,
);
/// Synchronize a runtime-only wallet witness.
pub const POP_WALLET_SYNCHRONIZE: RouteDescriptor = authenticated_post(
    "sorafs.pop.wallet.synchronize",
    "/v1/sorafs/pop/wallet/synchronize",
    RouteEffect::Mutation,
);
/// Generate a membership proof from local wallet custody.
pub const POP_WALLET_PROVE: RouteDescriptor = authenticated_post(
    "sorafs.pop.wallet.prove",
    "/v1/sorafs/pop/wallet/prove",
    RouteEffect::ExpensiveCompute,
);
/// Verify a membership proof and consume its nullifier.
pub const POP_VERIFY: RouteDescriptor = authenticated_post(
    "sorafs.pop.membership.verify",
    "/v1/sorafs/pop/verify",
    RouteEffect::Mutation,
);
