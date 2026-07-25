//! Canonical metadata for Torii HTTP routes.
//!
//! The catalog is deliberately independent of an HTTP framework. Torii uses it
//! to decide which routes are mounted for a build, while documentation and
//! client tooling consume explicit projections of the same descriptors.

use std::collections::{BTreeMap, BTreeSet};

/// HTTP methods supported by the Torii route catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    /// Match every HTTP method for a protocol-native gateway route.
    Any,
    /// Retrieve a resource or collection.
    Get,
    /// Submit a command or create a subordinate resource.
    Post,
    /// Replace a resource completely.
    Put,
    /// Update part of a resource.
    Patch,
    /// Delete a resource.
    Delete,
}

impl HttpMethod {
    /// Return the canonical uppercase spelling of the method.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "ANY",
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Security and audience boundary for a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiSurface {
    /// Application-facing API available through the public Torii listener.
    Public,
    /// Privileged administration API available through the operator listener.
    Operator,
    /// Operational diagnostics available only through the diagnostic listener.
    Diagnostic,
    /// Protocol-native endpoint whose listener is declared independently.
    Protocol,
}

/// Listener on which a route is eligible to be mounted.
///
/// Torii currently exposes one HTTP listener. Audience and authentication are
/// therefore modeled separately by [`ApiSurface`] and [`AuthenticationPolicy`]
/// instead of pretending that operator or diagnostic routes have a network
/// boundary which does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Listener {
    /// The single configured Torii HTTP listener.
    Torii,
}

/// Authentication contract enforced by the route boundary.
///
/// Most policies are middleware-backed. Protocol exchanges and explicitly
/// reviewed canonical-account handlers may enforce their credential while
/// entering the handler, before parsing or acting on protected request data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthenticationPolicy {
    /// The listener's configured API-token policy applies.
    ToriiDefault,
    /// The listener's configured API-token policy applies and the route also
    /// requires exactly one dedicated signer-backed onboarding token.
    OnboardingToken,
    /// The route requires canonical `X-Iroha-*` request authentication bound to
    /// an on-ledger account identity.
    CanonicalAccountSignature,
    /// The route requires an operator-style request signature bound to a
    /// handler-validated dynamic key identity.
    IdentityBoundSignature,
    /// The route additionally requires an operator signature.
    OperatorSignature,
    /// The operator credential exchange authenticates inside the handler.
    ///
    /// `WebAuthn` registration and login cannot require an already-established
    /// operator signature: registration accepts the configured bootstrap
    /// credential until enrollment, while login verifies a `WebAuthn` challenge.
    /// The handlers still enforce mTLS, rate limits, lockout, bootstrap/session
    /// policy, and challenge verification as appropriate.
    OperatorCredentialExchange,
    /// The protocol performs authentication inside its own handshake.
    ProtocolHandshake,
    /// The route is intentionally usable without route-specific credentials.
    ///
    /// Listener-wide controls can still restrict this route.
    Unauthenticated,
}

/// Router path normalization accepted by a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathNormalization {
    /// Match the declared path exactly; do not add trailing-slash redirects or
    /// duplicate-slash/case aliases.
    Strict,
}

/// How the router matches the path declared by a descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteMatch {
    /// Match only the exact path shape.
    Exact,
    /// Match a final `{*parameter}` wildcard segment.
    Wildcard,
}

/// Whether a path follows the canonical `/v1` grammar or is a reviewed exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathPolicy {
    /// Apply the canonical `/v1` path grammar.
    CanonicalV1,
    /// Permit a protocol-native path outside the canonical grammar.
    ProtocolException {
        /// Non-empty review rationale for the exception.
        reason: &'static str,
    },
}

/// Build-feature expression controlling whether a route is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureGate {
    /// The route is available in every build.
    Always,
    /// The route requires one Cargo feature.
    Feature(&'static str),
    /// The route requires all listed Cargo features.
    All(&'static [&'static str]),
    /// The route requires at least one listed Cargo feature.
    Any(&'static [&'static str]),
}

impl FeatureGate {
    /// Determine whether this expression is satisfied by `features`.
    #[must_use]
    pub fn is_enabled(self, features: EnabledFeatures<'_>) -> bool {
        match self {
            Self::Always => true,
            Self::Feature(name) => features.contains(name),
            Self::All(names) => names.iter().all(|name| features.contains(name)),
            Self::Any(names) => names.iter().any(|name| features.contains(name)),
        }
    }
}

/// Features enabled for one catalog projection.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnabledFeatures<'a> {
    names: &'a [&'a str],
}

impl<'a> EnabledFeatures<'a> {
    /// Construct a feature set from canonical Cargo feature names.
    #[must_use]
    pub const fn new(names: &'a [&'a str]) -> Self {
        Self { names }
    }

    /// Construct an empty feature set.
    #[must_use]
    pub const fn none() -> Self {
        Self { names: &[] }
    }

    /// Return whether `name` is enabled.
    #[must_use]
    pub fn contains(self, name: &str) -> bool {
        self.names.contains(&name)
    }
}

/// Explicit documentation and tooling exposure decisions for a route.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RouteProjections(u8);

impl RouteProjections {
    const OPENAPI_BIT: u8 = 1 << 0;
    const SDK_BIT: u8 = 1 << 1;
    const MCP_BIT: u8 = 1 << 2;

    /// Do not expose the route through a generated projection.
    pub const NONE: Self = Self(0);
    /// Expose the route in `OpenAPI` only.
    pub const OPENAPI: Self = Self(Self::OPENAPI_BIT);
    /// Expose the route to generated SDKs only.
    pub const SDK: Self = Self(Self::SDK_BIT);
    /// Expose the route as an explicitly allowlisted MCP operation only.
    pub const MCP: Self = Self(Self::MCP_BIT);
    /// Expose the route in `OpenAPI` and generated SDKs.
    pub const OPENAPI_AND_SDK: Self = Self(Self::OPENAPI_BIT | Self::SDK_BIT);
    /// Expose the route in every generated projection.
    pub const ALL: Self = Self(Self::OPENAPI_BIT | Self::SDK_BIT | Self::MCP_BIT);

    /// Return whether the route belongs in the `OpenAPI` projection.
    #[must_use]
    pub const fn openapi(self) -> bool {
        self.0 & Self::OPENAPI_BIT != 0
    }

    /// Return whether the route belongs in the canonical SDK projection.
    #[must_use]
    pub const fn sdk(self) -> bool {
        self.0 & Self::SDK_BIT != 0
    }

    /// Return whether the route is explicitly allowlisted for MCP.
    #[must_use]
    pub const fn mcp(self) -> bool {
        self.0 & Self::MCP_BIT != 0
    }

    /// Combine two projection sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A consumer-specific view of the canonical route catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogProjection {
    /// Routes mounted by a build with the supplied features.
    Mounted,
    /// Enabled routes explicitly included in `OpenAPI`.
    OpenApi,
    /// Canonical SDK superset, independent of one node's enabled features.
    Sdk,
    /// Enabled routes explicitly allowlisted for MCP.
    Mcp,
}

/// A framework-level route which is intentionally not an application
/// operation in `OpenAPI`, SDK, or MCP projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImplicitRouteKind {
    /// Axum's GET method router also answers HEAD without invoking a distinct
    /// application operation.
    Head,
    /// CORS middleware may terminate a preflight OPTIONS request before the
    /// application handler.
    CorsOptions,
}

/// Manifest entry for framework-generated HTTP behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplicitRouteDescriptor {
    parent_route_id: &'static str,
    path: &'static str,
    kind: ImplicitRouteKind,
}

impl ImplicitRouteDescriptor {
    /// Return the explicit route which authorizes this framework behavior.
    #[must_use]
    pub const fn parent_route_id(self) -> &'static str {
        self.parent_route_id
    }

    /// Return the exact path affected by the framework behavior.
    #[must_use]
    pub const fn path(self) -> &'static str {
        self.path
    }

    /// Return the framework behavior kind.
    #[must_use]
    pub const fn kind(self) -> ImplicitRouteKind {
        self.kind
    }
}

/// Static metadata describing one canonical Torii route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteDescriptor {
    stable_route_id: &'static str,
    method: HttpMethod,
    path: &'static str,
    surface: ApiSurface,
    listener: Listener,
    authentication: AuthenticationPolicy,
    feature_gate: FeatureGate,
    projections: RouteProjections,
    route_match: RouteMatch,
    path_policy: PathPolicy,
    path_normalization: PathNormalization,
    implicit_head: bool,
    cors_options: bool,
}

impl RouteDescriptor {
    /// Construct a route with conservative defaults and no generated exposure.
    #[must_use]
    pub const fn new(
        stable_route_id: &'static str,
        method: HttpMethod,
        path: &'static str,
        surface: ApiSurface,
        listener: Listener,
    ) -> Self {
        Self {
            stable_route_id,
            method,
            path,
            surface,
            listener,
            authentication: AuthenticationPolicy::ToriiDefault,
            feature_gate: FeatureGate::Always,
            projections: RouteProjections::NONE,
            route_match: RouteMatch::Exact,
            path_policy: PathPolicy::CanonicalV1,
            path_normalization: PathNormalization::Strict,
            implicit_head: false,
            cors_options: false,
        }
    }

    /// Set the build-feature expression for the route.
    #[must_use]
    pub const fn with_feature_gate(mut self, feature_gate: FeatureGate) -> Self {
        self.feature_gate = feature_gate;
        self
    }

    /// Set the route-specific authentication policy.
    #[must_use]
    pub const fn with_authentication(mut self, authentication: AuthenticationPolicy) -> Self {
        self.authentication = authentication;
        self
    }

    /// Set generated documentation and tooling projections.
    #[must_use]
    pub const fn with_projections(mut self, projections: RouteProjections) -> Self {
        self.projections = projections;
        self
    }

    /// Set the router match behavior.
    #[must_use]
    pub const fn with_route_match(mut self, route_match: RouteMatch) -> Self {
        self.route_match = route_match;
        self
    }

    /// Set the path grammar policy.
    #[must_use]
    pub const fn with_path_policy(mut self, path_policy: PathPolicy) -> Self {
        self.path_policy = path_policy;
        self
    }

    /// Declare whether GET may also generate framework-level HEAD behavior.
    #[must_use]
    pub const fn with_implicit_head(mut self, implicit_head: bool) -> Self {
        self.implicit_head = implicit_head;
        self
    }

    /// Declare whether CORS middleware may answer OPTIONS for this path.
    #[must_use]
    pub const fn with_cors_options(mut self, cors_options: bool) -> Self {
        self.cors_options = cors_options;
        self
    }

    /// Return the stable, low-cardinality telemetry and generation identifier.
    #[must_use]
    pub const fn stable_route_id(self) -> &'static str {
        self.stable_route_id
    }

    /// Return the HTTP method.
    #[must_use]
    pub const fn method(self) -> HttpMethod {
        self.method
    }

    /// Return the canonical router path.
    #[must_use]
    pub const fn path(self) -> &'static str {
        self.path
    }

    /// Return the security and audience surface.
    #[must_use]
    pub const fn surface(self) -> ApiSurface {
        self.surface
    }

    /// Return the listener on which the route is mounted.
    #[must_use]
    pub const fn listener(self) -> Listener {
        self.listener
    }

    /// Return the route-specific authentication policy.
    #[must_use]
    pub const fn authentication(self) -> AuthenticationPolicy {
        self.authentication
    }

    /// Return the build-feature expression.
    #[must_use]
    pub const fn feature_gate(self) -> FeatureGate {
        self.feature_gate
    }

    /// Return the explicit generated projections.
    #[must_use]
    pub const fn projections(self) -> RouteProjections {
        self.projections
    }

    /// Return the router match behavior.
    #[must_use]
    pub const fn route_match(self) -> RouteMatch {
        self.route_match
    }

    /// Return the path grammar policy.
    #[must_use]
    pub const fn path_policy(self) -> PathPolicy {
        self.path_policy
    }

    /// Return the path normalization policy.
    #[must_use]
    pub const fn path_normalization(self) -> PathNormalization {
        self.path_normalization
    }

    /// Return whether framework-level HEAD behavior is declared.
    #[must_use]
    pub const fn implicit_head(self) -> bool {
        self.implicit_head
    }

    /// Return whether CORS middleware may answer OPTIONS.
    #[must_use]
    pub const fn cors_options(self) -> bool {
        self.cors_options
    }

    fn is_in_projection(
        self,
        projection: CatalogProjection,
        features: EnabledFeatures<'_>,
    ) -> bool {
        match projection {
            CatalogProjection::Mounted => self.feature_gate.is_enabled(features),
            CatalogProjection::OpenApi => {
                self.projections.openapi() && self.feature_gate.is_enabled(features)
            }
            CatalogProjection::Sdk => self.projections.sdk(),
            CatalogProjection::Mcp => {
                self.projections.mcp() && self.feature_gate.is_enabled(features)
            }
        }
    }
}

/// Read-only view over a canonical set of route descriptors.
#[derive(Debug, Clone, Copy)]
pub struct RouteCatalog<'a> {
    routes: &'a [RouteDescriptor],
}

impl<'a> RouteCatalog<'a> {
    /// Construct a catalog over `routes`.
    #[must_use]
    pub const fn new(routes: &'a [RouteDescriptor]) -> Self {
        Self { routes }
    }

    /// Return every descriptor in declaration order.
    #[must_use]
    pub const fn routes(self) -> &'a [RouteDescriptor] {
        self.routes
    }

    /// Validate uniqueness, grammar, listener boundaries, and projection policy.
    ///
    /// All violations are returned in declaration order so CI can report a
    /// complete catalog failure in one run.
    ///
    /// # Errors
    ///
    /// Returns every detected [`CatalogValidationError`] when any descriptor
    /// violates the catalog contract.
    pub fn validate(self) -> Result<(), Vec<CatalogValidationError>> {
        validate_catalog(self.routes)
    }

    /// Materialize one consumer projection in declaration order.
    ///
    /// Mounted, `OpenAPI`, and MCP projections honor `features`. The SDK
    /// projection is the canonical feature-independent superset.
    #[must_use]
    pub fn project(
        self,
        projection: CatalogProjection,
        features: EnabledFeatures<'_>,
    ) -> Vec<&'a RouteDescriptor> {
        self.routes
            .iter()
            .filter(|route| route.is_in_projection(projection, features))
            .collect()
    }

    /// Materialize declared framework-level HEAD and CORS OPTIONS behavior.
    ///
    /// These entries remain separate from explicit application operations so
    /// projections never accidentally generate SDK or MCP methods for them.
    #[must_use]
    pub fn implicit_routes(self, features: EnabledFeatures<'_>) -> Vec<ImplicitRouteDescriptor> {
        let mut routes = Vec::new();
        let mut cors_paths = BTreeSet::new();
        for route in self
            .routes
            .iter()
            .filter(|route| route.is_in_projection(CatalogProjection::Mounted, features))
        {
            if route.implicit_head {
                routes.push(ImplicitRouteDescriptor {
                    parent_route_id: route.stable_route_id,
                    path: route.path,
                    kind: ImplicitRouteKind::Head,
                });
            }
            if route.cors_options && cors_paths.insert(route.path) {
                routes.push(ImplicitRouteDescriptor {
                    parent_route_id: route.stable_route_id,
                    path: route.path,
                    kind: ImplicitRouteKind::CorsOptions,
                });
            }
        }
        routes
    }
}

/// One catalog validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogValidationError {
    /// Route being validated.
    pub stable_route_id: &'static str,
    /// Machine-readable validation failure.
    pub kind: CatalogValidationErrorKind,
}

/// Machine-readable catalog validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogValidationErrorKind {
    /// The stable route ID does not use dot-separated lower-snake-case segments.
    InvalidStableRouteId,
    /// Another descriptor already uses the same stable route ID.
    DuplicateStableRouteId,
    /// Another descriptor already uses the same method and path.
    DuplicateMethodAndPath {
        /// Stable ID of the first descriptor with this method and path.
        existing_route_id: &'static str,
    },
    /// Another descriptor uses the same router shape with different parameter
    /// names.
    DuplicateMethodAndShape {
        /// Stable ID of the first descriptor with this method and shape.
        existing_route_id: &'static str,
    },
    /// The path violates its declared grammar policy.
    InvalidPath {
        /// Stable, human-readable reason suitable for CI diagnostics.
        reason: &'static str,
    },
    /// A feature name is not a canonical Cargo feature name.
    InvalidFeatureName {
        /// Invalid feature name.
        feature: &'static str,
    },
    /// An `All` or `Any` feature expression contains no features.
    EmptyFeatureExpression,
    /// Diagnostic routes cannot be projected into SDKs or MCP.
    DiagnosticToolingProjection,
    /// A protocol-handshake route cannot be represented as an ordinary MCP
    /// request/response tool.
    ProtocolHandshakeMcpProjection,
    /// Operator-surface routes must enforce an operator authentication policy.
    OperatorSurfaceRequiresAuthentication,
    /// Operator credential exchange is valid only on the operator surface.
    OperatorCredentialExchangeRequiresOperatorSurface,
    /// Only GET descriptors may request implicit HEAD handling.
    ImplicitHeadRequiresGet,
    /// Axum GET routing always provides framework-level HEAD handling.
    GetRequiresImplicitHead,
    /// Catch-all method routing is reserved for protocol-native gateways.
    AnyMethodRequiresProtocolSurface,
    /// Catch-all method routing cannot be projected into generated tooling.
    AnyMethodToolingProjection,
}

/// Validate a complete route catalog.
///
/// This function reports all detected violations rather than stopping at the
/// first one. An empty slice is valid so feature-composed catalogs can be
/// checked uniformly.
///
/// # Errors
///
/// Returns every detected [`CatalogValidationError`] when any descriptor
/// violates the catalog contract.
#[expect(
    clippy::too_many_lines,
    reason = "the single-pass closed-catalog validator keeps every route invariant and error ordering explicit"
)]
pub fn validate_catalog(routes: &[RouteDescriptor]) -> Result<(), Vec<CatalogValidationError>> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut method_paths = BTreeMap::new();
    let mut method_shapes = BTreeMap::new();

    for route in routes {
        let route_id = route.stable_route_id;

        if !valid_stable_route_id(route_id) {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::InvalidStableRouteId,
            });
        }
        if !ids.insert(route_id) {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::DuplicateStableRouteId,
            });
        }
        let duplicate_exact = method_paths
            .insert((route.method, route.path), route_id)
            .is_some_and(|existing_route_id| {
                errors.push(CatalogValidationError {
                    stable_route_id: route_id,
                    kind: CatalogValidationErrorKind::DuplicateMethodAndPath { existing_route_id },
                });
                true
            });
        let shape = normalized_route_shape(route.path);
        if let Some(existing_route_id) = method_shapes.insert((route.method, shape), route_id)
            && !duplicate_exact
        {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::DuplicateMethodAndShape { existing_route_id },
            });
        }

        if let Err(reason) = validate_path(route) {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::InvalidPath { reason },
            });
        }

        match route.feature_gate {
            FeatureGate::All(names) | FeatureGate::Any(names) if names.is_empty() => {
                errors.push(CatalogValidationError {
                    stable_route_id: route_id,
                    kind: CatalogValidationErrorKind::EmptyFeatureExpression,
                });
            }
            _ => {}
        }
        match route.feature_gate {
            FeatureGate::Always => {}
            FeatureGate::Feature(feature) => {
                validate_feature_name(route_id, feature, &mut errors);
            }
            FeatureGate::All(features) | FeatureGate::Any(features) => {
                for &feature in features {
                    validate_feature_name(route_id, feature, &mut errors);
                }
            }
        }

        if route.surface == ApiSurface::Diagnostic
            && (route.projections.sdk() || route.projections.mcp())
        {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::DiagnosticToolingProjection,
            });
        }

        if route.authentication == AuthenticationPolicy::ProtocolHandshake
            && route.projections.mcp()
        {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::ProtocolHandshakeMcpProjection,
            });
        }

        if route.surface == ApiSurface::Operator
            && !matches!(
                route.authentication,
                AuthenticationPolicy::OperatorSignature
                    | AuthenticationPolicy::OperatorCredentialExchange
            )
            && !(route.stable_route_id == "operator.internal_torii_proxy"
                && route.authentication == AuthenticationPolicy::IdentityBoundSignature)
        {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::OperatorSurfaceRequiresAuthentication,
            });
        }

        if route.authentication == AuthenticationPolicy::OperatorCredentialExchange
            && route.surface != ApiSurface::Operator
        {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::OperatorCredentialExchangeRequiresOperatorSurface,
            });
        }

        if route.implicit_head && route.method != HttpMethod::Get {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::ImplicitHeadRequiresGet,
            });
        } else if route.method == HttpMethod::Get && !route.implicit_head {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::GetRequiresImplicitHead,
            });
        }

        if route.method == HttpMethod::Any {
            if route.surface != ApiSurface::Protocol {
                errors.push(CatalogValidationError {
                    stable_route_id: route_id,
                    kind: CatalogValidationErrorKind::AnyMethodRequiresProtocolSurface,
                });
            }
            if route.projections != RouteProjections::NONE {
                errors.push(CatalogValidationError {
                    stable_route_id: route_id,
                    kind: CatalogValidationErrorKind::AnyMethodToolingProjection,
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn normalized_route_shape(path: &str) -> String {
    let mut shape = String::with_capacity(path.len());
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            shape.push('/');
        }
        if segment.starts_with("{*") || segment.ends_with("..}") {
            shape.push_str("{*}");
        } else if segment.starts_with('{') && segment.ends_with('}') {
            shape.push_str("{}");
        } else {
            shape.push_str(segment);
        }
    }
    shape
}

fn validate_path(route: &RouteDescriptor) -> Result<(), &'static str> {
    let path = route.path;
    if !path.starts_with('/') {
        return Err("path must begin with a slash");
    }
    if path.len() > 1 && path.ends_with('/') {
        return Err("path must not have a trailing slash");
    }
    if path.contains("//") {
        return Err("path must not contain duplicate slashes");
    }
    if path.contains(['?', '#', '%']) {
        return Err("path must not contain query, fragment, or percent-encoded text");
    }

    if let PathPolicy::ProtocolException { reason } = route.path_policy {
        if reason.trim().is_empty() {
            return Err("protocol exception must include a review rationale");
        }
        return validate_wildcard_shape(path, route.route_match);
    }

    let Some(remaining_path) = path.strip_prefix("/v1/") else {
        return Err("canonical path must begin with /v1/");
    };
    if remaining_path.is_empty() {
        return Err("canonical path must name a resource after /v1");
    }

    let segment_count = remaining_path.split('/').count();
    let mut parameters = BTreeSet::new();
    for (index, segment) in remaining_path.split('/').enumerate() {
        if let Some(parameter) = segment
            .strip_prefix("{*")
            .and_then(|value| value.strip_suffix('}'))
        {
            if route.route_match != RouteMatch::Wildcard || index + 1 != segment_count {
                return Err("wildcard parameter must be declared and appear last");
            }
            if !valid_snake_name(parameter) || !parameters.insert(parameter) {
                return Err("path parameters must be unique lower-snake-case names");
            }
        } else if let Some(parameter) = segment
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        {
            if !valid_snake_name(parameter) || !parameters.insert(parameter) {
                return Err("path parameters must be unique lower-snake-case names");
            }
        } else {
            if !valid_kebab_segment(segment) {
                return Err("static path segments must use lowercase kebab-case");
            }
            if matches!(segment, "get" | "list" | "json" | "sse") {
                return Err("static path segment uses a forbidden transport or CRUD word");
            }
            if segment.strip_prefix('v').is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            }) {
                return Err("nested API-version segment is forbidden");
            }
        }
    }

    validate_wildcard_shape(path, route.route_match)
}

fn validate_wildcard_shape(path: &str, route_match: RouteMatch) -> Result<(), &'static str> {
    let segment_count = path.split('/').count();
    let mut wildcard_count = 0;
    let mut wildcard_is_final = false;
    for (index, segment) in path.split('/').enumerate() {
        if segment.starts_with("{*") && segment.ends_with('}') {
            wildcard_count += 1;
            wildcard_is_final = index + 1 == segment_count;
        }
    }
    match (route_match, wildcard_count) {
        (RouteMatch::Exact, 0) => Ok(()),
        (RouteMatch::Wildcard, 1) if wildcard_is_final => Ok(()),
        (RouteMatch::Wildcard, 1) => Err("wildcard parameter must be the final path segment"),
        (RouteMatch::Exact, _) => Err("exact route must not contain a wildcard parameter"),
        (RouteMatch::Wildcard, 0) => Err("wildcard route must contain a wildcard parameter"),
        (RouteMatch::Wildcard, _) => {
            Err("wildcard route must contain exactly one wildcard parameter")
        }
    }
}

fn valid_stable_route_id(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(valid_snake_name)
}

fn valid_snake_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }

    let mut previous_underscore = false;
    for byte in bytes {
        if byte == b'_' {
            if previous_underscore {
                return false;
            }
            previous_underscore = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_underscore = false;
        } else {
            return false;
        }
    }
    !previous_underscore
}

fn valid_kebab_segment(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }

    let mut previous_hyphen = false;
    for byte in bytes {
        if byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_hyphen = false;
        } else {
            return false;
        }
    }
    !previous_hyphen
}

fn valid_feature_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn validate_feature_name(
    route_id: &'static str,
    feature: &'static str,
    errors: &mut Vec<CatalogValidationError>,
) {
    if !valid_feature_name(feature) {
        errors.push(CatalogValidationError {
            stable_route_id: route_id,
            kind: CatalogValidationErrorKind::InvalidFeatureName { feature },
        });
    }
}

/// Final first-release offline route descriptors.
pub mod offline {
    use super::{ApiSurface, FeatureGate, HttpMethod, Listener, RouteDescriptor, RouteProjections};

    /// Fetch evaluated offline-payment readiness for an asset definition.
    pub const READINESS_PATH: &str = "/v1/offline/readiness";
    /// Resolve proof-bearing active registration lineage for a signed receiver request.
    pub const RECIPIENT_LINEAGE_PATH: &str = "/v1/offline/receiver-lineage";
    /// Submit a signed online-to-offline top-up operation.
    pub const TOP_UP_PATH: &str = "/v1/offline/top-up";
    /// Submit a signed offline redemption operation.
    pub const REDEEM_PATH: &str = "/v1/offline/redeem";
    /// Fetch one offline operation by its canonical operation ID.
    pub const OPERATION_PATH: &str = "/v1/offline/operations/{operation_id}";

    /// Descriptor for offline-payment readiness evaluation.
    pub const READINESS: RouteDescriptor = RouteDescriptor::new(
        "offline.readiness",
        HttpMethod::Get,
        READINESS_PATH,
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Descriptor for proof-bearing receiver-registration lineage resolution.
    pub const RECIPIENT_LINEAGE: RouteDescriptor = RouteDescriptor::new(
        "offline.receiver_lineage",
        HttpMethod::Post,
        RECIPIENT_LINEAGE_PATH,
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Descriptor for online-to-offline top-up submission.
    pub const TOP_UP: RouteDescriptor = RouteDescriptor::new(
        "offline.top_up",
        HttpMethod::Post,
        TOP_UP_PATH,
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Descriptor for offline redemption submission.
    pub const REDEEM: RouteDescriptor = RouteDescriptor::new(
        "offline.redeem",
        HttpMethod::Post,
        REDEEM_PATH,
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Descriptor for reading one offline operation.
    pub const OPERATION: RouteDescriptor = RouteDescriptor::new(
        "offline.operation",
        HttpMethod::Get,
        OPERATION_PATH,
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);

    /// Canonical first-release offline API catalog.
    pub const ROUTES: &[RouteDescriptor] =
        &[READINESS, RECIPIENT_LINEAGE, TOP_UP, REDEEM, OPERATION];
}

/// Alias lookup, private evaluation, and recipient-resolution descriptors.
pub mod aliases {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, RouteDescriptor,
        RouteProjections,
    };

    const fn public_lookup(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::ALL)
        .with_cors_options(true)
    }

    /// Resolve an account alias.
    pub const RESOLVE: RouteDescriptor = public_lookup("aliases.resolve", "/v1/aliases/resolve");
    /// Plan one atomic declarative alias setup transaction.
    pub const SETUP_PLAN: RouteDescriptor =
        public_lookup("aliases.setup_plan", "/v1/aliases/setup/plan")
            .with_authentication(AuthenticationPolicy::CanonicalAccountSignature);
    /// Plan one guarded absolute-expiry alias lease renewal.
    pub const LEASE_RENEW_PLAN: RouteDescriptor =
        public_lookup("aliases.lease_renew_plan", "/v1/aliases/lease/renew/plan")
            .with_authentication(AuthenticationPolicy::CanonicalAccountSignature);
    /// Plan one owner-only alias auto-renew configuration CAS.
    pub const AUTO_RENEW_PLAN: RouteDescriptor =
        public_lookup("aliases.auto_renew_plan", "/v1/aliases/auto-renew/plan")
            .with_authentication(AuthenticationPolicy::CanonicalAccountSignature);
    /// Resolve the deterministic numeric alias index.
    pub const RESOLVE_INDEX: RouteDescriptor =
        public_lookup("aliases.resolve_index", "/v1/aliases/resolve-index");
    /// List aliases bound to an account.
    pub const BY_ACCOUNT: RouteDescriptor =
        public_lookup("aliases.by_account", "/v1/aliases/by-account");
    /// Resolve a retail recipient reference.
    pub const RETAIL_RECIPIENT_LOOKUP: RouteDescriptor =
        public_lookup("retail.recipient.lookup", "/v1/retail/recipients/lookup")
            .with_authentication(AuthenticationPolicy::CanonicalAccountSignature);
    /// Resolve a privacy-minimized retail recipient route.
    pub const RETAIL_RECIPIENT_ROUTE: RouteDescriptor =
        public_lookup("retail.recipient.route", "/v1/retail/recipients/route")
            .with_authentication(AuthenticationPolicy::CanonicalAccountSignature);
    /// Resolve an asset alias.
    pub const ASSET_RESOLVE: RouteDescriptor =
        public_lookup("assets.alias.resolve", "/v1/assets/aliases/resolve");

    /// Alias routes registered when `app_api` is compiled.
    pub const ROUTES: &[RouteDescriptor] = &[
        SETUP_PLAN,
        LEASE_RENEW_PLAN,
        AUTO_RENEW_PLAN,
        RESOLVE,
        RESOLVE_INDEX,
        BY_ACCOUNT,
        RETAIL_RECIPIENT_LOOKUP,
        RETAIL_RECIPIENT_ROUTE,
        ASSET_RESOLVE,
    ];
}

/// Fee quoting and sponsor-program read descriptors.
pub mod fees {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, RouteDescriptor,
        RouteProjections,
    };

    /// Canonical fee quote path.
    pub const QUOTE_PATH: &str = "/v1/fees/quote";
    /// Canonical exact sponsor-program lookup path.
    pub const SPONSOR_PROGRAM_BY_ID_PATH: &str = "/v1/fee-sponsor-programs/by-id";

    const fn account_signed_post(
        stable_route_id: &'static str,
        path: &'static str,
    ) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::ALL)
        .with_cors_options(true)
    }

    /// Quote the required signature-bound fee intent for one unsigned payload.
    pub const QUOTE: RouteDescriptor = account_signed_post("fees.quote", QUOTE_PATH);
    /// Read one exact on-chain sponsor program.
    pub const SPONSOR_PROGRAM_BY_ID: RouteDescriptor =
        account_signed_post("fee_sponsor_program.by_id", SPONSOR_PROGRAM_BY_ID_PATH);

    /// Canonical first-release fee API catalog.
    pub const ROUTES: &[RouteDescriptor] = &[QUOTE, SPONSOR_PROGRAM_BY_ID];
}

