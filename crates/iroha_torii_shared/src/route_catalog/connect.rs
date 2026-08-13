use super::{
    AdmissionPolicy, ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener,
    PathPolicy, RouteDescriptor, RouteEffect, RouteProjections,
};
/// Create a wallet-pairing session.
pub const SESSION_CREATE: RouteDescriptor = RouteDescriptor::new(
    "connect.session.create",
    HttpMethod::Post,
    "/v1/connect/session",
    ApiSurface::Protocol,
    Listener::Torii,
    RouteEffect::Mutation,
    AdmissionPolicy::AuthenticatedProtocolPrincipal,
)
.with_feature_gate(FeatureGate::Feature("connect"))
.with_authentication(AuthenticationPolicy::ProtocolHandshake)
.with_projections(RouteProjections::OPENAPI_AND_SDK)
.with_path_policy(PathPolicy::ProtocolException {
    reason: "Connect session bootstrap derives the protocol principal and role tokens",
})
.with_cors_options(true);
/// Delete a wallet-pairing session using its management token.
pub const SESSION_DELETE: RouteDescriptor = RouteDescriptor::new(
    "connect.session.delete",
    HttpMethod::Delete,
    "/v1/connect/session/{sid}",
    ApiSurface::Protocol,
    Listener::Torii,
    RouteEffect::Mutation,
    AdmissionPolicy::AuthenticatedProtocolPrincipal,
)
.with_feature_gate(FeatureGate::Feature("connect"))
.with_authentication(AuthenticationPolicy::ProtocolHandshake)
.with_projections(RouteProjections::OPENAPI_AND_SDK)
.with_path_policy(PathPolicy::ProtocolException {
    reason: "Connect management-token session teardown",
})
.with_cors_options(true);
/// Upgrade to the authenticated Connect relay WebSocket.
pub const WEBSOCKET: RouteDescriptor = RouteDescriptor::new(
    "connect.websocket",
    HttpMethod::Get,
    "/v1/connect/ws",
    ApiSurface::Protocol,
    Listener::Torii,
    RouteEffect::LongLivedStream,
    AdmissionPolicy::AuthenticatedProtocolPrincipal,
)
.with_feature_gate(FeatureGate::Feature("connect"))
.with_authentication(AuthenticationPolicy::ProtocolHandshake)
.with_projections(RouteProjections::OPENAPI_AND_SDK)
.with_path_policy(PathPolicy::ProtocolException {
    reason: "Connect WebSocket transport endpoint",
})
.with_implicit_head(true);
/// Read one management-token-authorized session status.
pub const SESSION_STATUS: RouteDescriptor = RouteDescriptor::new(
    "connect.session.status",
    HttpMethod::Get,
    "/v1/connect/status",
    ApiSurface::Protocol,
    Listener::Torii,
    RouteEffect::ReadOnly,
    AdmissionPolicy::AuthenticatedProtocolPrincipal,
)
.with_feature_gate(FeatureGate::Feature("connect"))
.with_authentication(AuthenticationPolicy::ProtocolHandshake)
.with_projections(RouteProjections::OPENAPI_AND_SDK)
.with_path_policy(PathPolicy::ProtocolException {
    reason: "Connect management-token session status endpoint",
})
.with_implicit_head(true)
.with_cors_options(true);
/// Read aggregate node-local Connect status as an authenticated operator.
pub const STATUS: RouteDescriptor = RouteDescriptor::new(
    "connect.status",
    HttpMethod::Get,
    "/v1/connect/status/aggregate",
    ApiSurface::Operator,
    Listener::Torii,
    RouteEffect::ReadOnly,
    AdmissionPolicy::Operator,
)
.with_feature_gate(FeatureGate::Feature("connect"))
.with_authentication(AuthenticationPolicy::OperatorSignature)
.with_projections(RouteProjections::OPENAPI_AND_SDK)
.with_implicit_head(true)
.with_cors_options(true);
/// Canonical Connect route set.
pub const ROUTES: &[RouteDescriptor] = &[
    SESSION_CREATE,
    SESSION_DELETE,
    WEBSOCKET,
    SESSION_STATUS,
    STATUS,
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_and_aggregate_status_have_disjoint_authentication() {
        for route in [SESSION_CREATE, SESSION_DELETE, WEBSOCKET, SESSION_STATUS] {
            assert_eq!(route.surface(), ApiSurface::Protocol);
            assert_eq!(
                route.admission(),
                AdmissionPolicy::AuthenticatedProtocolPrincipal
            );
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::ProtocolHandshake
            );
        }
        assert_eq!(SESSION_STATUS.path(), "/v1/connect/status");
        assert_eq!(STATUS.path(), "/v1/connect/status/aggregate");
        assert_eq!(STATUS.surface(), ApiSurface::Operator);
        assert_eq!(STATUS.admission(), AdmissionPolicy::Operator);
        assert_eq!(
            STATUS.authentication(),
            AuthenticationPolicy::OperatorSignature
        );
        assert_eq!(RouteEffect::ReadOnly, SESSION_STATUS.effect());
        assert_eq!(RouteEffect::ReadOnly, STATUS.effect());
    }
}
