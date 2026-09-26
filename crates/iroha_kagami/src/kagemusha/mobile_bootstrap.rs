//! Operator-owned canonical mobile bootstrap preparation, partial signing and assembly.

use super::*;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::consensus_v2::HeightContextId,
    kagemusha::{
        KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1, KagemushaMobileBootstrapApprovalV1,
        KagemushaMobileBootstrapCheckpointV1, KagemushaMobileBootstrapPackageV1,
        KagemushaMobileBootstrapPinsV1, KagemushaMobileBootstrapScopeV1,
    },
};

#[derive(Debug, ClapArgs)]
pub(super) struct PrepareArgs {
    #[command(flatten)]
    release: AuthenticateExperimentalReleaseV1Args,
    /// Independently verified first consensus height-context identity as lowercase hex.
    #[arg(long, value_name = "LOWER_HEX")]
    first_context_id: String,
    /// New monotonically increasing deployment checkpoint sequence; nonzero.
    #[arg(long)]
    sequence: u64,
    /// Inclusive authority issuance time in Unix milliseconds.
    #[arg(long)]
    issued_at_ms: u64,
    /// Exclusive authority expiry time in Unix milliseconds.
    #[arg(long)]
    expires_at_ms: u64,
    /// Current trusted operator time; not read from the checkpoint or handset clock.
    #[arg(long)]
    trusted_now_ms: u64,
    /// New owner-only file for the unsigned canonical checkpoint.
    #[arg(long, value_name = "PATH")]
    checkpoint_output: PathBuf,
}

#[derive(Clone, Debug, ClapArgs)]
struct CheckpointInputs {
    /// Canonical unsigned checkpoint prepared from the authenticated release.
    #[arg(long, value_name = "PATH")]
    checkpoint: PathBuf,
    /// Independently selected canonical release-authority policy.
    #[arg(long, value_name = "PATH")]
    authority_policy: PathBuf,
    #[command(flatten)]
    deployment: ExperimentalOperatorPinsV1,
    /// Independently authenticated exact release-attestation digest.
    #[arg(long, value_name = "LOWER_HEX")]
    expected_release_attestation_digest: String,
    /// Independently verified first consensus height-context identity.
    #[arg(long, value_name = "LOWER_HEX")]
    expected_first_context_id: String,
    /// Exact reviewed deployment checkpoint sequence, not a downloaded sequence floor.
    #[arg(long)]
    expected_sequence: u64,
    /// Exact reviewed issuance time in Unix milliseconds.
    #[arg(long)]
    expected_issued_at_ms: u64,
    /// Exact reviewed exclusive expiry in Unix milliseconds.
    #[arg(long)]
    expected_expires_at_ms: u64,
    /// Current trusted operator time; never inferred from the downloaded checkpoint.
    #[arg(long)]
    trusted_now_ms: u64,
}

#[derive(Debug, ClapArgs)]
pub(super) struct SignArgs {
    #[command(flatten)]
    input: CheckpointInputs,
    /// One owner-held mode-0600 Kagami private-key record.
    #[arg(long, value_name = "PATH")]
    signer_private_key: PathBuf,
    /// New owner-only file for this authority's canonical partial approval.
    #[arg(long, value_name = "PATH")]
    approval_output: PathBuf,
}

#[derive(Debug, ClapArgs)]
pub(super) struct AssembleArgs {
    #[command(flatten)]
    input: CheckpointInputs,
    /// Canonical partial approval; repeat for each distinct authority.
    #[arg(long, value_name = "PATH", required = true)]
    approval: Vec<PathBuf>,
    /// New owner-only file for the complete authenticated canonical package.
    #[arg(long, value_name = "PATH")]
    package_output: PathBuf,
}

fn height_context(value: &str) -> color_eyre::Result<HeightContextId> {
    let bytes = parse_lower_sha256(value, "first height-context identity")?;
    // Hash::prehashed normalizes a reserved bit. Reject rather than silently changing input.
    let hash = Hash::prehashed(bytes);
    if hash.as_ref() != &bytes || hash == Hash::prehashed([0; 32]) {
        bail!("first height-context identity is not a canonical nonzero hash");
    }
    Ok(HeightContextId(HashOf::from_untyped_unchecked(hash)))
}

