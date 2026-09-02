const fn operator_local_get(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
    RouteDescriptor::new(
        stable_route_id,
        HttpMethod::Get,
        path,
        ApiSurface::Operator,
        Listener::Torii,
        RouteEffect::ReadOnly,
        AdmissionPolicy::Operator,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::OperatorSignature)
    .with_projections(RouteProjections::NONE)
}
const fn operator_local_expensive_post(
    stable_route_id: &'static str,
    path: &'static str,
) -> RouteDescriptor {
    RouteDescriptor::new(
        stable_route_id,
        HttpMethod::Post,
        path,
        ApiSurface::Operator,
        Listener::Torii,
        RouteEffect::ExpensiveCompute,
        AdmissionPolicy::Operator,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::OperatorSignature)
    .with_projections(RouteProjections::NONE)
}
const fn delegated_routing_get(
    stable_route_id: &'static str,
    path: &'static str,
) -> RouteDescriptor {
    RouteDescriptor::new(
        stable_route_id,
        HttpMethod::Get,
        path,
        ApiSurface::Public,
        Listener::Torii,
        RouteEffect::ReadOnly,
        AdmissionPolicy::Public,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "vendor-neutral HTTP Routing V1 interoperability endpoint",
    })
    .with_cors_options(true)
}
