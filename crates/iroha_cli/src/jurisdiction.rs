//! Jurisdiction Data Guardian (JDG) tooling (attestation validation).
//!
//! Provides offline helpers to validate JDG attestations and their Secret Data
//! Node (SDN) commitments against a local registry/policy before submitting
//! them on-chain.

use std::{
    fs,
    io::{self, Cursor, Read},
    path::PathBuf,
};

use eyre::{Context, Result, eyre};
use iroha_core::jurisdiction::JdgSdnEnforcer;
use iroha_data_model::jurisdiction::{
    JdgAttestation, JdgBlockRange, JdgSdnKeyRecord, JdgSdnPolicy, JdgSdnRotationPolicy,
};
use iroha_data_model::nexus::DataSpaceId;
use norito::json::{self, JsonDeserialize, JsonSerialize};

use crate::{Run, RunContext};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Validate a JDG attestation (structural + SDN commitments).
    Verify(VerifyArgs),
}

#[derive(clap::Args, Debug, Default)]
pub struct VerifyArgs {
    /// Path to the JDG attestation payload (Norito JSON or binary). Reads stdin when omitted.
    #[arg(long, value_name = "PATH")]
    pub attestation: Option<PathBuf>,
    /// Optional SDN registry payload (Norito JSON or binary).
    #[arg(long, value_name = "PATH")]
    pub sdn_registry: Option<PathBuf>,
    /// Whether SDN commitments are mandatory for this attestation.
    #[arg(long, default_value_t = false)]
    pub require_sdn_commitments: bool,
    /// Number of blocks the previous SDN key remains valid after rotation.
    #[arg(long, default_value_t = 0)]
    pub dual_publish_blocks: u64,
    /// Current block height for expiry/block-window checks.
    #[arg(long, value_name = "HEIGHT")]
    pub current_height: Option<u64>,
    /// Expected dataspace id; validation fails if it does not match.
    #[arg(long, value_name = "ID")]
    pub expect_dataspace: Option<u64>,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Verify(args) => verify_attestation(&args, context),
        }
    }
}

fn verify_attestation<C: RunContext>(args: &VerifyArgs, context: &mut C) -> Result<()> {
    let attestation = load_attestation(args.attestation.as_ref())
        .wrap_err("failed to load JDG attestation payload")?;
    let policy = JdgSdnPolicy {
        require_commitments: args.require_sdn_commitments,
        rotation: JdgSdnRotationPolicy {
            dual_publish_blocks: args.dual_publish_blocks,
        },
    };

    let enforcer = if let Some(path) = args.sdn_registry.as_ref() {
        Some(load_sdn_enforcer(path, policy).wrap_err("failed to load SDN registry payload")?)
    } else {
        None
    };

    let summary = validate_attestation(
        &attestation,
        policy,
        enforcer.as_ref(),
        args.current_height,
        args.expect_dataspace.map(DataSpaceId::new),
    )?;

    context.println("JDG attestation verified successfully")?;
    context.print_data(&summary)
}

fn validate_attestation(
    attestation: &JdgAttestation,
    policy: JdgSdnPolicy,
    enforcer: Option<&JdgSdnEnforcer>,
    current_height: Option<u64>,
    expected_dataspace: Option<DataSpaceId>,
) -> Result<VerificationSummary> {
    if let Some(expected) = expected_dataspace && attestation.scope.dataspace != expected {
        return Err(eyre!(
            "attestation targets dataspace {} but {} was expected",
            attestation.scope.dataspace,
            expected
        ));
    }

    if let Some(height) = current_height {
        if height < attestation.scope.block_range.start_height
            || height > attestation.scope.block_range.end_height
        {
            return Err(eyre!(
                "current height {height} is outside the attested range {}..={}",
                attestation.scope.block_range.start_height,
                attestation.scope.block_range.end_height
            ));
        }
        if height >= attestation.expiry_height {
            return Err(eyre!(
                "attestation expired at height {} (current {height})",
                attestation.expiry_height
            ));
        }
    }

    if let Some(enforcer) = enforcer {
        enforcer
            .validate(attestation)
            .wrap_err("attestation failed SDN validation")?;
    } else {
        attestation
            .validate_with_sdn(policy.require_commitments)
            .wrap_err("attestation failed structural validation")?;
    }

    Ok(VerificationSummary::from_attestation(
        attestation,
        policy,
        enforcer.is_some(),
    ))
}

fn load_attestation(path: Option<&PathBuf>) -> Result<JdgAttestation> {
    let bytes = read_input_bytes(path)?;
    if let Ok(attestation) = json::from_slice::<JdgAttestation>(&bytes) {
        return Ok(attestation);
    }
    let mut cursor = Cursor::new(bytes);
    norito::decode_from_reader(&mut cursor).map_err(|err| eyre!("decode attestation: {err}"))
}