impl CheckpointInputs {
    fn pins<'a>(
        &self,
        policy: &'a KagemushaReleaseAuthorityPolicyV1,
    ) -> color_eyre::Result<KagemushaMobileBootstrapPinsV1<'a>> {
        let network_bytes =
            parse_lower_sha256(&self.deployment.expected_network_id, "network identity")?;
        let network_hash = Hash::prehashed(network_bytes);
        if network_hash.as_ref() != &network_bytes {
            bail!("network identity is not a canonical hash");
        }
        Ok(KagemushaMobileBootstrapPinsV1 {
            authority_policy: policy,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(network_hash)),
            scope: KagemushaMobileBootstrapScopeV1 {
                asset_identity_digest: parse_lower_sha256(
                    &self.deployment.expected_asset_identity_digest,
                    "asset identity",
                )?,
                asset_incarnation: parse_lower_sha256(
                    &self.deployment.expected_asset_incarnation,
                    "asset incarnation",
                )?,
                asset_scale: self.deployment.expected_asset_scale,
                liability_pool_id: parse_lower_sha256(
                    &self.deployment.expected_liability_pool_id,
                    "liability pool",
                )?,
            },
            release_id: parse_lower_sha256(
                &self.deployment.expected_release_id,
                "release identity",
            )?,
            release_attestation_digest: parse_lower_sha256(
                &self.expected_release_attestation_digest,
                "release attestation digest",
            )?,
            minimum_sequence: self.expected_sequence,
            previous: None,
            trusted_now_ms: self.trusted_now_ms,
        })
    }

    fn read(
        &self,
    ) -> color_eyre::Result<(
        KagemushaMobileBootstrapCheckpointV1,
        KagemushaReleaseAuthorityPolicyV1,
    )> {
        let bytes = read_bounded_immutable_file(
            &self.checkpoint,
            KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1,
            "mobile bootstrap checkpoint",
        )?;
        let checkpoint: KagemushaMobileBootstrapCheckpointV1 =
            norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .map_err(|_| eyre!("invalid canonical mobile bootstrap checkpoint"))?;
        let policy_bytes = read_bounded_immutable_file(
            &self.authority_policy,
            KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
            "mobile bootstrap authority policy",
        )?;
        let policy = KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes)
            .map_err(|_| eyre!("invalid canonical mobile bootstrap authority policy"))?;
        self.validate(&checkpoint, &policy)?;
        Ok((checkpoint, policy))
    }

    fn validate(
        &self,
        checkpoint: &KagemushaMobileBootstrapCheckpointV1,
        policy: &KagemushaReleaseAuthorityPolicyV1,
    ) -> Outcome {
        checkpoint
            .validate_pins(&self.pins(policy)?)
            .map_err(|error| eyre!(error))?;
        if checkpoint.first_context_id != height_context(&self.expected_first_context_id)?
            || checkpoint.sequence != self.expected_sequence
            || checkpoint.issued_at_ms != self.expected_issued_at_ms
            || checkpoint.expires_at_ms != self.expected_expires_at_ms
        {
            bail!(
                "mobile checkpoint differs from the exact independently reviewed context, sequence or lifetime"
            );
        }
        Ok(())
    }
}

pub(super) fn prepare<T: Write>(args: &PrepareArgs, writer: &mut std::io::BufWriter<T>) -> Outcome {
    let (_, policy, authenticated) = load_authenticated_experimental_release_v1(&args.release)?;
    let KagemushaReleasePurposeV1::TestnetExperiment(scope) = authenticated.purpose() else {
        bail!("mobile bootstrap preparation requires an authenticated Experimental release");
    };
    let checkpoint = KagemushaMobileBootstrapCheckpointV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        authority_policy_digest: authenticated.authority_policy_digest(),
        network_id: authenticated.network_id(),
        scope: KagemushaMobileBootstrapScopeV1 {
            asset_identity_digest: scope.asset_identity_digest,
            asset_incarnation: scope.asset_incarnation,
            asset_scale: scope.asset_scale,
            liability_pool_id: scope.liability_pool_id,
        },
        release_id: authenticated.release_id(),
        release_attestation_digest: authenticated.attestation_digest(),
        first_context_id: height_context(&args.first_context_id)?,
        sequence: args.sequence,
        issued_at_ms: args.issued_at_ms,
        expires_at_ms: args.expires_at_ms,
    };
    // Release/scope fields above come from an authenticated release matched to operator pins;
    // the authority explicitly selects context, sequence, time and lifetime before signing.
    let digest = checkpoint
        .validate_pins(&KagemushaMobileBootstrapPinsV1 {
            authority_policy: &policy,
            network_id: authenticated.network_id(),
            scope: checkpoint.scope,
            release_id: authenticated.release_id(),
            release_attestation_digest: authenticated.attestation_digest(),
            minimum_sequence: args.sequence,
            previous: None,
            trusted_now_ms: args.trusted_now_ms,
        })
        .map_err(|error| eyre!(error))?;
    crate::secure_fs::write_private_file_atomic(
        &args.checkpoint_output,
        &norito::encode_canonical(&checkpoint)?,
    )?;
    write_report(
        writer,
        "prepared_unsigned_mobile_checkpoint",
        &checkpoint,
        &digest,
    )
}

