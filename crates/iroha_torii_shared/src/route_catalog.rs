//! Canonical metadata for Torii HTTP routes.
//!
//! The catalog is deliberately independent of an HTTP framework. Torii uses it
//! to decide which routes are mounted for a build, while documentation and
//! client tooling consume explicit projections of the same descriptors.

use std::collections::{BTreeMap, BTreeSet};

/// HTTP methods supported by the Torii route catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Listener {
    /// Public application listener.
    Public,
    /// Restricted operator listener.
    Operator,
    /// Restricted diagnostic listener.
    Diagnostic,
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
    /// Expose the route in OpenAPI only.
    pub const OPENAPI: Self = Self(Self::OPENAPI_BIT);
    /// Expose the route to generated SDKs only.
    pub const SDK: Self = Self(Self::SDK_BIT);
    /// Expose the route as an explicitly allowlisted MCP operation only.
    pub const MCP: Self = Self(Self::MCP_BIT);
    /// Expose the route in OpenAPI and generated SDKs.
    pub const OPENAPI_AND_SDK: Self = Self(Self::OPENAPI_BIT | Self::SDK_BIT);
    /// Expose the route in every generated projection.
    pub const ALL: Self = Self(Self::OPENAPI_BIT | Self::SDK_BIT | Self::MCP_BIT);

    /// Return whether the route belongs in the OpenAPI projection.
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
    /// Enabled routes explicitly included in OpenAPI.
    OpenApi,
    /// Canonical SDK superset, independent of one node's enabled features.
    Sdk,
    /// Enabled routes explicitly allowlisted for MCP.
    Mcp,
}

