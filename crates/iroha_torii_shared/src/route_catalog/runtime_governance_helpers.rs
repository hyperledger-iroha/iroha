// Account-authenticated runtime/governance descriptor constructors.
const fn account_get(id: &'static str, path: &'static str, effect: RouteEffect) -> RouteDescriptor {
    public_get(id, path)
        .with_effect(effect)
        .with_admission(AdmissionPolicy::AuthenticatedAccount)
        .with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
}
const fn app_account_get(
    id: &'static str,
    path: &'static str,
    effect: RouteEffect,
) -> RouteDescriptor {
    account_get(id, path, effect).with_feature_gate(FeatureGate::Feature("app_api"))
}
const fn signed_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    account_get(id, path, RouteEffect::ReadOnly)
}
const fn account_compute_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    account_get(id, path, RouteEffect::ExpensiveCompute)
}
const fn app_signed_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    app_account_get(id, path, RouteEffect::ReadOnly)
}
const fn app_compute_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    app_account_get(id, path, RouteEffect::ExpensiveCompute)
}
const fn account_post(
    id: &'static str,
    path: &'static str,
    effect: RouteEffect,
) -> RouteDescriptor {
    public_post(id, path)
        .with_effect(effect)
        .with_admission(AdmissionPolicy::AuthenticatedAccount)
        .with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
}
const fn app_account_post(
    id: &'static str,
    path: &'static str,
    effect: RouteEffect,
) -> RouteDescriptor {
    account_post(id, path, effect).with_feature_gate(FeatureGate::Feature("app_api"))
}
const fn account_read_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    account_post(id, path, RouteEffect::ReadOnly)
}
const fn account_compute_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    account_post(id, path, RouteEffect::ExpensiveCompute)
}
const fn app_signed_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    app_account_post(id, path, RouteEffect::ReadOnly)
}
const fn app_compute_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    app_account_post(id, path, RouteEffect::ExpensiveCompute)
}
const fn app_signed_delete(id: &'static str, path: &'static str) -> RouteDescriptor {
    RouteDescriptor::new(
        id,
        HttpMethod::Delete,
        path,
        ApiSurface::Public,
        Listener::Torii,
        RouteEffect::Mutation,
        AdmissionPolicy::AuthenticatedAccount,
    )
    .with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true)
}