pub(super) fn sign<T: Write>(args: &SignArgs, writer: &mut std::io::BufWriter<T>) -> Outcome {
    let (checkpoint, policy) = args.input.read()?;
    let key = load_experimental_signing_key_v1(&args.signer_private_key)?;
    if policy
        .authorized_signers
        .binary_search(key.public_key())
        .is_err()
    {
        bail!("mobile bootstrap signing key is absent from the independent authority policy");
    }
    let approval = KagemushaMobileBootstrapApprovalV1 {
        public_key: key.public_key().clone(),
        signature: SignatureOf::try_new(key.private_key(), &checkpoint.approval_payload())
            .map_err(|_| eyre!("mobile bootstrap approval signing failed"))?,
    };
    approval
        .verify(&checkpoint, &policy)
        .map_err(|error| eyre!(error))?;
    let digest = checkpoint
        .validate_pins(&args.input.pins(&policy)?)
        .map_err(|error| eyre!(error))?;
    crate::secure_fs::write_private_file_atomic(
        &args.approval_output,
        &norito::encode_canonical(&approval)?,
    )?;
    write_report(
        writer,
        "signed_one_mobile_bootstrap_approval",
        &checkpoint,
        &digest,
    )
}

pub(super) fn assemble<T: Write>(
    args: &AssembleArgs,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let (checkpoint, policy) = args.input.read()?;
    if args.approval.len() < usize::from(policy.threshold)
        || args.approval.len() > policy.authorized_signers.len()
    {
        bail!("mobile bootstrap approval count is outside the independent authority threshold");
    }
    let mut approvals = Vec::with_capacity(args.approval.len());
    for path in &args.approval {
        let bytes = read_bounded_immutable_file(
            path,
            KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1,
            "mobile bootstrap partial approval",
        )?;
        let approval = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| eyre!("invalid canonical mobile bootstrap partial approval"))?;
        approvals.push(approval);
    }
    approvals.sort_by(|left: &KagemushaMobileBootstrapApprovalV1, right| {
        left.public_key.cmp(&right.public_key)
    });
    let package = KagemushaMobileBootstrapPackageV1 {
        checkpoint,
        approvals,
    };
    let digest = package
        .authenticate(&args.input.pins(&policy)?)
        .map_err(|error| eyre!(error))?;
    let encoded = norito::encode_canonical(&package)?;
    // The exact consumer decoder and shared verifier must accept the final bytes.
    KagemushaMobileBootstrapPackageV1::decode_canonical_exact(&encoded)
        .and_then(|decoded| {
            decoded.authenticate(
                &args
                    .input
                    .pins(&policy)
                    .map_err(|error| error.to_string())?,
            )
        })
        .map_err(|error| eyre!(error))?;
    crate::secure_fs::write_private_file_atomic(&args.package_output, &encoded)?;
    write_report(writer, "assembled_mobile_bootstrap", &checkpoint, &digest)
}

fn write_report<T: Write>(
    writer: &mut std::io::BufWriter<T>,
    status: &str,
    checkpoint: &KagemushaMobileBootstrapCheckpointV1,
    digest: &[u8; 32],
) -> Outcome {
    let mut report = JsonMap::new();
    insert_json_field(&mut report, "status", status)?;
    insert_json_field(&mut report, "checkpoint_digest", &hex::encode(digest))?;
    insert_json_field(&mut report, "sequence", &checkpoint.sequence)?;
    insert_json_field(
        &mut report,
        "release_id",
        &hex::encode(checkpoint.release_id),
    )?;
    insert_json_field(
        &mut report,
        "release_attestation_digest",
        &hex::encode(checkpoint.release_attestation_digest),
    )?;
    write!(
        writer,
        "{}",
        norito::json::to_json(&JsonValue::Object(report))?
    )?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
