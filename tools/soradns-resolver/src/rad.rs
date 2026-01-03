use eyre::{Result, WrapErr};
use iroha_data_model::soradns::{
    GatewayHostSet, RAD_VERSION_V1, ResolverAttestationDocumentV1, ResolverTransportBundle,
};
use iroha_primitives::soradns::derive_gateway_hosts;
use norito::{decode_from_bytes, json};
use thiserror::Error;

use crate::canonical::{canonicalize_norito_bytes, sha256_domain_digest};

/// Convenience alias for the SoraDNS RAD payload.
pub type ResolverAttestation = ResolverAttestationDocumentV1;

/// Domain separator used when hashing RAD payloads.
pub const RAD_HASH_DOMAIN: &[u8] = b"rad-v1";

/// Decode a RAD payload from Norito bytes.
pub fn decode_rad_entries(bytes: &[u8]) -> Result<Vec<ResolverAttestation>> {
    decode_from_bytes(bytes).wrap_err("failed to decode resolver attestation entries")
}

/// Perform structural validation for a RAD entry before it is added to state.
pub fn validate_rad(rad: &ResolverAttestation) -> Result<(), ResolverAttestationValidationError> {
    if rad.version != RAD_VERSION_V1 {
        return Err(ResolverAttestationValidationError::UnsupportedVersion { found: rad.version });
    }

    if rad.fqdn.trim().is_empty() {
        return Err(ResolverAttestationValidationError::EmptyFqdn);
    }

    if rad.valid_from_unix >= rad.valid_until_unix {
        return Err(ResolverAttestationValidationError::InvalidValidityWindow {
            valid_from: rad.valid_from_unix,
            valid_until: rad.valid_until_unix,
        });
    }

    if rad.rotation_policy.max_lifetime_days == 0 {
        return Err(ResolverAttestationValidationError::InvalidRotationPolicy);
    }

    if rad.rotation_policy.required_overlap_seconds == 0 {
        return Err(ResolverAttestationValidationError::InvalidRotationPolicy);
    }

    if let Some(endpoint) = &rad.telemetry_endpoint
        && endpoint.trim().is_empty()
    {
        return Err(ResolverAttestationValidationError::InvalidTelemetryEndpoint);
    }

    let derived = derive_gateway_hosts(&rad.fqdn)
        .map_err(ResolverAttestationValidationError::DerivedHostFailure)?;
    let expected = GatewayHostSet::from(&derived);
    if rad.canonical_hosts != expected {
        return Err(ResolverAttestationValidationError::HostMismatch {
            expected: Box::new(expected),
            found: Box::new(rad.canonical_hosts.clone()),
        });
    }

    validate_transport_bundle(&rad.transport)?;

    Ok(())
}

/// Compute the canonical digest of a RAD entry (matching the release tooling).
pub fn compute_rad_digest(rad: &ResolverAttestation) -> Result<[u8; 32]> {
    let value = json::to_value(rad).wrap_err("failed to convert RAD into JSON value")?;
    let canonical_bytes = canonicalize_norito_bytes(&value)
        .map_err(eyre::Error::from)
        .wrap_err("failed to canonicalize RAD JSON")?;
    Ok(sha256_domain_digest(RAD_HASH_DOMAIN, &canonical_bytes))
}

fn validate_transport_bundle(
    bundle: &ResolverTransportBundle,
) -> Result<(), ResolverAttestationValidationError> {
    if bundle.doh.is_none()
        && bundle.dot.is_none()
        && bundle.doq.is_none()
        && bundle.soranet_bridge.is_none()
    {
        return Err(ResolverAttestationValidationError::MissingTransport);
    }
    if bundle.padding_policy.min_bytes > bundle.padding_policy.max_bytes {
        return Err(ResolverAttestationValidationError::InvalidPaddingPolicy);
    }
    Ok(())
}

