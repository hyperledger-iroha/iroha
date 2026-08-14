use crate::{
    canonical::{canonicalize_norito_bytes, sha256_domain_digest},
    limits::{
        MAX_CHILD_STRINGS, MAX_FIELD_BYTES, MAX_IDENTIFIER_BYTES, MAX_RAD_ENTRIES,
        MAX_RAD_SNAPSHOT_BYTES, rad_snapshot_decode_limits,
    },
};
use eyre::{Result, WrapErr};
use iroha_data_model::soradns::{
    GatewayHostSet, RAD_VERSION_V1, ResolverAttestationDocumentV1, ResolverTransportBundle,
};
use iroha_primitives::soradns::derive_gateway_hosts;
use norito::{decode_from_bytes_with_limits, json};
use thiserror::Error;
/// Convenience alias for the SoraDNS RAD payload.
pub type ResolverAttestation = ResolverAttestationDocumentV1;
/// Domain separator used when hashing RAD payloads.
pub const RAD_HASH_DOMAIN: &[u8] = b"rad-v1";
/// Decode a RAD payload from Norito bytes.
pub fn decode_rad_entries(bytes: &[u8]) -> Result<Vec<ResolverAttestation>> {
    if bytes.len() > MAX_RAD_SNAPSHOT_BYTES {
        eyre::bail!(
            "resolver attestation snapshot exceeds the {MAX_RAD_SNAPSHOT_BYTES}-byte limit"
        );
    }
    let entries: Vec<ResolverAttestation> =
        decode_from_bytes_with_limits(bytes, rad_snapshot_decode_limits())
            .wrap_err("failed to decode resolver attestation entries")?;
    if entries.len() > MAX_RAD_ENTRIES {
        eyre::bail!(
            "resolver attestation snapshot contains {} entries; the limit is {MAX_RAD_ENTRIES}",
            entries.len()
        );
    }
    for entry in &entries {
        validate_rad_resource_bounds(entry).wrap_err_with(|| {
            format!("resolver attestation `{}` exceeds field limits", entry.fqdn)
        })?;
    }
    Ok(entries)
}
/// Perform structural validation for a RAD entry before it is added to state.
pub fn validate_rad(rad: &ResolverAttestation) -> Result<(), ResolverAttestationValidationError> {
    validate_rad_resource_bounds(rad)?;
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
/// Validate all variable-width fields in one decoded RAD.
pub(crate) fn validate_rad_resource_bounds(
    rad: &ResolverAttestation,
) -> Result<(), ResolverAttestationValidationError> {
    check_string("fqdn", &rad.fqdn, MAX_IDENTIFIER_BYTES)?;
    check_string(
        "canonical_label",
        &rad.canonical_hosts.canonical_label,
        MAX_IDENTIFIER_BYTES,
    )?;
    check_string(
        "canonical_host",
        &rad.canonical_hosts.canonical_host,
        MAX_IDENTIFIER_BYTES,
    )?;
    check_string(
        "canonical_wildcard",
        &rad.canonical_hosts.canonical_wildcard,
        MAX_IDENTIFIER_BYTES,
    )?;
    check_string(
        "pretty_host",
        &rad.canonical_hosts.pretty_host,
        MAX_IDENTIFIER_BYTES,
    )?;
    if let Some(doh) = &rad.transport.doh {
        check_string("DoH endpoint", &doh.endpoint, MAX_FIELD_BYTES)?;
    }
    if let Some(dot) = &rad.transport.dot {
        check_string("DoT endpoint", &dot.endpoint, MAX_FIELD_BYTES)?;
        check_string_vec("DoT ALPN protocols", &dot.alpn_protocols)?;
        check_string_vec("DoT cipher suites", &dot.cipher_suites)?;
    }
    if let Some(doq) = &rad.transport.doq {
        check_string("DoQ endpoint", &doq.endpoint, MAX_FIELD_BYTES)?;
        if let Some(profile) = &doq.congestion_profile {
            check_string("DoQ congestion profile", profile, MAX_IDENTIFIER_BYTES)?;
        }
    }
    if let Some(relay) = &rad.transport.odoh_relay {
        check_string("ODoH relay endpoint", &relay.endpoint, MAX_FIELD_BYTES)?;
        check_string("ODoH relay key id", &relay.key_id, MAX_IDENTIFIER_BYTES)?;
        check_len(
            "ODoH relay public key",
            relay.public_key.len(),
            MAX_FIELD_BYTES,
        )?;
    }
    if let Some(bridge) = &rad.transport.soranet_bridge {
        check_string(
            "SoraNet bridge multiaddr",
            &bridge.multiaddr,
            MAX_FIELD_BYTES,
        )?;
        check_string(
            "SoraNet bridge circuit policy",
            &bridge.circuit_policy,
            MAX_IDENTIFIER_BYTES,
        )?;
    }
    check_len(
        "TLS provisioning profiles",
        rad.tls.provisioning_profiles.len(),
        MAX_CHILD_STRINGS,
    )?;
    check_string_vec(
        "TLS certificate fingerprints",
        &rad.tls.certificate_fingerprints,
    )?;
    check_string_vec("TLS wildcard hosts", &rad.tls.wildcard_hosts)?;
    validate_operator_account_bounds(rad)?;
    check_len(
        "operator signature",
        rad.operator_signature.payload().len(),
        MAX_FIELD_BYTES,
    )?;
    check_len(
        "governance signature",
        rad.governance_signature.payload().len(),
        MAX_FIELD_BYTES,
    )?;
    if let Some(endpoint) = &rad.telemetry_endpoint {
        check_string("telemetry endpoint", endpoint, MAX_FIELD_BYTES)?;
    }
    Ok(())
}
/// Account the heap retained by one RAD, including decoded spare capacities.
pub(crate) fn rad_retained_bytes(
    rad: &ResolverAttestation,
) -> Result<usize, ResolverAttestationValidationError> {
    validate_rad_resource_bounds(rad)?;
    let mut bytes = std::mem::size_of::<ResolverAttestation>()
        // Account for allocator rounding hidden by compact crypto wrappers.
        .checked_add(4096)
        .ok_or(ResolverAttestationValidationError::RetainedSizeOverflow)?;
    for value in [
        &rad.fqdn,
        &rad.canonical_hosts.canonical_label,
        &rad.canonical_hosts.canonical_host,
        &rad.canonical_hosts.canonical_wildcard,
        &rad.canonical_hosts.pretty_host,
    ] {
        charge(&mut bytes, value.capacity())?;
    }
    if let Some(doh) = &rad.transport.doh {
        charge(&mut bytes, doh.endpoint.capacity())?;
    }
    if let Some(dot) = &rad.transport.dot {
        charge(&mut bytes, dot.endpoint.capacity())?;
        charge_string_vec(
            &mut bytes,
            &dot.alpn_protocols,
            dot.alpn_protocols.capacity(),
        )?;
        charge_string_vec(&mut bytes, &dot.cipher_suites, dot.cipher_suites.capacity())?;
    }
    if let Some(doq) = &rad.transport.doq {
        charge(&mut bytes, doq.endpoint.capacity())?;
        if let Some(profile) = &doq.congestion_profile {
            charge(&mut bytes, profile.capacity())?;
        }
    }
    if let Some(relay) = &rad.transport.odoh_relay {
        charge(&mut bytes, relay.endpoint.capacity())?;
        charge(&mut bytes, relay.key_id.capacity())?;
        charge(&mut bytes, relay.public_key.capacity())?;
    }
    if let Some(bridge) = &rad.transport.soranet_bridge {
        charge(&mut bytes, bridge.multiaddr.capacity())?;
        charge(&mut bytes, bridge.circuit_policy.capacity())?;
    }
    charge_vec_capacity::<iroha_data_model::soradns::TlsProvisioningProfile>(
        &mut bytes,
        rad.tls.provisioning_profiles.capacity(),
    )?;
    charge_string_vec(
        &mut bytes,
        &rad.tls.certificate_fingerprints,
        rad.tls.certificate_fingerprints.capacity(),
    )?;
    charge_string_vec(
        &mut bytes,
        &rad.tls.wildcard_hosts,
        rad.tls.wildcard_hosts.capacity(),
    )?;
    charge_operator_account(&mut bytes, rad)?;
    charge(&mut bytes, rad.operator_signature.payload().len())?;
    charge(&mut bytes, rad.governance_signature.payload().len())?;
    if let Some(endpoint) = &rad.telemetry_endpoint {
        charge(&mut bytes, endpoint.capacity())?;
    }
    Ok(bytes)
}
fn validate_operator_account_bounds(
    rad: &ResolverAttestation,
) -> Result<(), ResolverAttestationValidationError> {
    let controller = &rad.operator_account.controller;
    if let Some(key) = controller.single_signatory() {
        check_len(
            "operator account public key",
            public_key_payload_len(key)?,
            MAX_FIELD_BYTES,
        )?;
        return Ok(());
    }
    let policy = controller
        .multisig_policy()
        .ok_or(ResolverAttestationValidationError::MalformedCryptoMaterial)?;
    check_len(
        "operator account multisig members",
        policy.members().len(),
        MAX_CHILD_STRINGS,
    )?;
    for member in policy.members() {
        check_len(
            "operator account member public key",
            public_key_payload_len(member.public_key())?,
            MAX_FIELD_BYTES,
        )?;
    }
    Ok(())
}
fn charge_operator_account(
    total: &mut usize,
    rad: &ResolverAttestation,
) -> Result<(), ResolverAttestationValidationError> {
    let controller = &rad.operator_account.controller;
    if let Some(key) = controller.single_signatory() {
        return charge(total, public_key_payload_len(key)?);
    }
    let policy = controller
        .multisig_policy()
        .ok_or(ResolverAttestationValidationError::MalformedCryptoMaterial)?;
    charge_vec_capacity::<iroha_data_model::account::MultisigMember>(
        total,
        policy.members().len(),
    )?;
    for member in policy.members() {
        charge(total, public_key_payload_len(member.public_key())?)?;
    }
    Ok(())
}
fn public_key_payload_len(
    key: &iroha_crypto::PublicKey,
) -> Result<usize, ResolverAttestationValidationError> {
    key.try_to_bytes()
        .map(|(_, payload)| payload.len())
        .map_err(|_| ResolverAttestationValidationError::MalformedCryptoMaterial)
}
fn check_string(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), ResolverAttestationValidationError> {
    check_len(field, value.len(), maximum)
}
fn check_string_vec(
    field: &'static str,
    values: &[String],
) -> Result<(), ResolverAttestationValidationError> {
    check_len(field, values.len(), MAX_CHILD_STRINGS)?;
    for value in values {
        check_string(field, value, MAX_IDENTIFIER_BYTES)?;
    }
    Ok(())
}
fn check_len(
    field: &'static str,
    found: usize,
    maximum: usize,
) -> Result<(), ResolverAttestationValidationError> {
    if found > maximum {
        return Err(ResolverAttestationValidationError::ResourceLimit {
            field,
            found,
            maximum,
        });
    }
    Ok(())
}
fn charge(total: &mut usize, additional: usize) -> Result<(), ResolverAttestationValidationError> {
    *total = (*total)
        .checked_add(additional)
        .ok_or(ResolverAttestationValidationError::RetainedSizeOverflow)?;
    Ok(())
}
fn charge_vec_capacity<T>(
    total: &mut usize,
    capacity: usize,
) -> Result<(), ResolverAttestationValidationError> {
    let bytes = capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(ResolverAttestationValidationError::RetainedSizeOverflow)?;
    charge(total, bytes)
}
fn charge_string_vec(
    total: &mut usize,
    values: &[String],
    capacity: usize,
) -> Result<(), ResolverAttestationValidationError> {
    charge_vec_capacity::<String>(total, capacity)?;
    for value in values {
        charge(total, value.capacity())?;
    }
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
    /// A variable-width field or child collection exceeded its first-release limit.
    #[error("{field} contains {found} entries/bytes; the limit is {maximum}")]
    ResourceLimit {
        /// Stable field label used in diagnostics.
        field: &'static str,
        /// Observed element or byte count.
        found: usize,
        /// Maximum admitted element or byte count.
        maximum: usize,
    },
    /// Retained-memory accounting overflowed `usize`.
    #[error("resolver attestation retained-byte accounting overflowed")]
    RetainedSizeOverflow,
    /// A decoded account key could not expose canonical crypto material.
    #[error("resolver attestation contains malformed crypto material")]
    MalformedCryptoMaterial,
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
    use super::*;
    use expect_test::expect;
    use iroha_crypto::{PublicKey, Signature};
    use iroha_data_model::{
        account::AccountId,
        soradns::{
            HttpTransportV1, PaddingPolicyV1, ResolverTlsBundle, RotationPolicyV1,
            TlsProvisioningProfile, TlsTransportV1,
        },
    };
    fn base_rad() -> ResolverAttestation {
        let bindings = derive_gateway_hosts("docs.sora").expect("derive hosts");
        let operator_account = {
            let public_key: PublicKey =
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                    .parse()
                    .expect("valid public key literal");
            AccountId::new(public_key)
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
            operator_signature: Signature::try_from_bytes(&[0xA6; 64])
                .expect("RAD fixture operator signature is non-empty and nonzero"),
            governance_signature: Signature::try_from_bytes(&[1; 64])
                .expect("RAD fixture governance signature is non-empty and nonzero"),
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
        expect!["90ea6fb553abd091527d60c89ca8acf637b551abd127f9632878b4ef741a34da"]
            .assert_eq(&hex::encode(digest));
    }
    #[test]
    fn rad_collection_limit_accepts_exact_and_rejects_plus_one() {
        check_len("RAD entries", MAX_RAD_ENTRIES, MAX_RAD_ENTRIES)
            .expect("exact RAD count is admitted");
        assert!(matches!(
            check_len("RAD entries", MAX_RAD_ENTRIES + 1, MAX_RAD_ENTRIES),
            Err(ResolverAttestationValidationError::ResourceLimit {
                field: "RAD entries",
                found,
                maximum: MAX_RAD_ENTRIES,
            }) if found == MAX_RAD_ENTRIES + 1
        ));
    }
    #[test]
    fn rad_crypto_field_limit_accepts_exact_and_rejects_plus_one() {
        let mut rad = base_rad();
        rad.operator_signature = Signature::from_bytes(&vec![1; MAX_FIELD_BYTES]);
        validate_rad_resource_bounds(&rad).expect("exact signature boundary");
        rad.operator_signature = Signature::from_bytes(&vec![1; MAX_FIELD_BYTES + 1]);
        assert!(matches!(
            validate_rad_resource_bounds(&rad),
            Err(ResolverAttestationValidationError::ResourceLimit {
                field: "operator signature",
                found,
                maximum: MAX_FIELD_BYTES,
            }) if found == MAX_FIELD_BYTES + 1
        ));
    }
}