/// Operator `WebAuthn` credential-registration and login descriptors.
pub mod operator_authentication {
    use super::{
        ApiSurface, AuthenticationPolicy, HttpMethod, Listener, RouteDescriptor, RouteProjections,
    };

    const fn credential_exchange(
        stable_route_id: &'static str,
        path: &'static str,
    ) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Post,
            path,
            ApiSurface::Operator,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::OperatorCredentialExchange)
        .with_projections(RouteProjections::OPENAPI)
        .with_cors_options(true)
    }

    /// Start operator `WebAuthn` credential registration.
    pub const REGISTRATION_OPTIONS: RouteDescriptor = credential_exchange(
        "operator.authentication.registration_options",
        "/v1/operator/auth/registration/options",
    );
    /// Verify and persist an operator `WebAuthn` credential.
    pub const REGISTRATION_VERIFY: RouteDescriptor = credential_exchange(
        "operator.authentication.registration_verify",
        "/v1/operator/auth/registration/verify",
    );
    /// Start an operator `WebAuthn` login challenge.
    pub const LOGIN_OPTIONS: RouteDescriptor = credential_exchange(
        "operator.authentication.login_options",
        "/v1/operator/auth/login/options",
    );
    /// Verify an operator `WebAuthn` login challenge and issue a session.
    pub const LOGIN_VERIFY: RouteDescriptor = credential_exchange(
        "operator.authentication.login_verify",
        "/v1/operator/auth/login/verify",
    );

    /// Complete operator credential-exchange route family.
    pub const ROUTES: &[RouteDescriptor] = &[
        REGISTRATION_OPTIONS,
        REGISTRATION_VERIFY,
        LOGIN_OPTIONS,
        LOGIN_VERIFY,
    ];
}

/// Feature-gated governance VRF helper descriptors.
pub mod governance_vrf {
    use super::{ApiSurface, FeatureGate, HttpMethod, Listener, RouteDescriptor, RouteProjections};

    /// Derive deterministic governance-council VRF inputs.
    pub const DERIVE_COUNCIL: RouteDescriptor = RouteDescriptor::new(
        "governance.council.derive_vrf",
        HttpMethod::Post,
        "/v1/gov/council/derive-vrf",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("gov_vrf"))
    .with_projections(RouteProjections::ALL)
    .with_cors_options(true);

    /// Governance VRF routes registered when `gov_vrf` is compiled.
    pub const ROUTES: &[RouteDescriptor] = &[DERIVE_COUNCIL];
}

/// Core node information and operator configuration descriptors.
pub mod core {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteProjections,
    };

    /// Node API/build information.
    pub const API_VERSION: RouteDescriptor = RouteDescriptor::new(
        "core.api_version",
        HttpMethod::Get,
        "/v1/api/version",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::ALL)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Connected peers.
    pub const PEERS: RouteDescriptor = RouteDescriptor::new(
        "core.peers",
        HttpMethod::Get,
        "/v1/peers",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::ALL)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Orchestrator-compatible liveness probe.
    pub const HEALTH: RouteDescriptor = RouteDescriptor::new(
        "protocol.health",
        HttpMethod::Get,
        "/health",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::Unauthenticated)
    .with_projections(RouteProjections::ALL)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "orchestrator health-probe convention",
    })
    .with_implicit_head(true);
    /// Read the effective node configuration.
    pub const CONFIGURATION_GET: RouteDescriptor = RouteDescriptor::new(
        "operator.configuration.read",
        HttpMethod::Get,
        "/v1/configuration",
        ApiSurface::Operator,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::OperatorSignature)
    .with_projections(RouteProjections::OPENAPI)
    .with_implicit_head(true);
    /// Update mutable node configuration.
    pub const CONFIGURATION_POST: RouteDescriptor = RouteDescriptor::new(
        "operator.configuration.update",
        HttpMethod::Post,
        "/v1/configuration",
        ApiSurface::Operator,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::OperatorSignature)
    .with_projections(RouteProjections::OPENAPI);
    /// Read the current Nexus lane lifecycle commitment.
    pub const NEXUS_LIFECYCLE_GET: RouteDescriptor = RouteDescriptor::new(
        "nexus.lifecycle.read",
        HttpMethod::Get,
        "/v1/nexus/lifecycle",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read newest ledger headers.
    pub const LEDGER_HEADERS: RouteDescriptor = RouteDescriptor::new(
        "ledger.headers",
        HttpMethod::Get,
        "/v1/ledger/headers",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read a ledger execution-state root.
    pub const LEDGER_STATE_ROOT: RouteDescriptor = RouteDescriptor::new(
        "ledger.state_root",
        HttpMethod::Get,
        "/v1/ledger/state/{height}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read a ledger execution-state proof.
    pub const LEDGER_STATE_PROOF: RouteDescriptor = RouteDescriptor::new(
        "ledger.state_proof",
        HttpMethod::Get,
        "/v1/ledger/state-proof/{height}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read a transaction-entry proof from a block.
    pub const LEDGER_BLOCK_PROOF: RouteDescriptor = RouteDescriptor::new(
        "ledger.block_proof",
        HttpMethod::Get,
        "/v1/ledger/block/{height}/proof/{entry_hash}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Internal peer-to-peer Torii HTTP proxy.
    pub const INTERNAL_PROXY: RouteDescriptor = RouteDescriptor::new(
        "operator.internal_torii_proxy",
        HttpMethod::Post,
        "/v1/internal/torii/proxy",
        ApiSurface::Operator,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Any(&["p2p_ws", "connect"]))
    .with_authentication(AuthenticationPolicy::IdentityBoundSignature);
    /// Read the VPN client profile.
    pub const VPN_PROFILE: RouteDescriptor = RouteDescriptor::new(
        "vpn.profile",
        HttpMethod::Get,
        "/v1/vpn/profile",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Create a VPN price quote.
    pub const VPN_QUOTE_CREATE: RouteDescriptor = RouteDescriptor::new(
        "vpn.quote.create",
        HttpMethod::Post,
        "/v1/vpn/quotes",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Create a VPN session.
    pub const VPN_SESSION_CREATE: RouteDescriptor = RouteDescriptor::new(
        "vpn.session.create",
        HttpMethod::Post,
        "/v1/vpn/sessions",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// List VPN settlement receipts.
    pub const VPN_RECEIPTS: RouteDescriptor = RouteDescriptor::new(
        "vpn.receipt.list",
        HttpMethod::Get,
        "/v1/vpn/receipts",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Submit a VPN settlement receipt.
    pub const VPN_RECEIPT_SUBMIT: RouteDescriptor = RouteDescriptor::new(
        "vpn.receipt.submit",
        HttpMethod::Post,
        "/v1/vpn/receipts",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Read one VPN session.
    pub const VPN_SESSION: RouteDescriptor = RouteDescriptor::new(
        "vpn.session.read",
        HttpMethod::Get,
        "/v1/vpn/sessions/{session_id}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Delete one VPN session.
    pub const VPN_SESSION_DELETE: RouteDescriptor = RouteDescriptor::new(
        "vpn.session.delete",
        HttpMethod::Delete,
        "/v1/vpn/sessions/{session_id}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Read the node's wall-clock sample.
    pub const TIME_NOW: RouteDescriptor = RouteDescriptor::new(
        "time.now",
        HttpMethod::Get,
        "/v1/time/now",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read time synchronization status.
    pub const TIME_STATUS: RouteDescriptor = RouteDescriptor::new(
        "time.status",
        HttpMethod::Get,
        "/v1/time/status",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);

    /// Core information routes registered by `add_core_info_routes`.
    pub const INFO_ROUTES: &[RouteDescriptor] = &[
        API_VERSION,
        PEERS,
        HEALTH,
        CONFIGURATION_GET,
        CONFIGURATION_POST,
        NEXUS_LIFECYCLE_GET,
        LEDGER_HEADERS,
        LEDGER_STATE_ROOT,
        LEDGER_STATE_PROOF,
        LEDGER_BLOCK_PROOF,
        INTERNAL_PROXY,
        VPN_PROFILE,
        VPN_QUOTE_CREATE,
        VPN_SESSION_CREATE,
        VPN_RECEIPTS,
        VPN_RECEIPT_SUBMIT,
        VPN_SESSION,
        VPN_SESSION_DELETE,
    ];
    /// Time routes registered by `add_time_routes`.
    pub const TIME_ROUTES: &[RouteDescriptor] = &[TIME_NOW, TIME_STATUS];
}

/// Diagnostic and self-description protocol exceptions.
pub mod diagnostic {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteMatch, RouteProjections,
    };

    /// Root diagnostic status document.
    pub const STATUS: RouteDescriptor = RouteDescriptor::new(
        "diagnostic.status",
        HttpMethod::Get,
        "/status",
        ApiSurface::Diagnostic,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("telemetry"))
    .with_authentication(AuthenticationPolicy::Unauthenticated)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "established infrastructure status endpoint",
    })
    .with_implicit_head(true);
    /// Namespaced diagnostic status documents.
    pub const STATUS_TAIL: RouteDescriptor = RouteDescriptor::new(
        "diagnostic.status_namespace",
        HttpMethod::Get,
        "/status/{*tail}",
        ApiSurface::Diagnostic,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("telemetry"))
    .with_authentication(AuthenticationPolicy::Unauthenticated)
    .with_projections(RouteProjections::OPENAPI)
    .with_route_match(RouteMatch::Wildcard)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "status namespace is a reviewed diagnostic wildcard",
    })
    .with_implicit_head(true);
    /// Prometheus metrics exposition.
    pub const METRICS: RouteDescriptor = RouteDescriptor::new(
        "diagnostic.metrics",
        HttpMethod::Get,
        "/metrics",
        ApiSurface::Diagnostic,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("telemetry"))
    .with_authentication(AuthenticationPolicy::Unauthenticated)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "Prometheus exposition convention",
    })
    .with_implicit_head(true);
    /// CPU profiling capture.
    pub const PROFILE: RouteDescriptor = RouteDescriptor::new(
        "diagnostic.cpu_profile",
        HttpMethod::Get,
        "/debug/pprof/profile",
        ApiSurface::Diagnostic,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("profiling"))
    .with_authentication(AuthenticationPolicy::Unauthenticated)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "pprof tooling convention",
    })
    .with_implicit_head(true);
    /// Runtime schema document.
    pub const SCHEMA: RouteDescriptor = RouteDescriptor::new(
        "protocol.schema",
        HttpMethod::Get,
        "/v1/schema",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("schema"))
    .with_projections(RouteProjections::OPENAPI)
    .with_implicit_head(true);
    /// `OpenAPI` document at its media-typed filename.
    pub const OPENAPI_JSON: RouteDescriptor = RouteDescriptor::new(
        "protocol.openapi_json",
        HttpMethod::Get,
        "/openapi.json",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "OpenAPI document discovery convention",
    })
    .with_implicit_head(true);
    /// `OpenAPI` document convenience endpoint.
    pub const OPENAPI: RouteDescriptor = RouteDescriptor::new(
        "protocol.openapi",
        HttpMethod::Get,
        "/openapi",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "OpenAPI document discovery convention",
    })
    .with_implicit_head(true);

    /// Schema route registered by `add_schema_routes`.
    pub const SCHEMA_ROUTES: &[RouteDescriptor] = &[SCHEMA];
    /// `OpenAPI` routes registered by `add_openapi_routes`.
    pub const OPENAPI_ROUTES: &[RouteDescriptor] = &[OPENAPI_JSON, OPENAPI];
    /// Profiling route registered by `add_profiling_routes`.
    pub const PROFILE_ROUTES: &[RouteDescriptor] = &[PROFILE];

    /// Diagnostic and self-description routes registered by the builder.
    pub const ROUTES: &[RouteDescriptor] = &[
        STATUS,
        STATUS_TAIL,
        METRICS,
        PROFILE,
        SCHEMA,
        OPENAPI_JSON,
        OPENAPI,
    ];
}

/// Transaction, query, proof, and pipeline routes.
pub mod pipeline {
    use super::{ApiSurface, FeatureGate, HttpMethod, Listener, RouteDescriptor, RouteProjections};

    /// Submit one signed transaction.
    pub const TRANSACTION: RouteDescriptor = RouteDescriptor::new(
        "pipeline.transaction.submit",
        HttpMethod::Post,
        "/v1/pipeline/transactions",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::ALL)
    .with_cors_options(true);
    /// Submit a transaction entrypoint envelope.
    pub const TRANSACTION_ENTRYPOINT: RouteDescriptor = RouteDescriptor::new(
        "pipeline.transaction_entrypoint.submit",
        HttpMethod::Post,
        "/v1/pipeline/transaction-entrypoints",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Submit a batch of signed transactions.
    pub const TRANSACTIONS_BATCH: RouteDescriptor = RouteDescriptor::new(
        "pipeline.transaction_batch.submit",
        HttpMethod::Post,
        "/v1/pipeline/transactions/batch",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Execute a signed query.
    pub const QUERY: RouteDescriptor = RouteDescriptor::new(
        "query.execute",
        HttpMethod::Post,
        "/v1/query",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::ALL)
    .with_cors_options(true);
    /// Read one proof record.
    pub const PROOF: RouteDescriptor = RouteDescriptor::new(
        "proof.read",
        HttpMethod::Get,
        "/v1/proofs/{id}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read proof-retention state.
    pub const PROOF_RETENTION: RouteDescriptor = RouteDescriptor::new(
        "proof.retention",
        HttpMethod::Get,
        "/v1/proofs/retention",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read the status of a submitted pipeline transaction.
    pub const TRANSACTION_STATUS: RouteDescriptor = RouteDescriptor::new(
        "pipeline.transaction_status",
        HttpMethod::Get,
        "/v1/pipeline/transactions/status",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read pipeline admission readiness before submitting work.
    pub const PREFLIGHT: RouteDescriptor = RouteDescriptor::new(
        "pipeline.preflight",
        HttpMethod::Get,
        "/v1/pipeline/preflight",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// List trigger completion records.
    pub const TRIGGER_COMPLETIONS: RouteDescriptor = RouteDescriptor::new(
        "trigger.completion.list",
        HttpMethod::Get,
        "/v1/triggers/completed",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read recovery information for one block height.
    pub const RECOVERY: RouteDescriptor = RouteDescriptor::new(
        "pipeline.recovery",
        HttpMethod::Get,
        "/v1/pipeline/recovery/{height}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read `FastPQ` proofs associated with one recovery height.
    pub const RECOVERY_FASTPQ_PROOFS: RouteDescriptor = RouteDescriptor::new(
        "pipeline.recovery_fastpq_proofs",
        HttpMethod::Get,
        "/v1/pipeline/recovery/{height}/fastpq-proofs",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read the effective public policy document.
    pub const POLICY: RouteDescriptor = RouteDescriptor::new(
        "policy.read",
        HttpMethod::Get,
        "/v1/policy",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);

    /// Pipeline routes currently registered through the authoritative builder.
    pub const ROUTES: &[RouteDescriptor] = &[
        TRANSACTION,
        TRANSACTION_ENTRYPOINT,
        TRANSACTIONS_BATCH,
        QUERY,
        PROOF,
        PROOF_RETENTION,
        TRANSACTION_STATUS,
        PREFLIGHT,
        TRIGGER_COMPLETIONS,
        RECOVERY,
        RECOVERY_FASTPQ_PROOFS,
        POLICY,
    ];
}

/// ISO 20022 bridge submission, record, audit, and XML-view descriptors.
pub mod iso20022 {
    use super::{ApiSurface, HttpMethod, Listener, RouteDescriptor, RouteProjections};

    const fn public_post(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::ALL)
        .with_cors_options(true)
    }

    const fn public_get(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::ALL)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    /// Submit a pacs.008 customer-credit-transfer message.
    pub const PACS008_SUBMIT: RouteDescriptor =
        public_post("iso20022.pacs008.submit", "/v1/iso20022/pacs008");
    /// Submit a pacs.009 financial-institution-transfer message.
    pub const PACS009_SUBMIT: RouteDescriptor =
        public_post("iso20022.pacs009.submit", "/v1/iso20022/pacs009");
    /// Submit a pacs.002 lifecycle message.
    pub const PACS002_SUBMIT: RouteDescriptor =
        public_post("iso20022.pacs002.submit", "/v1/iso20022/pacs002");
    /// Submit a pacs.004 lifecycle message.
    pub const PACS004_SUBMIT: RouteDescriptor =
        public_post("iso20022.pacs004.submit", "/v1/iso20022/pacs004");
    /// Submit a camt.056 lifecycle message.
    pub const CAMT056_SUBMIT: RouteDescriptor =
        public_post("iso20022.camt056.submit", "/v1/iso20022/camt056");
    /// Submit a sese.023 settlement-instruction message.
    pub const SESE023_SUBMIT: RouteDescriptor =
        public_post("iso20022.sese023.submit", "/v1/iso20022/sese023");
    /// Submit a sese.024 settlement-status message.
    pub const SESE024_SUBMIT: RouteDescriptor =
        public_post("iso20022.sese024.submit", "/v1/iso20022/sese024");
    /// Submit a sese.025 settlement-confirmation message.
    pub const SESE025_SUBMIT: RouteDescriptor =
        public_post("iso20022.sese025.submit", "/v1/iso20022/sese025");
    /// Submit a colr.012 collateral-substitution message.
    pub const COLR012_SUBMIT: RouteDescriptor =
        public_post("iso20022.colr012.submit", "/v1/iso20022/colr012");
    /// Read the canonical rich record for one ISO 20022 message.
    pub const MESSAGE: RouteDescriptor =
        public_get("iso20022.message.read", "/v1/iso20022/messages/{msg_id}");
    /// Read the tamper-evident ISO 20022 audit manifest.
    pub const AUDIT_MESSAGES: RouteDescriptor =
        public_get("iso20022.audit.read", "/v1/iso20022/audit/messages");
    /// Render the current pacs.002 XML for one message.
    pub const MESSAGE_PACS002: RouteDescriptor = public_get(
        "iso20022.message.pacs002",
        "/v1/iso20022/messages/{msg_id}/pacs002",
    );
    /// Render the current pacs.004 XML for one message.
    pub const MESSAGE_PACS004: RouteDescriptor = public_get(
        "iso20022.message.pacs004",
        "/v1/iso20022/messages/{msg_id}/pacs004",
    );
    /// Render the current camt.029 XML for one message.
    pub const MESSAGE_CAMT029: RouteDescriptor = public_get(
        "iso20022.message.camt029",
        "/v1/iso20022/messages/{msg_id}/camt029",
    );
    /// Render the current sese.024 XML for one message.
    pub const MESSAGE_SESE024: RouteDescriptor = public_get(
        "iso20022.message.sese024",
        "/v1/iso20022/messages/{msg_id}/sese024",
    );
    /// Render the current sese.025 XML for one message.
    pub const MESSAGE_SESE025: RouteDescriptor = public_get(
        "iso20022.message.sese025",
        "/v1/iso20022/messages/{msg_id}/sese025",
    );

    /// Complete first-release ISO 20022 route family.
    pub const ROUTES: &[RouteDescriptor] = &[
        PACS008_SUBMIT,
        PACS009_SUBMIT,
        PACS002_SUBMIT,
        PACS004_SUBMIT,
        CAMT056_SUBMIT,
        SESE023_SUBMIT,
        SESE024_SUBMIT,
        SESE025_SUBMIT,
        COLR012_SUBMIT,
        MESSAGE,
        AUDIT_MESSAGES,
        MESSAGE_PACS002,
        MESSAGE_PACS004,
        MESSAGE_CAMT029,
        MESSAGE_SESE024,
        MESSAGE_SESE025,
    ];
}

/// Data-availability ingestion, proof-policy, commitment, and pin descriptors.
pub mod data_availability {
    use super::{ApiSurface, FeatureGate, HttpMethod, Listener, RouteDescriptor, RouteProjections};

    const fn public_post(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::ALL)
        .with_cors_options(true)
    }

    const fn public_get(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::ALL)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    /// Ingest a data-availability blob and routing manifest.
    pub const INGEST: RouteDescriptor = public_post("data_availability.ingest", "/v1/da/ingest")
        .with_feature_gate(FeatureGate::Feature("app_api"));
    /// Read the manifest addressed by a storage ticket.
    pub const MANIFEST: RouteDescriptor = public_get(
        "data_availability.manifest.read",
        "/v1/da/manifests/{ticket}",
    )
    .with_feature_gate(FeatureGate::Feature("app_api"));
    /// List supported data-availability proof policies.
    pub const PROOF_POLICIES: RouteDescriptor = public_get(
        "data_availability.proof_policy.list",
        "/v1/da/proof-policies",
    );
    /// Read the signed proof-policy snapshot.
    pub const PROOF_POLICY_SNAPSHOT: RouteDescriptor = public_get(
        "data_availability.proof_policy.snapshot",
        "/v1/da/proof-policies/snapshot",
    );
    /// List commitments selected by a typed filter request.
    pub const COMMITMENTS: RouteDescriptor =
        public_post("data_availability.commitment.list", "/v1/da/commitments");
    /// Produce a proof for a data-availability commitment.
    pub const COMMITMENTS_PROVE: RouteDescriptor = public_post(
        "data_availability.commitment.prove",
        "/v1/da/commitments/prove",
    );
    /// Verify a proof for a data-availability commitment.
    pub const COMMITMENTS_VERIFY: RouteDescriptor = public_post(
        "data_availability.commitment.verify",
        "/v1/da/commitments/verify",
    );
    /// List pin intents selected by a typed filter request.
    pub const PIN_INTENTS: RouteDescriptor =
        public_post("data_availability.pin_intent.list", "/v1/da/pin-intents");
    /// Produce a proof for a pin intent.
    pub const PIN_INTENTS_PROVE: RouteDescriptor = public_post(
        "data_availability.pin_intent.prove",
        "/v1/da/pin-intents/prove",
    );
    /// Verify a proof for a pin intent.
    pub const PIN_INTENTS_VERIFY: RouteDescriptor = public_post(
        "data_availability.pin_intent.verify",
        "/v1/da/pin-intents/verify",
    );

    /// Complete data-availability route family.
    pub const ROUTES: &[RouteDescriptor] = &[
        INGEST,
        MANIFEST,
        PROOF_POLICIES,
        PROOF_POLICY_SNAPSHOT,
        COMMITMENTS,
        COMMITMENTS_PROVE,
        COMMITMENTS_VERIFY,
        PIN_INTENTS,
        PIN_INTENTS_PROVE,
        PIN_INTENTS_VERIFY,
    ];
}

/// Musubi package-registry and unsigned-instruction builder descriptors.
pub mod musubi {
    use super::{ApiSurface, FeatureGate, HttpMethod, Listener, RouteDescriptor, RouteProjections};

    const fn app_get(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::ALL)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn app_post(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::ALL)
        .with_cors_options(true)
    }

    /// Search package records.
    pub const PACKAGES: RouteDescriptor = app_get("musubi.package.search", "/v1/musubi/packages");
    /// Read one package release selected by query parameters.
    pub const RELEASE: RouteDescriptor = app_get("musubi.release.read", "/v1/musubi/release");
    /// List releases for a package selected by query parameters.
    pub const RELEASES: RouteDescriptor = app_get("musubi.release.list", "/v1/musubi/releases");
    /// List versions for a package selected by query parameters.
    pub const VERSIONS: RouteDescriptor = app_get("musubi.version.list", "/v1/musubi/versions");
    /// Resolve a package alias.
    pub const ALIAS: RouteDescriptor =
        app_get("musubi.alias.resolve", "/v1/musubi/aliases/{alias}");
    /// Build an unsigned publish-release instruction.
    pub const PUBLISH_RELEASE: RouteDescriptor = app_post(
        "musubi.instruction.publish_release",
        "/v1/musubi/instructions/publish-release",
    );
    /// Build an unsigned yank-release instruction.
    pub const YANK_RELEASE: RouteDescriptor = app_post(
        "musubi.instruction.yank_release",
        "/v1/musubi/instructions/yank-release",
    );
    /// Build an unsigned set-alias instruction.
    pub const SET_ALIAS: RouteDescriptor = app_post(
        "musubi.instruction.set_alias",
        "/v1/musubi/instructions/set-alias",
    );
    /// Build an unsigned assert-release-exists instruction.
    pub const ASSERT_RELEASE_EXISTS: RouteDescriptor = app_post(
        "musubi.instruction.assert_release_exists",
        "/v1/musubi/instructions/assert-release-exists",
    );

    /// Complete Musubi route family registered when `app_api` is compiled.
    pub const ROUTES: &[RouteDescriptor] = &[
        PACKAGES,
        RELEASE,
        RELEASES,
        VERSIONS,
        ALIAS,
        PUBLISH_RELEASE,
        YANK_RELEASE,
        SET_ALIAS,
        ASSERT_RELEASE_EXISTS,
    ];
}

/// Protocol-native event and peer transports.
pub mod streaming {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteProjections,
    };

    /// Peer-to-peer WebSocket upgrade endpoint.
    pub const P2P: RouteDescriptor = RouteDescriptor::new(
        "protocol.p2p_websocket",
        HttpMethod::Get,
        "/p2p",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "peer transport handshake path",
    })
    .with_implicit_head(true);
    /// SSE event stream.
    pub const EVENTS_SSE: RouteDescriptor = RouteDescriptor::new(
        "events.stream_sse",
        HttpMethod::Get,
        "/v1/events/sse",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "SSE transport endpoint; stream contract is not an ordinary resource",
    })
    .with_implicit_head(true);
    /// Contract-event SSE stream.
    pub const CONTRACT_EVENTS_SSE: RouteDescriptor = RouteDescriptor::new(
        "contracts.events_stream_sse",
        HttpMethod::Get,
        "/v1/contracts/events/sse",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "SSE transport endpoint; stream contract is not an ordinary resource",
    })
    .with_implicit_head(true);
    /// Event subscription WebSocket.
    pub const SUBSCRIPTION_WS: RouteDescriptor = RouteDescriptor::new(
        "events.stream_websocket",
        HttpMethod::Get,
        "/v1/events/ws",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "WebSocket transport endpoint",
    })
    .with_implicit_head(true);
    /// Block stream WebSocket.
    pub const BLOCKS_WS: RouteDescriptor = RouteDescriptor::new(
        "blocks.stream_websocket",
        HttpMethod::Get,
        "/v1/blocks/stream",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "WebSocket transport endpoint",
    })
    .with_implicit_head(true);

    /// Application streaming routes registered when `app_api` is compiled.
    pub const APP_ROUTES: &[RouteDescriptor] =
        &[EVENTS_SSE, CONTRACT_EVENTS_SSE, SUBSCRIPTION_WS, BLOCKS_WS];
}

/// Native MCP transport routes.
pub mod mcp_transport {
    use super::{ApiSurface, HttpMethod, Listener, RouteDescriptor, RouteProjections};

    /// Read MCP server capabilities.
    pub const CAPABILITIES: RouteDescriptor = RouteDescriptor::new(
        "protocol.mcp.capabilities",
        HttpMethod::Get,
        "/v1/mcp",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Execute an MCP JSON-RPC request.
    pub const JSON_RPC: RouteDescriptor = RouteDescriptor::new(
        "protocol.mcp.json_rpc",
        HttpMethod::Post,
        "/v1/mcp",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_projections(RouteProjections::OPENAPI)
    .with_cors_options(true);

    /// Canonical native MCP route set.
    pub const ROUTES: &[RouteDescriptor] = &[CAPABILITIES, JSON_RPC];
}

/// Iroha Connect pairing and relay routes.
pub mod connect {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteProjections,
    };

    /// Create a wallet-pairing session.
    pub const SESSION_CREATE: RouteDescriptor = RouteDescriptor::new(
        "connect.session.create",
        HttpMethod::Post,
        "/v1/connect/session",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("connect"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Delete a wallet-pairing session using its management token.
    pub const SESSION_DELETE: RouteDescriptor = RouteDescriptor::new(
        "connect.session.delete",
        HttpMethod::Delete,
        "/v1/connect/session/{sid}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("connect"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Upgrade to the authenticated Connect relay WebSocket.
    pub const WEBSOCKET: RouteDescriptor = RouteDescriptor::new(
        "connect.websocket",
        HttpMethod::Get,
        "/v1/connect/ws",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("connect"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "Connect WebSocket transport endpoint",
    })
    .with_implicit_head(true);
    /// Read aggregate or management-token-authorized session status.
    pub const STATUS: RouteDescriptor = RouteDescriptor::new(
        "connect.status",
        HttpMethod::Get,
        "/v1/connect/status",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("connect"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);

    /// Canonical Connect route set.
    pub const ROUTES: &[RouteDescriptor] = &[SESSION_CREATE, SESSION_DELETE, WEBSOCKET, STATUS];
}

/// Telemetry-gated operator diagnostics, privacy ingestion, and asset-holder routes.
pub mod telemetry {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, RouteDescriptor,
        RouteProjections,
    };

    const fn telemetry_operator_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Operator,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::OperatorSignature)
        .with_feature_gate(FeatureGate::Feature("telemetry"))
        .with_projections(RouteProjections::ALL)
        .with_implicit_head(true)
    }

    const fn telemetry_public_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("telemetry"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_cors_options(true)
    }

    const fn app_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::ALL)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn app_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::ALL)
        .with_cors_options(true)
    }

    /// Read pacemaker status.
    pub const PACEMAKER: RouteDescriptor =
        telemetry_operator_get("operator.sumeragi.pacemaker", "/v1/sumeragi/pacemaker");
    /// Read consensus phase timings.
    pub const PHASES: RouteDescriptor =
        telemetry_operator_get("operator.sumeragi.phases", "/v1/sumeragi/phases");
    /// Read the atomic cross-transaction cache diagnostic.
    pub const DEBUG_AXT_CACHE: RouteDescriptor =
        telemetry_operator_get("operator.debug.axt_cache", "/v1/debug/axt/cache")
            .with_projections(RouteProjections::OPENAPI);
    /// Read the consensus witness diagnostic.
    pub const DEBUG_WITNESS: RouteDescriptor =
        telemetry_operator_get("operator.debug.witness", "/v1/debug/witness")
            .with_projections(RouteProjections::NONE);
    /// Ingest one `SoraNet` privacy observation.
    pub const SORANET_PRIVACY_EVENT: RouteDescriptor =
        telemetry_public_post("soranet.privacy_event.ingest", "/v1/soranet/privacy/event");
    /// Ingest one `SoraNet` privacy collector share.
    pub const SORANET_PRIVACY_SHARE: RouteDescriptor =
        telemetry_public_post("soranet.privacy_share.ingest", "/v1/soranet/privacy/share");
    /// List holders of one asset definition.
    pub const ASSET_HOLDERS: RouteDescriptor =
        app_get("asset.holder.list", "/v1/assets/{definition_id}/holders");
    /// Query holders of one asset definition with a typed request body.
    pub const ASSET_HOLDERS_QUERY: RouteDescriptor = app_post(
        "asset.holder.query",
        "/v1/assets/{definition_id}/holders/query",
    );

    /// Complete route family registered by `add_telemetry_routes`.
    pub const ROUTES: &[RouteDescriptor] = &[
        PACEMAKER,
        PHASES,
        DEBUG_AXT_CACHE,
        DEBUG_WITNESS,
        SORANET_PRIVACY_EVENT,
        SORANET_PRIVACY_SHARE,
        ASSET_HOLDERS,
        ASSET_HOLDERS_QUERY,
    ];
}