/// Structural validation errors surfaced when ingesting a RAD entry.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolverAttestationValidationError {
    #[error("unsupported RAD version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("resolver fqdn must not be empty")]
    EmptyFqdn,
    #[error("invalid validity window: valid_from={valid_from}, valid_until={valid_until}")]
    InvalidValidityWindow { valid_from: u64, valid_until: u64 },
    #[error("rotation policy must specify positive lifetime/overlap windows")]
    InvalidRotationPolicy,
    #[error("telemetry endpoint must not be empty when provided")]
    InvalidTelemetryEndpoint,
    #[error("resolver attestation missing transport definitions")]
    MissingTransport,
    #[error("padding policy min_bytes must be ≤ max_bytes")]
    InvalidPaddingPolicy,
    #[error("canonical host set does not match derived value")]
    HostMismatch {
        /// Expected canonical host set derived from the resolver FQDN.
        expected: Box<GatewayHostSet>,
        /// Host set advertised by the RAD (boxed to keep the enum small).
        found: Box<GatewayHostSet>,
    },
    #[error("failed to derive gateway hosts: {0}")]
    DerivedHostFailure(#[from] iroha_primitives::soradns::GatewayHostError),
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use iroha_crypto::{PublicKey, Signature};
    use iroha_data_model::{
        account::AccountId,
        domain::DomainId,
        soradns::{
            HttpTransportV1, PaddingPolicyV1, ResolverTlsBundle, RotationPolicyV1,
            TlsProvisioningProfile, TlsTransportV1,
        },
    };

    use super::*;

    fn base_rad() -> ResolverAttestation {
        let bindings = derive_gateway_hosts("docs.sora").expect("derive hosts");
        let operator_account = {
            let domain: DomainId = "sora".parse().expect("valid domain");
            let public_key: PublicKey =
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                    .parse()
                    .expect("valid public key literal");
            AccountId::new(domain, public_key)
        };
        ResolverAttestation {
            version: RAD_VERSION_V1,
            resolver_id: [1; 32],
            fqdn: "docs.sora".to_string(),
            canonical_hosts: GatewayHostSet::from(&bindings),
            transport: ResolverTransportBundle {
                doh: Some(HttpTransportV1 {
                    endpoint: "https://docs.sora/dns-query".to_string(),
                    supports_get: true,
                    supports_post: true,
                    max_response_bytes: 2048,
                }),
                dot: Some(TlsTransportV1 {
                    endpoint: "tls://docs.sora:853".to_string(),
                    alpn_protocols: vec!["dot".to_string()],
                    cipher_suites: vec!["TLS_AES_256_GCM_SHA384".to_string()],
                }),
                doq: None,
                odoh_relay: None,
                soranet_bridge: None,
                qname_minimisation: true,
                padding_policy: PaddingPolicyV1 {
                    min_bytes: 32,
                    max_bytes: 64,
                    pad_to_block: 16,
                },
            },
            tls: ResolverTlsBundle {
                provisioning_profiles: vec![TlsProvisioningProfile::Dns01],
                certificate_fingerprints: vec!["fp".to_string()],
                wildcard_hosts: vec!["*.gw.sora.id".to_string()],
                not_after_unix: 1_800_000_000,
            },
            resolver_manifest_hash: [2; 32],
            gar_manifest_hash: [3; 32],
            issued_at_unix: 1_700_000_000,
            valid_from_unix: 1_700_000_000,
            valid_until_unix: 1_700_086_400,
            operator_account,
            operator_signature: Signature::from_bytes(&[0; 64]),
            governance_signature: Signature::from_bytes(&[1; 64]),
            rotation_policy: RotationPolicyV1 {
                max_lifetime_days: 30,
                required_overlap_seconds: 86_400,
                require_dual_signatures: true,
            },
            telemetry_endpoint: None,
        }
    }

    #[test]
    fn rad_validation_succeeds() {
        let rad = base_rad();
        assert!(validate_rad(&rad).is_ok());
    }

    #[test]
    fn rad_detects_host_mismatch() {
        let mut rad = base_rad();
        rad.canonical_hosts.pretty_host = "example.com".to_string();
        let err = validate_rad(&rad).expect_err("validation must fail");
        assert!(matches!(
            err,
            ResolverAttestationValidationError::HostMismatch { .. }
        ));
    }

    #[test]
    fn rad_digest_remains_stable() {
        let rad = base_rad();
        let digest = compute_rad_digest(&rad).expect("digest");
        expect!["c5fbc711e0ea79ddf2afe0a09afbd81fe84a87f3164e149cbf7ba09a8dc318a5"]
            .assert_eq(&hex::encode(digest));
    }
}
