//! Native MCP transport route descriptors.

use super::{
    AdmissionPolicy, ApiSurface, AuthenticationPolicy, HttpMethod, Listener, RouteDescriptor,
    RouteEffect, RouteProjections,
};

/// Read MCP server capabilities.
pub const CAPABILITIES: RouteDescriptor = RouteDescriptor::new(
    "protocol.mcp.capabilities",
    HttpMethod::Get,
    "/v1/mcp",
    ApiSurface::Protocol,
    Listener::Torii,
    RouteEffect::ReadOnly,
    AdmissionPolicy::Public,
)
.with_projections(RouteProjections::OPENAPI)
.with_implicit_head(true)
.with_cors_options(true);

/// Execute a bounded MCP JSON-RPC request through its exact cataloged target.
pub const JSON_RPC: RouteDescriptor = RouteDescriptor::new(
    "protocol.mcp.json_rpc",
    HttpMethod::Post,
    "/v1/mcp",
    ApiSurface::Protocol,
    Listener::Torii,
    RouteEffect::Mutation,
    AdmissionPolicy::TargetRoute,
)
.with_authentication(AuthenticationPolicy::NestedRouteAuthentication)
.with_projections(RouteProjections::OPENAPI)
.with_cors_options(true);

/// Canonical native MCP route set.
pub const ROUTES: &[RouteDescriptor] = &[CAPABILITIES, JSON_RPC];