/// Consensus evidence, SCCP, VRF, finality, and Sumeragi introspection routes.
pub mod sumeragi {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteProjections,
    };

    const fn public_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::ALL)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn public_sccp_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        public_get(id, path).with_projections(RouteProjections::OPENAPI_AND_SDK)
    }

    const fn telemetry_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        public_get(id, path).with_feature_gate(FeatureGate::Feature("telemetry"))
    }

    const fn operator_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Operator,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::OperatorSignature)
        .with_projections(RouteProjections::ALL)
    }

    const fn telemetry_sse(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::ProtocolHandshake)
        .with_feature_gate(FeatureGate::Feature("telemetry"))
        .with_projections(RouteProjections::OPENAPI)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "Sumeragi SSE transport endpoint",
        })
        .with_implicit_head(true)
    }

    /// Count persisted consensus evidence records.
    pub const EVIDENCE_COUNT: RouteDescriptor =
        public_get("sumeragi.evidence.count", "/v1/sumeragi/evidence/count");
    /// List persisted consensus evidence records.
    pub const EVIDENCE_LIST: RouteDescriptor =
        public_get("sumeragi.evidence.list", "/v1/sumeragi/evidence");
    /// Read one generic SCCP message proof bundle.
    pub const SCCP_MESSAGE_PROOF: RouteDescriptor = public_sccp_get(
        "sccp.message_proof.read",
        "/v1/sccp/proofs/message/{message_id}",
    );
    /// Read one exact state-derived SCCP proof request.
    pub const SCCP_PROOF_REQUEST: RouteDescriptor = public_sccp_get(
        "sccp.proof_request.read",
        "/v1/sccp/proof-requests/{message_id}",
    );
    /// List recently committed SCCP messages.
    pub const SCCP_MESSAGES_RECENT: RouteDescriptor =
        public_sccp_get("sccp.message.list_recent", "/v1/sccp/messages/recent");
    /// Discover supported SCCP proof capabilities.
    pub const SCCP_CAPABILITIES: RouteDescriptor =
        public_sccp_get("sccp.capability.list", "/v1/sccp/capabilities");
    /// Read the authoritative SCCP route registry.
    pub const SCCP_REGISTRY: RouteDescriptor =
        public_sccp_get("sccp.registry.read", "/v1/sccp/registry");
    /// Read the exact registered SORA-side IVM material for one enabled route.
    pub const SCCP_SORA_OUTBOUND_MATERIAL: RouteDescriptor = public_sccp_get(
        "sccp.sora_outbound_material.read",
        "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
    );
    /// Read one epoch's VRF penalty state.
    pub const VRF_PENALTIES: RouteDescriptor = public_get(
        "sumeragi.vrf.penalty.read",
        "/v1/sumeragi/vrf/penalties/{epoch}",
    );
    /// Read one persisted VRF epoch snapshot.
    pub const VRF_EPOCH: RouteDescriptor =
        public_get("sumeragi.vrf.epoch.read", "/v1/sumeragi/vrf/epoch/{epoch}");

    /// Read the authoritative Sumeragi status snapshot.
    pub const STATUS: RouteDescriptor =
        telemetry_get("sumeragi.status.read", "/v1/sumeragi/status");
    /// Read non-authoritative Sumeragi operator and lane diagnostics.
    pub const DIAGNOSTICS: RouteDescriptor =
        telemetry_get("sumeragi.diagnostics.read", "/v1/sumeragi/diagnostics");
    /// Stream authoritative Sumeragi status snapshots over SSE.
    pub const STATUS_SSE: RouteDescriptor =
        telemetry_sse("sumeragi.status.stream_sse", "/v1/sumeragi/status/sse");
    /// Read the current leader snapshot.
    pub const LEADER: RouteDescriptor =
        telemetry_get("sumeragi.leader.read", "/v1/sumeragi/leader");
    /// Read the consensus BLS key roster.
    pub const BLS_KEYS: RouteDescriptor =
        telemetry_get("sumeragi.bls_key.list", "/v1/sumeragi/bls-keys");
    /// Read highest and locked quorum-certificate snapshots.
    pub const QC: RouteDescriptor = telemetry_get("sumeragi.qc.read", "/v1/sumeragi/qc");
    /// List validator-set checkpoints.
    pub const CHECKPOINTS: RouteDescriptor =
        telemetry_get("sumeragi.checkpoint.list", "/v1/sumeragi/checkpoints");
    /// List recent commit certificates.
    pub const COMMIT_CERTIFICATES: RouteDescriptor = telemetry_get(
        "sumeragi.commit_certificate.list",
        "/v1/sumeragi/commit-certificates",
    );
    /// Read a self-contained bridge finality proof.
    pub const BRIDGE_FINALITY: RouteDescriptor =
        public_get("bridge.finality_proof.read", "/v1/bridge/finality/{height}");
    /// Read a challenge-bound node-signed durable-tip finality attestation.
    pub const BRIDGE_FINALITY_ATTESTATION: RouteDescriptor = public_get(
        "bridge.finality_attestation.read",
        "/v1/bridge/finality/attestation/{height}",
    )
    .with_projections(RouteProjections::OPENAPI);
    /// Read a bridge finality commitment and justification bundle.
    pub const BRIDGE_FINALITY_BUNDLE: RouteDescriptor = public_get(
        "bridge.finality_bundle.read",
        "/v1/bridge/finality/bundle/{height}",
    );
    /// List retained Sumeragi validator sets.
    pub const VALIDATOR_SETS: RouteDescriptor =
        telemetry_get("sumeragi.validator_set.list", "/v1/sumeragi/validator-sets");
    /// Read the validator set active at one block height.
    pub const VALIDATOR_SET_BY_HEIGHT: RouteDescriptor = telemetry_get(
        "sumeragi.validator_set.read",
        "/v1/sumeragi/validator-sets/{height}",
    );
    /// List registered consensus keys.
    pub const CONSENSUS_KEYS: RouteDescriptor =
        telemetry_get("sumeragi.consensus_key.list", "/v1/sumeragi/consensus-keys");
    /// List consensus-key lifecycle records.
    pub const KEY_LIFECYCLE: RouteDescriptor =
        telemetry_get("sumeragi.key_lifecycle.list", "/v1/sumeragi/key-lifecycle");
    /// Read aggregated consensus telemetry.
    pub const TELEMETRY: RouteDescriptor =
        telemetry_get("sumeragi.telemetry.read", "/v1/sumeragi/telemetry");
    /// Read effective Sumeragi parameters.
    pub const PARAMETERS: RouteDescriptor =
        telemetry_get("sumeragi.parameter.read", "/v1/sumeragi/params");
    /// Read one commit quorum certificate by block hash.
    pub const COMMIT_QC: RouteDescriptor = telemetry_get(
        "sumeragi.commit_qc.read",
        "/v1/sumeragi/commit-qcs/{block_hash}",
    );

    /// Submit authenticated consensus evidence.
    pub const EVIDENCE_SUBMIT: RouteDescriptor =
        operator_post("operator.sumeragi.evidence.submit", "/v1/sumeragi/evidence");
    /// Submit an authenticated VRF commitment.
    pub const VRF_COMMIT: RouteDescriptor =
        operator_post("operator.sumeragi.vrf.commit", "/v1/sumeragi/vrf/commit");
    /// Submit an authenticated VRF reveal.
    pub const VRF_REVEAL: RouteDescriptor =
        operator_post("operator.sumeragi.vrf.reveal", "/v1/sumeragi/vrf/reveal");

    /// Complete route family registered by `add_sumeragi_routes`.
    pub const ROUTES: &[RouteDescriptor] = &[
        EVIDENCE_COUNT,
        EVIDENCE_LIST,
        SCCP_MESSAGE_PROOF,
        SCCP_PROOF_REQUEST,
        SCCP_MESSAGES_RECENT,
        SCCP_CAPABILITIES,
        SCCP_REGISTRY,
        SCCP_SORA_OUTBOUND_MATERIAL,
        VRF_PENALTIES,
        VRF_EPOCH,
        STATUS,
        DIAGNOSTICS,
        STATUS_SSE,
        LEADER,
        BLS_KEYS,
        QC,
        CHECKPOINTS,
        COMMIT_CERTIFICATES,
        BRIDGE_FINALITY,
        BRIDGE_FINALITY_ATTESTATION,
        BRIDGE_FINALITY_BUNDLE,
        VALIDATOR_SETS,
        VALIDATOR_SET_BY_HEIGHT,
        CONSENSUS_KEYS,
        KEY_LIFECYCLE,
        TELEMETRY,
        PARAMETERS,
        COMMIT_QC,
        EVIDENCE_SUBMIT,
        VRF_COMMIT,
        VRF_REVEAL,
    ];
}

/// Runtime, zero-knowledge, node-projection, and governance routes.
pub mod runtime_governance {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteProjections,
    };

    const fn public_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn public_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_cors_options(true)
    }

    const fn app_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        public_get(id, path).with_feature_gate(FeatureGate::Feature("app_api"))
    }

    const fn app_signed_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_get(id, path).with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
    }

    const fn app_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        public_post(id, path).with_feature_gate(FeatureGate::Feature("app_api"))
    }

    const fn operator_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Operator,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::OperatorSignature)
        .with_projections(RouteProjections::OPENAPI)
        .with_implicit_head(true)
    }

    const fn operator_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Operator,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::OperatorSignature)
        .with_projections(RouteProjections::OPENAPI)
    }

    const fn app_operator_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        operator_get(id, path).with_feature_gate(FeatureGate::Feature("app_api"))
    }

    const fn app_operator_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        operator_post(id, path).with_feature_gate(FeatureGate::Feature("app_api"))
    }

    /// Read registered zero-knowledge roots.
    pub const ZK_ROOTS: RouteDescriptor = public_post("zk.roots.read", "/v1/zk/roots");
    /// Build a zero-knowledge Merkle path.
    pub const ZK_MERKLE_PATH: RouteDescriptor =
        public_post("zk.merkle_path.build", "/v1/zk/merkle-path");
    /// Verify one zero-knowledge proof.
    pub const ZK_VERIFY: RouteDescriptor = public_post("zk.proof.verify", "/v1/zk/verify");
    /// Submit one zero-knowledge proof record.
    pub const ZK_SUBMIT_PROOF: RouteDescriptor =
        public_post("zk.proof.submit", "/v1/zk/submit-proof");
    /// Read a zero-knowledge vote tally.
    pub const ZK_VOTE_TALLY: RouteDescriptor = public_post("zk.vote.tally", "/v1/zk/vote/tally");
    /// Derive an IVM zero-knowledge executable.
    pub const ZK_IVM_DERIVE: RouteDescriptor = app_post("zk.ivm.derive", "/v1/zk/ivm/derive");
    /// Start an IVM zero-knowledge proving job.
    pub const ZK_IVM_PROVE: RouteDescriptor = app_post("zk.ivm.prove", "/v1/zk/ivm/prove");
    /// Read an IVM zero-knowledge proving job.
    pub const ZK_IVM_PROVE_GET: RouteDescriptor =
        app_get("zk.ivm.prove_job.read", "/v1/zk/ivm/prove/{job_id}");
    /// Cancel and delete an IVM zero-knowledge proving job.
    pub const ZK_IVM_PROVE_DELETE: RouteDescriptor = RouteDescriptor::new(
        "zk.ivm.prove_job.delete",
        HttpMethod::Delete,
        "/v1/zk/ivm/prove/{job_id}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Verify a bounded batch of zero-knowledge proofs.
    pub const ZK_VERIFY_BATCH: RouteDescriptor =
        public_post("zk.proof.verify_batch", "/v1/zk/verify-batch")
            .with_feature_gate(FeatureGate::Feature("zk-verify-batch"));
    /// List filtered zero-knowledge attachments.
    pub const ZK_ATTACHMENTS_GET: RouteDescriptor =
        app_get("zk.attachment.list", "/v1/zk/attachments");
    /// Create a zero-knowledge attachment.
    pub const ZK_ATTACHMENTS_POST: RouteDescriptor =
        app_post("zk.attachment.create", "/v1/zk/attachments");
    /// Read one zero-knowledge attachment.
    pub const ZK_ATTACHMENT_GET: RouteDescriptor =
        app_get("zk.attachment.read", "/v1/zk/attachments/{id}");
    /// Delete one zero-knowledge attachment.
    pub const ZK_ATTACHMENT_DELETE: RouteDescriptor = RouteDescriptor::new(
        "zk.attachment.delete",
        HttpMethod::Delete,
        "/v1/zk/attachments/{id}",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_cors_options(true);
    /// Count filtered zero-knowledge attachments.
    pub const ZK_ATTACHMENTS_COUNT: RouteDescriptor =
        app_get("zk.attachment.count", "/v1/zk/attachments/count");

    /// Read the active runtime ABI version.
    pub const RUNTIME_ABI_ACTIVE: RouteDescriptor =
        public_get("runtime.abi.active", "/v1/runtime/abi/active");
    /// Read the active runtime ABI hash.
    pub const RUNTIME_ABI_HASH: RouteDescriptor =
        public_get("runtime.abi.hash", "/v1/runtime/abi/hash");
    /// Read bounded runtime metrics.
    pub const RUNTIME_METRICS: RouteDescriptor =
        public_get("runtime.metrics", "/v1/runtime/metrics");
    /// Read node capability metadata.
    pub const NODE_CAPABILITIES: RouteDescriptor =
        public_get("node.capabilities", "/v1/node/capabilities");
    /// Read the latest query-projection checkpoint.
    pub const NODE_PROJECTION_CHECKPOINT: RouteDescriptor = public_get(
        "node.query_projection.checkpoint",
        "/v1/node/query/projection/checkpoint",
    );
    /// Validate a query-projection checkpoint plan.
    pub const NODE_PROJECTION_CHECKPOINT_PLAN: RouteDescriptor = app_operator_post(
        "operator.node.query_projection.checkpoint_plan",
        "/v1/node/query/projection/checkpoint/plan",
    );
    /// Publish a rebuilt query-projection checkpoint.
    pub const NODE_PROJECTION_CHECKPOINT_PUBLISH: RouteDescriptor = app_operator_post(
        "operator.node.query_projection.checkpoint_publish",
        "/v1/node/query/projection/checkpoint/publish",
    );
    /// List canonical query-projection shards.
    pub const NODE_PROJECTION_SHARD_CATALOG: RouteDescriptor = app_operator_get(
        "operator.node.query_projection.shard_catalog",
        "/v1/node/query/projection/catalog/{resource}",
    );
    /// Export one canonical query-projection shard archive.
    pub const NODE_PROJECTION_SHARD_EXPORT: RouteDescriptor = app_operator_get(
        "operator.node.query_projection.shard_export",
        "/v1/node/query/projection/shards/{resource}/{partition_id}",
    );
    /// List staged runtime upgrades.
    pub const RUNTIME_UPGRADES: RouteDescriptor =
        operator_get("operator.runtime.upgrade.list", "/v1/runtime/upgrades");
    /// Propose a runtime upgrade.
    pub const RUNTIME_UPGRADE_PROPOSE: RouteDescriptor = operator_post(
        "operator.runtime.upgrade.propose",
        "/v1/runtime/upgrades/propose",
    );
    /// Activate a runtime upgrade.
    pub const RUNTIME_UPGRADE_ACTIVATE: RouteDescriptor = operator_post(
        "operator.runtime.upgrade.activate",
        "/v1/runtime/upgrades/activate/{id}",
    );
    /// Cancel a runtime upgrade.
    pub const RUNTIME_UPGRADE_CANCEL: RouteDescriptor = operator_post(
        "operator.runtime.upgrade.cancel",
        "/v1/runtime/upgrades/cancel/{id}",
    );

    /// Draft a ministry agenda proposal for local signing.
    pub const MINISTRY_AGENDA_DRAFT: RouteDescriptor = app_post(
        "ministry.agenda_proposal.draft",
        "/v1/ministry/agenda/proposals/draft",
    );
    /// Read a submitted ministry agenda proposal.
    pub const MINISTRY_AGENDA_GET: RouteDescriptor = app_get(
        "ministry.agenda_proposal.read",
        "/v1/ministry/agenda/proposals/{proposal_id}",
    );
    /// Draft a contract-deployment proposal.
    pub const GOV_PROPOSE_DEPLOY: RouteDescriptor = app_post(
        "governance.proposal.deploy_contract",
        "/v1/gov/proposals/deploy-contract",
    );
    /// Draft an SCCP route-governance proposal.
    pub const GOV_PROPOSE_SCCP: RouteDescriptor = app_post(
        "governance.proposal.sccp_route_governance",
        "/v1/gov/proposals/sccp-route-governance",
    );
    /// Read strict public governance readiness and policy capabilities.
    pub const GOV_CAPABILITIES: RouteDescriptor =
        public_get("governance.capabilities.read", "/v1/gov/capabilities");
    /// Draft the exact configured citizenship registration instruction.
    pub const GOV_CITIZEN_DRAFT: RouteDescriptor =
        app_post("governance.citizen.draft", "/v1/gov/citizens/draft");
    /// Finality-bound current validation-fee policy proof path.
    pub const VALIDATION_FEE_CURRENT_POLICY_PROOF_PATH: &str =
        "/v1/validation-fee/policy/current/proof";
    /// Typed validation-fee proposal list path.
    pub const VALIDATION_FEE_PROPOSALS_PATH: &str = "/v1/validation-fee/proposals";
    /// Typed validation-fee proposal detail path.
    pub const VALIDATION_FEE_PROPOSAL_DETAIL_PATH: &str =
        "/v1/validation-fee/proposals/{proposal_id}";
    /// Strict native validation-fee proposal draft path.
    pub const VALIDATION_FEE_PROPOSAL_DRAFT_PATH: &str = "/v1/validation-fee/proposals/draft";
    /// Fetch a finality-bound current validation-fee registry.
    pub const VALIDATION_FEE_CURRENT_POLICY_PROOF: RouteDescriptor = app_post(
        "validation_fee.policy.current_proof",
        VALIDATION_FEE_CURRENT_POLICY_PROOF_PATH,
    );
    /// List typed validation-fee Parliament proposals.
    pub const VALIDATION_FEE_PROPOSALS: RouteDescriptor = app_get(
        "validation_fee.proposal.list",
        VALIDATION_FEE_PROPOSALS_PATH,
    );
    /// Read one typed validation-fee Parliament proposal.
    pub const VALIDATION_FEE_PROPOSAL_DETAIL: RouteDescriptor = app_get(
        "validation_fee.proposal.read",
        VALIDATION_FEE_PROPOSAL_DETAIL_PATH,
    );
    /// Draft one strict native validation-fee Parliament proposal.
    pub const VALIDATION_FEE_PROPOSAL_DRAFT: RouteDescriptor = app_post(
        "validation_fee.proposal.draft",
        VALIDATION_FEE_PROPOSAL_DRAFT_PATH,
    );
    /// Read one governance proposal.
    pub const GOV_PROPOSAL_GET: RouteDescriptor =
        app_get("governance.proposal.read", "/v1/gov/proposals/{id}");
    /// Read token locks for one referendum.
    pub const GOV_LOCKS_GET: RouteDescriptor =
        app_get("governance.lock.list", "/v1/gov/locks/{rid}");
    /// Read one referendum.
    pub const GOV_REFERENDUM_GET: RouteDescriptor =
        app_get("governance.referendum.read", "/v1/gov/referenda/{id}");
    /// Read a referendum tally snapshot.
    pub const GOV_TALLY_GET: RouteDescriptor =
        app_get("governance.tally.read", "/v1/gov/tally/{id}");
    /// Submit a zero-knowledge governance ballot.
    pub const GOV_BALLOT_ZK: RouteDescriptor =
        app_post("governance.ballot.zk", "/v1/gov/ballots/zk");
    /// Submit a version-one zero-knowledge governance ballot.
    pub const GOV_BALLOT_ZK_V1: RouteDescriptor =
        app_post("governance.ballot.zk_v1", "/v1/gov/ballots/zk-v1");
    /// Build a version-one zero-knowledge ballot proof.
    pub const GOV_BALLOT_ZK_V1_PROOF: RouteDescriptor = app_post(
        "governance.ballot.zk_v1_proof",
        "/v1/gov/ballots/zk-v1/ballot-proof",
    );
    /// Submit a plain governance ballot.
    pub const GOV_BALLOT_PLAIN: RouteDescriptor =
        app_post("governance.ballot.plain", "/v1/gov/ballots/plain");
    /// Draft a parliament ballot.
    pub const GOV_PARLIAMENT_BALLOT: RouteDescriptor =
        app_post("governance.parliament.ballot", "/v1/gov/parliament/ballots");
    /// Finalize a referendum.
    pub const GOV_FINALIZE: RouteDescriptor =
        app_post("governance.referendum.finalize", "/v1/gov/finalize");
    /// Replace the protected namespace set.
    pub const GOV_PROTECTED_POST: RouteDescriptor = app_operator_post(
        "operator.governance.protected_namespaces.update",
        "/v1/gov/protected-namespaces",
    )
    .with_projections(RouteProjections::OPENAPI.union(RouteProjections::MCP));
    /// Read the protected namespace set.
    pub const GOV_PROTECTED_GET: RouteDescriptor = app_get(
        "governance.protected_namespaces.read",
        "/v1/gov/protected-namespaces",
    );
    /// Stream governance events over SSE.
    pub const GOV_STREAM: RouteDescriptor = RouteDescriptor::new(
        "governance.events.stream_sse",
        HttpMethod::Get,
        "/v1/gov/stream",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_projections(RouteProjections::OPENAPI)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "governance SSE transport endpoint",
    })
    .with_implicit_head(true);
    /// Read governance unlock statistics.
    pub const GOV_UNLOCK_STATS: RouteDescriptor =
        app_get("governance.unlock.stats", "/v1/gov/unlocks/stats");
    /// Read an active governance contract binding.
    pub const GOV_CONTRACT_GET: RouteDescriptor = app_signed_get(
        "governance.contract.read",
        "/v1/gov/contracts/{contract_address}",
    );
    /// Draft enactment of an approved referendum.
    pub const GOV_ENACT: RouteDescriptor = app_post("governance.referendum.enact", "/v1/gov/enact");
    /// Read the current sortition council.
    pub const GOV_COUNCIL_CURRENT: RouteDescriptor =
        app_get("governance.council.current", "/v1/gov/council/current");
    /// Read the exact citizenship registry count.
    pub const GOV_CITIZENS_COUNT: RouteDescriptor =
        app_get("governance.citizen.count", "/v1/gov/citizens");
    /// Read citizenship status for one account.
    pub const GOV_CITIZEN_STATUS: RouteDescriptor =
        app_get("governance.citizen.status", "/v1/gov/citizens/{account_id}");
    /// Read council derivation audit metadata.
    pub const GOV_COUNCIL_AUDIT: RouteDescriptor =
        app_get("governance.council.audit", "/v1/gov/council/audit");
    /// Persist a VRF-derived council.
    pub const GOV_COUNCIL_PERSIST: RouteDescriptor =
        app_post("governance.council.persist", "/v1/gov/council/persist")
            .with_feature_gate(FeatureGate::All(&["app_api", "gov_vrf"]));
    /// Replace a council member with the next alternate.
    pub const GOV_COUNCIL_REPLACE: RouteDescriptor =
        app_post("governance.council.replace", "/v1/gov/council/replace")
            .with_feature_gate(FeatureGate::All(&["app_api", "gov_vrf"]));

    /// Complete route family registered by `add_runtime_governance_routes`.
    pub const ROUTES: &[RouteDescriptor] = &[
        ZK_ROOTS,
        ZK_MERKLE_PATH,
        ZK_VERIFY,
        ZK_SUBMIT_PROOF,
        ZK_VOTE_TALLY,
        ZK_IVM_DERIVE,
        ZK_IVM_PROVE,
        ZK_IVM_PROVE_GET,
        ZK_IVM_PROVE_DELETE,
        ZK_VERIFY_BATCH,
        ZK_ATTACHMENTS_GET,
        ZK_ATTACHMENTS_POST,
        ZK_ATTACHMENT_GET,
        ZK_ATTACHMENT_DELETE,
        ZK_ATTACHMENTS_COUNT,
        RUNTIME_ABI_ACTIVE,
        RUNTIME_ABI_HASH,
        RUNTIME_METRICS,
        NODE_CAPABILITIES,
        NODE_PROJECTION_CHECKPOINT,
        NODE_PROJECTION_CHECKPOINT_PLAN,
        NODE_PROJECTION_CHECKPOINT_PUBLISH,
        NODE_PROJECTION_SHARD_CATALOG,
        NODE_PROJECTION_SHARD_EXPORT,
        RUNTIME_UPGRADES,
        RUNTIME_UPGRADE_PROPOSE,
        RUNTIME_UPGRADE_ACTIVATE,
        RUNTIME_UPGRADE_CANCEL,
        MINISTRY_AGENDA_DRAFT,
        MINISTRY_AGENDA_GET,
        GOV_PROPOSE_DEPLOY,
        GOV_PROPOSE_SCCP,
        GOV_CAPABILITIES,
        GOV_CITIZEN_DRAFT,
        VALIDATION_FEE_CURRENT_POLICY_PROOF,
        VALIDATION_FEE_PROPOSALS,
        VALIDATION_FEE_PROPOSAL_DETAIL,
        VALIDATION_FEE_PROPOSAL_DRAFT,
        GOV_PROPOSAL_GET,
        GOV_LOCKS_GET,
        GOV_REFERENDUM_GET,
        GOV_TALLY_GET,
        GOV_BALLOT_ZK,
        GOV_BALLOT_ZK_V1,
        GOV_BALLOT_ZK_V1_PROOF,
        GOV_BALLOT_PLAIN,
        GOV_PARLIAMENT_BALLOT,
        GOV_FINALIZE,
        GOV_PROTECTED_POST,
        GOV_PROTECTED_GET,
        GOV_STREAM,
        GOV_UNLOCK_STATS,
        GOV_CONTRACT_GET,
        GOV_ENACT,
        GOV_COUNCIL_CURRENT,
        GOV_CITIZENS_COUNT,
        GOV_CITIZEN_STATUS,
        GOV_COUNCIL_AUDIT,
        GOV_COUNCIL_PERSIST,
        GOV_COUNCIL_REPLACE,
    ];
}