/// Static metadata describing one canonical Torii route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteDescriptor {
    stable_route_id: &'static str,
    method: HttpMethod,
    path: &'static str,
    surface: ApiSurface,
    listener: Listener,
    feature_gate: FeatureGate,
    projections: RouteProjections,
    route_match: RouteMatch,
    path_policy: PathPolicy,
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
            feature_gate: FeatureGate::Always,
            projections: RouteProjections::NONE,
            route_match: RouteMatch::Exact,
            path_policy: PathPolicy::CanonicalV1,
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
    /// Mounted, OpenAPI, and MCP projections honor `features`. The SDK
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
    /// The route surface is incompatible with its listener.
    SurfaceListenerMismatch,
    /// Diagnostic routes cannot be projected into SDKs or MCP.
    DiagnosticToolingProjection,
    /// Only GET descriptors may request implicit HEAD handling.
    ImplicitHeadRequiresGet,
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
pub fn validate_catalog(routes: &[RouteDescriptor]) -> Result<(), Vec<CatalogValidationError>> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut method_paths = BTreeMap::new();

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
        if let Some(existing_route_id) = method_paths.insert((route.method, route.path), route_id) {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::DuplicateMethodAndPath { existing_route_id },
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

        let listener_matches_surface = match route.surface {
            ApiSurface::Public => route.listener == Listener::Public,
            ApiSurface::Operator => route.listener == Listener::Operator,
            ApiSurface::Diagnostic => route.listener == Listener::Diagnostic,
            ApiSurface::Protocol => true,
        };
        if !listener_matches_surface {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::SurfaceListenerMismatch,
            });
        }

        if route.surface == ApiSurface::Diagnostic
            && (route.projections.sdk() || route.projections.mcp())
        {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::DiagnosticToolingProjection,
            });
        }

        if route.implicit_head && route.method != HttpMethod::Get {
            errors.push(CatalogValidationError {
                stable_route_id: route_id,
                kind: CatalogValidationErrorKind::ImplicitHeadRequiresGet,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
    if path.contains(|character| matches!(character, '?' | '#' | '%')) {
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
        Listener::Public,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);
    /// Descriptor for online-to-offline top-up submission.
    pub const TOP_UP: RouteDescriptor = RouteDescriptor::new(
        "offline.top_up",
        HttpMethod::Post,
        TOP_UP_PATH,
        ApiSurface::Public,
        Listener::Public,
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
        Listener::Public,
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
        Listener::Public,
    )
    .with_feature_gate(FeatureGate::Feature("app_api"))
    .with_projections(RouteProjections::OPENAPI_AND_SDK)
    .with_implicit_head(true)
    .with_cors_options(true);

    /// Canonical first-release offline API catalog.
    pub const ROUTES: &[RouteDescriptor] = &[READINESS, TOP_UP, REDEEM, OPERATION];
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEATURED_ROUTES: &[RouteDescriptor] = &[
        RouteDescriptor::new(
            "test.always",
            HttpMethod::Get,
            "/v1/tests/always",
            ApiSurface::Public,
            Listener::Public,
        )
        .with_projections(RouteProjections::ALL),
        RouteDescriptor::new(
            "test.featured",
            HttpMethod::Get,
            "/v1/tests/featured",
            ApiSurface::Public,
            Listener::Public,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK),
        RouteDescriptor::new(
            "test.diagnostic",
            HttpMethod::Get,
            "/v1/tests/diagnostic",
            ApiSurface::Diagnostic,
            Listener::Diagnostic,
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
        let projections = RouteProjections::OPENAPI.union(RouteProjections::MCP);
        let descriptor = RouteDescriptor::new(
            "protocol.content",
            HttpMethod::Get,
            "/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Operator,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
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
        assert_eq!(descriptor.listener(), Listener::Operator);
        assert_eq!(descriptor.feature_gate(), FeatureGate::Feature("app_api"));
        assert_eq!(descriptor.projections(), projections);
        assert!(descriptor.projections().openapi());
        assert!(!descriptor.projections().sdk());
        assert!(descriptor.projections().mcp());
        assert_eq!(descriptor.route_match(), RouteMatch::Wildcard);
        assert!(matches!(
            descriptor.path_policy(),
            PathPolicy::ProtocolException { .. }
        ));
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
                Listener::Public,
            ),
            RouteDescriptor::new(
                "test.duplicate",
                HttpMethod::Get,
                "/v1/tests/two",
                ApiSurface::Public,
                Listener::Public,
            ),
            RouteDescriptor::new(
                "test.same_path",
                HttpMethod::Get,
                "/v1/tests/one",
                ApiSurface::Public,
                Listener::Public,
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
    }

    #[test]
    fn canonical_path_grammar_rejects_ambiguous_shapes() {
        let invalid_paths = [
            "/offline/readiness",
            "/v1/offline/v2/readiness",
            "/v1/offline/note_issue",
            "/v1/offline/list",
            "/v1/offline/{operationId}",
            "/v1/offline/{operation_id}/{operation_id}",
            "/v1/offline//readiness",
            "/v1/offline/readiness/",
            "/v1/offline/%72edeem",
        ];

        for path in invalid_paths {
            let descriptor = RouteDescriptor::new(
                "test.invalid_path",
                HttpMethod::Get,
                path,
                ApiSurface::Public,
                Listener::Public,
            );
            assert!(
                validate_catalog(&[descriptor]).is_err(),
                "path should be rejected: {path}"
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
            Listener::Public,
        )
        .with_route_match(RouteMatch::Wildcard);
        let health = RouteDescriptor::new(
            "protocol.health",
            HttpMethod::Get,
            "/health",
            ApiSurface::Protocol,
            Listener::Public,
        )
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "orchestrator health-probe convention",
        });
        assert_eq!(validate_catalog(&[wildcard, health]), Ok(()));

        let implicit_wildcard = RouteDescriptor::new(
            "test.implicit_wildcard",
            HttpMethod::Get,
            "/v1/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Public,
        );
        assert!(validate_catalog(&[implicit_wildcard]).is_err());
    }

    #[test]
    fn validation_enforces_listener_and_projection_boundaries() {
        let routes = [
            RouteDescriptor::new(
                "test.wrong_listener",
                HttpMethod::Get,
                "/v1/tests/wrong-listener",
                ApiSurface::Operator,
                Listener::Public,
            ),
            RouteDescriptor::new(
                "test.diagnostic_sdk",
                HttpMethod::Get,
                "/v1/tests/diagnostic-sdk",
                ApiSurface::Diagnostic,
                Listener::Diagnostic,
            )
            .with_projections(RouteProjections::SDK),
            RouteDescriptor::new(
                "test.head_on_post",
                HttpMethod::Post,
                "/v1/tests/head-on-post",
                ApiSurface::Public,
                Listener::Public,
            )
            .with_implicit_head(true),
        ];

        let errors = validate_catalog(&routes).expect_err("invalid boundaries must be rejected");
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == CatalogValidationErrorKind::SurfaceListenerMismatch })
        );
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::DiagnosticToolingProjection
        }));
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == CatalogValidationErrorKind::ImplicitHeadRequiresGet })
        );
    }
}