fn load_sdn_enforcer(path: &PathBuf, policy: JdgSdnPolicy) -> Result<JdgSdnEnforcer> {
    let bytes = read_input_bytes(Some(path))?;
    if let Ok(records) = json::from_slice::<Vec<JdgSdnKeyRecord>>(&bytes) {
        return JdgSdnEnforcer::from_records(policy, records)
            .map_err(|err| eyre!("SDN registry violates rotation policy: {err}"));
    }
    let mut cursor = Cursor::new(bytes);
    JdgSdnEnforcer::from_reader(&mut cursor, policy)
        .map_err(|err| eyre!("decode SDN registry: {err}"))
}

fn read_input_bytes(path: Option<&PathBuf>) -> Result<Vec<u8>> {
    if let Some(path) = path {
        fs::read(path).with_context(|| format!("failed to read payload from {}", path.display()))
    } else {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .wrap_err("failed to read stdin")?;
        Ok(buf)
    }
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct VerificationSummary {
    jurisdiction_id_hex: String,
    dataspace: DataSpaceId,
    block_range: JdgBlockRange,
    attested_block_height: u64,
    expiry_height: u64,
    committee_id_hex: String,
    committee_threshold: u16,
    signer_count: usize,
    sdn_commitments: usize,
    require_sdn_commitments: bool,
    sdn_dual_publish_blocks: u64,
    registry_loaded: bool,
}

impl VerificationSummary {
    fn from_attestation(
        attestation: &JdgAttestation,
        policy: JdgSdnPolicy,
        registry_loaded: bool,
    ) -> Self {
        Self {
            jurisdiction_id_hex: hex::encode(attestation.scope.jurisdiction_id.as_bytes()),
            dataspace: attestation.scope.dataspace,
            block_range: attestation.scope.block_range,
            attested_block_height: attestation.block_height,
            expiry_height: attestation.expiry_height,
            committee_id_hex: hex::encode(attestation.committee_id.0),
            committee_threshold: attestation.committee_threshold,
            signer_count: attestation.signer_set.len(),
            sdn_commitments: attestation.sdn_commitments.len(),
            require_sdn_commitments: policy.require_commitments,
            sdn_dual_publish_blocks: policy.rotation.dual_publish_blocks,
            registry_loaded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    use iroha_crypto::{Hash, KeyPair, Signature, SignatureOf};
    use iroha_data_model::jurisdiction::{
        JDG_ATTESTATION_VERSION_V1, JDG_SDN_COMMITMENT_VERSION_V1, JdgAttestationScope,
        JdgCommitteeId, JdgSdnCommitment, JdgStateAccessSet, JdgVerdict,
    };
    use iroha_data_model::{ChainId, prelude::AccountId};
    use iroha_i18n::{Bundle, Language, Localizer};
    use std::str::FromStr;
    use url::Url;

    use crate::config_utils::{
        default_alias_cache_policy, default_anonymity_policy, default_rollout_phase,
    };

    struct TestContext {
        cfg: iroha::config::Config,
        printed: Vec<String>,
        json_outputs: Vec<String>,
        i18n: Localizer,
    }

    impl TestContext {
        fn new() -> Self {
            let key_pair = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::Ed25519);
            let account_id =
                AccountId::new("wonderland".parse().unwrap(), key_pair.public_key().clone());
            let cfg = iroha::config::Config {
                chain: ChainId::from_str("00000000-0000-0000-0000-000000000000")
                    .expect("valid chain id"),
                account: account_id,
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                sorafs_alias_cache: default_alias_cache_policy(),
                sorafs_anonymity_policy: default_anonymity_policy(),
                sorafs_rollout_phase: default_rollout_phase(),
            };
            Self {
                cfg,
                printed: Vec::new(),
                json_outputs: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&iroha_data_model::metadata::Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            let rendered = norito::json::to_json_pretty(data)
                .map_err(|err| eyre!("failed to render JSON: {err}"))?;
            self.json_outputs.push(rendered);
            Ok(())
        }

        fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    fn sample_scope() -> JdgAttestationScope {
        JdgAttestationScope {
            jurisdiction_id: iroha_data_model::jurisdiction::JurisdictionId::new(b"JUR1".to_vec())
                .expect("valid id"),
            dataspace: DataSpaceId::new(7),
            block_range: JdgBlockRange::new(10, 15).expect("valid range"),
        }
    }

    fn sample_attestation(
        scope: &JdgAttestationScope,
        signer: &KeyPair,
        sdn_keypair: &KeyPair,
        include_sdn_commitment: bool,
    ) -> JdgAttestation {
        let mut sdn_commitments = Vec::new();
        if include_sdn_commitment {
            let mut commitment = JdgSdnCommitment {
                version: JDG_SDN_COMMITMENT_VERSION_V1,
                scope: scope.clone(),
                encrypted_payload_hash: Hash::prehashed([0xAA; 32]),
                seal: SignatureOf::from_signature(Signature::from_bytes(&[0u8])),
                sdn_public_key: sdn_keypair.public_key().clone(),
            };
            commitment.seal =
                SignatureOf::from_hash(sdn_keypair.private_key(), commitment.signing_hash());
            sdn_commitments.push(commitment);
        }

        JdgAttestation {
            version: JDG_ATTESTATION_VERSION_V1,
            scope: scope.clone(),
            pre_state_version: Hash::prehashed([0x11; 32]),
            access_set: JdgStateAccessSet::normalized(vec![b"a".to_vec()], vec![b"b".to_vec()]),
            verdict: JdgVerdict::Accept,
            post_state_delta: vec![0xBB],
            jurisdiction_root: Hash::prehashed([0x22; 32]),
            sdn_commitments,
            da_ack: None,
            expiry_height: 99,
            committee_id: JdgCommitteeId::new([0x33; 32]),
            committee_threshold: 1,
            statement_hash: Hash::prehashed([0x44; 32]),
            proof: None,
            signer_set: vec![signer.public_key().clone()],
            epoch: 0,
            block_height: 12,
            signature: iroha_data_model::jurisdiction::JdgThresholdSignature {
                scheme_id: iroha_data_model::jurisdiction::JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD,
                signer_bitmap: Some(vec![0b0000_0001]),
                signatures: vec![vec![0x55]],
            },
        }
    }

    fn write_json_payload<T: JsonSerialize>(value: &T) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp file");
        let rendered = norito::json::to_json_pretty(value).expect("serialize");
        file.write_all(rendered.as_bytes()).expect("write payload");
        file
    }

    use std::io::Write;

    #[test]
    fn verifies_attestation_with_registry() {
        let scope = sample_scope();
        let signer = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::Ed25519);
        let sdn_keypair = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::Ed25519);
        let attestation = sample_attestation(&scope, &signer, &sdn_keypair, true);
        let registry = vec![JdgSdnKeyRecord {
            public_key: sdn_keypair.public_key().clone(),
            activated_at: scope.block_range.start_height,
            retired_at: None,
            rotation_parent: None,
        }];

        let attestation_file = write_json_payload(&attestation);
        let registry_file = write_json_payload(&registry);

        let mut ctx = TestContext::new();
        let args = VerifyArgs {
            attestation: Some(attestation_file.path().to_path_buf()),
            sdn_registry: Some(registry_file.path().to_path_buf()),
            require_sdn_commitments: true,
            dual_publish_blocks: 2,
            current_height: Some(12),
            expect_dataspace: Some(scope.dataspace.as_u64()),
        };

        verify_attestation(&args, &mut ctx).expect("validation succeeds");
        assert!(
            ctx.printed
                .iter()
                .any(|line| line.contains("verified successfully"))
        );
        assert_eq!(ctx.json_outputs.len(), 1);
        let summary: VerificationSummary =
            json::from_str(&ctx.json_outputs[0]).expect("summary json");
        assert!(summary.registry_loaded);
        assert_eq!(summary.sdn_commitments, 1);
        assert_eq!(summary.signer_count, 1);
    }

    #[test]
    fn rejects_missing_sdn_commitments_when_required() {
        let scope = sample_scope();
        let signer = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::Ed25519);
        let sdn_keypair = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::Ed25519);
        let attestation = sample_attestation(&scope, &signer, &sdn_keypair, false);
        let registry = vec![JdgSdnKeyRecord {
            public_key: sdn_keypair.public_key().clone(),
            activated_at: scope.block_range.start_height,
            retired_at: None,
            rotation_parent: None,
        }];

        let attestation_file = write_json_payload(&attestation);
        let registry_file = write_json_payload(&registry);

        let mut ctx = TestContext::new();
        let args = VerifyArgs {
            attestation: Some(attestation_file.path().to_path_buf()),
            sdn_registry: Some(registry_file.path().to_path_buf()),
            require_sdn_commitments: true,
            dual_publish_blocks: 0,
            current_height: Some(12),
            expect_dataspace: None,
        };

        let err = verify_attestation(&args, &mut ctx).expect_err("validation must fail");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("SDN commitments required by policy")
                || msg.contains("missing commitments"),
            "unexpected error: {msg}"
        );
    }
}