/// `SoraFS` discovery, storage, transparency, reputation, and gateway routes.
pub mod sorafs {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteMatch, RouteProjections,
    };

    const fn public_get(
        stable_route_id: &'static str,
        path: &'static str,
        projections: RouteProjections,
    ) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(projections)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn public_post(
        stable_route_id: &'static str,
        path: &'static str,
        projections: RouteProjections,
    ) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(projections)
        .with_cors_options(true)
    }

    const fn documented_get(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        public_get(stable_route_id, path, RouteProjections::OPENAPI_AND_SDK)
    }

    const fn documented_post(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        public_post(stable_route_id, path, RouteProjections::OPENAPI_AND_SDK)
    }

    const fn local_get(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        public_get(stable_route_id, path, RouteProjections::NONE)
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
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "vendor-neutral HTTP Routing V1 interoperability endpoint",
        })
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn stream_get(stable_route_id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Get,
            path,
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_authentication(AuthenticationPolicy::ProtocolHandshake)
        .with_projections(RouteProjections::OPENAPI)
        .with_implicit_head(true)
    }

    const fn protocol_get(
        stable_route_id: &'static str,
        path: &'static str,
        route_match: RouteMatch,
        reason: &'static str,
    ) -> RouteDescriptor {
        RouteDescriptor::new(
            stable_route_id,
            HttpMethod::Get,
            path,
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_authentication(AuthenticationPolicy::ProtocolHandshake)
        .with_route_match(route_match)
        .with_path_policy(PathPolicy::ProtocolException { reason })
        .with_implicit_head(true)
    }

    /// Read configured `SoraFS` publication peers.
    pub const STORAGE_PEERS: RouteDescriptor =
        documented_get("sorafs.storage_peer.list", "/v1/sorafs/storage/peers");
    /// List admitted `SoraFS` provider advertisements.
    pub const PROVIDERS: RouteDescriptor =
        documented_get("sorafs.provider.list", "/v1/sorafs/providers");
    /// Submit a `SoraFS` provider advertisement.
    pub const PROVIDER_ADVERT: RouteDescriptor = documented_post(
        "sorafs.provider_advert.submit",
        "/v1/sorafs/providers/advert",
    );
    /// Resolve admitted providers assigned to one approved content root.
    pub const ROUTING_PROVIDERS: RouteDescriptor = delegated_routing_get(
        "sorafs.delegated_routing.providers",
        "/routing/v1/providers/{cid}",
    );
    /// Resolve one admitted and actively assigned provider peer.
    pub const ROUTING_PEERS: RouteDescriptor = delegated_routing_get(
        "sorafs.delegated_routing.peers",
        "/routing/v1/peers/{peer_id}",
    );
    /// Read the local `SoraFS` capacity state.
    pub const CAPACITY_STATE: RouteDescriptor =
        documented_get("sorafs.capacity.read", "/v1/sorafs/capacity/state");

    /// Read the local Governance DAG dashboard.
    pub const GOVERNANCE_DAG_DASHBOARD: RouteDescriptor = local_get(
        "sorafs.governance_dag.dashboard",
        "/v1/sorafs/governance/dag/dashboard",
    );
    /// Read the local Governance DAG head.
    pub const GOVERNANCE_DAG_HEAD: RouteDescriptor = local_get(
        "sorafs.governance_dag.head",
        "/v1/sorafs/governance/dag/head",
    );
    /// Read one local Governance DAG block.
    pub const GOVERNANCE_DAG_BLOCK: RouteDescriptor = local_get(
        "sorafs.governance_dag.block",
        "/v1/sorafs/governance/dag/blocks/{block_cid_hex}",
    );
    /// Read one local Governance DAG node.
    pub const GOVERNANCE_DAG_NODE: RouteDescriptor = local_get(
        "sorafs.governance_dag.node",
        "/v1/sorafs/governance/dag/nodes/{node_cid_hex}",
    );
    /// Read the local Governance DAG publication index.
    pub const GOVERNANCE_DAG_PUBLISH_INDEX: RouteDescriptor = local_get(
        "sorafs.governance_dag.publish_index",
        "/v1/sorafs/governance/dag/publish-index",
    );
    /// Read a publication-index entry by digest.
    pub const GOVERNANCE_DAG_PUBLISH_DIGEST: RouteDescriptor = local_get(
        "sorafs.governance_dag.publish_digest",
        "/v1/sorafs/governance/dag/publish-index/digests/{encoded_blake3_hex}",
    );
    /// Read publication-index entries by payload kind.
    pub const GOVERNANCE_DAG_PUBLISH_KIND: RouteDescriptor = local_get(
        "sorafs.governance_dag.publish_kind",
        "/v1/sorafs/governance/dag/publish-index/kinds/{payload_kind}",
    );

    /// List published transparency cycles.
    pub const TRANSPARENCY_CYCLES: RouteDescriptor = documented_get(
        "sorafs.transparency_cycle.list",
        "/v1/sorafs/transparency/cycles",
    );
    /// Read one published transparency cycle.
    pub const TRANSPARENCY_CYCLE: RouteDescriptor = documented_get(
        "sorafs.transparency_cycle.read",
        "/v1/sorafs/transparency/cycles/{cycle_id_hex}",
    );
    /// Read one transparency-cycle entry proof.
    pub const TRANSPARENCY_CYCLE_ENTRY: RouteDescriptor = documented_get(
        "sorafs.transparency_cycle_entry.read",
        "/v1/sorafs/transparency/cycles/{cycle_id_hex}/entries/{entry_id_hex}",
    );
    /// Read a transparency explorer snapshot.
    pub const TRANSPARENCY_EXPLORER: RouteDescriptor = documented_get(
        "sorafs.transparency_explorer.read",
        "/v1/sorafs/transparency/explorer",
    );
    /// Read the browser-native transparency explorer UI.
    pub const TRANSPARENCY_EXPLORER_UI: RouteDescriptor = public_get(
        "sorafs.transparency_explorer.ui",
        "/v1/sorafs/transparency/explorer/ui",
        RouteProjections::OPENAPI,
    );
    /// Submit a typed transparency source entry.
    pub const TRANSPARENCY_SOURCE_ENTRY: RouteDescriptor = documented_post(
        "sorafs.transparency_source_entry.submit",
        "/v1/sorafs/transparency/source-entries/{source_kind}",
    );
    /// Submit a privacy-aggregate source event.
    pub const TRANSPARENCY_PRIVACY_SOURCE_EVENT: RouteDescriptor = documented_post(
        "sorafs.transparency_privacy_aggregate.source_event",
        "/v1/sorafs/transparency/privacy-aggregates/source-events",
    );
    /// Publish the oldest due privacy-aggregate cycle.
    pub const TRANSPARENCY_PRIVACY_PUBLISH_DUE: RouteDescriptor = documented_post(
        "sorafs.transparency_privacy_aggregate.publish_due",
        "/v1/sorafs/transparency/privacy-aggregates/publish-due",
    );
    /// List published proof-token issuances.
    pub const TRANSPARENCY_TOKENS: RouteDescriptor = documented_get(
        "sorafs.transparency_token.list",
        "/v1/sorafs/transparency/tokens",
    );
    /// Submit a proof-token issuance.
    pub const TRANSPARENCY_TOKEN_ISSUANCE: RouteDescriptor = documented_post(
        "sorafs.transparency_token.issue",
        "/v1/sorafs/transparency/tokens/issuances",
    );
    /// Verify a `SoraFS` proof token.
    pub const TRANSPARENCY_TOKEN_VERIFY: RouteDescriptor = documented_post(
        "sorafs.transparency_token.verify",
        "/v1/sorafs/transparency/tokens/verify",
    );

    /// List local appeal-finance reports.
    pub const APPEAL_FINANCE_REPORTS_GET: RouteDescriptor = documented_get(
        "sorafs.appeal_finance_report.list",
        "/v1/sorafs/appeals/finance/reports",
    );
    /// Publish an appeal-finance report.
    pub const APPEAL_FINANCE_REPORTS_POST: RouteDescriptor = documented_post(
        "sorafs.appeal_finance_report.publish",
        "/v1/sorafs/appeals/finance/reports",
    );
    /// List local appeal-finance weekly rollups.
    pub const APPEAL_FINANCE_WEEKLY_ROLLUPS_GET: RouteDescriptor = documented_get(
        "sorafs.appeal_finance_weekly_rollup.list",
        "/v1/sorafs/appeals/finance/weekly-rollups",
    );
    /// Publish an appeal-finance weekly rollup.
    pub const APPEAL_FINANCE_WEEKLY_ROLLUPS_POST: RouteDescriptor = documented_post(
        "sorafs.appeal_finance_weekly_rollup.publish",
        "/v1/sorafs/appeals/finance/weekly-rollups",
    );
    /// List local appeal-finance settlement receipts.
    pub const APPEAL_FINANCE_SETTLEMENT_RECEIPTS: RouteDescriptor = documented_get(
        "sorafs.appeal_finance_settlement_receipt.list",
        "/v1/sorafs/appeals/finance/settlement-receipts",
    );

    /// Read the Governance DAG CAR-publication queue.
    pub const GOVERNANCE_DAG_CAR_QUEUE: RouteDescriptor = local_get(
        "sorafs.governance_dag.car_queue",
        "/v1/sorafs/governance/dag/car-queue",
    );
    /// Read a queued CAR publication by digest.
    pub const GOVERNANCE_DAG_CAR_QUEUE_DIGEST: RouteDescriptor = local_get(
        "sorafs.governance_dag.car_queue_digest",
        "/v1/sorafs/governance/dag/car-queue/digests/{encoded_blake3_hex}",
    );
    /// Read queued CAR publications by payload kind.
    pub const GOVERNANCE_DAG_CAR_QUEUE_KIND: RouteDescriptor = local_get(
        "sorafs.governance_dag.car_queue_kind",
        "/v1/sorafs/governance/dag/car-queue/kinds/{payload_kind}",
    );
    /// Read a queued Governance DAG CAR archive.
    pub const GOVERNANCE_DAG_CAR_QUEUE_ARCHIVE: RouteDescriptor = local_get(
        "sorafs.governance_dag.car_queue_archive",
        "/v1/sorafs/governance/dag/car-queue/archives/{car_archive_blake3_hex}",
    );
    /// Read the local Governance DAG runtime snapshot.
    pub const GOVERNANCE_DAG_RUNTIME: RouteDescriptor = local_get(
        "sorafs.governance_dag.runtime",
        "/v1/sorafs/governance/dag/runtime",
    );
    /// Read the local Governance DAG runtime head.
    pub const GOVERNANCE_DAG_RUNTIME_HEAD: RouteDescriptor = local_get(
        "sorafs.governance_dag.runtime_head",
        "/v1/sorafs/governance/dag/runtime/head",
    );
    /// Read one Governance DAG runtime block.
    pub const GOVERNANCE_DAG_RUNTIME_BLOCK: RouteDescriptor = local_get(
        "sorafs.governance_dag.runtime_block",
        "/v1/sorafs/governance/dag/runtime/blocks/{block_cid_hex}",
    );
    /// Read one Governance DAG runtime node.
    pub const GOVERNANCE_DAG_RUNTIME_NODE: RouteDescriptor = local_get(
        "sorafs.governance_dag.runtime_node",
        "/v1/sorafs/governance/dag/runtime/nodes/{node_cid_hex}",
    );
    /// Read a Governance DAG runtime entry by digest.
    pub const GOVERNANCE_DAG_RUNTIME_DIGEST: RouteDescriptor = local_get(
        "sorafs.governance_dag.runtime_digest",
        "/v1/sorafs/governance/dag/runtime/digests/{encoded_blake3_hex}",
    );
    /// Read Governance DAG runtime entries by payload kind.
    pub const GOVERNANCE_DAG_RUNTIME_KIND: RouteDescriptor = local_get(
        "sorafs.governance_dag.runtime_kind",
        "/v1/sorafs/governance/dag/runtime/kinds/{payload_kind}",
    );

    /// Read the latest reputation snapshot.
    pub const REPUTATION_LATEST_GET: RouteDescriptor = documented_get(
        "sorafs.reputation_snapshot.latest",
        "/v1/sorafs/reputation/latest",
    );
    /// Publish a reputation snapshot.
    pub const REPUTATION_LATEST_POST: RouteDescriptor = documented_post(
        "sorafs.reputation_snapshot.publish",
        "/v1/sorafs/reputation/latest",
    );
    /// Read one historical reputation snapshot.
    pub const REPUTATION_SNAPSHOT: RouteDescriptor = documented_get(
        "sorafs.reputation_snapshot.read",
        "/v1/sorafs/reputation/snapshots/{snapshot_id_hex}",
    );
    /// Read one provider's reputation record and proof.
    pub const REPUTATION_PROVIDER: RouteDescriptor = documented_get(
        "sorafs.reputation_provider.read",
        "/v1/sorafs/reputation/providers/{provider_id}",
    );
    /// Read the active reputation weights.
    pub const REPUTATION_WEIGHTS: RouteDescriptor = documented_get(
        "sorafs.reputation_weight.read",
        "/v1/sorafs/reputation/weights",
    );
    /// Read a bounded reputation-event snapshot.
    pub const REPUTATION_EVENTS: RouteDescriptor = documented_get(
        "sorafs.reputation_event.list",
        "/v1/sorafs/reputation/events",
    );
    /// Stream reputation events over SSE.
    pub const REPUTATION_EVENTS_STREAM: RouteDescriptor = stream_get(
        "protocol.sorafs.reputation_event_stream",
        "/v1/sorafs/reputation/events/stream",
    );
    /// Stream reputation events over WebSocket.
    pub const REPUTATION_EVENTS_WEBSOCKET: RouteDescriptor = stream_get(
        "protocol.sorafs.reputation_event_websocket",
        "/v1/sorafs/reputation/events/ws",
    );

    /// Read the `SoraFS` pin registry.
    pub const PIN_REGISTRY: RouteDescriptor = documented_get("sorafs.pin.list", "/v1/sorafs/pin");
    /// Read one `SoraFS` pin manifest.
    pub const PIN_MANIFEST: RouteDescriptor =
        documented_get("sorafs.pin.read", "/v1/sorafs/pin/{digest_hex}");
    /// Register a paid `SoraFS` pin manifest.
    pub const PIN_REGISTER: RouteDescriptor =
        documented_post("sorafs.pin.register", "/v1/sorafs/pin/register");
    /// List `SoraFS` aliases.
    pub const ALIASES: RouteDescriptor = documented_get("sorafs.alias.list", "/v1/sorafs/aliases");
    /// List `SoraFS` replication orders.
    pub const REPLICATION: RouteDescriptor =
        documented_get("sorafs.replication_order.list", "/v1/sorafs/replication");
    /// Read local `SoraFS` storage state.
    pub const STORAGE_STATE: RouteDescriptor =
        documented_get("sorafs.storage_state.read", "/v1/sorafs/storage/state");
    /// Resolve a content identifier to stored manifest metadata.
    pub const CID_LOOKUP: RouteDescriptor = public_get(
        "sorafs.content_identifier.read",
        "/v1/sorafs/cid/{cid}",
        RouteProjections::SDK,
    );
    /// Read the configured denylist catalog.
    pub const DENYLIST_CATALOG: RouteDescriptor = documented_get(
        "sorafs.denylist_catalog.read",
        "/v1/sorafs/denylist/catalog",
    );
    /// Read one configured denylist pack.
    pub const DENYLIST_PACK: RouteDescriptor = documented_get(
        "sorafs.denylist_pack.read",
        "/v1/sorafs/denylist/packs/{pack_id}",
    );
    /// Read one stored manifest.
    pub const STORAGE_MANIFEST: RouteDescriptor = documented_get(
        "sorafs.storage_manifest.read",
        "/v1/sorafs/storage/manifest/{manifest_id}",
    );
    /// Read one stored manifest's chunk plan.
    pub const STORAGE_PLAN: RouteDescriptor = documented_get(
        "sorafs.storage_plan.read",
        "/v1/sorafs/storage/plan/{manifest_id}",
    );
    /// Pin staged storage content.
    pub const STORAGE_PIN: RouteDescriptor =
        documented_post("sorafs.storage.pin", "/v1/sorafs/storage/pin");
    /// Submit a storage fetch request.
    pub const STORAGE_FETCH: RouteDescriptor =
        documented_post("sorafs.storage.fetch", "/v1/sorafs/storage/fetch");
    /// Request a storage access token.
    pub const STORAGE_TOKEN: RouteDescriptor =
        documented_post("sorafs.storage_token.issue", "/v1/sorafs/storage/token");
    /// Read CAR bytes for a stored manifest.
    pub const STORAGE_CAR: RouteDescriptor = documented_get(
        "sorafs.storage_car.read",
        "/v1/sorafs/storage/car/{manifest_id}",
    );
    /// Read one stored chunk.
    pub const STORAGE_CHUNK: RouteDescriptor = documented_get(
        "sorafs.storage_chunk.read",
        "/v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}",
    );
    /// Build a bounded proof-stream payload.
    pub const PROOF_STREAM: RouteDescriptor =
        documented_post("sorafs.proof_stream.build", "/v1/sorafs/proof/stream")
            .with_authentication(AuthenticationPolicy::OperatorSignature);
    /// Enqueue one council-admitted PDP challenge.
    pub const PDP_CHALLENGE: RouteDescriptor =
        documented_post("sorafs.pdp.challenge", "/v1/sorafs/pdp/challenge")
            .with_authentication(AuthenticationPolicy::OperatorSignature);
    /// Fetch the next pending PDP challenge for one provider.
    pub const PDP_NEXT: RouteDescriptor = documented_post("sorafs.pdp.next", "/v1/sorafs/pdp/next")
        .with_authentication(AuthenticationPolicy::OperatorSignature);
    /// Submit one challenge-bound PDP proof.
    pub const PDP_PROOF: RouteDescriptor =
        documented_post("sorafs.pdp.proof", "/v1/sorafs/pdp/proof")
            .with_authentication(AuthenticationPolicy::OperatorSignature);
    /// Read one retained PDP challenge status.
    pub const PDP_STATUS: RouteDescriptor =
        documented_post("sorafs.pdp.status", "/v1/sorafs/pdp/status")
            .with_authentication(AuthenticationPolicy::OperatorSignature);
    /// Export one bounded page of retained PDP statuses.
    pub const PDP_EXPORT: RouteDescriptor =
        documented_post("sorafs.pdp.export", "/v1/sorafs/pdp/export")
            .with_authentication(AuthenticationPolicy::OperatorSignature);
    /// Submit one canonical encrypted `PoP` enrollment.
    pub const POP_ENROLLMENT: RouteDescriptor =
        documented_post("sorafs.pop.enrollment.submit", "/v1/sorafs/pop/enrollments")
            .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Read payload-free `PoP` enrollment status.
    pub const POP_ENROLLMENT_STATUS: RouteDescriptor = documented_post(
        "sorafs.pop.enrollment.status",
        "/v1/sorafs/pop/enrollments/status",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Record one governed dual-control `PoP` approval.
    pub const POP_APPROVAL: RouteDescriptor =
        documented_post("sorafs.pop.approval.record", "/v1/sorafs/pop/approvals")
            .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Trigger runtime-resolved HSM-backed `PoP` issuance.
    pub const POP_ISSUE: RouteDescriptor =
        documented_post("sorafs.pop.credential.issue", "/v1/sorafs/pop/issue")
            .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Enqueue a governed `PoP` revocation successor.
    pub const POP_REVOCATION: RouteDescriptor = documented_post(
        "sorafs.pop.revocation.enqueue",
        "/v1/sorafs/pop/revocations",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Submit the next durable `PoP` registry outbox entry.
    pub const POP_REGISTRY_SUBMIT: RouteDescriptor = documented_post(
        "sorafs.pop.registry.submit",
        "/v1/sorafs/pop/registry/submit-next",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Reconcile the next finalized `PoP` registry projection.
    pub const POP_REGISTRY_RECONCILE: RouteDescriptor = documented_post(
        "sorafs.pop.registry.reconcile",
        "/v1/sorafs/pop/registry/reconcile-next",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Read the current finalized `PoP` registry projection.
    pub const POP_REGISTRY_PROJECTION: RouteDescriptor = documented_post(
        "sorafs.pop.registry.projection",
        "/v1/sorafs/pop/registry/projection",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Fetch finalized encrypted `PoP` wallet delivery.
    pub const POP_WALLET_DELIVERY: RouteDescriptor = documented_post(
        "sorafs.pop.wallet.delivery",
        "/v1/sorafs/pop/wallet/delivery",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Import finalized encrypted `PoP` wallet delivery.
    pub const POP_WALLET_IMPORT: RouteDescriptor =
        documented_post("sorafs.pop.wallet.import", "/v1/sorafs/pop/wallet/import")
            .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Acknowledge durable `PoP` wallet delivery.
    pub const POP_WALLET_ACKNOWLEDGE: RouteDescriptor = documented_post(
        "sorafs.pop.wallet.acknowledge",
        "/v1/sorafs/pop/wallet/acknowledge",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Synchronize a runtime-only `PoP` wallet witness.
    pub const POP_WALLET_SYNCHRONIZE: RouteDescriptor = documented_post(
        "sorafs.pop.wallet.synchronize",
        "/v1/sorafs/pop/wallet/synchronize",
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Generate a `PoP` membership proof from local wallet custody.
    pub const POP_WALLET_PROVE: RouteDescriptor =
        documented_post("sorafs.pop.wallet.prove", "/v1/sorafs/pop/wallet/prove")
            .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    /// Verify a `PoP` membership proof and consume its nullifier.
    pub const POP_VERIFY: RouteDescriptor =
        documented_post("sorafs.pop.membership.verify", "/v1/sorafs/pop/verify")
            .with_authentication(AuthenticationPolicy::ProtocolHandshake);
    const fn authenticated_deal_post(
        stable_route_id: &'static str,
        path: &'static str,
        authentication: AuthenticationPolicy,
    ) -> RouteDescriptor {
        documented_post(stable_route_id, path).with_authentication(authentication)
    }

    /// Fund a provider's `SoraFS` deal escrow.
    pub const DEAL_FUND_PROVIDER: RouteDescriptor = authenticated_deal_post(
        "sorafs.deal.provider_fund",
        "/v1/sorafs/deal/fund-provider",
        AuthenticationPolicy::IdentityBoundSignature,
    );
    /// Fund a client's `SoraFS` deal escrow.
    pub const DEAL_FUND_CLIENT: RouteDescriptor = authenticated_deal_post(
        "sorafs.deal.client_fund",
        "/v1/sorafs/deal/fund-client",
        AuthenticationPolicy::OperatorSignature,
    );
    /// Open a funded `SoraFS` deal.
    pub const DEAL_OPEN: RouteDescriptor = authenticated_deal_post(
        "sorafs.deal.open",
        "/v1/sorafs/deal/open",
        AuthenticationPolicy::OperatorSignature,
    );
    /// Cancel a `SoraFS` deal.
    pub const DEAL_CANCEL: RouteDescriptor = authenticated_deal_post(
        "sorafs.deal.cancel",
        "/v1/sorafs/deal/cancel",
        AuthenticationPolicy::OperatorSignature,
    );
    /// Submit a `SoraFS` deal-usage report.
    pub const DEAL_USAGE: RouteDescriptor = authenticated_deal_post(
        "sorafs.deal_usage.submit",
        "/v1/sorafs/deal/usage",
        AuthenticationPolicy::IdentityBoundSignature,
    );
    /// Submit a `SoraFS` deal settlement.
    pub const DEAL_SETTLE: RouteDescriptor = authenticated_deal_post(
        "sorafs.deal_settlement.submit",
        "/v1/sorafs/deal/settle",
        AuthenticationPolicy::OperatorSignature,
    );

    /// Publish a signed `SoraFS` pricing manifest.
    ///
    /// The handler authenticates the canonical request with the application
    /// signing headers, so this descriptor intentionally keeps Torii's default
    /// route authentication policy instead of applying operator middleware.
    pub const ECONOMICS_PRICING_MANIFEST: RouteDescriptor = documented_post(
        "sorafs.economics.pricing_manifest.publish",
        "/v1/sorafs/economics/pricing/manifests",
    );
    /// Publish a signed `SoraFS` hedging feed.
    ///
    /// The handler authenticates the canonical request with the application
    /// signing headers.
    pub const ECONOMICS_HEDGING_FEED: RouteDescriptor = documented_post(
        "sorafs.economics.hedging_feed.publish",
        "/v1/sorafs/economics/hedging/feeds",
    );
    /// Read the effective `SoraFS` economics status.
    pub const ECONOMICS_STATUS: RouteDescriptor = documented_get(
        "sorafs.economics.status.read",
        "/v1/sorafs/economics/status",
    );
    /// Read the active `SoraFS` pricing manifest.
    pub const ECONOMICS_ACTIVE_PRICING: RouteDescriptor = documented_get(
        "sorafs.economics.active_pricing.read",
        "/v1/sorafs/economics/pricing/active",
    );
    /// Read the current `SoraFS` hedging reference price.
    pub const ECONOMICS_HEDGING_REFERENCE: RouteDescriptor = documented_get(
        "sorafs.economics.hedging_reference.read",
        "/v1/sorafs/economics/hedging/reference",
    );

    /// Read the manifest selected by the request's `SoraFS` site binding.
    pub const SITE_MANIFEST: RouteDescriptor = protocol_get(
        "protocol.sorafs.site_manifest",
        "/.well-known/sorafs/manifest",
        RouteMatch::Exact,
        "well-known SoraFS site-manifest discovery endpoint",
    );
    /// Read the root document for one content-addressed `SoraFS` site.
    pub const CID_ROOT: RouteDescriptor = protocol_get(
        "protocol.sorafs.cid_root",
        "/sorafs/cid/{cid}",
        RouteMatch::Exact,
        "content-addressed SoraFS gateway root",
    );
    /// Read a path under one content-addressed `SoraFS` site.
    pub const CID_PATH: RouteDescriptor = protocol_get(
        "protocol.sorafs.cid_path",
        "/sorafs/cid/{cid}/{*path}",
        RouteMatch::Wildcard,
        "content-addressed SoraFS gateway wildcard",
    );

    /// Complete route family registered by `add_sorafs_routes`.
    pub const ROUTES: &[RouteDescriptor] = &[
        STORAGE_PEERS,
        PROVIDERS,
        PROVIDER_ADVERT,
        ROUTING_PROVIDERS,
        ROUTING_PEERS,
        CAPACITY_STATE,
        GOVERNANCE_DAG_DASHBOARD,
        GOVERNANCE_DAG_HEAD,
        GOVERNANCE_DAG_BLOCK,
        GOVERNANCE_DAG_NODE,
        GOVERNANCE_DAG_PUBLISH_INDEX,
        GOVERNANCE_DAG_PUBLISH_DIGEST,
        GOVERNANCE_DAG_PUBLISH_KIND,
        TRANSPARENCY_CYCLES,
        TRANSPARENCY_CYCLE,
        TRANSPARENCY_CYCLE_ENTRY,
        TRANSPARENCY_EXPLORER,
        TRANSPARENCY_EXPLORER_UI,
        TRANSPARENCY_SOURCE_ENTRY,
        TRANSPARENCY_PRIVACY_SOURCE_EVENT,
        TRANSPARENCY_PRIVACY_PUBLISH_DUE,
        TRANSPARENCY_TOKENS,
        TRANSPARENCY_TOKEN_ISSUANCE,
        TRANSPARENCY_TOKEN_VERIFY,
        APPEAL_FINANCE_REPORTS_GET,
        APPEAL_FINANCE_REPORTS_POST,
        APPEAL_FINANCE_WEEKLY_ROLLUPS_GET,
        APPEAL_FINANCE_WEEKLY_ROLLUPS_POST,
        APPEAL_FINANCE_SETTLEMENT_RECEIPTS,
        GOVERNANCE_DAG_CAR_QUEUE,
        GOVERNANCE_DAG_CAR_QUEUE_DIGEST,
        GOVERNANCE_DAG_CAR_QUEUE_KIND,
        GOVERNANCE_DAG_CAR_QUEUE_ARCHIVE,
        GOVERNANCE_DAG_RUNTIME,
        GOVERNANCE_DAG_RUNTIME_HEAD,
        GOVERNANCE_DAG_RUNTIME_BLOCK,
        GOVERNANCE_DAG_RUNTIME_NODE,
        GOVERNANCE_DAG_RUNTIME_DIGEST,
        GOVERNANCE_DAG_RUNTIME_KIND,
        REPUTATION_LATEST_GET,
        REPUTATION_LATEST_POST,
        REPUTATION_SNAPSHOT,
        REPUTATION_PROVIDER,
        REPUTATION_WEIGHTS,
        REPUTATION_EVENTS,
        REPUTATION_EVENTS_STREAM,
        REPUTATION_EVENTS_WEBSOCKET,
        PIN_REGISTRY,
        PIN_MANIFEST,
        PIN_REGISTER,
        ALIASES,
        REPLICATION,
        STORAGE_STATE,
        CID_LOOKUP,
        DENYLIST_CATALOG,
        DENYLIST_PACK,
        STORAGE_MANIFEST,
        STORAGE_PLAN,
        STORAGE_PIN,
        STORAGE_FETCH,
        STORAGE_TOKEN,
        STORAGE_CAR,
        STORAGE_CHUNK,
        PROOF_STREAM,
        PDP_CHALLENGE,
        PDP_NEXT,
        PDP_PROOF,
        PDP_STATUS,
        PDP_EXPORT,
        POP_ENROLLMENT,
        POP_ENROLLMENT_STATUS,
        POP_APPROVAL,
        POP_ISSUE,
        POP_REVOCATION,
        POP_REGISTRY_SUBMIT,
        POP_REGISTRY_RECONCILE,
        POP_REGISTRY_PROJECTION,
        POP_WALLET_DELIVERY,
        POP_WALLET_IMPORT,
        POP_WALLET_ACKNOWLEDGE,
        POP_WALLET_SYNCHRONIZE,
        POP_WALLET_PROVE,
        POP_VERIFY,
        DEAL_FUND_PROVIDER,
        DEAL_FUND_CLIENT,
        DEAL_OPEN,
        DEAL_CANCEL,
        DEAL_USAGE,
        DEAL_SETTLE,
        ECONOMICS_PRICING_MANIFEST,
        ECONOMICS_HEDGING_FEED,
        ECONOMICS_STATUS,
        ECONOMICS_ACTIVE_PRICING,
        ECONOMICS_HEDGING_REFERENCE,
        SITE_MANIFEST,
        CID_ROOT,
        CID_PATH,
    ];
}

/// Application-facing resource, explorer, webhook, and protocol routes.
pub mod application_api {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteMatch, RouteProjections,
    };

    const fn app_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn internal_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::NONE)
        .with_implicit_head(true)
    }

    const fn app_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_cors_options(true)
    }

    const fn app_sdk_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_get(id, path).with_projections(RouteProjections::SDK)
    }

    const fn app_sdk_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_post(id, path).with_projections(RouteProjections::SDK)
    }

    const fn onboarding_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_post(id, path).with_authentication(AuthenticationPolicy::OnboardingToken)
    }

    const fn onboarding_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_get(id, path).with_authentication(AuthenticationPolicy::OnboardingToken)
    }

    const fn app_delete(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Delete,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_cors_options(true)
    }

    const fn app_wildcard_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_sdk_get(id, path).with_route_match(RouteMatch::Wildcard)
    }

    const fn app_wildcard_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_sdk_post(id, path).with_route_match(RouteMatch::Wildcard)
    }

    const fn push_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_post(id, path).with_feature_gate(FeatureGate::All(&["app_api", "push"]))
    }

    const fn push_delete(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_delete(id, path).with_feature_gate(FeatureGate::All(&["app_api", "push"]))
    }

    const fn app_protocol_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::ProtocolHandshake)
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "streaming transport endpoint",
        })
        .with_implicit_head(true)
    }

    const fn app_unprojected_protocol_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_protocol_get(id, path).with_projections(RouteProjections::NONE)
    }

    const fn telemetry_protocol_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_protocol_get(id, path).with_feature_gate(FeatureGate::All(&["app_api", "telemetry"]))
    }

    const fn telemetry_diagnostic_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Diagnostic,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::All(&["app_api", "telemetry"]))
        .with_implicit_head(true)
    }

    const fn telemetry_documented_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        telemetry_diagnostic_get(id, path).with_projections(RouteProjections::OPENAPI)
    }

    macro_rules! declare_routes {
        ($($name:ident => $factory:ident($id:literal, $path:literal);)+) => {
            $(
                #[doc = concat!("Descriptor for `", $path, "`.")]
                pub const $name: RouteDescriptor = $factory($id, $path);
            )+

            /// Complete application API route family.
            pub const ROUTES: &[RouteDescriptor] = &[$($name),+];
        };
    }

    declare_routes! {
        APP_API_BINDINGS_GET => app_sdk_get("application.app_api_bindings_get", "/v1/app-api/bindings");
        APP_API_CID_BY_CID_GET => app_sdk_get("application.app_api_cid_by_cid_get", "/v1/app-api/cid/{cid}");
        APP_API_CID_BY_CID_BY_PATH_GET => app_wildcard_get("application.app_api_cid_by_cid_by_path_get", "/v1/app-api/cid/{cid}/{*path}");
        APP_API_CID_BY_CID_BY_PATH_POST => app_wildcard_post("application.app_api_cid_by_cid_by_path_post", "/v1/app-api/cid/{cid}/{*path}");
        APP_API_ACTIVE_BY_PATH_GET => app_wildcard_get("application.app_api_active_by_path_get", "/v1/app-api/active/{*path}");
        APP_API_ACTIVE_BY_PATH_POST => app_wildcard_post("application.app_api_active_by_path_post", "/v1/app-api/active/{*path}");
        API_CID_BY_CID_GET => app_sdk_get("application.api_cid_by_cid_get", "/v1/api/cid/{cid}");
        API_CID_BY_CID_BY_PATH_GET => app_wildcard_get("application.api_cid_by_cid_by_path_get", "/v1/api/cid/{cid}/{*path}");
        API_CID_BY_CID_BY_PATH_POST => app_wildcard_post("application.api_cid_by_cid_by_path_post", "/v1/api/cid/{cid}/{*path}");
        ACCOUNTS_BY_ACCOUNT_ID_GET => app_get("application.accounts_by_account_id_get", "/v1/accounts/{account_id}");
        INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_GET => internal_get("application.internal_accounts_by_account_id_get", "/v1/internal/accounts/{account_id}");
        INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_BY_ENTRYPOINT_HASH_GET => internal_get("application.internal_accounts_by_account_id_transactions_by_entrypoint_hash_get", "/v1/internal/accounts/{account_id}/transactions/{entrypoint_hash}");
        INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_ASSETS_BY_ASSET_DEFINITION_ID_GET => internal_get("application.internal_accounts_by_account_id_assets_by_asset_definition_id_get", "/v1/internal/accounts/{account_id}/assets/{asset_definition_id}");
        ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_QUERY_POST => app_post("application.accounts_by_account_id_transactions_query_post", "/v1/accounts/{account_id}/transactions/query");
        TRANSACTIONS_HISTORY_GET => app_get("application.transactions_history_get", "/v1/transactions/history");
        CONTRACTS_ACTIVITY_GET => app_get("application.contracts_activity_get", "/v1/contracts/activity");
        CONTRACTS_EVENTS_GET => app_get("application.contracts_events_get", "/v1/contracts/events");
        CONTRACTS_ROLLUPS_SWAPS_FILLS_GET => app_get("application.contracts_rollups_swaps_fills_get", "/v1/contracts/rollups/swaps/fills");
        CONTRACTS_ROLLUPS_SWAPS_CANDLES_GET => app_get("application.contracts_rollups_swaps_candles_get", "/v1/contracts/rollups/swaps/candles");
        CONTRACTS_ROLLUPS_URANAI_MARKETS_HISTORY_GET => app_get("application.contracts_rollups_uranai_markets_history_get", "/v1/contracts/rollups/uranai/markets/history");
        CONTRACTS_ROLLUPS_TRADER_ACTIVITY_GET => app_get("application.contracts_rollups_trader_activity_get", "/v1/contracts/rollups/trader/activity");
        CONTRACTS_ROLLUPS_TRADER_ACCOUNT_GET => app_get("application.contracts_rollups_trader_account_get", "/v1/contracts/rollups/trader/account");
        CONTRACTS_ROLLUPS_INTENTS_GET => app_sdk_get("application.contracts_rollups_intents_get", "/v1/contracts/rollups/intents");
        CONTRACTS_ROLLUPS_VAULTS_POSITIONS_GET => app_sdk_get("application.contracts_rollups_vaults_positions_get", "/v1/contracts/rollups/vaults/positions");
        CONTRACTS_ROLLUPS_OPERATORS_STATUS_GET => app_sdk_get("application.contracts_rollups_operators_status_get", "/v1/contracts/rollups/operators/status");
        CONTRACTS_ROLLUPS_MARGIN_HEALTH_GET => app_sdk_get("application.contracts_rollups_margin_health_get", "/v1/contracts/rollups/margin/health");
        CONTRACTS_ROLLUPS_RWA_LOTS_GET => app_sdk_get("application.contracts_rollups_rwa_lots_get", "/v1/contracts/rollups/rwa/lots");
        CONTRACTS_ROLLUPS_DLMM_HOOKS_GET => app_sdk_get("application.contracts_rollups_dlmm_hooks_get", "/v1/contracts/rollups/dlmm/hooks");
        ACCOUNTS_BY_ACCOUNT_ID_ASSETS_GET => app_get("application.accounts_by_account_id_assets_get", "/v1/accounts/{account_id}/assets");
        ACCOUNTS_BY_ACCOUNT_ID_ASSETS_QUERY_POST => app_post("application.accounts_by_account_id_assets_query_post", "/v1/accounts/{account_id}/assets/query");
        ACCOUNTS_BY_ACCOUNT_ID_PERMISSIONS_GET => app_get("application.accounts_by_account_id_permissions_get", "/v1/accounts/{account_id}/permissions");
        ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_GET => app_get("application.accounts_by_account_id_transactions_get", "/v1/accounts/{account_id}/transactions");
        ACCOUNTS_BY_ACCOUNT_ID_HISTORY_GET => app_get("application.accounts_by_account_id_history_get", "/v1/accounts/{account_id}/history");
        PROOFS_QUERY_POST => app_post("application.proofs_query_post", "/v1/proofs/query");
        ZK_PROOF_TAGS_BY_BACKEND_BY_HASH_GET => app_get("application.zk_proof_tags_by_backend_by_hash_get", "/v1/zk/proof-tags/{backend}/{hash}");
        DOMAINS_GET => app_get("application.domains_get", "/v1/domains");
        DOMAINS_QUERY_POST => app_post("application.domains_query_post", "/v1/domains/query");
        ACCOUNTS_GET => app_get("application.accounts_get", "/v1/accounts");
        ACCOUNTS_QUERY_POST => app_post("application.accounts_query_post", "/v1/accounts/query");
        TRANSACTIONS_QUERY_POST => app_post("application.transactions_query_post", "/v1/transactions/query");
        TRANSACTIONS_VISIBLE_QUERY_POST => app_post("application.transactions_visible_query_post", "/v1/transactions/visible/query");
        ACCOUNTS_ONBOARD_PLAN_POST => onboarding_post("application.accounts_onboard_plan_post", "/v1/accounts/onboard/plan");
        ACCOUNTS_ONBOARD_POST => onboarding_post("application.accounts_onboard_post", "/v1/accounts/onboard");
        ACCOUNTS_ONBOARDING_READINESS_GET => onboarding_get("application.accounts_onboarding_readiness_get", "/v1/accounts/onboarding/readiness");
        ACCOUNTS_FAUCET_PUZZLE_GET => app_get("application.accounts_faucet_puzzle_get", "/v1/accounts/faucet/puzzle");
        ACCOUNTS_FAUCET_POST => app_post("application.accounts_faucet_post", "/v1/accounts/faucet");
        ACCOUNTS_BY_ACCOUNT_ID_ALIASES_GET => app_sdk_get("application.accounts_by_account_id_aliases_get", "/v1/accounts/{account_id}/aliases");
        ACCOUNTS_BY_UAID_PORTFOLIO_GET => app_get("application.accounts_by_uaid_portfolio_get", "/v1/accounts/{uaid}/portfolio");
        NEXUS_PUBLIC_LANES_BY_LANE_ID_VALIDATORS_GET => app_get("application.nexus_public_lanes_by_lane_id_validators_get", "/v1/nexus/public-lanes/{lane_id}/validators");
        NEXUS_PUBLIC_LANES_BY_LANE_ID_STAKE_GET => app_get("application.nexus_public_lanes_by_lane_id_stake_get", "/v1/nexus/public-lanes/{lane_id}/stake");
        NEXUS_PUBLIC_LANES_BY_LANE_ID_REWARDS_PENDING_GET => app_get("application.nexus_public_lanes_by_lane_id_rewards_pending_get", "/v1/nexus/public-lanes/{lane_id}/rewards/pending");
        NEXUS_DATASPACES_ACCOUNTS_BY_LITERAL_SUMMARY_GET => app_get("application.nexus_dataspaces_accounts_by_literal_summary_get", "/v1/nexus/dataspaces/accounts/{literal}/summary");
        SPACE_DIRECTORY_UAIDS_BY_UAID_GET => app_get("application.space_directory_uaids_by_uaid_get", "/v1/space-directory/uaids/{uaid}");
        SPACE_DIRECTORY_UAIDS_BY_UAID_MANIFESTS_GET => app_get("application.space_directory_uaids_by_uaid_manifests_get", "/v1/space-directory/uaids/{uaid}/manifests");
        SPACE_DIRECTORY_MANIFESTS_POST => app_sdk_post("application.space_directory_manifests_post", "/v1/space-directory/manifests");
        SPACE_DIRECTORY_MANIFESTS_REVOKE_POST => app_sdk_post("application.space_directory_manifests_revoke_post", "/v1/space-directory/manifests/revoke");
        RAM_LFE_PROGRAM_POLICIES_GET => app_get("application.ram_lfe_program_policies_get", "/v1/ram-lfe/program-policies");
        RAM_LFE_PROGRAMS_BY_PROGRAM_ID_EXECUTE_POST => app_post("application.ram_lfe_programs_by_program_id_execute_post", "/v1/ram-lfe/programs/{program_id}/execute");
        RAM_LFE_RECEIPTS_VERIFY_POST => app_post("application.ram_lfe_receipts_verify_post", "/v1/ram-lfe/receipts/verify");
        IDENTIFIER_POLICIES_GET => app_get("application.identifier_policies_get", "/v1/identifier-policies");
        ACCOUNTS_BY_ACCOUNT_ID_IDENTIFIERS_CLAIM_RECEIPT_POST => app_post("application.accounts_by_account_id_identifiers_claim_receipt_post", "/v1/accounts/{account_id}/identifiers/claim-receipt");
        IDENTIFIERS_RECEIPTS_BY_RECEIPT_HASH_GET => app_get("application.identifiers_receipts_by_receipt_hash_get", "/v1/identifiers/receipts/{receipt_hash}");
        IDENTIFIERS_RESOLVE_POST => app_post("application.identifiers_resolve_post", "/v1/identifiers/resolve");
        REPO_AGREEMENTS_GET => app_get("application.repo_agreements_get", "/v1/repo/agreements");
        REPO_AGREEMENTS_QUERY_POST => app_post("application.repo_agreements_query_post", "/v1/repo/agreements/query");
        NOTIFY_DEVICES_POST => push_post("application.notify_devices_post", "/v1/notify/devices");
        NOTIFY_DEVICES_DELETE => push_delete("application.notify_devices_delete", "/v1/notify/devices");
        SNS_NAMES_BY_NAMESPACE_BY_LITERAL_GET => app_get("application.sns_names_by_namespace_by_literal_get", "/v1/sns/names/{namespace}/{literal}");
        SNS_POLICIES_BY_SUFFIX_ID_GET => app_get("application.sns_policies_by_suffix_id_get", "/v1/sns/policies/{suffix_id}");
        SORACLOUD_STATUS_GET => app_get("application.soracloud_status_get", "/v1/soracloud/status");
        SORACLOUD_SERVICES_BY_SERVICE_NAME_PUBLIC_DISCOVERY_GET => app_sdk_get("application.soracloud_services_by_service_name_public_discovery_get", "/v1/soracloud/services/{service_name}/public-discovery");
        SORACLOUD_SERVICES_BY_SERVICE_NAME_REVISIONS_BY_SERVICE_VERSION_PUBLIC_DISCOVERY_GET => app_sdk_get("application.soracloud_services_by_service_name_revisions_by_service_version_public_discovery_get", "/v1/soracloud/services/{service_name}/revisions/{service_version}/public-discovery");
        SORACLOUD_DEPLOY_POST => app_sdk_post("application.soracloud_deploy_post", "/v1/soracloud/deploy");
        SORACLOUD_UPGRADE_POST => app_sdk_post("application.soracloud_upgrade_post", "/v1/soracloud/upgrade");
        SORACLOUD_APPS_DEPLOY_POST => app_sdk_post("application.soracloud_apps_deploy_post", "/v1/soracloud/apps/deploy");
        SORACLOUD_APPS_UPGRADE_POST => app_sdk_post("application.soracloud_apps_upgrade_post", "/v1/soracloud/apps/upgrade");
        SORACLOUD_APPS_STATUS_GET => app_sdk_get("application.soracloud_apps_status_get", "/v1/soracloud/apps/status");
        SORACLOUD_APPS_BY_APP_NAME_STATUS_GET => app_sdk_get("application.soracloud_apps_by_app_name_status_get", "/v1/soracloud/apps/{app_name}/status");
        SORACLOUD_ROLLBACK_POST => app_sdk_post("application.soracloud_rollback_post", "/v1/soracloud/rollback");
        SORACLOUD_ROLLOUT_POST => app_sdk_post("application.soracloud_rollout_post", "/v1/soracloud/rollout");
        SORACLOUD_STATE_MUTATE_POST => app_sdk_post("application.soracloud_state_mutate_post", "/v1/soracloud/state/mutate");
        SORACLOUD_SERVICE_CONFIG_SET_POST => app_sdk_post("application.soracloud_service_config_set_post", "/v1/soracloud/service/config/set");
        SORACLOUD_SERVICE_CONFIG_DELETE_POST => app_sdk_post("application.soracloud_service_config_delete_post", "/v1/soracloud/service/config/delete");
        SORACLOUD_SERVICE_CONFIG_STATUS_GET => app_sdk_get("application.soracloud_service_config_status_get", "/v1/soracloud/service/config/status");
        SORACLOUD_SERVICE_SECRET_SET_POST => app_sdk_post("application.soracloud_service_secret_set_post", "/v1/soracloud/service/secret/set");
        SORACLOUD_SERVICE_SECRET_DELETE_POST => app_sdk_post("application.soracloud_service_secret_delete_post", "/v1/soracloud/service/secret/delete");
        SORACLOUD_SERVICE_SECRET_STATUS_GET => app_sdk_get("application.soracloud_service_secret_status_get", "/v1/soracloud/service/secret/status");
        SORACLOUD_FHE_JOB_RUN_POST => app_sdk_post("application.soracloud_fhe_job_run_post", "/v1/soracloud/fhe/job/run");
        SORACLOUD_DECRYPT_REQUEST_POST => app_sdk_post("application.soracloud_decrypt_request_post", "/v1/soracloud/decrypt/request");
        SORACLOUD_HEALTH_ACCESS_REQUEST_POST => app_sdk_post("application.soracloud_health_access_request_post", "/v1/soracloud/health/access/request");
        SORACLOUD_HEALTH_COMPLIANCE_REPORT_GET => app_sdk_get("application.soracloud_health_compliance_report_get", "/v1/soracloud/health/compliance/report");
        SORACLOUD_CIPHERTEXT_QUERY_POST => app_sdk_post("application.soracloud_ciphertext_query_post", "/v1/soracloud/ciphertext/query");
        SORACLOUD_TRAINING_JOB_START_POST => app_sdk_post("application.soracloud_training_job_start_post", "/v1/soracloud/training/job/start");
        SORACLOUD_TRAINING_JOB_CHECKPOINT_POST => app_sdk_post("application.soracloud_training_job_checkpoint_post", "/v1/soracloud/training/job/checkpoint");
        SORACLOUD_TRAINING_JOB_RETRY_POST => app_sdk_post("application.soracloud_training_job_retry_post", "/v1/soracloud/training/job/retry");
        SORACLOUD_TRAINING_JOB_STATUS_GET => app_sdk_get("application.soracloud_training_job_status_get", "/v1/soracloud/training/job/status");
        SORACLOUD_MODEL_WEIGHT_REGISTER_POST => app_sdk_post("application.soracloud_model_weight_register_post", "/v1/soracloud/model/weight/register");
        SORACLOUD_MODEL_WEIGHT_PROMOTE_POST => app_sdk_post("application.soracloud_model_weight_promote_post", "/v1/soracloud/model/weight/promote");
        SORACLOUD_MODEL_WEIGHT_ROLLBACK_POST => app_sdk_post("application.soracloud_model_weight_rollback_post", "/v1/soracloud/model/weight/rollback");
        SORACLOUD_MODEL_WEIGHT_STATUS_GET => app_sdk_get("application.soracloud_model_weight_status_get", "/v1/soracloud/model/weight/status");
        SORACLOUD_MODEL_ARTIFACT_REGISTER_POST => app_sdk_post("application.soracloud_model_artifact_register_post", "/v1/soracloud/model/artifact/register");
        SORACLOUD_MODEL_ARTIFACT_STATUS_GET => app_sdk_get("application.soracloud_model_artifact_status_get", "/v1/soracloud/model/artifact/status");
        SORACLOUD_MODEL_UPLOAD_REGISTER_POST => app_sdk_post("application.soracloud_model_upload_register_post", "/v1/soracloud/model/upload/register");
        SORACLOUD_MODEL_UPLOAD_ENCRYPTION_RECIPIENT_GET => app_sdk_get("application.soracloud_model_upload_encryption_recipient_get", "/v1/soracloud/model/upload/encryption-recipient");
        SORACLOUD_MODEL_UPLOAD_STATUS_GET => app_sdk_get("application.soracloud_model_upload_status_get", "/v1/soracloud/model/upload/status");
        SORACLOUD_MODEL_UPLOAD_PRIVATE_EXECUTE_POST => app_post("application.soracloud_model_upload_private_execute_post", "/v1/soracloud/model/upload/private/execute");
        SORACLOUD_MODEL_UPLOAD_PRIVATE_RECEIPTS_GET => app_get("application.soracloud_model_upload_private_receipts_get", "/v1/soracloud/model/upload/private/receipts");
        SORACLOUD_HF_DEPLOY_POST => app_post("application.soracloud_hf_deploy_post", "/v1/soracloud/hf/deploy");
        SORACLOUD_HF_STATUS_GET => app_sdk_get("application.soracloud_hf_status_get", "/v1/soracloud/hf/status");
        SORACLOUD_HF_LEASE_LEAVE_POST => app_sdk_post("application.soracloud_hf_lease_leave_post", "/v1/soracloud/hf/lease/leave");
        SORACLOUD_HF_LEASE_RENEW_POST => app_sdk_post("application.soracloud_hf_lease_renew_post", "/v1/soracloud/hf/lease/renew");
        SORACLOUD_MODEL_HOST_ADVERTISE_POST => app_sdk_post("application.soracloud_model_host_advertise_post", "/v1/soracloud/model-host/advertise");
        SORACLOUD_MODEL_HOST_HEARTBEAT_POST => app_sdk_post("application.soracloud_model_host_heartbeat_post", "/v1/soracloud/model-host/heartbeat");
        SORACLOUD_MODEL_HOST_WITHDRAW_POST => app_sdk_post("application.soracloud_model_host_withdraw_post", "/v1/soracloud/model-host/withdraw");
        SORACLOUD_MODEL_HOST_STATUS_GET => app_sdk_get("application.soracloud_model_host_status_get", "/v1/soracloud/model-host/status");
        SORACLOUD_AGENT_DEPLOY_POST => app_sdk_post("application.soracloud_agent_deploy_post", "/v1/soracloud/agent/deploy");
        SORACLOUD_AGENT_LEASE_RENEW_POST => app_sdk_post("application.soracloud_agent_lease_renew_post", "/v1/soracloud/agent/lease/renew");
        SORACLOUD_AGENT_RESTART_POST => app_sdk_post("application.soracloud_agent_restart_post", "/v1/soracloud/agent/restart");
        SORACLOUD_AGENT_STATUS_GET => app_sdk_get("application.soracloud_agent_status_get", "/v1/soracloud/agent/status");
        SORACLOUD_AGENT_WALLET_SPEND_POST => app_sdk_post("application.soracloud_agent_wallet_spend_post", "/v1/soracloud/agent/wallet/spend");
        SORACLOUD_AGENT_WALLET_APPROVE_POST => app_sdk_post("application.soracloud_agent_wallet_approve_post", "/v1/soracloud/agent/wallet/approve");
        SORACLOUD_AGENT_POLICY_REVOKE_POST => app_sdk_post("application.soracloud_agent_policy_revoke_post", "/v1/soracloud/agent/policy/revoke");
        SORACLOUD_AGENT_MESSAGE_SEND_POST => app_sdk_post("application.soracloud_agent_message_send_post", "/v1/soracloud/agent/message/send");
        SORACLOUD_AGENT_MESSAGE_ACK_POST => app_sdk_post("application.soracloud_agent_message_ack_post", "/v1/soracloud/agent/message/ack");
        SORACLOUD_AGENT_MAILBOX_STATUS_GET => app_sdk_get("application.soracloud_agent_mailbox_status_get", "/v1/soracloud/agent/mailbox/status");
        SORACLOUD_AGENT_AUTONOMY_ALLOW_POST => app_sdk_post("application.soracloud_agent_autonomy_allow_post", "/v1/soracloud/agent/autonomy/allow");
        SORACLOUD_AGENT_AUTONOMY_RUN_POST => app_sdk_post("application.soracloud_agent_autonomy_run_post", "/v1/soracloud/agent/autonomy/run");
        SORACLOUD_AGENT_AUTONOMY_RUN_FINALIZE_POST => app_sdk_post("application.soracloud_agent_autonomy_run_finalize_post", "/v1/soracloud/agent/autonomy/run/finalize");
        SORACLOUD_AGENT_AUTONOMY_STATUS_GET => app_sdk_get("application.soracloud_agent_autonomy_status_get", "/v1/soracloud/agent/autonomy/status");
        ASSETS_DEFINITIONS_GET => app_get("application.assets_definitions_get", "/v1/assets/definitions");
        ASSETS_DEFINITIONS_BY_ASSET_GET => app_get("application.assets_definitions_by_asset_get", "/v1/assets/definitions/{asset}");
        ASSETS_DEFINITIONS_QUERY_POST => app_post("application.assets_definitions_query_post", "/v1/assets/definitions/query");
        CONFIDENTIAL_ASSETS_BY_DEFINITION_ID_TRANSITIONS_GET => app_get("application.confidential_assets_by_definition_id_transitions_get", "/v1/confidential/assets/{definition_id}/transitions");
        CONFIDENTIAL_NOTES_GET => app_sdk_get("application.confidential_notes_get", "/v1/confidential/notes");
        CONFIDENTIAL_RELAY_SUBMIT_POST => app_sdk_post("application.confidential_relay_submit_post", "/v1/confidential/relay/submit");
        NFTS_GET => app_get("application.nfts_get", "/v1/nfts");
        NFTS_QUERY_POST => app_post("application.nfts_query_post", "/v1/nfts/query");
        RWAS_GET => app_get("application.rwas_get", "/v1/rwas");
        RWAS_QUERY_POST => app_post("application.rwas_query_post", "/v1/rwas/query");
        SUBSCRIPTIONS_PLANS_GET => app_get("application.subscriptions_plans_get", "/v1/subscriptions/plans");
        SUBSCRIPTIONS_PLANS_POST => app_sdk_post("application.subscriptions_plans_post", "/v1/subscriptions/plans");
        SUBSCRIPTIONS_GET => app_get("application.subscriptions_get", "/v1/subscriptions");
        SUBSCRIPTIONS_POST => app_sdk_post("application.subscriptions_post", "/v1/subscriptions");
        SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_GET => app_get("application.subscriptions_by_subscription_id_get", "/v1/subscriptions/{subscription_id}");
        SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_PAUSE_POST => app_sdk_post("application.subscriptions_by_subscription_id_pause_post", "/v1/subscriptions/{subscription_id}/pause");
        SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_RESUME_POST => app_sdk_post("application.subscriptions_by_subscription_id_resume_post", "/v1/subscriptions/{subscription_id}/resume");
        SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CANCEL_POST => app_sdk_post("application.subscriptions_by_subscription_id_cancel_post", "/v1/subscriptions/{subscription_id}/cancel");
        SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_KEEP_POST => app_sdk_post("application.subscriptions_by_subscription_id_keep_post", "/v1/subscriptions/{subscription_id}/keep");
        SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_USAGE_POST => app_sdk_post("application.subscriptions_by_subscription_id_usage_post", "/v1/subscriptions/{subscription_id}/usage");
        SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CHARGE_NOW_POST => app_sdk_post("application.subscriptions_by_subscription_id_charge_now_post", "/v1/subscriptions/{subscription_id}/charge-now");
        PARAMETERS_GET => app_get("application.parameters_get", "/v1/parameters");
        EXPLORER_ACCOUNTS_GET => app_get("application.explorer_accounts_get", "/v1/explorer/accounts");
        EXPLORER_DOMAINS_GET => app_get("application.explorer_domains_get", "/v1/explorer/domains");
        EXPLORER_ASSET_DEFINITIONS_GET => app_get("application.explorer_asset_definitions_get", "/v1/explorer/asset-definitions");
        EXPLORER_ASSETS_GET => app_get("application.explorer_assets_get", "/v1/explorer/assets");
        EXPLORER_NFTS_GET => app_get("application.explorer_nfts_get", "/v1/explorer/nfts");
        EXPLORER_RWAS_GET => app_get("application.explorer_rwas_get", "/v1/explorer/rwas");
        EXPLORER_BLOCKS_GET => app_get("application.explorer_blocks_get", "/v1/explorer/blocks");
        EXPLORER_HEALTH_GET => app_sdk_get("application.explorer_health_get", "/v1/explorer/health");
        EXPLORER_BLOCKS_STREAM_GET => app_protocol_get("application.explorer_blocks_stream_get", "/v1/explorer/blocks/stream");
        EXPLORER_TRANSACTIONS_GET => app_get("application.explorer_transactions_get", "/v1/explorer/transactions");
        EXPLORER_TRANSACTIONS_LATEST_GET => app_sdk_get("application.explorer_transactions_latest_get", "/v1/explorer/transactions/latest");
        EXPLORER_TRANSACTIONS_STREAM_GET => app_protocol_get("application.explorer_transactions_stream_get", "/v1/explorer/transactions/stream");
        EXPLORER_INSTRUCTIONS_GET => app_get("application.explorer_instructions_get", "/v1/explorer/instructions");
        EXPLORER_INSTRUCTIONS_LATEST_GET => app_sdk_get("application.explorer_instructions_latest_get", "/v1/explorer/instructions/latest");
        SORACLES_DEFI_ATTESTATIONS_LATEST_GET => app_sdk_get("application.soracles_defi_attestations_latest_get", "/v1/soracles/defi/attestations/latest");
        SORACLES_FEEDS_GET => app_sdk_get("application.soracles_feeds_get", "/v1/soracles/feeds");
        SORACLES_FEEDS_BY_FEED_ID_HISTORY_GET => app_sdk_get("application.soracles_feeds_by_feed_id_history_get", "/v1/soracles/feeds/{feed_id}/history");
        EXPLORER_METRICS_GET => telemetry_documented_get("application.explorer_metrics_get", "/v1/explorer/metrics");
        EXPLORER_INSTRUCTIONS_STREAM_GET => telemetry_protocol_get("application.explorer_instructions_stream_get", "/v1/explorer/instructions/stream");
        TELEMETRY_PEERS_INFO_GET => telemetry_documented_get("application.telemetry_peers_info_get", "/v1/telemetry/peers-info");
        TELEMETRY_PROPAGATION_GET => telemetry_diagnostic_get("application.telemetry_propagation_get", "/v1/telemetry/propagation");
        TELEMETRY_LIVE_GET => telemetry_documented_get("application.telemetry_live_get", "/v1/telemetry/live");
        EXPLORER_ACCOUNTS_BY_ACCOUNT_ID_GET => app_get("application.explorer_accounts_by_account_id_get", "/v1/explorer/accounts/{account_id}");
        EXPLORER_ACCOUNTS_BY_ACCOUNT_ID_QR_GET => app_get("application.explorer_accounts_by_account_id_qr_get", "/v1/explorer/accounts/{account_id}/qr");
        EXPLORER_DOMAINS_BY_DOMAIN_ID_GET => app_get("application.explorer_domains_by_domain_id_get", "/v1/explorer/domains/{domain_id}");
        EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_GET => app_get("application.explorer_asset_definitions_by_definition_id_get", "/v1/explorer/asset-definitions/{definition_id}");
        EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_ECONOMETRICS_GET => app_get("application.explorer_asset_definitions_by_definition_id_econometrics_get", "/v1/explorer/asset-definitions/{definition_id}/econometrics");
        EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_SNAPSHOT_GET => app_get("application.explorer_asset_definitions_by_definition_id_snapshot_get", "/v1/explorer/asset-definitions/{definition_id}/snapshot");
        EXPLORER_ASSETS_BY_ASSET_ID_GET => app_get("application.explorer_assets_by_asset_id_get", "/v1/explorer/assets/{asset_id}");
        EXPLORER_NFTS_BY_NFT_ID_GET => app_get("application.explorer_nfts_by_nft_id_get", "/v1/explorer/nfts/{nft_id}");
        EXPLORER_RWAS_BY_RWA_ID_GET => app_get("application.explorer_rwas_by_rwa_id_get", "/v1/explorer/rwas/{rwa_id}");
        EXPLORER_BLOCKS_BY_IDENTIFIER_GET => app_get("application.explorer_blocks_by_identifier_get", "/v1/explorer/blocks/{identifier}");
        EXPLORER_TRANSACTIONS_BY_HASH_GET => app_get("application.explorer_transactions_by_hash_get", "/v1/explorer/transactions/{hash}");
        EXPLORER_INSTRUCTIONS_BY_HASH_BY_INDEX_GET => app_get("application.explorer_instructions_by_hash_by_index_get", "/v1/explorer/instructions/{hash}/{index}");
        EXPLORER_INSTRUCTIONS_BY_HASH_BY_INDEX_CONTRACT_VIEW_GET => app_sdk_get("application.explorer_instructions_by_hash_by_index_contract_view_get", "/v1/explorer/instructions/{hash}/{index}/contract-view");
        KAIGI_CALLS_BY_CALL_ID_GET => app_sdk_get("application.kaigi_calls_by_call_id_get", "/v1/kaigi/calls/{call_id}");
        KAIGI_CALLS_BY_CALL_ID_SIGNALS_GET => app_sdk_get("application.kaigi_calls_by_call_id_signals_get", "/v1/kaigi/calls/{call_id}/signals");
        KAIGI_CALLS_BY_CALL_ID_EVENTS_GET => app_unprojected_protocol_get("application.kaigi_calls_by_call_id_events_get", "/v1/kaigi/calls/{call_id}/events");
        KAIGI_RELAYS_GET => app_get("application.kaigi_relays_get", "/v1/kaigi/relays");
        KAIGI_RELAYS_BY_RELAY_ID_GET => app_get("application.kaigi_relays_by_relay_id_get", "/v1/kaigi/relays/{relay_id}");
        KAIGI_RELAYS_HEALTH_GET => app_get("application.kaigi_relays_health_get", "/v1/kaigi/relays/health");
        KAIGI_RELAYS_EVENTS_GET => app_protocol_get("application.kaigi_relays_events_get", "/v1/kaigi/relays/events");
        WEBHOOKS_GET => app_get("application.webhooks_get", "/v1/webhooks");
        WEBHOOKS_POST => app_post("application.webhooks_post", "/v1/webhooks");
        WEBHOOKS_BY_ID_DELETE => app_delete("application.webhooks_by_id_delete", "/v1/webhooks/{id}");
    }
}

