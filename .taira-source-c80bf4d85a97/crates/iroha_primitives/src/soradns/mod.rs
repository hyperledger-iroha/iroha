//! Shared `SoraDNS` utilities used across client SDKs and tooling.
//!
//! The helpers in this module describe the deterministic hostname mapping
//! between registered `SoraDNS` names and the HTTPS gateways that serve their
//! content. Clients can derive the canonical and pretty gateway hosts from a
//! fully-qualified domain name (FQDN) without consulting a resolver.

pub mod hosts;

pub use hosts::{
    GatewayHostBindings, GatewayHostError, GatewayHostProfile, canonical_gateway_suffix,
    canonical_gateway_wildcard_pattern, derive_gateway_hosts, derive_gateway_hosts_with_profile,
    pretty_gateway_suffix, taira_mon_pretty_gateway_suffix,
};
