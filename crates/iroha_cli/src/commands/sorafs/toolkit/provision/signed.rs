//! Public signatures using native protocol preimages and independent public trust policies.

use super::{
    InputArgs, MAX_PUBLIC_BYTES, PolicyArgs, ed25519_public, parse_digest, read_pinned,
    read_private_key, write_new,
};
use eyre::{Result, ensure, eyre};
use iroha_crypto::{PrivateKey, Signature};
use iroha_data_model::{
    NetworkId,
    sorafs::{bootstrap::SorafsBootstrapPayloadV1, provider_admission::governance::decode_frame},
};
use sorafs_manifest::{
    gateway_compliance::{
        GatewayComplianceAcknowledgementPayloadV1, GatewayComplianceAcknowledgementV1,
        GatewayComplianceCatalogApprovalV1, GatewayComplianceCatalogPayloadV1,
        GatewayComplianceCatalogV1, GatewayComplianceTrustPolicyV1,
        gateway_compliance_feed_transport_policy_digest,
    },
    provider_admission::ProviderAdmissionEnvelopeV1,
    provider_advert::{AdvertSignature, ProviderAdvertV1, SignatureAlgorithm},
};
use std::path::PathBuf;

/// Native digests for an independently pinned trust policy with no external feeds.
#[derive(Debug, clap::Args)]
pub struct CompliancePolicyArgs {
    /// Public trust policy; no credential is read by this command.
    #[command(flatten)]
    policy: PolicyArgs,
    /// Fresh public digest receipt. The transport digest is for an empty feed inventory.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

pub(super) fn compliance_policy(args: CompliancePolicyArgs) -> Result<()> {
    let policy = args.policy.load()?;
    let receipt = norito::json!({
        "schema": "sorafs.provision.compliance-policy-digests.v1",
        "policy_id_hex": (hex::encode(policy.policy_id)),
        "policy_digest_hex": (hex::encode(policy.canonical_digest()?)),
        "feed_transport_policy_digest_hex": (hex::encode(
            gateway_compliance_feed_transport_policy_digest(&std::collections::BTreeMap::new())?
        )),
        "feed_host_count": 0,
    });
    write_new(&args.output, &norito::json::to_vec(&receipt)?)
}

/// Native compliance catalog payload, independent policy and runtime signer custody.
#[derive(Debug, clap::Args)]
pub struct CatalogArgs {
    /// Pinned native GatewayComplianceCatalogPayloadV1 request JSON and fresh output.
    #[command(flatten)]
    input: InputArgs,
    /// Independent policy used to verify signatures and authorize the selected public keys.
    #[command(flatten)]
    policy: PolicyArgs,
    /// Owner-private native Ed25519 key; repeat for exactly the configured catalog threshold.
    #[arg(long, value_name = "PATH", required = true)]
    catalog_key_file: Vec<PathBuf>,
}

/// An explicit gateway result bound to a verified catalog and reviewed trust policy.
#[derive(Debug, clap::Args)]
pub struct AcknowledgementArgs {
    /// Pinned native GatewayComplianceAcknowledgementPayloadV1 request and fresh output.
    #[command(flatten)]
    input: InputArgs,
    /// Independent trust policy.
    #[command(flatten)]
    policy: PolicyArgs,
    /// Native signed GatewayComplianceCatalogV1 JSON.
    #[arg(long, value_name = "PATH")]
    catalog_json: PathBuf,
    /// Independently reviewed BLAKE3 of the signed catalog file.
    #[arg(long, value_parser = parse_digest)]
    catalog_blake3_hex: [u8; 32],
    /// Allowed observation skew, chosen to match the gateway controller (0 through 300 seconds).
    #[arg(long, value_parser = clap::value_parser!(u64).range(0..=300))]
    max_clock_skew_secs: u64,
    /// Owner-private native gateway acknowledgement key.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
}

/// Initial advert from the exact reviewed bootstrap admission and its advert credential.
#[derive(Debug, clap::Args)]
pub struct AdvertArgs {
    /// Independently selected network identity.
    #[arg(long)]
    network_id: NetworkId,
    /// Public bootstrap payload JSON with its signed initial admissions.
    #[arg(long, value_name = "PATH")]
    bootstrap_payload_json: PathBuf,
    /// Independently reviewed BLAKE3 of the bootstrap payload file.
    #[arg(long, value_parser = parse_digest)]
    bootstrap_payload_blake3_hex: [u8; 32],
    /// Exact provider id, lowercase 64-character hex.
    #[arg(long, value_parser = parse_digest)]
    provider_id_hex: [u8; 32],
    /// Explicit issued-at time in Unix seconds.
    #[arg(long)]
    issued_at: u64,
    /// Explicit exclusive advert expiry in Unix seconds, within admission and TLS validity.
    #[arg(long)]
    expires_at: u64,
    /// Owner-private native Ed25519 advert signing credential.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
    /// Fresh canonical Norito ProviderAdvertV1 output.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl PolicyArgs {
    fn load(&self) -> Result<GatewayComplianceTrustPolicyV1> {
        let policy: GatewayComplianceTrustPolicyV1 = norito::json::from_slice(&read_pinned(
            &self.policy_json,
            MAX_PUBLIC_BYTES,
            self.policy_blake3_hex,
        )?)?;
        policy.validate()?;
        Ok(policy)
    }
}

pub(super) fn catalog(args: CatalogArgs, now: u64) -> Result<()> {
    let policy = args.policy.load()?;
    let payload: GatewayComplianceCatalogPayloadV1 = args.input.read()?;
    payload.validate()?;
    ensure!(
        payload.policy_digest == policy.canonical_digest()?
            && payload.generated_at_unix <= now
            && now < payload.valid_until_unix,
        "catalog policy or validity does not match the independently selected context"
    );
    ensure!(
        args.catalog_key_file.len() == usize::from(policy.catalog_threshold),
        "supply exactly the catalog threshold of credentials"
    );
    let keys = args
        .catalog_key_file
        .iter()
        .map(|path| read_private_key(path))
        .collect::<Result<Vec<_>>>()?;
    let signed = sign_catalog(payload, &policy, &keys, now)?;
    println!(
        "catalog_digest_hex={}",
        hex::encode(signed.verify(&policy, now, 0)?)
    );
    write_new(&args.input.output, &norito::json::to_vec(&signed)?)
}

fn sign_catalog(
    payload: GatewayComplianceCatalogPayloadV1,
    policy: &GatewayComplianceTrustPolicyV1,
    keys: &[PrivateKey],
    now: u64,
) -> Result<GatewayComplianceCatalogV1> {
    policy.validate()?;
    ensure!(
        keys.len() == usize::from(policy.catalog_threshold),
        "catalog key count must equal threshold"
    );
    ensure!(
        payload.policy_digest == policy.canonical_digest()?,
        "catalog policy binding mismatch"
    );
    let digest = payload.signing_digest()?;
    let mut approvals = Vec::new();
    for key in keys {
        let public = ed25519_public(key)?;
        let trusted = policy
            .catalog_signers
            .iter()
            .find(|signer| signer.public_key == public)
            .ok_or_else(|| eyre!("catalog credential is outside the selected trust policy"))?;
        ensure!(
            !policy
                .revoked_catalog_signer_ids
                .contains(&trusted.signer_id),
            "catalog signer is revoked"
        );
        let signature =
            Signature::try_new(key, &digest).map_err(|_| eyre!("catalog signature failed"))?;
        approvals.push(GatewayComplianceCatalogApprovalV1 {
            version: 1,
            signer_id: trusted.signer_id.clone(),
            signature: signature
                .payload()
                .try_into()
                .map_err(|_| eyre!("invalid catalog signature length"))?,
        });
    }
    approvals.sort_by(|a, b| a.signer_id.cmp(&b.signer_id));
    let catalog = GatewayComplianceCatalogV1 { payload, approvals };
    catalog.verify(policy, now, 0)?;
    Ok(catalog)
}

pub(super) fn acknowledgement(args: AcknowledgementArgs, now: u64) -> Result<()> {
    let policy = args.policy.load()?;
    let catalog: GatewayComplianceCatalogV1 = norito::json::from_slice(&read_pinned(
        &args.catalog_json,
        MAX_PUBLIC_BYTES,
        args.catalog_blake3_hex,
    )?)?;
    let digest = catalog.verify(&policy, now, 0)?;
    let payload: GatewayComplianceAcknowledgementPayloadV1 = args.input.read()?;
    payload.signing_digest()?;
    ensure!(
        payload.catalog_digest == digest
            && payload.observed_at_unix.abs_diff(now) <= args.max_clock_skew_secs,
        "acknowledgement catalog or observation time mismatch"
    );
    ensure!(
        policy
            .gateway_signers
            .iter()
            .any(|signer| signer.signer_id == payload.gateway_id)
            && !policy
                .revoked_gateway_signer_ids
                .contains(&payload.gateway_id),
        "unknown or revoked gateway acknowledgement identity"
    );
    let key = read_private_key(&args.private_key_file)?;
    let signed = sign_acknowledgement(
        payload,
        &policy,
        digest,
        &key,
        now,
        args.max_clock_skew_secs,
    )?;
    write_new(&args.input.output, &norito::json::to_vec(&signed)?)
}

fn sign_acknowledgement(
    payload: GatewayComplianceAcknowledgementPayloadV1,
    policy: &GatewayComplianceTrustPolicyV1,
    digest: [u8; 32],
    key: &PrivateKey,
    now: u64,
    skew: u64,
) -> Result<GatewayComplianceAcknowledgementV1> {
    let public = ed25519_public(key)?;
    ensure!(
        policy
            .gateway_signers
            .iter()
            .any(|signer| signer.signer_id == payload.gateway_id && signer.public_key == public),
        "gateway key does not match the selected gateway identity"
    );
    let signature = Signature::try_new(key, &payload.signing_digest()?)
        .map_err(|_| eyre!("gateway acknowledgement signature failed"))?;
    let acknowledgement = GatewayComplianceAcknowledgementV1 {
        payload,
        signature: signature
            .payload()
            .try_into()
            .map_err(|_| eyre!("invalid acknowledgement signature length"))?,
    };
    acknowledgement.verify(policy, digest, now, skew)?;
    Ok(acknowledgement)
}

pub(super) fn advert(args: AdvertArgs, now: u64) -> Result<()> {
    let payload: SorafsBootstrapPayloadV1 = norito::json::from_slice(&read_pinned(
        &args.bootstrap_payload_json,
        MAX_PUBLIC_BYTES,
        args.bootstrap_payload_blake3_hex,
    )?)?;
    super::super::bootstrap::validate_payload(&payload, &args.network_id, now)?;
    let envelope = payload
        .providers
        .iter()
        .map(|provider| decode_frame::<ProviderAdmissionEnvelopeV1>(&provider.envelope))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .find(|envelope| envelope.proposal.provider_id == args.provider_id_hex)
        .ok_or_else(|| eyre!("provider is not present in the reviewed bootstrap payload"))?;
    validate_advert_window(&envelope, args.issued_at, args.expires_at, now)?;
    let key = read_private_key(&args.private_key_file)?;
    let advert = sign_advert(
        &envelope,
        &args.network_id,
        args.issued_at,
        args.expires_at,
        now,
        &key,
    )?;
    write_new(&args.output, &norito::encode_canonical(&advert)?)
}

fn validate_advert_window(
    envelope: &ProviderAdmissionEnvelopeV1,
    issued_at: u64,
    expires_at: u64,
    now: u64,
) -> Result<()> {
    ensure!(
        envelope.issued_at <= issued_at
            && issued_at <= now
            && now < expires_at
            && expires_at <= envelope.retention_epoch
            && envelope
                .proposal
                .endpoints
                .iter()
                .all(|endpoint| endpoint.attestation.attested_at <= issued_at
                    && expires_at <= endpoint.attestation.expires_at),
        "advert lifetime must fit admission and every endpoint attestation and contain current time"
    );
    Ok(())
}

fn sign_advert(
    envelope: &ProviderAdmissionEnvelopeV1,
    network: &NetworkId,
    issued_at: u64,
    expires_at: u64,
    now: u64,
    key: &PrivateKey,
) -> Result<ProviderAdvertV1> {
    validate_advert_window(envelope, issued_at, expires_at, now)?;
    ensure!(
        envelope.network_id == *network.as_bytes(),
        "advert network differs from admission"
    );
    let public = ed25519_public(key)?;
    ensure!(
        public == envelope.proposal.advert_key,
        "advert credential does not match admitted public key"
    );
    let mut advert = ProviderAdvertV1 {
        version: 1,
        network_id: *network.as_bytes(),
        issued_at,
        expires_at,
        body: envelope.advert_body.clone(),
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public.to_vec(),
            signature: Vec::new(),
        },
        signature_strict: true,
        allow_unknown_capabilities: false,
    };
    advert.signature.signature = Signature::try_new(key, &advert.signature_payload_bytes()?)
        .map_err(|_| eyre!("advert signature failed"))?
        .payload()
        .to_vec();
    advert.validate(now)?;
    advert.verify_signature()?;
    Ok(advert)
}

#[cfg(test)]
mod tests;