/// Contract execution, multisig, verification-key, and proof-service routes.
pub mod contracts_and_verification_keys {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteProjections,
    };

    const fn app_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_implicit_head(true)
        .with_cors_options(true)
    }

    const fn app_signed_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_get(id, path).with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
    }

    const fn app_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK)
        .with_cors_options(true)
    }

    const fn app_signed_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_post(id, path).with_authentication(AuthenticationPolicy::CanonicalAccountSignature)
    }

    const fn app_sdk_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_get(id, path).with_projections(RouteProjections::SDK)
    }

    const fn app_signed_sdk_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_signed_get(id, path).with_projections(RouteProjections::SDK)
    }

    const fn app_sdk_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_post(id, path).with_projections(RouteProjections::SDK)
    }

    const fn app_operator_post(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Post,
            path,
            ApiSurface::Operator,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::OperatorSignature)
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI)
    }

    const fn app_protocol_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        RouteDescriptor::new(
            id,
            HttpMethod::Get,
            path,
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::ProtocolHandshake)
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "streaming transport endpoint",
        })
        .with_implicit_head(true)
    }

    const fn app_unprojected_protocol_get(id: &'static str, path: &'static str) -> RouteDescriptor {
        app_protocol_get(id, path).with_projections(RouteProjections::NONE)
    }

    macro_rules! declare_routes {
        ($($name:ident => $factory:ident($id:literal, $path:literal);)+) => {
            $(
                #[doc = concat!("Descriptor for `", $path, "`.")]
                pub const $name: RouteDescriptor = $factory($id, $path);
            )+

            /// Complete contract and verification-key route family.
            pub const ROUTES: &[RouteDescriptor] = &[$($name),+];
        };
    }

    declare_routes! {
        CONTRACTS_CODE_BYTES_BY_CODE_HASH_GET => app_signed_get("contracts.contracts_code_bytes_by_code_hash_get", "/v1/contracts/code-bytes/{code_hash}");
        CONTRACTS_ALIASES_POST => app_post("contracts.contracts_aliases_post", "/v1/contracts/aliases");
        CONTRACTS_ALIASES_RESOLVE_POST => app_signed_post("contracts.contracts_aliases_resolve_post", "/v1/contracts/aliases/resolve");
        CONTRACTS_DEPLOYMENT_STATE_POST => app_signed_post("contracts.contracts_deployment_state_post", "/v1/contracts/deployment-state");
        ASSETS_TRANSFER_POST => app_post("assets.assets_transfer_post", "/v1/assets/transfer");
        CONTRACTS_CALL_POST => app_post("contracts.contracts_call_post", "/v1/contracts/call");
        CONTRACTS_CALL_SIMULATE_POST => app_post("contracts.contracts_call_simulate_post", "/v1/contracts/call/simulate");
        BRIDGE_PROOFS_SUBMIT_POST => app_post("contracts.bridge_proofs_submit_post", "/v1/bridge/proofs/submit");
        BRIDGE_MESSAGES_POST => app_post("contracts.bridge_messages_post", "/v1/bridge/messages");
        CONTRACTS_VIEW_POST => app_post("contracts.contracts_view_post", "/v1/contracts/view");
        CONTRACTS_VIEW_BATCH_POST => app_post("contracts.contracts_view_batch_post", "/v1/contracts/view/batch");
        CONTRACTS_CALL_MULTISIG_PROPOSE_POST => app_post("contracts.contracts_call_multisig_propose_post", "/v1/contracts/call/multisig/propose");
        CONTRACTS_CALL_MULTISIG_APPROVE_POST => app_post("contracts.contracts_call_multisig_approve_post", "/v1/contracts/call/multisig/approve");
        CONTRACTS_STATE_GET => app_get("contracts.contracts_state_get", "/v1/contracts/state");
        MINT_REQUESTS_GET => app_sdk_get("contracts.mint_requests_get", "/v1/mint-requests");
        MINT_REQUESTS_BY_REQUEST_ID_GET => app_sdk_get("contracts.mint_requests_by_request_id_get", "/v1/mint-requests/{request_id}");
        MULTISIG_PROPOSE_POST => app_post("contracts.multisig_propose_post", "/v1/multisig/propose");
        MULTISIG_APPROVE_POST => app_post("contracts.multisig_approve_post", "/v1/multisig/approve");
        MULTISIG_CANCEL_POST => app_post("contracts.multisig_cancel_post", "/v1/multisig/cancel");
        MULTISIG_SPEC_POST => app_signed_post("contracts.multisig_spec_post", "/v1/multisig/spec");
        MULTISIG_PROPOSALS_QUERY_POST => app_signed_post("contracts.multisig_proposals_query_post", "/v1/multisig/proposals/query");
        MULTISIG_PROPOSALS_RESOLVE_POST => app_signed_post("contracts.multisig_proposals_resolve_post", "/v1/multisig/proposals/resolve");
        CONTROLS_ASSET_TRANSFER_QUERY_POST => app_post("contracts.controls_asset_transfer_query_post", "/v1/controls/asset-transfer/query");
        ZK_VK_REGISTER_POST => app_sdk_post("contracts.zk_vk_register_post", "/v1/zk/vk/register");
        ZK_VK_UPDATE_POST => app_sdk_post("contracts.zk_vk_update_post", "/v1/zk/vk/update");
        SORAFS_CAPACITY_DECLARE_POST => app_sdk_post("contracts.sorafs_capacity_declare_post", "/v1/sorafs/capacity/declare");
        SORAFS_CAPACITY_TELEMETRY_POST => app_sdk_post("contracts.sorafs_capacity_telemetry_post", "/v1/sorafs/capacity/telemetry");
        SORAFS_CAPACITY_SCHEDULE_POST => app_post("contracts.sorafs_capacity_schedule_post", "/v1/sorafs/capacity/schedule");
        SORAFS_CAPACITY_COMPLETE_POST => app_post("contracts.sorafs_capacity_complete_post", "/v1/sorafs/capacity/complete");
        SORAFS_CAPACITY_UPTIME_POST => app_post("contracts.sorafs_capacity_uptime_post", "/v1/sorafs/capacity/uptime");
        SORAFS_CAPACITY_POR_PROOF_POST => app_operator_post("contracts.sorafs_capacity_por_proof_post", "/v1/sorafs/capacity/por-proof");
        SORAFS_CAPACITY_POR_VERDICT_POST => app_operator_post("contracts.sorafs_capacity_por_verdict_post", "/v1/sorafs/capacity/por-verdict");
        SORAFS_POR_STATUS_GET => app_get("contracts.sorafs_por_status_get", "/v1/sorafs/por/status");
        SORAFS_POR_EXPORT_GET => app_get("contracts.sorafs_por_export_get", "/v1/sorafs/por/export");
        SORAFS_POR_INGESTION_BY_MANIFEST_DIGEST_HEX_GET => app_get("contracts.sorafs_por_ingestion_by_manifest_digest_hex_get", "/v1/sorafs/por/ingestion/{manifest_digest_hex}");
        SORAFS_POR_REPORT_BY_ISO_WEEK_GET => app_get("contracts.sorafs_por_report_by_iso_week_get", "/v1/sorafs/por/report/{iso_week}");
        SORAFS_POR_VRF_POST => app_sdk_post("contracts.sorafs_por_vrf_post", "/v1/sorafs/por/vrf");
        SORAFS_CAPACITY_FAILURE_POST => app_post("contracts.sorafs_capacity_failure_post", "/v1/sorafs/capacity/failure");
        SORAFS_ORDERBOOK_ORDERS_POST => app_sdk_post("contracts.sorafs_orderbook_orders_post", "/v1/sorafs/orderbook/orders");
        SORAFS_ORDERBOOK_CANCEL_POST => app_sdk_post("contracts.sorafs_orderbook_cancel_post", "/v1/sorafs/orderbook/cancel");
        SORAFS_ORDERBOOK_RECEIPTS_POST => app_sdk_post("contracts.sorafs_orderbook_receipts_post", "/v1/sorafs/orderbook/receipts");
        SORAFS_ORDERBOOK_RECEIPTS_GET => app_sdk_get("contracts.sorafs_orderbook_receipts_get", "/v1/sorafs/orderbook/receipts");
        SORAFS_ORDERBOOK_BOOK_GET => app_sdk_get("contracts.sorafs_orderbook_book_get", "/v1/sorafs/orderbook/book");
        SORAFS_ORDERBOOK_TRADES_GET => app_sdk_get("contracts.sorafs_orderbook_trades_get", "/v1/sorafs/orderbook/trades");
        SORAFS_ORDERBOOK_CHANNELS_GET => app_sdk_get("contracts.sorafs_orderbook_channels_get", "/v1/sorafs/orderbook/channels");
        SORAFS_ORDERBOOK_EVENTS_GET => app_sdk_get("contracts.sorafs_orderbook_events_get", "/v1/sorafs/orderbook/events");
        SORAFS_ORDERBOOK_EVENTS_STREAM_GET => app_unprojected_protocol_get("contracts.sorafs_orderbook_events_stream_get", "/v1/sorafs/orderbook/events/stream");
        SORAFS_ORDERBOOK_EVENTS_WS_GET => app_unprojected_protocol_get("contracts.sorafs_orderbook_events_ws_get", "/v1/sorafs/orderbook/events/ws");
        SORAFS_RESERVE_POLICY_GET => app_signed_sdk_get("contracts.sorafs_reserve_policy_get", "/v1/sorafs/reserve/policy");
        SORAFS_RESERVE_PROVIDERS_GET => app_signed_sdk_get("contracts.sorafs_reserve_providers_get", "/v1/sorafs/reserve/providers");
        SORAFS_RESERVE_PROVIDERS_BY_PROVIDER_ID_HEX_GET => app_signed_sdk_get("contracts.sorafs_reserve_providers_by_provider_id_hex_get", "/v1/sorafs/reserve/providers/{provider_id_hex}");
        SORAFS_RESERVE_TOP_UP_POST => app_sdk_post("contracts.sorafs_reserve_top_up_post", "/v1/sorafs/reserve/top-up");
        SORAFS_RESERVE_WITHDRAW_POST => app_sdk_post("contracts.sorafs_reserve_withdraw_post", "/v1/sorafs/reserve/withdraw");
        SORAFS_RESERVE_MOVEMENTS_GET => app_signed_sdk_get("contracts.sorafs_reserve_movements_get", "/v1/sorafs/reserve/movements");
        SORAFS_RESERVE_MOVEMENTS_BY_MOVEMENT_ID_HEX_GET => app_signed_sdk_get("contracts.sorafs_reserve_movements_by_movement_id_hex_get", "/v1/sorafs/reserve/movements/{movement_id_hex}");
        SORAFS_RESERVE_MOVEMENTS_BY_MOVEMENT_ID_HEX_DECISION_POST => app_sdk_post("contracts.sorafs_reserve_movements_by_movement_id_hex_decision_post", "/v1/sorafs/reserve/movements/{movement_id_hex}/decision");
        SORAFS_RESERVE_CREDIT_DRAW_POST => app_sdk_post("contracts.sorafs_reserve_credit_draw_post", "/v1/sorafs/reserve/credit/draw");
        SORAFS_RESERVE_CREDIT_REPAY_POST => app_sdk_post("contracts.sorafs_reserve_credit_repay_post", "/v1/sorafs/reserve/credit/repay");
        SORAFS_RESERVE_APPEALS_POST => app_sdk_post("contracts.sorafs_reserve_appeals_post", "/v1/sorafs/reserve/appeals");
        SORAFS_RESERVE_APPEALS_GET => app_signed_sdk_get("contracts.sorafs_reserve_appeals_get", "/v1/sorafs/reserve/appeals");
        SORAFS_RESERVE_APPEALS_BY_APPEAL_ID_HEX_GET => app_signed_sdk_get("contracts.sorafs_reserve_appeals_by_appeal_id_hex_get", "/v1/sorafs/reserve/appeals/{appeal_id_hex}");
        SORAFS_RESERVE_APPEALS_BY_APPEAL_ID_HEX_DECISION_POST => app_sdk_post("contracts.sorafs_reserve_appeals_by_appeal_id_hex_decision_post", "/v1/sorafs/reserve/appeals/{appeal_id_hex}/decision");
        SORAFS_RESERVE_EVENTS_GET => app_signed_sdk_get("contracts.sorafs_reserve_events_get", "/v1/sorafs/reserve/events");
        SORAFS_RESERVE_EVENTS_STREAM_GET => app_unprojected_protocol_get("contracts.sorafs_reserve_events_stream_get", "/v1/sorafs/reserve/events/stream");
        SORAFS_RESERVE_EVENTS_WS_GET => app_unprojected_protocol_get("contracts.sorafs_reserve_events_ws_get", "/v1/sorafs/reserve/events/ws");
        SORAFS_GATEWAY_COMPLIANCE_FEEDS_BY_FEED_ID_GET => app_signed_get("contracts.sorafs_gateway_compliance_feeds_by_feed_id_get", "/v1/sorafs/gateway/compliance/feeds/{feed_id}");
        SORAFS_GATEWAY_COMPLIANCE_STATUS_GET => app_signed_get("contracts.sorafs_gateway_compliance_status_get", "/v1/sorafs/gateway/compliance/status");
        SORAFS_GATEWAY_COMPLIANCE_STAGE_POST => app_signed_post("contracts.sorafs_gateway_compliance_stage_post", "/v1/sorafs/gateway/compliance/stage");
        SORAFS_GATEWAY_COMPLIANCE_ACKNOWLEDGE_POST => app_signed_post("contracts.sorafs_gateway_compliance_acknowledge_post", "/v1/sorafs/gateway/compliance/acknowledge");
        SORAFS_GATEWAY_COMPLIANCE_PROMOTE_POST => app_signed_post("contracts.sorafs_gateway_compliance_promote_post", "/v1/sorafs/gateway/compliance/promote");
        SORAFS_GATEWAY_COMPLIANCE_ROLLBACK_POST => app_signed_post("contracts.sorafs_gateway_compliance_rollback_post", "/v1/sorafs/gateway/compliance/rollback");
        SORAFS_APPEALS_PRICING_CONFIG_GET => app_get("contracts.sorafs_appeals_pricing_config_get", "/v1/sorafs/appeals/pricing/config");
        SORAFS_APPEALS_PRICING_STATUS_GET => app_get("contracts.sorafs_appeals_pricing_status_get", "/v1/sorafs/appeals/pricing/status");
        SORAFS_APPEALS_PRICING_QUOTE_POST => app_post("contracts.sorafs_appeals_pricing_quote_post", "/v1/sorafs/appeals/pricing/quote");
        SORAFS_APPEALS_FINANCE_SETTLE_POST => app_post("contracts.sorafs_appeals_finance_settle_post", "/v1/sorafs/appeals/finance/settle");
        SORAFS_APPEALS_FINANCE_DISBURSE_POST => app_post("contracts.sorafs_appeals_finance_disburse_post", "/v1/sorafs/appeals/finance/disburse");
        SORAFS_APPEALS_FINANCE_DEPOSITS_POST => app_post("contracts.sorafs_appeals_finance_deposits_post", "/v1/sorafs/appeals/finance/deposits");
        SORAFS_APPEALS_FINANCE_DEPOSITS_CONFIRM_POST => app_post("contracts.sorafs_appeals_finance_deposits_confirm_post", "/v1/sorafs/appeals/finance/deposits/confirm");
        SORAFS_APPEALS_FINANCE_DEPOSITS_SETTLE_POST => app_post("contracts.sorafs_appeals_finance_deposits_settle_post", "/v1/sorafs/appeals/finance/deposits/settle");
        SORAFS_APPEALS_FINANCE_DEPOSITS_SUBMIT_SETTLEMENT_POST => app_post("contracts.sorafs_appeals_finance_deposits_submit_settlement_post", "/v1/sorafs/appeals/finance/deposits/submit-settlement");
        SORAFS_APPEALS_FINANCE_DEPOSITS_RECONCILE_POST => app_post("contracts.sorafs_appeals_finance_deposits_reconcile_post", "/v1/sorafs/appeals/finance/deposits/reconcile");
        SORAFS_APPEALS_FINANCE_DEPOSITS_BY_ESCROW_ID_HEX_GET => app_get("contracts.sorafs_appeals_finance_deposits_by_escrow_id_hex_get", "/v1/sorafs/appeals/finance/deposits/{escrow_id_hex}");
        SORAFS_MODERATION_BALLOTS_POST => app_post("contracts.sorafs_moderation_ballots_post", "/v1/sorafs/moderation/ballots");
        SORAFS_MODERATION_BALLOTS_GET => app_get("contracts.sorafs_moderation_ballots_get", "/v1/sorafs/moderation/ballots");
        SORAFS_MODERATION_BALLOTS_BY_CASE_ID_BY_ROUND_ID_GET => app_get("contracts.sorafs_moderation_ballots_by_case_id_by_round_id_get", "/v1/sorafs/moderation/ballots/{case_id}/{round_id}");
        SORAFS_MODERATION_BALLOTS_BY_CASE_ID_BY_ROUND_ID_NO_SHOW_PLAN_GET => app_get("contracts.sorafs_moderation_ballots_by_case_id_by_round_id_no_show_plan_get", "/v1/sorafs/moderation/ballots/{case_id}/{round_id}/no-show-plan");
        SORAFS_MODERATION_BALLOTS_ELIGIBILITY_POST => app_post("contracts.sorafs_moderation_ballots_eligibility_post", "/v1/sorafs/moderation/ballots/eligibility");
        SORAFS_MODERATION_BALLOTS_SORTITION_POST => app_post("contracts.sorafs_moderation_ballots_sortition_post", "/v1/sorafs/moderation/ballots/sortition");
        SORAFS_MODERATION_BALLOTS_ASSIGNMENTS_ACCEPT_POST => app_post("contracts.sorafs_moderation_ballots_assignments_accept_post", "/v1/sorafs/moderation/ballots/assignments/accept");
        SORAFS_MODERATION_BALLOTS_ACTIVATE_POST => app_post("contracts.sorafs_moderation_ballots_activate_post", "/v1/sorafs/moderation/ballots/activate");
        SORAFS_MODERATION_BALLOTS_COMMITS_POST => app_post("contracts.sorafs_moderation_ballots_commits_post", "/v1/sorafs/moderation/ballots/commits");
        SORAFS_MODERATION_BALLOTS_CHALLENGES_POST => app_post("contracts.sorafs_moderation_ballots_challenges_post", "/v1/sorafs/moderation/ballots/challenges");
        SORAFS_MODERATION_BALLOTS_CHALLENGES_RESOLVE_POST => app_post("contracts.sorafs_moderation_ballots_challenges_resolve_post", "/v1/sorafs/moderation/ballots/challenges/resolve");
        SORAFS_MODERATION_BALLOTS_REVEALS_POST => app_post("contracts.sorafs_moderation_ballots_reveals_post", "/v1/sorafs/moderation/ballots/reveals");
        SORAFS_MODERATION_BALLOTS_TALLY_POST => app_post("contracts.sorafs_moderation_ballots_tally_post", "/v1/sorafs/moderation/ballots/tally");
        SORAFS_MODERATION_BALLOTS_EVENTS_GET => app_get("contracts.sorafs_moderation_ballots_events_get", "/v1/sorafs/moderation/ballots/events");
        SORAFS_MODERATION_MODEL_REGISTRY_GET => app_get("contracts.sorafs_moderation_model_registry_get", "/v1/sorafs/moderation/model-registry");
        SORAFS_MODERATION_MODEL_REGISTRY_REPRO_MANIFESTS_POST => app_post("contracts.sorafs_moderation_model_registry_repro_manifests_post", "/v1/sorafs/moderation/model-registry/repro-manifests");
        SORAFS_MODERATION_MODEL_REGISTRY_CORPORA_POST => app_post("contracts.sorafs_moderation_model_registry_corpora_post", "/v1/sorafs/moderation/model-registry/corpora");
        SORAFS_MODERATION_SCREENING_RESULTS_POST => app_post("contracts.sorafs_moderation_screening_results_post", "/v1/sorafs/moderation/screening-results");
        SORAFS_MODERATION_SCREENING_RESULTS_GET => app_get("contracts.sorafs_moderation_screening_results_get", "/v1/sorafs/moderation/screening-results");
        SORAFS_MODERATION_QUARANTINE_GET => app_get("contracts.sorafs_moderation_quarantine_get", "/v1/sorafs/moderation/quarantine");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_REVIEW_POST => app_post("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_review_post", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_RELEASE_POST => app_post("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_release_post", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/release");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_APPEAL_HANDOFF_POST => app_post("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_appeal_handoff_post", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_OPERATOR_PANEL_GET => app_get("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_operator_panel_get", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_OBJECT_POST => app_post("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_object_post", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/object");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_OBJECT_GET => app_get("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_object_get", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/object");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_VIEWER_SESSIONS_POST => app_post("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_viewer_sessions_post", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-sessions");
        SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_VIEWER_ACCESS_POST => app_post("contracts.sorafs_moderation_quarantine_by_quarantine_id_hex_viewer_access_post", "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-access");
        SORAFS_MODERATION_VIEWER_AUDIT_REPORTS_POST => app_post("contracts.sorafs_moderation_viewer_audit_reports_post", "/v1/sorafs/moderation/viewer-audit-reports");
        SORAFS_MODERATION_VIEWER_AUDIT_REPORTS_PUBLISH_DUE_POST => app_post("contracts.sorafs_moderation_viewer_audit_reports_publish_due_post", "/v1/sorafs/moderation/viewer-audit-reports/publish-due");
        SORAFS_AUDIT_REPAIR_REPORT_POST => app_post("contracts.sorafs_audit_repair_report_post", "/v1/sorafs/audit/repair/report");
        SORAFS_AUDIT_REPAIR_SLASH_POST => app_post("contracts.sorafs_audit_repair_slash_post", "/v1/sorafs/audit/repair/slash");
        SORAFS_AUDIT_REPAIR_CLAIM_POST => app_post("contracts.sorafs_audit_repair_claim_post", "/v1/sorafs/audit/repair/claim");
        SORAFS_AUDIT_REPAIR_HEARTBEAT_POST => app_post("contracts.sorafs_audit_repair_heartbeat_post", "/v1/sorafs/audit/repair/heartbeat");
        SORAFS_AUDIT_REPAIR_COMPLETE_POST => app_post("contracts.sorafs_audit_repair_complete_post", "/v1/sorafs/audit/repair/complete");
        SORAFS_AUDIT_REPAIR_FAIL_POST => app_post("contracts.sorafs_audit_repair_fail_post", "/v1/sorafs/audit/repair/fail");
        SORAFS_AUDIT_REPAIR_APPEAL_POST => app_post("contracts.sorafs_audit_repair_appeal_post", "/v1/sorafs/audit/repair/appeal");
        SORAFS_AUDIT_REPAIR_STATUS_GET => app_get("contracts.sorafs_audit_repair_status_get", "/v1/sorafs/audit/repair/status");
        SORAFS_AUDIT_REPAIR_TASKS_GET => app_get("contracts.sorafs_audit_repair_tasks_get", "/v1/sorafs/audit/repair/tasks");
        SORAFS_AUDIT_REPAIR_TASKS_BY_TICKET_ID_GET => app_get("contracts.sorafs_audit_repair_tasks_by_ticket_id_get", "/v1/sorafs/audit/repair/tasks/{ticket_id}");
        SORAFS_AUDIT_REPAIR_EVENTS_GET => app_get("contracts.sorafs_audit_repair_events_get", "/v1/sorafs/audit/repair/events");
        ZK_VK_BY_BACKEND_BY_NAME_GET => app_get("contracts.zk_vk_by_backend_by_name_get", "/v1/zk/vk/{backend}/{name}");
        ZK_VK_GET => app_get("contracts.zk_vk_get", "/v1/zk/vk");
        ZK_PROOFS_GET => app_get("contracts.zk_proofs_get", "/v1/zk/proofs");
        ZK_PROOFS_COUNT_GET => app_get("contracts.zk_proofs_count_get", "/v1/zk/proofs/count");
        ZK_PROOF_BY_BACKEND_BY_HASH_GET => app_get("contracts.zk_proof_by_backend_by_hash_get", "/v1/zk/proof/{backend}/{hash}");
        CONTRACTS_CODE_BY_CODE_HASH_GET => app_get("contracts.contracts_code_by_code_hash_get", "/v1/contracts/code/{code_hash}");
        CONTRACTS_CODE_BY_CODE_HASH_CONTRACT_VIEW_GET => app_sdk_get("contracts.contracts_code_by_code_hash_contract_view_get", "/v1/contracts/code/{code_hash}/contract-view");
        CONTRACTS_CODE_BY_CODE_HASH_VERIFIED_SOURCE_JOBS_POST => app_sdk_post("contracts.contracts_code_by_code_hash_verified_source_jobs_post", "/v1/contracts/code/{code_hash}/verified-source/jobs");
        CONTRACTS_CODE_BY_CODE_HASH_VERIFIED_SOURCE_JOBS_BY_JOB_ID_GET => app_sdk_get("contracts.contracts_code_by_code_hash_verified_source_jobs_by_job_id_get", "/v1/contracts/code/{code_hash}/verified-source-jobs/{job_id}");
    }
}

