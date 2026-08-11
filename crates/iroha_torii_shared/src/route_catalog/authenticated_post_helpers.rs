// Same-scope authenticated route constructors kept separate from the route inventory.

const fn authenticated_account_route(
    route: RouteDescriptor,
    effect: RouteEffect,
) -> RouteDescriptor {
    route
        .with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
        .with_effect(effect)
        .with_admission(AdmissionPolicy::AuthenticatedAccount)
}

const fn operator_signed_route(route: RouteDescriptor, effect: RouteEffect) -> RouteDescriptor {
    route
        .with_authentication(AuthenticationPolicy::OperatorSignature)
        .with_effect(effect)
        .with_admission(AdmissionPolicy::Operator)
}

const fn operator_signed_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    operator_signed_route(app_get(id, path), RouteEffect::ReadOnly)
}

const fn operator_expensive_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    operator_signed_route(app_get(id, path), RouteEffect::ExpensiveCompute)
}

const fn operator_signed_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    operator_signed_route(app_post(id, path), RouteEffect::Mutation)
}

const fn operator_signed_delete(id: &'static str, path: &'static str) -> RouteDescriptor {
    operator_signed_route(app_delete(id, path), RouteEffect::Mutation)
}

const fn account_read_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_get(id, path), RouteEffect::ReadOnly)
}

const fn account_read_sdk_get(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_sdk_get(id, path), RouteEffect::ReadOnly)
}

const fn account_mutation_sdk_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_sdk_post(id, path), RouteEffect::Mutation)
}

const fn soracloud_mutation_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    account_mutation_sdk_post(id, path)
}

const fn soracloud_read_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_sdk_post(id, path), RouteEffect::ReadOnly)
}

const fn soracloud_compute_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_post(id, path), RouteEffect::ExpensiveCompute)
}

const fn account_compute_sdk_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_sdk_post(id, path), RouteEffect::ExpensiveCompute)
}

const fn account_compute_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_post(id, path), RouteEffect::ExpensiveCompute)
}

const fn signed_compute_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    app_post(id, path)
        .with_authentication(AuthenticationPolicy::CanonicalSignedBody)
        .with_effect(RouteEffect::ExpensiveCompute)
        .with_admission(AdmissionPolicy::AuthenticatedAccount)
}

const fn soracloud_openapi_mutation_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_route(app_post(id, path), RouteEffect::Mutation)
}

const fn onboarding_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    app_post(id, path)
        .with_authentication(AuthenticationPolicy::OnboardingToken)
        .with_effect(RouteEffect::Mutation)
        .with_admission(AdmissionPolicy::Operator)
}
