// Same-scope authenticated POST constructors kept separate from the route inventory.

const fn authenticated_account_post(
    route: RouteDescriptor,
    effect: RouteEffect,
) -> RouteDescriptor {
    route
        .with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
        .with_effect(effect)
        .with_admission(AdmissionPolicy::AuthenticatedAccount)
}

const fn soracloud_mutation_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_post(app_sdk_post(id, path), RouteEffect::Mutation)
}

const fn soracloud_read_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_post(app_sdk_post(id, path), RouteEffect::ReadOnly)
}

const fn soracloud_compute_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_post(app_post(id, path), RouteEffect::ExpensiveCompute)
}

const fn soracloud_openapi_mutation_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    authenticated_account_post(app_post(id, path), RouteEffect::Mutation)
}

const fn onboarding_post(id: &'static str, path: &'static str) -> RouteDescriptor {
    app_post(id, path)
        .with_authentication(AuthenticationPolicy::OnboardingToken)
        .with_effect(RouteEffect::Mutation)
        .with_admission(AdmissionPolicy::Operator)
}