/// Protocol-native `SoraCloud` public gateway routes.
pub mod soracloud_gateway {
    use super::{
        ApiSurface, AuthenticationPolicy, FeatureGate, HttpMethod, Listener, PathPolicy,
        RouteDescriptor, RouteMatch,
    };

    /// Resolve a `SoraDNS` name to the root of its active public runtime.
    pub const SORADNS_ROOT: RouteDescriptor = RouteDescriptor::new(
        "protocol.soracloud.soradns_root",
        HttpMethod::Any,
        "/soradns/{fqdn}",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "public SoraDNS virtual-host gateway",
    });
    /// Forward a path under a `SoraDNS` public runtime.
    pub const SORADNS_PATH: RouteDescriptor = RouteDescriptor::new(
        "protocol.soracloud.soradns_path",
        HttpMethod::Any,
        "/soradns/{fqdn}/{*path}",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_route_match(RouteMatch::Wildcard)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "public SoraDNS virtual-host gateway wildcard",
    });
    /// Forward the root path for a local `SoraCloud` public runtime.
    pub const LOCAL_ROOT: RouteDescriptor = RouteDescriptor::new(
        "protocol.soracloud.local_root",
        HttpMethod::Any,
        "/api",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "public SoraCloud runtime gateway",
    });
    /// Forward a path under a local `SoraCloud` public runtime.
    pub const LOCAL_PATH: RouteDescriptor = RouteDescriptor::new(
        "protocol.soracloud.local_path",
        HttpMethod::Any,
        "/api/{*tail}",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_authentication(AuthenticationPolicy::ProtocolHandshake)
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_route_match(RouteMatch::Wildcard)
    .with_path_policy(PathPolicy::ProtocolException {
        reason: "public SoraCloud runtime gateway wildcard",
    });

    /// Canonical public-runtime gateway route set.
    pub const ROUTES: &[RouteDescriptor] = &[SORADNS_ROOT, SORADNS_PATH, LOCAL_ROOT, LOCAL_PATH];
}

/// Raw content and `SoraDNS` directory routes.
pub mod content_directory {
    use super::{
        ApiSurface, FeatureGate, HttpMethod, Listener, RouteDescriptor, RouteMatch,
        RouteProjections,
    };

    /// Read one path from a registered content bundle.
    pub const CONTENT: RouteDescriptor = RouteDescriptor::new(
        "protocol.content.read",
        HttpMethod::Get,
        "/v1/content/{bundle}/{*path}",
        ApiSurface::Protocol,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI)
    .with_route_match(RouteMatch::Wildcard)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read the latest signed `SoraDNS` directory snapshot.
    pub const SORADNS_LATEST: RouteDescriptor = RouteDescriptor::new(
        "soradns.directory.latest",
        HttpMethod::Get,
        "/v1/soradns/directory/latest",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Read the bounded `SoraDNS` directory event snapshot.
    pub const SORADNS_EVENTS: RouteDescriptor = RouteDescriptor::new(
        "soradns.directory.events",
        HttpMethod::Get,
        "/v1/soradns/directory/events",
        ApiSurface::Public,
        Listener::Torii,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI)
    .with_implicit_head(true)
    .with_cors_options(true);

    /// Canonical raw-content and directory route set.
    pub const ROUTES: &[RouteDescriptor] = &[CONTENT, SORADNS_LATEST, SORADNS_EVENTS];
}

/// Canonical descriptors enforced by Torii's mounted-route registry.
///
/// Router assembly fails when any enabled descriptor is missing or when a
/// registration does not match this catalog exactly.
pub const CATALOGED_ROUTES: &[RouteDescriptor] = &[
    aliases::SETUP_PLAN,
    aliases::LEASE_RENEW_PLAN,
    aliases::AUTO_RENEW_PLAN,
    aliases::RESOLVE,
    aliases::RESOLVE_INDEX,
    aliases::BY_ACCOUNT,
    aliases::RETAIL_RECIPIENT_LOOKUP,
    aliases::RETAIL_RECIPIENT_ROUTE,
    aliases::ASSET_RESOLVE,
    fees::QUOTE,
    fees::SPONSOR_PROGRAM_BY_ID,
    operator_authentication::REGISTRATION_OPTIONS,
    operator_authentication::REGISTRATION_VERIFY,
    operator_authentication::LOGIN_OPTIONS,
    operator_authentication::LOGIN_VERIFY,
    governance_vrf::DERIVE_COUNCIL,
    core::API_VERSION,
    core::PEERS,
    core::HEALTH,
    core::CONFIGURATION_GET,
    core::CONFIGURATION_POST,
    core::NEXUS_LIFECYCLE_GET,
    core::LEDGER_HEADERS,
    core::LEDGER_STATE_ROOT,
    core::LEDGER_STATE_PROOF,
    core::LEDGER_BLOCK_PROOF,
    core::INTERNAL_PROXY,
    core::VPN_PROFILE,
    core::VPN_QUOTE_CREATE,
    core::VPN_SESSION_CREATE,
    core::VPN_RECEIPTS,
    core::VPN_RECEIPT_SUBMIT,
    core::VPN_SESSION,
    core::VPN_SESSION_DELETE,
    core::TIME_NOW,
    core::TIME_STATUS,
    diagnostic::STATUS,
    diagnostic::STATUS_TAIL,
    diagnostic::METRICS,
    diagnostic::PROFILE,
    diagnostic::SCHEMA,
    diagnostic::OPENAPI_JSON,
    diagnostic::OPENAPI,
    pipeline::TRANSACTION,
    pipeline::TRANSACTION_ENTRYPOINT,
    pipeline::TRANSACTIONS_BATCH,
    pipeline::QUERY,
    pipeline::PROOF,
    pipeline::PROOF_RETENTION,
    pipeline::TRANSACTION_STATUS,
    pipeline::PREFLIGHT,
    pipeline::TRIGGER_COMPLETIONS,
    pipeline::RECOVERY,
    pipeline::RECOVERY_FASTPQ_PROOFS,
    pipeline::POLICY,
    iso20022::PACS008_SUBMIT,
    iso20022::PACS009_SUBMIT,
    iso20022::PACS002_SUBMIT,
    iso20022::PACS004_SUBMIT,
    iso20022::CAMT056_SUBMIT,
    iso20022::SESE023_SUBMIT,
    iso20022::SESE024_SUBMIT,
    iso20022::SESE025_SUBMIT,
    iso20022::COLR012_SUBMIT,
    iso20022::MESSAGE,
    iso20022::AUDIT_MESSAGES,
    iso20022::MESSAGE_PACS002,
    iso20022::MESSAGE_PACS004,
    iso20022::MESSAGE_CAMT029,
    iso20022::MESSAGE_SESE024,
    iso20022::MESSAGE_SESE025,
    data_availability::INGEST,
    data_availability::MANIFEST,
    data_availability::PROOF_POLICIES,
    data_availability::PROOF_POLICY_SNAPSHOT,
    data_availability::COMMITMENTS,
    data_availability::COMMITMENTS_PROVE,
    data_availability::COMMITMENTS_VERIFY,
    data_availability::PIN_INTENTS,
    data_availability::PIN_INTENTS_PROVE,
    data_availability::PIN_INTENTS_VERIFY,
    musubi::PACKAGES,
    musubi::RELEASE,
    musubi::RELEASES,
    musubi::VERSIONS,
    musubi::ALIAS,
    musubi::PUBLISH_RELEASE,
    musubi::YANK_RELEASE,
    musubi::SET_ALIAS,
    musubi::ASSERT_RELEASE_EXISTS,
    streaming::P2P,
    streaming::EVENTS_SSE,
    streaming::CONTRACT_EVENTS_SSE,
    streaming::SUBSCRIPTION_WS,
    streaming::BLOCKS_WS,
    mcp_transport::CAPABILITIES,
    mcp_transport::JSON_RPC,
    connect::SESSION_CREATE,
    connect::SESSION_DELETE,
    connect::WEBSOCKET,
    connect::STATUS,
    telemetry::PACEMAKER,
    telemetry::PHASES,
    telemetry::DEBUG_AXT_CACHE,
    telemetry::DEBUG_WITNESS,
    telemetry::SORANET_PRIVACY_EVENT,
    telemetry::SORANET_PRIVACY_SHARE,
    telemetry::ASSET_HOLDERS,
    telemetry::ASSET_HOLDERS_QUERY,
    sumeragi::EVIDENCE_COUNT,
    sumeragi::EVIDENCE_LIST,
    sumeragi::SCCP_MESSAGE_PROOF,
    sumeragi::SCCP_PROOF_REQUEST,
    sumeragi::SCCP_MESSAGES_RECENT,
    sumeragi::SCCP_CAPABILITIES,
    sumeragi::SCCP_REGISTRY,
    sumeragi::SCCP_SORA_OUTBOUND_MATERIAL,
    sumeragi::VRF_PENALTIES,
    sumeragi::VRF_EPOCH,
    sumeragi::STATUS,
    sumeragi::DIAGNOSTICS,
    sumeragi::STATUS_SSE,
    sumeragi::LEADER,
    sumeragi::BLS_KEYS,
    sumeragi::QC,
    sumeragi::CHECKPOINTS,
    sumeragi::COMMIT_CERTIFICATES,
    sumeragi::BRIDGE_FINALITY,
    sumeragi::BRIDGE_FINALITY_ATTESTATION,
    sumeragi::BRIDGE_FINALITY_BUNDLE,
    sumeragi::VALIDATOR_SETS,
    sumeragi::VALIDATOR_SET_BY_HEIGHT,
    sumeragi::CONSENSUS_KEYS,
    sumeragi::KEY_LIFECYCLE,
    sumeragi::TELEMETRY,
    sumeragi::PARAMETERS,
    sumeragi::COMMIT_QC,
    sumeragi::EVIDENCE_SUBMIT,
    sumeragi::VRF_COMMIT,
    sumeragi::VRF_REVEAL,
    runtime_governance::ZK_ROOTS,
    runtime_governance::ZK_MERKLE_PATH,
    runtime_governance::ZK_VERIFY,
    runtime_governance::ZK_SUBMIT_PROOF,
    runtime_governance::ZK_VOTE_TALLY,
    runtime_governance::ZK_IVM_DERIVE,
    runtime_governance::ZK_IVM_PROVE,
    runtime_governance::ZK_IVM_PROVE_GET,
    runtime_governance::ZK_IVM_PROVE_DELETE,
    runtime_governance::ZK_VERIFY_BATCH,
    runtime_governance::ZK_ATTACHMENTS_GET,
    runtime_governance::ZK_ATTACHMENTS_POST,
    runtime_governance::ZK_ATTACHMENT_GET,
    runtime_governance::ZK_ATTACHMENT_DELETE,
    runtime_governance::ZK_ATTACHMENTS_COUNT,
    runtime_governance::RUNTIME_ABI_ACTIVE,
    runtime_governance::RUNTIME_ABI_HASH,
    runtime_governance::RUNTIME_METRICS,
    runtime_governance::NODE_CAPABILITIES,
    runtime_governance::NODE_PROJECTION_CHECKPOINT,
    runtime_governance::NODE_PROJECTION_CHECKPOINT_PLAN,
    runtime_governance::NODE_PROJECTION_CHECKPOINT_PUBLISH,
    runtime_governance::NODE_PROJECTION_SHARD_CATALOG,
    runtime_governance::NODE_PROJECTION_SHARD_EXPORT,
    runtime_governance::RUNTIME_UPGRADES,
    runtime_governance::RUNTIME_UPGRADE_PROPOSE,
    runtime_governance::RUNTIME_UPGRADE_ACTIVATE,
    runtime_governance::RUNTIME_UPGRADE_CANCEL,
    runtime_governance::MINISTRY_AGENDA_DRAFT,
    runtime_governance::MINISTRY_AGENDA_GET,
    runtime_governance::GOV_PROPOSE_DEPLOY,
    runtime_governance::GOV_PROPOSE_SCCP,
    runtime_governance::GOV_CAPABILITIES,
    runtime_governance::GOV_CITIZEN_DRAFT,
    runtime_governance::VALIDATION_FEE_CURRENT_POLICY_PROOF,
    runtime_governance::VALIDATION_FEE_PROPOSALS,
    runtime_governance::VALIDATION_FEE_PROPOSAL_DETAIL,
    runtime_governance::VALIDATION_FEE_PROPOSAL_DRAFT,
    runtime_governance::GOV_PROPOSAL_GET,
    runtime_governance::GOV_LOCKS_GET,
    runtime_governance::GOV_REFERENDUM_GET,
    runtime_governance::GOV_TALLY_GET,
    runtime_governance::GOV_BALLOT_ZK,
    runtime_governance::GOV_BALLOT_ZK_V1,
    runtime_governance::GOV_BALLOT_ZK_V1_PROOF,
    runtime_governance::GOV_BALLOT_PLAIN,
    runtime_governance::GOV_PARLIAMENT_BALLOT,
    runtime_governance::GOV_FINALIZE,
    runtime_governance::GOV_PROTECTED_POST,
    runtime_governance::GOV_PROTECTED_GET,
    runtime_governance::GOV_STREAM,
    runtime_governance::GOV_UNLOCK_STATS,
    runtime_governance::GOV_CONTRACT_GET,
    runtime_governance::GOV_ENACT,
    runtime_governance::GOV_COUNCIL_CURRENT,
    runtime_governance::GOV_CITIZENS_COUNT,
    runtime_governance::GOV_CITIZEN_STATUS,
    runtime_governance::GOV_COUNCIL_AUDIT,
    runtime_governance::GOV_COUNCIL_PERSIST,
    runtime_governance::GOV_COUNCIL_REPLACE,
    sorafs::STORAGE_PEERS,
    sorafs::PROVIDERS,
    sorafs::PROVIDER_ADVERT,
    sorafs::ROUTING_PROVIDERS,
    sorafs::ROUTING_PEERS,
    sorafs::CAPACITY_STATE,
    sorafs::GOVERNANCE_DAG_DASHBOARD,
    sorafs::GOVERNANCE_DAG_HEAD,
    sorafs::GOVERNANCE_DAG_BLOCK,
    sorafs::GOVERNANCE_DAG_NODE,
    sorafs::GOVERNANCE_DAG_PUBLISH_INDEX,
    sorafs::GOVERNANCE_DAG_PUBLISH_DIGEST,
    sorafs::GOVERNANCE_DAG_PUBLISH_KIND,
    sorafs::TRANSPARENCY_CYCLES,
    sorafs::TRANSPARENCY_CYCLE,
    sorafs::TRANSPARENCY_CYCLE_ENTRY,
    sorafs::TRANSPARENCY_EXPLORER,
    sorafs::TRANSPARENCY_EXPLORER_UI,
    sorafs::TRANSPARENCY_SOURCE_ENTRY,
    sorafs::TRANSPARENCY_PRIVACY_SOURCE_EVENT,
    sorafs::TRANSPARENCY_PRIVACY_PUBLISH_DUE,
    sorafs::TRANSPARENCY_TOKENS,
    sorafs::TRANSPARENCY_TOKEN_ISSUANCE,
    sorafs::TRANSPARENCY_TOKEN_VERIFY,
    sorafs::APPEAL_FINANCE_REPORTS_GET,
    sorafs::APPEAL_FINANCE_REPORTS_POST,
    sorafs::APPEAL_FINANCE_WEEKLY_ROLLUPS_GET,
    sorafs::APPEAL_FINANCE_WEEKLY_ROLLUPS_POST,
    sorafs::APPEAL_FINANCE_SETTLEMENT_RECEIPTS,
    sorafs::GOVERNANCE_DAG_CAR_QUEUE,
    sorafs::GOVERNANCE_DAG_CAR_QUEUE_DIGEST,
    sorafs::GOVERNANCE_DAG_CAR_QUEUE_KIND,
    sorafs::GOVERNANCE_DAG_CAR_QUEUE_ARCHIVE,
    sorafs::GOVERNANCE_DAG_RUNTIME,
    sorafs::GOVERNANCE_DAG_RUNTIME_HEAD,
    sorafs::GOVERNANCE_DAG_RUNTIME_BLOCK,
    sorafs::GOVERNANCE_DAG_RUNTIME_NODE,
    sorafs::GOVERNANCE_DAG_RUNTIME_DIGEST,
    sorafs::GOVERNANCE_DAG_RUNTIME_KIND,
    sorafs::REPUTATION_LATEST_GET,
    sorafs::REPUTATION_LATEST_POST,
    sorafs::REPUTATION_SNAPSHOT,
    sorafs::REPUTATION_PROVIDER,
    sorafs::REPUTATION_WEIGHTS,
    sorafs::REPUTATION_EVENTS,
    sorafs::REPUTATION_EVENTS_STREAM,
    sorafs::REPUTATION_EVENTS_WEBSOCKET,
    sorafs::PIN_REGISTRY,
    sorafs::PIN_MANIFEST,
    sorafs::PIN_REGISTER,
    sorafs::ALIASES,
    sorafs::REPLICATION,
    sorafs::STORAGE_STATE,
    sorafs::CID_LOOKUP,
    sorafs::DENYLIST_CATALOG,
    sorafs::DENYLIST_PACK,
    sorafs::STORAGE_MANIFEST,
    sorafs::STORAGE_PLAN,
    sorafs::STORAGE_PIN,
    sorafs::STORAGE_FETCH,
    sorafs::STORAGE_TOKEN,
    sorafs::STORAGE_CAR,
    sorafs::STORAGE_CHUNK,
    sorafs::PROOF_STREAM,
    sorafs::PDP_CHALLENGE,
    sorafs::PDP_NEXT,
    sorafs::PDP_PROOF,
    sorafs::PDP_STATUS,
    sorafs::PDP_EXPORT,
    sorafs::POP_ENROLLMENT,
    sorafs::POP_ENROLLMENT_STATUS,
    sorafs::POP_APPROVAL,
    sorafs::POP_ISSUE,
    sorafs::POP_REVOCATION,
    sorafs::POP_REGISTRY_SUBMIT,
    sorafs::POP_REGISTRY_RECONCILE,
    sorafs::POP_REGISTRY_PROJECTION,
    sorafs::POP_WALLET_DELIVERY,
    sorafs::POP_WALLET_IMPORT,
    sorafs::POP_WALLET_ACKNOWLEDGE,
    sorafs::POP_WALLET_SYNCHRONIZE,
    sorafs::POP_WALLET_PROVE,
    sorafs::POP_VERIFY,
    sorafs::DEAL_FUND_PROVIDER,
    sorafs::DEAL_FUND_CLIENT,
    sorafs::DEAL_OPEN,
    sorafs::DEAL_CANCEL,
    sorafs::DEAL_USAGE,
    sorafs::DEAL_SETTLE,
    sorafs::ECONOMICS_PRICING_MANIFEST,
    sorafs::ECONOMICS_HEDGING_FEED,
    sorafs::ECONOMICS_STATUS,
    sorafs::ECONOMICS_ACTIVE_PRICING,
    sorafs::ECONOMICS_HEDGING_REFERENCE,
    sorafs::SITE_MANIFEST,
    sorafs::CID_ROOT,
    sorafs::CID_PATH,
    application_api::APP_API_BINDINGS_GET,
    application_api::APP_API_CID_BY_CID_GET,
    application_api::APP_API_CID_BY_CID_BY_PATH_GET,
    application_api::APP_API_CID_BY_CID_BY_PATH_POST,
    application_api::APP_API_ACTIVE_BY_PATH_GET,
    application_api::APP_API_ACTIVE_BY_PATH_POST,
    application_api::API_CID_BY_CID_GET,
    application_api::API_CID_BY_CID_BY_PATH_GET,
    application_api::API_CID_BY_CID_BY_PATH_POST,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_GET,
    application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_GET,
    application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_BY_ENTRYPOINT_HASH_GET,
    application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_ASSETS_BY_ASSET_DEFINITION_ID_GET,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_QUERY_POST,
    application_api::TRANSACTIONS_HISTORY_GET,
    application_api::CONTRACTS_ACTIVITY_GET,
    application_api::CONTRACTS_EVENTS_GET,
    application_api::CONTRACTS_ROLLUPS_SWAPS_FILLS_GET,
    application_api::CONTRACTS_ROLLUPS_SWAPS_CANDLES_GET,
    application_api::CONTRACTS_ROLLUPS_URANAI_MARKETS_HISTORY_GET,
    application_api::CONTRACTS_ROLLUPS_TRADER_ACTIVITY_GET,
    application_api::CONTRACTS_ROLLUPS_TRADER_ACCOUNT_GET,
    application_api::CONTRACTS_ROLLUPS_INTENTS_GET,
    application_api::CONTRACTS_ROLLUPS_VAULTS_POSITIONS_GET,
    application_api::CONTRACTS_ROLLUPS_OPERATORS_STATUS_GET,
    application_api::CONTRACTS_ROLLUPS_MARGIN_HEALTH_GET,
    application_api::CONTRACTS_ROLLUPS_RWA_LOTS_GET,
    application_api::CONTRACTS_ROLLUPS_DLMM_HOOKS_GET,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_ASSETS_GET,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_ASSETS_QUERY_POST,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_PERMISSIONS_GET,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_GET,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_HISTORY_GET,
    application_api::PROOFS_QUERY_POST,
    application_api::ZK_PROOF_TAGS_BY_BACKEND_BY_HASH_GET,
    application_api::DOMAINS_GET,
    application_api::DOMAINS_QUERY_POST,
    application_api::ACCOUNTS_GET,
    application_api::ACCOUNTS_QUERY_POST,
    application_api::TRANSACTIONS_QUERY_POST,
    application_api::TRANSACTIONS_VISIBLE_QUERY_POST,
    application_api::ACCOUNTS_ONBOARD_PLAN_POST,
    application_api::ACCOUNTS_ONBOARD_POST,
    application_api::ACCOUNTS_ONBOARDING_READINESS_GET,
    application_api::ACCOUNTS_FAUCET_PUZZLE_GET,
    application_api::ACCOUNTS_FAUCET_POST,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_ALIASES_GET,
    application_api::ACCOUNTS_BY_UAID_PORTFOLIO_GET,
    application_api::NEXUS_PUBLIC_LANES_BY_LANE_ID_VALIDATORS_GET,
    application_api::NEXUS_PUBLIC_LANES_BY_LANE_ID_STAKE_GET,
    application_api::NEXUS_PUBLIC_LANES_BY_LANE_ID_REWARDS_PENDING_GET,
    application_api::NEXUS_DATASPACES_ACCOUNTS_BY_LITERAL_SUMMARY_GET,
    application_api::SPACE_DIRECTORY_UAIDS_BY_UAID_GET,
    application_api::SPACE_DIRECTORY_UAIDS_BY_UAID_MANIFESTS_GET,
    application_api::SPACE_DIRECTORY_MANIFESTS_POST,
    application_api::SPACE_DIRECTORY_MANIFESTS_REVOKE_POST,
    application_api::RAM_LFE_PROGRAM_POLICIES_GET,
    application_api::RAM_LFE_PROGRAMS_BY_PROGRAM_ID_EXECUTE_POST,
    application_api::RAM_LFE_RECEIPTS_VERIFY_POST,
    application_api::IDENTIFIER_POLICIES_GET,
    application_api::ACCOUNTS_BY_ACCOUNT_ID_IDENTIFIERS_CLAIM_RECEIPT_POST,
    application_api::IDENTIFIERS_RECEIPTS_BY_RECEIPT_HASH_GET,
    application_api::IDENTIFIERS_RESOLVE_POST,
    application_api::REPO_AGREEMENTS_GET,
    application_api::REPO_AGREEMENTS_QUERY_POST,
    application_api::NOTIFY_DEVICES_POST,
    application_api::NOTIFY_DEVICES_DELETE,
    application_api::SNS_NAMES_BY_NAMESPACE_BY_LITERAL_GET,
    application_api::SNS_POLICIES_BY_SUFFIX_ID_GET,
    application_api::SORACLOUD_STATUS_GET,
    application_api::SORACLOUD_SERVICES_BY_SERVICE_NAME_PUBLIC_DISCOVERY_GET,
    application_api::SORACLOUD_SERVICES_BY_SERVICE_NAME_REVISIONS_BY_SERVICE_VERSION_PUBLIC_DISCOVERY_GET,
    application_api::SORACLOUD_DEPLOY_POST,
    application_api::SORACLOUD_UPGRADE_POST,
    application_api::SORACLOUD_APPS_DEPLOY_POST,
    application_api::SORACLOUD_APPS_UPGRADE_POST,
    application_api::SORACLOUD_APPS_STATUS_GET,
    application_api::SORACLOUD_APPS_BY_APP_NAME_STATUS_GET,
    application_api::SORACLOUD_ROLLBACK_POST,
    application_api::SORACLOUD_ROLLOUT_POST,
    application_api::SORACLOUD_STATE_MUTATE_POST,
    application_api::SORACLOUD_SERVICE_CONFIG_SET_POST,
    application_api::SORACLOUD_SERVICE_CONFIG_DELETE_POST,
    application_api::SORACLOUD_SERVICE_CONFIG_STATUS_GET,
    application_api::SORACLOUD_SERVICE_SECRET_SET_POST,
    application_api::SORACLOUD_SERVICE_SECRET_DELETE_POST,
    application_api::SORACLOUD_SERVICE_SECRET_STATUS_GET,
    application_api::SORACLOUD_FHE_JOB_RUN_POST,
    application_api::SORACLOUD_DECRYPT_REQUEST_POST,
    application_api::SORACLOUD_HEALTH_ACCESS_REQUEST_POST,
    application_api::SORACLOUD_HEALTH_COMPLIANCE_REPORT_GET,
    application_api::SORACLOUD_CIPHERTEXT_QUERY_POST,
    application_api::SORACLOUD_TRAINING_JOB_START_POST,
    application_api::SORACLOUD_TRAINING_JOB_CHECKPOINT_POST,
    application_api::SORACLOUD_TRAINING_JOB_RETRY_POST,
    application_api::SORACLOUD_TRAINING_JOB_STATUS_GET,
    application_api::SORACLOUD_MODEL_WEIGHT_REGISTER_POST,
    application_api::SORACLOUD_MODEL_WEIGHT_PROMOTE_POST,
    application_api::SORACLOUD_MODEL_WEIGHT_ROLLBACK_POST,
    application_api::SORACLOUD_MODEL_WEIGHT_STATUS_GET,
    application_api::SORACLOUD_MODEL_ARTIFACT_REGISTER_POST,
    application_api::SORACLOUD_MODEL_ARTIFACT_STATUS_GET,
    application_api::SORACLOUD_MODEL_UPLOAD_REGISTER_POST,
    application_api::SORACLOUD_MODEL_UPLOAD_ENCRYPTION_RECIPIENT_GET,
    application_api::SORACLOUD_MODEL_UPLOAD_STATUS_GET,
    application_api::SORACLOUD_MODEL_UPLOAD_PRIVATE_EXECUTE_POST,
    application_api::SORACLOUD_MODEL_UPLOAD_PRIVATE_RECEIPTS_GET,
    application_api::SORACLOUD_HF_DEPLOY_POST,
    application_api::SORACLOUD_HF_STATUS_GET,
    application_api::SORACLOUD_HF_LEASE_LEAVE_POST,
    application_api::SORACLOUD_HF_LEASE_RENEW_POST,
    application_api::SORACLOUD_MODEL_HOST_ADVERTISE_POST,
    application_api::SORACLOUD_MODEL_HOST_HEARTBEAT_POST,
    application_api::SORACLOUD_MODEL_HOST_WITHDRAW_POST,
    application_api::SORACLOUD_MODEL_HOST_STATUS_GET,
    application_api::SORACLOUD_AGENT_DEPLOY_POST,
    application_api::SORACLOUD_AGENT_LEASE_RENEW_POST,
    application_api::SORACLOUD_AGENT_RESTART_POST,
    application_api::SORACLOUD_AGENT_STATUS_GET,
    application_api::SORACLOUD_AGENT_WALLET_SPEND_POST,
    application_api::SORACLOUD_AGENT_WALLET_APPROVE_POST,
    application_api::SORACLOUD_AGENT_POLICY_REVOKE_POST,
    application_api::SORACLOUD_AGENT_MESSAGE_SEND_POST,
    application_api::SORACLOUD_AGENT_MESSAGE_ACK_POST,
    application_api::SORACLOUD_AGENT_MAILBOX_STATUS_GET,
    application_api::SORACLOUD_AGENT_AUTONOMY_ALLOW_POST,
    application_api::SORACLOUD_AGENT_AUTONOMY_RUN_POST,
    application_api::SORACLOUD_AGENT_AUTONOMY_RUN_FINALIZE_POST,
    application_api::SORACLOUD_AGENT_AUTONOMY_STATUS_GET,
    application_api::ASSETS_DEFINITIONS_GET,
    application_api::ASSETS_DEFINITIONS_BY_ASSET_GET,
    application_api::ASSETS_DEFINITIONS_QUERY_POST,
    application_api::CONFIDENTIAL_ASSETS_BY_DEFINITION_ID_TRANSITIONS_GET,
    application_api::CONFIDENTIAL_NOTES_GET,
    application_api::CONFIDENTIAL_RELAY_SUBMIT_POST,
    application_api::NFTS_GET,
    application_api::NFTS_QUERY_POST,
    application_api::RWAS_GET,
    application_api::RWAS_QUERY_POST,
    application_api::SUBSCRIPTIONS_PLANS_GET,
    application_api::SUBSCRIPTIONS_PLANS_POST,
    application_api::SUBSCRIPTIONS_GET,
    application_api::SUBSCRIPTIONS_POST,
    application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_GET,
    application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_PAUSE_POST,
    application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_RESUME_POST,
    application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CANCEL_POST,
    application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_KEEP_POST,
    application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_USAGE_POST,
    application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CHARGE_NOW_POST,
    application_api::PARAMETERS_GET,
    application_api::EXPLORER_ACCOUNTS_GET,
    application_api::EXPLORER_DOMAINS_GET,
    application_api::EXPLORER_ASSET_DEFINITIONS_GET,
    application_api::EXPLORER_ASSETS_GET,
    application_api::EXPLORER_NFTS_GET,
    application_api::EXPLORER_RWAS_GET,
    application_api::EXPLORER_BLOCKS_GET,
    application_api::EXPLORER_HEALTH_GET,
    application_api::EXPLORER_BLOCKS_STREAM_GET,
    application_api::EXPLORER_TRANSACTIONS_GET,
    application_api::EXPLORER_TRANSACTIONS_LATEST_GET,
    application_api::EXPLORER_TRANSACTIONS_STREAM_GET,
    application_api::EXPLORER_INSTRUCTIONS_GET,
    application_api::EXPLORER_INSTRUCTIONS_LATEST_GET,
    application_api::SORACLES_DEFI_ATTESTATIONS_LATEST_GET,
    application_api::SORACLES_FEEDS_GET,
    application_api::SORACLES_FEEDS_BY_FEED_ID_HISTORY_GET,
    application_api::EXPLORER_METRICS_GET,
    application_api::EXPLORER_INSTRUCTIONS_STREAM_GET,
    application_api::TELEMETRY_PEERS_INFO_GET,
    application_api::TELEMETRY_PROPAGATION_GET,
    application_api::TELEMETRY_LIVE_GET,
    application_api::EXPLORER_ACCOUNTS_BY_ACCOUNT_ID_GET,
    application_api::EXPLORER_ACCOUNTS_BY_ACCOUNT_ID_QR_GET,
    application_api::EXPLORER_DOMAINS_BY_DOMAIN_ID_GET,
    application_api::EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_GET,
    application_api::EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_ECONOMETRICS_GET,
    application_api::EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_SNAPSHOT_GET,
    application_api::EXPLORER_ASSETS_BY_ASSET_ID_GET,
    application_api::EXPLORER_NFTS_BY_NFT_ID_GET,
    application_api::EXPLORER_RWAS_BY_RWA_ID_GET,
    application_api::EXPLORER_BLOCKS_BY_IDENTIFIER_GET,
    application_api::EXPLORER_TRANSACTIONS_BY_HASH_GET,
    application_api::EXPLORER_INSTRUCTIONS_BY_HASH_BY_INDEX_GET,
    application_api::EXPLORER_INSTRUCTIONS_BY_HASH_BY_INDEX_CONTRACT_VIEW_GET,
    application_api::KAIGI_CALLS_BY_CALL_ID_GET,
    application_api::KAIGI_CALLS_BY_CALL_ID_SIGNALS_GET,
    application_api::KAIGI_CALLS_BY_CALL_ID_EVENTS_GET,
    application_api::KAIGI_RELAYS_GET,
    application_api::KAIGI_RELAYS_BY_RELAY_ID_GET,
    application_api::KAIGI_RELAYS_HEALTH_GET,
    application_api::KAIGI_RELAYS_EVENTS_GET,
    application_api::WEBHOOKS_GET,
    application_api::WEBHOOKS_POST,
    application_api::WEBHOOKS_BY_ID_DELETE,
    contracts_and_verification_keys::CONTRACTS_CODE_BYTES_BY_CODE_HASH_GET,
    contracts_and_verification_keys::CONTRACTS_ALIASES_POST,
    contracts_and_verification_keys::CONTRACTS_ALIASES_RESOLVE_POST,
    contracts_and_verification_keys::CONTRACTS_DEPLOYMENT_STATE_POST,
    contracts_and_verification_keys::ASSETS_TRANSFER_POST,
    contracts_and_verification_keys::CONTRACTS_CALL_POST,
    contracts_and_verification_keys::CONTRACTS_CALL_SIMULATE_POST,
    contracts_and_verification_keys::BRIDGE_PROOFS_SUBMIT_POST,
    contracts_and_verification_keys::BRIDGE_MESSAGES_POST,
    contracts_and_verification_keys::CONTRACTS_VIEW_POST,
    contracts_and_verification_keys::CONTRACTS_VIEW_BATCH_POST,
    contracts_and_verification_keys::CONTRACTS_CALL_MULTISIG_PROPOSE_POST,
    contracts_and_verification_keys::CONTRACTS_CALL_MULTISIG_APPROVE_POST,
    contracts_and_verification_keys::CONTRACTS_STATE_GET,
    contracts_and_verification_keys::MINT_REQUESTS_GET,
    contracts_and_verification_keys::MINT_REQUESTS_BY_REQUEST_ID_GET,
    contracts_and_verification_keys::MULTISIG_PROPOSE_POST,
    contracts_and_verification_keys::MULTISIG_APPROVE_POST,
    contracts_and_verification_keys::MULTISIG_CANCEL_POST,
    contracts_and_verification_keys::MULTISIG_SPEC_POST,
    contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST,
    contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST,
    contracts_and_verification_keys::CONTROLS_ASSET_TRANSFER_QUERY_POST,
    contracts_and_verification_keys::ZK_VK_REGISTER_POST,
    contracts_and_verification_keys::ZK_VK_UPDATE_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_DECLARE_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_TELEMETRY_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_SCHEDULE_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_COMPLETE_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_UPTIME_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_POR_PROOF_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_POR_VERDICT_POST,
    contracts_and_verification_keys::SORAFS_POR_STATUS_GET,
    contracts_and_verification_keys::SORAFS_POR_EXPORT_GET,
    contracts_and_verification_keys::SORAFS_POR_INGESTION_BY_MANIFEST_DIGEST_HEX_GET,
    contracts_and_verification_keys::SORAFS_POR_REPORT_BY_ISO_WEEK_GET,
    contracts_and_verification_keys::SORAFS_POR_VRF_POST,
    contracts_and_verification_keys::SORAFS_CAPACITY_FAILURE_POST,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_ORDERS_POST,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_CANCEL_POST,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_RECEIPTS_POST,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_RECEIPTS_GET,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_BOOK_GET,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_TRADES_GET,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_CHANNELS_GET,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_EVENTS_GET,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_EVENTS_STREAM_GET,
    contracts_and_verification_keys::SORAFS_ORDERBOOK_EVENTS_WS_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_POLICY_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_PROVIDERS_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_PROVIDERS_BY_PROVIDER_ID_HEX_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_TOP_UP_POST,
    contracts_and_verification_keys::SORAFS_RESERVE_WITHDRAW_POST,
    contracts_and_verification_keys::SORAFS_RESERVE_MOVEMENTS_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_MOVEMENTS_BY_MOVEMENT_ID_HEX_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_MOVEMENTS_BY_MOVEMENT_ID_HEX_DECISION_POST,
    contracts_and_verification_keys::SORAFS_RESERVE_CREDIT_DRAW_POST,
    contracts_and_verification_keys::SORAFS_RESERVE_CREDIT_REPAY_POST,
    contracts_and_verification_keys::SORAFS_RESERVE_APPEALS_POST,
    contracts_and_verification_keys::SORAFS_RESERVE_APPEALS_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_APPEALS_BY_APPEAL_ID_HEX_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_APPEALS_BY_APPEAL_ID_HEX_DECISION_POST,
    contracts_and_verification_keys::SORAFS_RESERVE_EVENTS_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_EVENTS_STREAM_GET,
    contracts_and_verification_keys::SORAFS_RESERVE_EVENTS_WS_GET,
    contracts_and_verification_keys::SORAFS_APPEALS_PRICING_CONFIG_GET,
    contracts_and_verification_keys::SORAFS_APPEALS_PRICING_STATUS_GET,
    contracts_and_verification_keys::SORAFS_APPEALS_PRICING_QUOTE_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_SETTLE_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DISBURSE_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DEPOSITS_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DEPOSITS_CONFIRM_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DEPOSITS_SETTLE_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DEPOSITS_SUBMIT_SETTLEMENT_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DEPOSITS_RECONCILE_POST,
    contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DEPOSITS_BY_ESCROW_ID_HEX_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_BY_CASE_ID_BY_ROUND_ID_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_BY_CASE_ID_BY_ROUND_ID_NO_SHOW_PLAN_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_ELIGIBILITY_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_SORTITION_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_ASSIGNMENTS_ACCEPT_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_ACTIVATE_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_COMMITS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_CHALLENGES_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_CHALLENGES_RESOLVE_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_REVEALS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_TALLY_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_BALLOTS_EVENTS_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_MODEL_REGISTRY_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_MODEL_REGISTRY_REPRO_MANIFESTS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_MODEL_REGISTRY_CORPORA_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_SCREENING_RESULTS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_SCREENING_RESULTS_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_REVIEW_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_RELEASE_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_APPEAL_HANDOFF_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_OPERATOR_PANEL_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_OBJECT_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_OBJECT_GET,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_VIEWER_SESSIONS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_QUARANTINE_BY_QUARANTINE_ID_HEX_VIEWER_ACCESS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_VIEWER_AUDIT_REPORTS_POST,
    contracts_and_verification_keys::SORAFS_MODERATION_VIEWER_AUDIT_REPORTS_PUBLISH_DUE_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_REPORT_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_SLASH_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_CLAIM_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_HEARTBEAT_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_COMPLETE_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_FAIL_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_APPEAL_POST,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_STATUS_GET,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_TASKS_GET,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_TASKS_BY_TICKET_ID_GET,
    contracts_and_verification_keys::SORAFS_AUDIT_REPAIR_EVENTS_GET,
    contracts_and_verification_keys::ZK_VK_BY_BACKEND_BY_NAME_GET,
    contracts_and_verification_keys::ZK_VK_GET,
    contracts_and_verification_keys::ZK_PROOFS_GET,
    contracts_and_verification_keys::ZK_PROOFS_COUNT_GET,
    contracts_and_verification_keys::ZK_PROOF_BY_BACKEND_BY_HASH_GET,
    contracts_and_verification_keys::CONTRACTS_CODE_BY_CODE_HASH_GET,
    contracts_and_verification_keys::CONTRACTS_CODE_BY_CODE_HASH_CONTRACT_VIEW_GET,
    contracts_and_verification_keys::CONTRACTS_CODE_BY_CODE_HASH_VERIFIED_SOURCE_JOBS_POST,
    contracts_and_verification_keys::CONTRACTS_CODE_BY_CODE_HASH_VERIFIED_SOURCE_JOBS_BY_JOB_ID_GET,
    soracloud_gateway::SORADNS_ROOT,
    soracloud_gateway::SORADNS_PATH,
    soracloud_gateway::LOCAL_ROOT,
    soracloud_gateway::LOCAL_PATH,
    content_directory::CONTENT,
    content_directory::SORADNS_LATEST,
    content_directory::SORADNS_EVENTS,
    offline::READINESS,
    offline::RECIPIENT_LINEAGE,
    offline::TOP_UP,
    offline::REDEEM,
    offline::OPERATION,
];

#[cfg(test)]
mod tests {
    use super::*;

    const FEATURED_ROUTES: &[RouteDescriptor] = &[
        RouteDescriptor::new(
            "test.always",
            HttpMethod::Get,
            "/v1/tests/always",
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_projections(RouteProjections::ALL),
        RouteDescriptor::new(
            "test.featured",
            HttpMethod::Get,
            "/v1/tests/featured",
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK),
        RouteDescriptor::new(
            "test.diagnostic",
            HttpMethod::Get,
            "/v1/tests/diagnostic",
            ApiSurface::Diagnostic,
            Listener::Torii,
        ),
    ];

    #[test]
    fn final_offline_catalog_is_valid_and_unique() {
        let catalog = RouteCatalog::new(offline::ROUTES);
        assert_eq!(catalog.validate(), Ok(()));

        let ids: BTreeSet<_> = catalog
            .routes()
            .iter()
            .map(|route| route.stable_route_id())
            .collect();
        let method_paths: BTreeSet<_> = catalog
            .routes()
            .iter()
            .map(|route| (route.method(), route.path()))
            .collect();
        assert_eq!(ids.len(), offline::ROUTES.len());
        assert_eq!(method_paths.len(), offline::ROUTES.len());
    }

    #[test]
    fn canonical_catalog_retires_global_sumeragi_rbc_and_collectors() {
        assert!(
            CATALOGED_ROUTES
                .iter()
                .any(|route| route.path() == "/v1/sumeragi/status")
        );
        for retired in [
            "/v1/sumeragi/rbc",
            "/v1/sumeragi/rbc/delivered/{height}/{view}",
            "/v1/sumeragi/rbc/sessions",
            "/v1/sumeragi/rbc/sample",
            "/v1/sumeragi/collectors",
        ] {
            assert!(
                CATALOGED_ROUTES.iter().all(|route| route.path() != retired),
                "retired route {retired} leaked into the canonical catalog"
            );
        }
    }

    #[test]
    fn internal_torii_proxy_is_the_only_identity_bound_operator_route() {
        assert_eq!(core::INTERNAL_PROXY.surface(), ApiSurface::Operator);
        assert_eq!(
            core::INTERNAL_PROXY.authentication(),
            AuthenticationPolicy::IdentityBoundSignature
        );
        assert_eq!(validate_catalog(&[core::INTERNAL_PROXY]), Ok(()));

        let generic_identity_bound_operator = RouteDescriptor::new(
            "test.identity_bound_operator",
            HttpMethod::Post,
            "/v1/tests/identity-bound-operator",
            ApiSurface::Operator,
            Listener::Torii,
        )
        .with_authentication(AuthenticationPolicy::IdentityBoundSignature);
        let errors = validate_catalog(&[generic_identity_bound_operator])
            .expect_err("generic identity-bound keys must not receive operator privileges");
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::OperatorSurfaceRequiresAuthentication
        }));
    }

    #[test]
    fn sccp_governance_descriptor_uses_the_canonical_uri() {
        assert_eq!(
            runtime_governance::GOV_PROPOSE_SCCP.path(),
            crate::uri::GOV_PROPOSE_SCCP_ROUTE_GOVERNANCE
        );
    }

    #[test]
    fn protected_namespace_update_is_an_explicit_operator_mcp_route() {
        let route = runtime_governance::GOV_PROTECTED_POST;
        assert_eq!(route.surface(), ApiSurface::Operator);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OperatorSignature
        );
        assert!(route.projections().openapi());
        assert!(!route.projections().sdk());
        assert!(route.projections().mcp());

        let routes = [route];
        let projected = RouteCatalog::new(&routes)
            .project(CatalogProjection::Mcp, EnabledFeatures::new(&["app_api"]));
        assert_eq!(projected, vec![&routes[0]]);
    }

    #[test]
    fn bridge_finality_routes_are_not_telemetry_gated() {
        for route in [
            sumeragi::BRIDGE_FINALITY,
            sumeragi::BRIDGE_FINALITY_ATTESTATION,
            sumeragi::BRIDGE_FINALITY_BUNDLE,
        ] {
            assert_eq!(route.feature_gate(), FeatureGate::Always);
        }
    }

    #[test]
    fn canonical_websocket_streams_are_openapi_projected() {
        let enabled = EnabledFeatures::new(&["app_api"]);
        let projected =
            RouteCatalog::new(streaming::APP_ROUTES).project(CatalogProjection::OpenApi, enabled);

        for route in [streaming::SUBSCRIPTION_WS, streaming::BLOCKS_WS] {
            assert!(route.projections().openapi());
            assert!(!route.projections().sdk());
            assert!(!route.projections().mcp());
            assert!(projected.iter().any(|projected| **projected == route));
        }
    }

    #[test]
    fn documented_system_and_telemetry_routes_are_openapi_projected() {
        let enabled = EnabledFeatures::new(&["app_api", "telemetry", "profiling"]);
        let projected =
            RouteCatalog::new(CATALOGED_ROUTES).project(CatalogProjection::OpenApi, enabled);

        for route in [
            diagnostic::STATUS,
            diagnostic::STATUS_TAIL,
            diagnostic::METRICS,
            diagnostic::PROFILE,
            diagnostic::OPENAPI_JSON,
            diagnostic::OPENAPI,
            streaming::P2P,
            application_api::EXPLORER_METRICS_GET,
            application_api::TELEMETRY_PEERS_INFO_GET,
            application_api::TELEMETRY_LIVE_GET,
        ] {
            assert!(route.projections().openapi());
            assert!(!route.projections().sdk());
            assert!(!route.projections().mcp());
            assert!(projected.iter().any(|projected| **projected == route));
        }

        assert!(
            !application_api::TELEMETRY_PROPAGATION_GET
                .projections()
                .openapi()
        );
        assert!(
            !projected
                .iter()
                .any(|route| **route == application_api::TELEMETRY_PROPAGATION_GET)
        );
    }

    #[test]
    fn first_release_catalog_excludes_unsupported_method_paths() {
        for (method, path) in [
            (HttpMethod::Post, "/v1/nexus/lifecycle"),
            (HttpMethod::Post, "/v1/sorafs/capacity/por-challenge"),
            (HttpMethod::Post, "/v1/sorafs/capacity/por"),
            (HttpMethod::Post, "/v1/sorafs/por/trigger"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-sample"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-challenge"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-proof"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-verdict"),
        ] {
            assert!(
                CATALOGED_ROUTES
                    .iter()
                    .all(|route| route.method() != method || route.path() != path),
                "unsupported route leaked into the first-release catalog: {method:?} {path}"
            );
        }

        assert!(CATALOGED_ROUTES.contains(&core::NEXUS_LIFECYCLE_GET));
        assert!(
            CATALOGED_ROUTES
                .contains(&contracts_and_verification_keys::SORAFS_CAPACITY_POR_PROOF_POST)
        );
        assert!(
            CATALOGED_ROUTES
                .contains(&contracts_and_verification_keys::SORAFS_CAPACITY_POR_VERDICT_POST)
        );
    }

    #[test]
    fn canonical_catalog_includes_exact_gateway_and_directory_routes() {
        let catalog = RouteCatalog::new(CATALOGED_ROUTES);
        assert_eq!(catalog.validate(), Ok(()));

        for expected in soracloud_gateway::ROUTES
            .iter()
            .chain(content_directory::ROUTES)
        {
            assert!(
                catalog.routes().iter().any(|route| route == expected),
                "missing canonical route {}",
                expected.stable_route_id()
            );
        }
        assert!(
            catalog
                .routes()
                .iter()
                .all(|route| route.path() != "/soradns/{fqdn}/"),
            "the first-release gateway must not expose a trailing-slash alias"
        );
    }

    #[test]
    fn dedicated_onboarding_authentication_is_exactly_scoped() {
        for route in [
            application_api::ACCOUNTS_ONBOARD_PLAN_POST,
            application_api::ACCOUNTS_ONBOARD_POST,
            application_api::ACCOUNTS_ONBOARDING_READINESS_GET,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OnboardingToken,
                "{} must advertise its dedicated credential",
                route.stable_route_id()
            );
        }
        assert_eq!(
            CATALOGED_ROUTES
                .iter()
                .filter(|route| { route.authentication() == AuthenticationPolicy::OnboardingToken })
                .count(),
            3,
            "no unrelated route may inherit the onboarding credential policy"
        );
    }

    #[test]
    fn trusted_internal_account_reads_are_not_projected_to_public_tooling() {
        let routes = [
            application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_GET,
            application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_BY_ENTRYPOINT_HASH_GET,
            application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_ASSETS_BY_ASSET_DEFINITION_ID_GET,
        ];
        let catalog = RouteCatalog::new(&routes);
        assert_eq!(catalog.validate(), Ok(()));
        for route in routes {
            assert_eq!(route.projections(), RouteProjections::NONE);
            assert!(!route.cors_options());
        }
        let enabled = EnabledFeatures::new(&["app_api"]);
        assert!(
            catalog
                .project(CatalogProjection::OpenApi, enabled)
                .is_empty()
        );
        assert!(catalog.project(CatalogProjection::Sdk, enabled).is_empty());
        assert!(catalog.project(CatalogProjection::Mcp, enabled).is_empty());
    }

    #[test]
    fn account_alias_visibility_and_signed_operator_routes_declare_exact_authentication() {
        for route in [
            aliases::RESOLVE,
            aliases::RESOLVE_INDEX,
            aliases::BY_ACCOUNT,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::ToriiDefault,
                "{} conditionally authenticates restricted dataspace reads in its handler",
                route.stable_route_id()
            );
        }

        for route in [
            aliases::SETUP_PLAN,
            aliases::LEASE_RENEW_PLAN,
            aliases::AUTO_RENEW_PLAN,
            aliases::RETAIL_RECIPIENT_LOOKUP,
            aliases::RETAIL_RECIPIENT_ROUTE,
            fees::QUOTE,
            fees::SPONSOR_PROGRAM_BY_ID,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature,
                "{} must require canonical account authentication",
                route.stable_route_id()
            );
        }

        assert_eq!(
            aliases::ASSET_RESOLVE.authentication(),
            AuthenticationPolicy::ToriiDefault,
            "public asset aliases do not expose an account binding"
        );

        for route in [
            contracts_and_verification_keys::CONTRACTS_ALIASES_RESOLVE_POST,
            contracts_and_verification_keys::CONTRACTS_DEPLOYMENT_STATE_POST,
            runtime_governance::GOV_CONTRACT_GET,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature,
                "{} exposes contract identity and must require a canonical account signature",
                route.stable_route_id()
            );
        }
    }

    #[test]
    fn sorafs_catalog_has_one_strict_first_release_path_per_operation() {
        let catalog = RouteCatalog::new(sorafs::ROUTES);
        assert_eq!(catalog.validate(), Ok(()));

        for expected in sorafs::ROUTES {
            assert!(
                CATALOGED_ROUTES.iter().any(|route| route == expected),
                "missing canonical SoraFS route {}",
                expected.stable_route_id()
            );
            assert_eq!(
                expected.path_normalization(),
                PathNormalization::Strict,
                "SoraFS route must reject normalization aliases: {}",
                expected.stable_route_id()
            );
        }

        for unsupported_path in [
            "/ws/reputation",
            "/sorafs/cid/{cid}/",
            "/v1/sorafs/storage/por-sample",
            "/v1/sorafs/storage/por-challenge",
            "/v1/sorafs/storage/por-proof",
            "/v1/sorafs/storage/por-verdict",
        ] {
            assert!(
                sorafs::ROUTES
                    .iter()
                    .all(|route| route.path() != unsupported_path),
                "unsupported SoraFS path leaked into the catalog: {unsupported_path}"
            );
        }
        assert_eq!(
            sorafs::REPUTATION_EVENTS_WEBSOCKET.path(),
            "/v1/sorafs/reputation/events/ws"
        );
        assert_eq!(sorafs::CID_ROOT.path(), "/sorafs/cid/{cid}");
        assert_eq!(sorafs::CID_ROOT.route_match(), RouteMatch::Exact);
        assert_eq!(sorafs::CID_PATH.route_match(), RouteMatch::Wildcard);
        for route in [sorafs::DEAL_FUND_PROVIDER, sorafs::DEAL_USAGE] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::IdentityBoundSignature
            );
        }
        for route in [
            sorafs::DEAL_FUND_CLIENT,
            sorafs::DEAL_OPEN,
            sorafs::DEAL_CANCEL,
            sorafs::DEAL_SETTLE,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OperatorSignature
            );
        }
        for route in [
            sorafs::ECONOMICS_PRICING_MANIFEST,
            sorafs::ECONOMICS_HEDGING_FEED,
            sorafs::ECONOMICS_STATUS,
            sorafs::ECONOMICS_ACTIVE_PRICING,
            sorafs::ECONOMICS_HEDGING_REFERENCE,
        ] {
            assert_eq!(route.feature_gate(), FeatureGate::Feature("app_api"));
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::ToriiDefault,
                "economics canonical X-Iroha authentication is enforced inside the handler"
            );
            assert!(route.projections().openapi());
        }

        for invalid_path in [
            "/v1/sorafs//providers",
            "/v1/sorafs/providers/%2fadmin",
            "/v1/sorafs/providers/%5Cadmin",
            "/v1/SoraFs/providers",
        ] {
            let descriptor = RouteDescriptor::new(
                "test.sorafs_invalid_path",
                HttpMethod::Get,
                invalid_path,
                ApiSurface::Public,
                Listener::Torii,
            )
            .with_implicit_head(true);
            assert!(
                validate_catalog(&[descriptor]).is_err(),
                "normalization alias must be rejected: {invalid_path}"
            );
        }

        let trailing_root = RouteDescriptor::new(
            "protocol.sorafs_invalid_root",
            HttpMethod::Get,
            "/sorafs/cid/{cid}/",
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "adversarial trailing-slash test",
        })
        .with_implicit_head(true);
        assert!(validate_catalog(&[trailing_root]).is_err());
    }

    #[test]
    fn converted_route_families_are_valid_and_exclude_retired_spellings() {
        let routes = aliases::ROUTES
            .iter()
            .chain(fees::ROUTES)
            .chain(operator_authentication::ROUTES)
            .chain(governance_vrf::ROUTES)
            .chain(iso20022::ROUTES)
            .chain(data_availability::ROUTES)
            .chain(musubi::ROUTES)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(validate_catalog(&routes), Ok(()));

        for unsupported_path in [
            "/v1/aliases/resolve_index",
            "/v1/aliases/by_account",
            "/v1/fee-sponsor-policies/by-id",
            "/v1/da/proof_policies",
            "/v1/da/proof_policy_snapshot",
            "/v1/da/pin_intents",
            "/v1/iso20022/status/{msg_id}",
        ] {
            assert!(
                routes.iter().all(|route| route.path() != unsupported_path),
                "unsupported route must not enter the first-release catalog: {unsupported_path}"
            );
        }
        assert!(operator_authentication::ROUTES.iter().all(|route| {
            route.surface() == ApiSurface::Operator
                && route.authentication() == AuthenticationPolicy::OperatorCredentialExchange
                && !route.projections().sdk()
                && !route.projections().mcp()
        }));
    }

    fn contract_and_application_routes() -> Vec<RouteDescriptor> {
        contracts_and_verification_keys::ROUTES
            .iter()
            .chain(application_api::ROUTES)
            .copied()
            .collect()
    }

    #[test]
    fn contract_and_application_routes_are_canonical() {
        let routes = contract_and_application_routes();
        assert_eq!(validate_catalog(&routes), Ok(()));

        for expected in &routes {
            assert!(
                CATALOGED_ROUTES.iter().any(|route| route == expected),
                "missing canonical route {}",
                expected.stable_route_id()
            );
            assert_eq!(expected.path_normalization(), PathNormalization::Strict);
        }
    }

    #[test]
    fn contract_and_application_routes_exclude_retired_spellings() {
        let routes = contract_and_application_routes();
        let openapi = RouteCatalog::new(CATALOGED_ROUTES).project(
            CatalogProjection::OpenApi,
            EnabledFeatures::new(&["app_api"]),
        );
        for unsupported_path in [
            "/v1/multisig/proposals/lookup",
            "/v1/multisig/proposals/list",
            "/v1/multisig/proposals/get",
            "/v1/multisig/proposals/search",
            "/v1/multisig/approvals/list",
            "/v1/multisig/approvals/get",
            "/v1/multisig/approvals/list_for_authority",
            "/v1/multisig/approvals/get_for_authority",
            "/v1/multisig/approvals/query",
            "/v1/multisig/approvals/lookup",
            "/v1/multisig/approvals/query-for-authority",
            "/v1/multisig/approvals/lookup-for-authority",
            "/v1/controls/asset-transfer/get",
            "/v1/nexus/public_lanes/{lane_id}/validators",
            "/v1/sorafs/capacity/por-challenge",
            "/v1/sorafs/capacity/por",
            "/v1/sorafs/por/trigger",
        ] {
            assert!(
                routes.iter().all(|route| route.path() != unsupported_path),
                "unsupported first-release spelling leaked into the catalog: {unsupported_path}"
            );
            assert!(
                openapi.iter().all(|route| route.path() != unsupported_path),
                "unsupported first-release spelling leaked into OpenAPI projection: {unsupported_path}"
            );
        }
    }

    #[test]
    fn contract_and_application_routes_include_first_release_spellings() {
        let routes = contract_and_application_routes();
        for canonical_path in [
            "/v1/assets/transfer",
            "/v1/multisig/proposals/query",
            "/v1/multisig/proposals/resolve",
            "/v1/controls/asset-transfer/query",
            "/v1/nexus/public-lanes/{lane_id}/validators",
        ] {
            assert!(
                routes.iter().any(|route| route.path() == canonical_path),
                "missing canonical first-release route: {canonical_path}"
            );
        }
    }

    #[test]
    fn contract_and_application_route_policies_are_projection_safe() {
        for route in [
            contracts_and_verification_keys::CONTRACTS_CODE_BYTES_BY_CODE_HASH_GET,
            contracts_and_verification_keys::MULTISIG_SPEC_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature,
                "{}",
                route.stable_route_id()
            );
        }

        for route in [
            contracts_and_verification_keys::SORAFS_CAPACITY_POR_PROOF_POST,
            contracts_and_verification_keys::SORAFS_CAPACITY_POR_VERDICT_POST,
        ] {
            assert_eq!(route.surface(), ApiSurface::Operator);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OperatorSignature
            );
            assert!(route.projections().openapi());
            assert!(!route.projections().sdk());
            assert!(!route.projections().mcp());
        }

        assert_eq!(
            application_api::NOTIFY_DEVICES_POST.feature_gate(),
            FeatureGate::All(&["app_api", "push"])
        );
        assert_eq!(
            application_api::TELEMETRY_LIVE_GET.surface(),
            ApiSurface::Diagnostic
        );
        assert_eq!(
            application_api::APP_API_CID_BY_CID_BY_PATH_GET.route_match(),
            RouteMatch::Wildcard
        );
    }

    #[test]
    fn contract_and_application_route_projections_are_explicit() {
        for route in [
            contracts_and_verification_keys::BRIDGE_PROOFS_SUBMIT_POST,
            contracts_and_verification_keys::BRIDGE_MESSAGES_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST,
        ] {
            assert!(route.projections().openapi(), "{}", route.stable_route_id());
        }
        for route in [
            contracts_and_verification_keys::BRIDGE_PROOFS_SUBMIT_POST,
            contracts_and_verification_keys::BRIDGE_MESSAGES_POST,
        ] {
            assert!(route.projections().sdk(), "{}", route.stable_route_id());
        }
        assert!(application_api::SORACLOUD_DEPLOY_POST.projections().sdk());
        assert!(
            !application_api::SORACLOUD_DEPLOY_POST
                .projections()
                .openapi()
        );
        assert!(
            application_api::APP_API_CID_BY_CID_BY_PATH_GET
                .projections()
                .sdk()
        );
        assert!(
            !application_api::APP_API_CID_BY_CID_BY_PATH_GET
                .projections()
                .openapi()
        );
    }

    #[test]
    fn telemetry_and_sumeragi_routes_are_valid_sharp_first_release_surfaces() {
        let routes = telemetry::ROUTES
            .iter()
            .chain(sumeragi::ROUTES)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(validate_catalog(&routes), Ok(()));

        for unsupported_path in [
            "/v1/sumeragi/new_view/json",
            "/v1/sumeragi/new_view/sse",
            "/v1/sumeragi/bls_keys",
            "/v1/sumeragi/commit_qc/{hash}",
        ] {
            assert!(
                routes.iter().all(|route| route.path() != unsupported_path),
                "unsupported route must not enter the first-release catalog: {unsupported_path}"
            );
        }
        for canonical_path in [
            "/v1/sumeragi/bls-keys",
            "/v1/sumeragi/commit-qcs/{block_hash}",
            "/v1/sumeragi/diagnostics",
        ] {
            assert!(
                routes.iter().any(|route| route.path() == canonical_path),
                "missing canonical first-release route: {canonical_path}"
            );
        }

        assert!(
            telemetry::ROUTES
                .iter()
                .filter(|route| route.surface() == ApiSurface::Operator)
                .all(|route| route.authentication() == AuthenticationPolicy::OperatorSignature)
        );
        assert!(
            [sumeragi::STATUS_SSE]
                .into_iter()
                .all(|route| route.surface() == ApiSurface::Protocol
                    && route.authentication() == AuthenticationPolicy::ProtocolHandshake
                    && route.projections().openapi()
                    && !route.projections().sdk()
                    && !route.projections().mcp())
        );
        assert!(sumeragi::STATUS.projections().mcp());
        assert!(!sumeragi::SCCP_CAPABILITIES.projections().mcp());
        assert!(!telemetry::DEBUG_WITNESS.projections().openapi());

        let catalog = RouteCatalog::new(&routes);
        let without_features = catalog.project(CatalogProjection::Mounted, EnabledFeatures::none());
        assert!(
            without_features
                .iter()
                .any(|route| route.stable_route_id() == sumeragi::EVIDENCE_LIST.stable_route_id())
        );
        assert!(without_features.iter().all(|route| {
            route.stable_route_id() != sumeragi::STATUS.stable_route_id()
                && route.stable_route_id() != telemetry::ASSET_HOLDERS.stable_route_id()
        }));
        let all_features = catalog.project(
            CatalogProjection::Mounted,
            EnabledFeatures::new(&["telemetry", "app_api"]),
        );
        assert_eq!(all_features.len(), routes.len());
    }

    #[test]
    fn projections_are_explicit_and_sdk_is_a_canonical_superset() {
        let catalog = RouteCatalog::new(FEATURED_ROUTES);
        let no_features = EnabledFeatures::none();
        let app_api = EnabledFeatures::new(&["app_api"]);

        assert_eq!(
            catalog
                .project(CatalogProjection::Mounted, no_features)
                .len(),
            2
        );
        assert_eq!(
            catalog.project(CatalogProjection::Mounted, app_api).len(),
            3
        );
        assert_eq!(
            catalog
                .project(CatalogProjection::OpenApi, no_features)
                .len(),
            1
        );
        assert_eq!(
            catalog.project(CatalogProjection::OpenApi, app_api).len(),
            2
        );
        assert_eq!(
            catalog.project(CatalogProjection::Sdk, no_features).len(),
            2
        );
        assert_eq!(
            catalog.project(CatalogProjection::Mcp, no_features).len(),
            1
        );
        assert_eq!(catalog.project(CatalogProjection::Mcp, app_api).len(), 1);

        let route_ids = |projection| {
            catalog
                .project(projection, app_api)
                .into_iter()
                .map(|route| route.stable_route_id())
                .collect::<BTreeSet<_>>()
        };
        let mounted = route_ids(CatalogProjection::Mounted);
        let openapi = route_ids(CatalogProjection::OpenApi);
        let sdk = route_ids(CatalogProjection::Sdk);
        let mcp = route_ids(CatalogProjection::Mcp);
        assert_ne!(mounted, openapi);
        assert_ne!(mounted, sdk);
        assert_ne!(mounted, mcp);
        assert_ne!(openapi, mcp);
    }

    #[test]
    fn feature_expressions_have_deterministic_semantics() {
        let enabled = EnabledFeatures::new(&["app_api", "telemetry"]);
        assert!(FeatureGate::Always.is_enabled(enabled));
        assert!(FeatureGate::Feature("app_api").is_enabled(enabled));
        assert!(FeatureGate::All(&["app_api", "telemetry"]).is_enabled(enabled));
        assert!(!FeatureGate::All(&["app_api", "profiling"]).is_enabled(enabled));
        assert!(FeatureGate::Any(&["profiling", "telemetry"]).is_enabled(enabled));
        assert!(!FeatureGate::Any(&["profiling", "schema"]).is_enabled(enabled));
    }

    #[test]
    fn descriptor_builders_and_accessors_preserve_metadata() {
        let projections = RouteProjections::OPENAPI;
        let descriptor = RouteDescriptor::new(
            "protocol.content",
            HttpMethod::Get,
            "/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_authentication(AuthenticationPolicy::ProtocolHandshake)
        .with_projections(projections)
        .with_route_match(RouteMatch::Wildcard)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "content-addressed protocol namespace",
        })
        .with_implicit_head(true)
        .with_cors_options(true);

        assert_eq!(descriptor.stable_route_id(), "protocol.content");
        assert_eq!(descriptor.method(), HttpMethod::Get);
        assert_eq!(descriptor.method().as_str(), "GET");
        assert_eq!(descriptor.path(), "/content/{*tail}");
        assert_eq!(descriptor.surface(), ApiSurface::Protocol);
        assert_eq!(descriptor.listener(), Listener::Torii);
        assert_eq!(
            descriptor.authentication(),
            AuthenticationPolicy::ProtocolHandshake
        );
        assert_eq!(descriptor.feature_gate(), FeatureGate::Feature("app_api"));
        assert_eq!(descriptor.projections(), projections);
        assert!(descriptor.projections().openapi());
        assert!(!descriptor.projections().sdk());
        assert!(!descriptor.projections().mcp());
        assert_eq!(descriptor.route_match(), RouteMatch::Wildcard);
        assert!(matches!(
            descriptor.path_policy(),
            PathPolicy::ProtocolException { .. }
        ));
        assert_eq!(descriptor.path_normalization(), PathNormalization::Strict);
        assert!(descriptor.implicit_head());
        assert!(descriptor.cors_options());
        assert_eq!(validate_catalog(&[descriptor]), Ok(()));
    }

    #[test]
    fn validation_reports_duplicate_ids_and_method_paths() {
        let routes = [
            RouteDescriptor::new(
                "test.duplicate",
                HttpMethod::Get,
                "/v1/tests/one",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.duplicate",
                HttpMethod::Get,
                "/v1/tests/two",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.same_path",
                HttpMethod::Get,
                "/v1/tests/one",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.same_shape_one",
                HttpMethod::Get,
                "/v1/tests/shapes/{first_id}",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.same_shape_two",
                HttpMethod::Get,
                "/v1/tests/shapes/{second_id}",
                ApiSurface::Public,
                Listener::Torii,
            ),
        ];

        let errors = validate_catalog(&routes).expect_err("duplicates must fail validation");
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == CatalogValidationErrorKind::DuplicateStableRouteId })
        );
        assert!(errors.iter().any(|error| {
            matches!(
                error.kind,
                CatalogValidationErrorKind::DuplicateMethodAndPath {
                    existing_route_id: "test.duplicate"
                }
            )
        }));
        assert!(errors.iter().any(|error| {
            matches!(
                error.kind,
                CatalogValidationErrorKind::DuplicateMethodAndShape {
                    existing_route_id: "test.same_shape_one"
                }
            )
        }));
    }

    #[test]
    fn canonical_path_grammar_rejects_ambiguous_shapes() {
        let invalid_paths = [
            "/tests/readiness",
            "/v1/tests/snake_case",
            "/v1/tests/{itemId}",
            "/v1/tests/{item_id}/{item_id}",
            "/v1/tests//readiness",
            "/v1/tests/readiness/",
            "/v1/tests/%72eadiness",
        ];

        for path in invalid_paths {
            let descriptor = RouteDescriptor::new(
                "test.invalid_path",
                HttpMethod::Get,
                path,
                ApiSurface::Public,
                Listener::Torii,
            );
            assert!(
                validate_catalog(&[descriptor]).is_err(),
                "path should be rejected: {path}"
            );
        }
    }

    #[test]
    fn canonical_path_grammar_rejects_crud_read_operation_segments() {
        for descriptor in [
            RouteDescriptor::new(
                "test.resources_list_post",
                HttpMethod::Post,
                "/v1/tests/resources/list",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.resources_get_post",
                HttpMethod::Post,
                "/v1/tests/resources/get",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.resources_list_get",
                HttpMethod::Get,
                "/v1/tests/resources/list",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.resources_list_post",
                HttpMethod::Post,
                "/v1/tests/resources/list/details",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.resources_query_post",
                HttpMethod::Post,
                "/v1/tests/resources/list",
                ApiSurface::Public,
                Listener::Torii,
            ),
        ] {
            assert_eq!(
                validate_path(&descriptor),
                Err("static path segment uses a forbidden transport or CRUD word")
            );
        }

        for descriptor in [
            RouteDescriptor::new(
                "test.resources_json_post",
                HttpMethod::Post,
                "/v1/tests/resources/json",
                ApiSurface::Public,
                Listener::Torii,
            ),
            RouteDescriptor::new(
                "test.resources_sse_post",
                HttpMethod::Post,
                "/v1/tests/resources/sse",
                ApiSurface::Public,
                Listener::Torii,
            ),
        ] {
            assert_eq!(
                validate_path(&descriptor),
                Err("static path segment uses a forbidden transport or CRUD word")
            );
        }
    }

    #[test]
    fn wildcard_and_protocol_exceptions_must_be_explicit() {
        let wildcard = RouteDescriptor::new(
            "test.wildcard",
            HttpMethod::Get,
            "/v1/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_route_match(RouteMatch::Wildcard)
        .with_implicit_head(true);
        let health = RouteDescriptor::new(
            "protocol.health",
            HttpMethod::Get,
            "/health",
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "orchestrator health-probe convention",
        })
        .with_implicit_head(true);
        assert_eq!(validate_catalog(&[wildcard, health]), Ok(()));

        let implicit_wildcard = RouteDescriptor::new(
            "test.implicit_wildcard",
            HttpMethod::Get,
            "/v1/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
        );
        assert!(validate_catalog(&[implicit_wildcard]).is_err());
    }

    #[test]
    fn validation_enforces_projection_and_implicit_method_boundaries() {
        let routes = [
            RouteDescriptor::new(
                "test.diagnostic_sdk",
                HttpMethod::Get,
                "/v1/tests/diagnostic-sdk",
                ApiSurface::Diagnostic,
                Listener::Torii,
            )
            .with_projections(RouteProjections::SDK),
            RouteDescriptor::new(
                "test.protocol_handshake_mcp",
                HttpMethod::Get,
                "/v1/tests/protocol-handshake",
                ApiSurface::Protocol,
                Listener::Torii,
            )
            .with_authentication(AuthenticationPolicy::ProtocolHandshake)
            .with_projections(RouteProjections::MCP),
            RouteDescriptor::new(
                "test.operator_without_signature",
                HttpMethod::Post,
                "/v1/tests/operator-without-signature",
                ApiSurface::Operator,
                Listener::Torii,
            )
            .with_projections(RouteProjections::MCP),
            RouteDescriptor::new(
                "test.head_on_post",
                HttpMethod::Post,
                "/v1/tests/head-on-post",
                ApiSurface::Public,
                Listener::Torii,
            )
            .with_implicit_head(true),
            RouteDescriptor::new(
                "test.public_credential_exchange",
                HttpMethod::Post,
                "/v1/tests/public-credential-exchange",
                ApiSurface::Public,
                Listener::Torii,
            )
            .with_authentication(AuthenticationPolicy::OperatorCredentialExchange),
        ];

        let errors = validate_catalog(&routes).expect_err("invalid boundaries must be rejected");
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::DiagnosticToolingProjection
        }));
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::ProtocolHandshakeMcpProjection
        }));
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::OperatorSurfaceRequiresAuthentication
        }));
        assert!(errors.iter().any(|error| {
            error.kind
                == CatalogValidationErrorKind::OperatorCredentialExchangeRequiresOperatorSurface
        }));
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == CatalogValidationErrorKind::ImplicitHeadRequiresGet })
        );
    }

    #[test]
    fn implicit_head_and_cors_routes_are_separate_from_explicit_operations() {
        let routes = [
            RouteDescriptor::new(
                "test.read",
                HttpMethod::Get,
                "/v1/tests/resource",
                ApiSurface::Public,
                Listener::Torii,
            )
            .with_implicit_head(true)
            .with_cors_options(true),
            RouteDescriptor::new(
                "test.write",
                HttpMethod::Post,
                "/v1/tests/resource",
                ApiSurface::Public,
                Listener::Torii,
            )
            .with_cors_options(true),
        ];
        let catalog = RouteCatalog::new(&routes);
        let implicit = catalog.implicit_routes(EnabledFeatures::none());

        assert_eq!(implicit.len(), 2, "OPTIONS is emitted once per path");
        assert!(implicit.iter().any(|route| {
            route.kind() == ImplicitRouteKind::Head
                && route.parent_route_id() == "test.read"
                && route.path() == "/v1/tests/resource"
        }));
        assert!(implicit.iter().any(|route| {
            route.kind() == ImplicitRouteKind::CorsOptions && route.path() == "/v1/tests/resource"
        }));
        assert_eq!(
            catalog
                .project(CatalogProjection::Mounted, EnabledFeatures::none())
                .len(),
            2,
            "framework routes do not enter the application projection"
        );
    }

    #[test]
    fn any_method_is_protocol_only_and_never_generated() {
        let valid = RouteDescriptor::new(
            "protocol.gateway",
            HttpMethod::Any,
            "/gateway/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
        )
        .with_route_match(RouteMatch::Wildcard)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "protocol-native HTTP gateway",
        });
        assert_eq!(validate_catalog(&[valid]), Ok(()));

        let invalid = RouteDescriptor::new(
            "test.gateway",
            HttpMethod::Any,
            "/v1/tests/{*tail}",
            ApiSurface::Public,
            Listener::Torii,
        )
        .with_route_match(RouteMatch::Wildcard)
        .with_projections(RouteProjections::OPENAPI);
        let errors = validate_catalog(&[invalid]).expect_err("unsafe ANY route must fail");
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::AnyMethodRequiresProtocolSurface
        }));
        assert!(
            errors.iter().any(|error| {
                error.kind == CatalogValidationErrorKind::AnyMethodToolingProjection
            })
        );
    }
}
