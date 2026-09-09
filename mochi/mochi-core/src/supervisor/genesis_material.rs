//! Genesis artifact construction, Kagami invocation, and exact generation validation.
//!
//! This owner keeps temporary signing material, manifest/block bindings and bounded
//! public-record reads together. The supervisor coordinates generation publication.

use super::{
    BinaryPaths, PeerConfigOverrides, PeerSpec, Result, SupervisorError, timestamp_ms,
    validate_managed_peer_paths,
};
use crate::{
    config::GenesisProfile, genesis, path_safety::open_existing_file_no_follow_nonblocking,
};
use iroha_crypto::{ExposedPrivateKey, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    block::BlockHeader,
    isi::kagemusha_v1::KagemushaMintFinalityGenesisParametersV1,
    parameter::system::SumeragiConsensusMode,
    prelude::{AccountId, ChainId, NetworkId},
};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
#[cfg(any(test, feature = "test"))]
use izanami::genesis_support::sign_prepared_genesis_from_config;
use izanami::genesis_support::{ManagedNodeConfig, validate_prepared_genesis_for_startup};
use norito::json::{self, Value};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use zeroize::Zeroizing;

pub(super) const GENESIS_FILE_NAME: &str = "genesis.json";
pub(super) const GENESIS_SIGNED_FILE_NAME: &str = "genesis.signed.nrt";
pub(super) const GENESIS_EXPECTED_HASH_FILE_NAME: &str = "genesis.expected_hash";
pub(super) const GENESIS_PUBLIC_KEY_FILE_NAME: &str = "genesis.public_key";
pub(super) const GENERATED_GENESIS_RECORD_MAX_BYTES_V1: usize = 4 * 1024;
#[cfg(any(test, feature = "test"))]
pub(super) const TEST_FINALIZE_KAGAMI_STUB_SIGNATURE: &str =
    "MOCHI_TEST_FINALIZE_KAGAMI_STUB_SIGNATURE";
const VRF_SEED_HEX_CHARS: usize = 64;

#[derive(Debug)]
pub(super) struct GenesisMaterial {
    pub(super) generation_id: String,
    pub(super) key_pair: KeyPair,
    pub(super) manifest_path: PathBuf,
    pub(super) block_path: PathBuf,
    pub(super) expected_hash_path: PathBuf,
    pub(super) public_key_path: PathBuf,
    pub(super) expected_hash: Option<HashOf<BlockHeader>>,
    pub(super) chain_discriminant: u16,
    pub(super) consensus_fingerprint: Option<String>,
}
#[derive(Clone, Copy)]
pub(super) struct GenesisCreateContext<'a> {
    pub(super) generation_id: &'a str,
    pub(super) generation_root: &'a Path,
    pub(super) chain_id: &'a str,
    pub(super) peers: &'a [PeerSpec],
    pub(super) config_overrides: &'a PeerConfigOverrides,
    pub(super) consensus_mode: SumeragiConsensusMode,
    pub(super) block_cadence_ms: NonZeroU64,
    pub(super) genesis_profile: Option<GenesisProfile>,
    pub(super) vrf_seed_hex: Option<&'a str>,
    pub(super) onboarding_authority: &'a AccountId,
}
#[derive(Debug)]
pub(super) struct TemporaryGenesisKeyFile {
    path: PathBuf,
}
impl TemporaryGenesisKeyFile {
    #[cfg(unix)]
    pub(super) fn create(genesis_dir: &Path, key_pair: &KeyPair) -> Result<Self> {
        const MAX_CREATE_ATTEMPTS: u8 = 32;
        // Kagami rejects private-key paths containing any symbolic-link
        // component. macOS commonly exposes its temporary directory through
        // `/var`, so resolve the managed directory before deriving the file.
        let genesis_dir = fs::canonicalize(genesis_dir)?;
        for attempt in 0..MAX_CREATE_ATTEMPTS {
            let path = genesis_dir.join(format!(
                ".mochi-genesis-signing-key-{}-{}-{attempt}",
                std::process::id(),
                timestamp_ms()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true).mode(0o600);
            let mut file = match options.open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            let guard = Self { path };
            let canonical =
                Zeroizing::new(ExposedPrivateKey(key_pair.private_key().clone()).to_string());
            file.write_all(canonical.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            return Ok(guard);
        }
        Err(SupervisorError::Config(format!(
            "failed to allocate an owner-only genesis signing key beneath `{}`",
            genesis_dir.display()
        )))
    }
    #[cfg(not(unix))]
    pub(super) fn create(_genesis_dir: &Path, _key_pair: &KeyPair) -> Result<Self> {
        Err(SupervisorError::Config(
            "config-bound genesis signing requires owner-only private-key file support".to_owned(),
        ))
    }
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for TemporaryGenesisKeyFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "warning: failed to remove temporary genesis signing key `{}`: {error}",
                self.path.display()
            );
        }
    }
}

#[derive(Debug)]
struct TemporaryKagemushaMintFinalityParametersFile {
    path: PathBuf,
}

impl TemporaryKagemushaMintFinalityParametersFile {
    fn create(
        genesis_dir: &Path,
        parameters: &KagemushaMintFinalityGenesisParametersV1,
    ) -> Result<Self> {
        const MAX_CREATE_ATTEMPTS: u8 = 32;
        let genesis_dir = fs::canonicalize(genesis_dir)?;
        let encoded = json::to_vec_pretty(parameters)?;
        for attempt in 0..MAX_CREATE_ATTEMPTS {
            let path = genesis_dir.join(format!(
                ".mochi-kagemusha-mint-finality-{}-{}-{attempt}.json",
                std::process::id(),
                timestamp_ms()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = match options.open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            let guard = Self { path };
            file.write_all(&encoded)?;
            file.sync_all()?;
            return Ok(guard);
        }
        Err(SupervisorError::Config(format!(
            "failed to allocate a KAGEMUSHA mint-finality parameter file beneath `{}`",
            genesis_dir.display()
        )))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryKagemushaMintFinalityParametersFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "warning: failed to remove temporary KAGEMUSHA mint-finality parameters `{}`: {error}",
                self.path.display()
            );
        }
    }
}
/// Build the bound manifest and canonical signed genesis used by Mochi's Kagami test stub.
///
/// This helper is intentionally available only to tests and consumers which
/// opt into `test`; production supervision always invokes Kagami. It does not modify either input
/// file. Callers must publish the returned manifest after writing the signed block successfully.
///
/// # Errors
///
/// Returns an error when the prepared manifest or node configuration cannot
/// be parsed, their genesis bindings differ, or canonical signing fails.
#[cfg(any(test, feature = "test"))]
pub fn sign_kagami_stub_genesis_from_config(
    manifest_path: &Path,
    config_path: &Path,
    key_pair: &KeyPair,
    expected_consensus_mode: Option<SumeragiConsensusMode>,
) -> Result<(RawGenesisTransaction, iroha_data_model::block::SignedBlock)> {
    let bound_manifest = RawGenesisTransaction::from_path(manifest_path)
        .map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "test Kagami stub failed signing canonical genesis: {error:#}"
            ))
        })?
        .with_consensus_meta();
    let block = sign_prepared_genesis_from_config(
        manifest_path,
        config_path,
        key_pair,
        expected_consensus_mode,
    )
    .map_err(|error| {
        SupervisorError::KagamiInvocation(format!(
            "test Kagami stub failed signing canonical genesis: {error:#}"
        ))
    })?;
    // Pre-execution records results and re-signs the block. Prove that the returned manifest
    // still binds its actual instruction, authority, and consensus context before publishing it.
    let wire = block.encode_wire().map_err(|error| {
        SupervisorError::KagamiInvocation(format!(
            "test Kagami stub failed encoding canonical genesis: {error}"
        ))
    })?;
    iroha_genesis::validate_prepared_genesis_bundle(
        &wire,
        &bound_manifest,
        key_pair.public_key(),
        block.hash(),
    )
    .map_err(|error| {
        SupervisorError::KagamiInvocation(format!(
            "test Kagami stub produced inconsistent bound genesis: {error:#}"
        ))
    })?;
    Ok((bound_manifest, block))
}
/// Derive the exact genesis policies selected by a finalized node config.
///
/// This test-only companion keeps config parsing behind Mochi's existing orchestration dependency
/// while allowing the Kagami mock to verify the canonical block it emitted.
///
/// # Errors
///
/// Returns an error when the node configuration cannot be parsed or omits a
/// required genesis binding.
#[cfg(any(test, feature = "test"))]
pub fn kagami_stub_genesis_policies_from_config(
    config_path: &Path,
) -> Result<(
    iroha_data_model::da::commitment::DaProofPolicyBundle,
    [u8; 32],
)> {
    let config = ManagedNodeConfig::from_path(config_path).map_err(|error| {
        SupervisorError::KagamiInvocation(format!(
            "test Kagami stub failed loading config `{}`: {error:#}",
            config_path.display()
        ))
    })?;
    Ok((
        config.da_proof_policies,
        config.genesis_confidential_policy_hash,
    ))
}
/// Inputs for one canonical `kagami genesis generate` invocation.
struct GenesisManifestRequest<'a> {
    genesis_dir: &'a Path,
    chain_id: &'a str,
    genesis_public_key: &'a PublicKey,
    consensus_mode: SumeragiConsensusMode,
    genesis_profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<&'a str>,
    kagemusha_mint_finality: &'a KagemushaMintFinalityGenesisParametersV1,
}

impl GenesisMaterial {
    pub(super) fn create(
        binaries: &mut BinaryPaths,
        context: GenesisCreateContext<'_>,
    ) -> Result<Self> {
        let GenesisCreateContext {
            generation_id,
            generation_root,
            chain_id,
            peers,
            config_overrides,
            consensus_mode,
            block_cadence_ms,
            genesis_profile,
            vrf_seed_hex,
            onboarding_authority,
        } = context;
        let genesis_dir = generation_root.join("genesis");
        fs::create_dir_all(&genesis_dir)?;
        let manifest_path = genesis_dir.join(GENESIS_FILE_NAME);
        let block_path = genesis_dir.join(GENESIS_SIGNED_FILE_NAME);
        let expected_hash_path = genesis_dir.join(GENESIS_EXPECTED_HASH_FILE_NAME);
        let public_key_path = genesis_dir.join(GENESIS_PUBLIC_KEY_FILE_NAME);
        let key_pair = KeyPair::random();
        let kagemusha_mint_finality = genesis::dev_sandbox_kagemusha_mint_finality_parameters(
            chain_id,
            generation_id,
            peers.iter().map(PeerSpec::peer_id),
        )?;
        let manifest = Self::generate_manifest(
            binaries,
            GenesisManifestRequest {
                genesis_dir: &genesis_dir,
                chain_id,
                genesis_public_key: key_pair.public_key(),
                consensus_mode,
                genesis_profile,
                vrf_seed_hex,
                kagemusha_mint_finality: &kagemusha_mint_finality,
            },
        )?;
        // Public Kagami profiles own their signed cadence and are checked by
        // `kagami verify`. Unprofiled Mochi sandboxes bind the documented
        // one-second localnet cadence into their exact validator committee.
        let manifest = if genesis_profile.is_some() {
            manifest
        } else {
            manifest
                .into_builder()
                .with_block_cadence_ms(block_cadence_ms)
                .build_raw()?
                .with_consensus_meta()
        };
        let manifest =
            genesis::with_local_account_onboarding_bootstrap(manifest, onboarding_authority)?;
        let topology: Vec<GenesisTopologyEntry> = peers
            .iter()
            .map(|spec| GenesisTopologyEntry::new(spec.peer_id(), spec.pop_bytes().to_vec()))
            .collect();
        let manifest = genesis::with_topology(manifest, topology)?;
        let json = norito::json::to_vec_pretty(&manifest)?;
        fs::write(&manifest_path, json)?;
        fs::write(&public_key_path, format!("{}\n", key_pair.public_key()))?;
        let mut material = Self {
            generation_id: generation_id.to_owned(),
            key_pair,
            manifest_path,
            block_path,
            expected_hash_path,
            public_key_path,
            expected_hash: None,
            chain_discriminant: manifest.chain_discriminant(),
            consensus_fingerprint: None,
        };
        let primary = peers.first().ok_or_else(|| {
            SupervisorError::Config(
                "Mochi genesis requires an exact 3f+1 validator committee".to_owned(),
            )
        })?;
        // Kagami must stage genesis against the exact peer configuration that
        // irohad will consume. The paths and public key are already stable, so
        // render the primary config once before signing, then let the caller
        // rewrite every peer config with the final bound fingerprint header.
        primary.write_config(chain_id, &material, peers, config_overrides, &[])?;
        let (manifest, expected_hash) = material.sign_manifest_with_kagami(
            binaries,
            primary.config_path.as_path(),
            #[cfg(any(test, feature = "test"))]
            consensus_mode,
        )?;
        material.expected_hash = Some(expected_hash);
        if let Some(profile) = genesis_profile {
            Self::verify_manifest_with_kagami(
                binaries,
                &material.manifest_path,
                profile,
                vrf_seed_hex,
            )?;
        }
        material.consensus_fingerprint = manifest
            .clone()
            .with_consensus_meta()
            .consensus_fingerprint()
            .map(|value| value.to_string());
        Ok(material)
    }
    pub(super) fn copy_into_generation(
        &self,
        generation_id: &str,
        generation_root: &Path,
    ) -> Result<Self> {
        let genesis_dir = generation_root.join("genesis");
        fs::create_dir_all(&genesis_dir)?;
        let manifest_path = genesis_dir.join(GENESIS_FILE_NAME);
        let block_path = genesis_dir.join(GENESIS_SIGNED_FILE_NAME);
        let expected_hash_path = genesis_dir.join(GENESIS_EXPECTED_HASH_FILE_NAME);
        let public_key_path = genesis_dir.join(GENESIS_PUBLIC_KEY_FILE_NAME);
        fs::copy(&self.manifest_path, &manifest_path)?;
        fs::copy(&self.block_path, &block_path)?;
        fs::copy(&self.expected_hash_path, &expected_hash_path)?;
        fs::copy(&self.public_key_path, &public_key_path)?;
        Ok(Self {
            generation_id: generation_id.to_owned(),
            key_pair: self.key_pair.clone(),
            manifest_path,
            block_path,
            expected_hash_path,
            public_key_path,
            expected_hash: self.expected_hash,
            chain_discriminant: self.chain_discriminant,
            consensus_fingerprint: self.consensus_fingerprint.clone(),
        })
    }
    fn sign_manifest_with_kagami(
        &self,
        binaries: &mut BinaryPaths,
        config_path: &Path,
        #[cfg(any(test, feature = "test"))] consensus_mode: SumeragiConsensusMode,
    ) -> Result<(RawGenesisTransaction, HashOf<BlockHeader>)> {
        let kagami = binaries.ensure_kagami_ready()?;
        let genesis_dir = self.manifest_path.parent().ok_or_else(|| {
            SupervisorError::Config(format!(
                "genesis manifest path `{}` has no parent directory",
                self.manifest_path.display()
            ))
        })?;
        let private_key_file = TemporaryGenesisKeyFile::create(genesis_dir, &self.key_pair)?;
        let mut command = Command::new(kagami);
        command
            .current_dir(genesis_dir)
            .arg("genesis")
            .arg("sign")
            .arg(&self.manifest_path)
            .arg("--out-file")
            .arg(&self.block_path)
            .arg("--bound-manifest-out")
            .arg(&self.manifest_path)
            .arg("--expected-hash-out")
            .arg(&self.expected_hash_path)
            .arg("--private-key-file")
            .arg(private_key_file.path())
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = command.output().map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "failed to invoke `kagami genesis sign`: {error}"
            ))
        })?;
        drop(private_key_file);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` exited with status {}: {stderr}",
                output.status
            )));
        }
        #[cfg(any(test, feature = "test"))]
        if std::env::var_os(TEST_FINALIZE_KAGAMI_STUB_SIGNATURE).is_some() {
            self.finalize_kagami_stub_signature(config_path, consensus_mode)?;
        }
        let signed_metadata = fs::metadata(&self.block_path).map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` did not create `{}`: {error}",
                self.block_path.display()
            ))
        })?;
        if !signed_metadata.is_file() || signed_metadata.len() == 0 {
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` emitted an empty signed block at `{}`",
                self.block_path.display()
            )));
        }
        let expected_hash_record = read_generated_genesis_record(
            &self.expected_hash_path,
            "generated checked genesis network identity",
        )
        .map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` did not create an exact checked genesis network identity `{}`: {error}",
                self.expected_hash_path.display()
            ))
        })?;
        let expected_hash_literal = expected_hash_record
            .strip_suffix('\n')
            .expect("exact-record reader preserves one trailing LF");
        let expected_network_id = expected_hash_literal
            .parse::<NetworkId>()
            .map_err(|error| {
                SupervisorError::KagamiInvocation(format!(
                    "failed to parse checked genesis network identity `{}`: {error}",
                    self.expected_hash_path.display()
                ))
            })?;
        if expected_hash_record != format!("{expected_network_id}\n") {
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` produced a non-canonical checked genesis network identity at `{}`",
                self.expected_hash_path.display()
            )));
        }
        let expected_hash = expected_network_id.into_genesis_hash();
        let manifest = RawGenesisTransaction::from_path(&self.manifest_path)?;
        Ok((manifest, expected_hash))
    }
    #[cfg(any(test, feature = "test"))]
    fn finalize_kagami_stub_signature(
        &self,
        config_path: &Path,
        consensus_mode: SumeragiConsensusMode,
    ) -> Result<()> {
        let (bound_manifest, block) = sign_kagami_stub_genesis_from_config(
            &self.manifest_path,
            config_path,
            &self.key_pair,
            Some(consensus_mode),
        )?;
        let wire = block.encode_wire().map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "test Kagami stub failed encoding canonical genesis: {error}"
            ))
        })?;
        fs::write(&self.block_path, wire)?;
        fs::write(&self.manifest_path, json::to_vec_pretty(&bound_manifest)?)?;
        fs::write(
            &self.expected_hash_path,
            format!("{}\n", NetworkId::from_genesis_hash(block.hash())),
        )?;
        Ok(())
    }
    pub(super) fn validate_generation(&self, chain_id: &str, peers: &[PeerSpec]) -> Result<()> {
        let signed = iroha_genesis::read_signed_genesis_bytes(&self.block_path).map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "failed to read signed genesis `{}` under the {}-byte first-release limit: {error}",
                self.block_path.display(),
                iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1
            ))
        })?;
        let manifest = RawGenesisTransaction::from_path(&self.manifest_path)?;
        let expected_chain = chain_id.parse::<ChainId>().map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "configured chain id `{chain_id}` is invalid: {error}"
            ))
        })?;
        if manifest.chain_id() != &expected_chain
            || manifest.chain_discriminant() != self.chain_discriminant
        {
            return Err(SupervisorError::GenerationValidation(
                "persisted genesis manifest changed its chain or discriminant".to_owned(),
            ));
        }
        let expected_hash = self.expected_hash.ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "candidate generation has no exact genesis hash".to_owned(),
            )
        })?;
        let public_record = read_generated_genesis_record(
            &self.public_key_path,
            "generated genesis public-key record",
        )
        .map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "failed to read candidate genesis public-key record `{}`: {error}",
                self.public_key_path.display()
            ))
        })?;
        let public_literal = public_record
            .strip_suffix('\n')
            .expect("exact-record reader preserves one trailing LF");
        let public_key = public_literal.parse::<PublicKey>().map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "candidate genesis public-key record `{}` is invalid: {error}",
                self.public_key_path.display()
            ))
        })?;
        if public_record != format!("{public_key}\n") || &public_key != self.public_key() {
            return Err(SupervisorError::GenerationValidation(
                "candidate genesis public-key record is not exact and canonical".to_owned(),
            ));
        }
        let validated = validate_prepared_genesis_for_startup(
            &signed,
            &manifest,
            self.public_key(),
            expected_hash,
            &expected_chain,
        )
        .map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "prepared signed-genesis startup validation failed: {error:#}"
            ))
        })?;
        drop(signed);
        let expected_roster = peers
            .iter()
            .map(|peer| (peer.keys.public_key.clone(), peer.keys.pop.clone()))
            .collect::<BTreeMap<_, _>>();
        if validated.validator_pops() != &expected_roster {
            return Err(SupervisorError::GenerationValidation(
                "signed genesis validator roster differs from the candidate peers".to_owned(),
            ));
        }
        let canonical_block = fs::canonicalize(&self.block_path)?;
        let canonical_manifest = fs::canonicalize(&self.manifest_path)?;
        for peer in peers {
            let config = ManagedNodeConfig::from_path(&peer.config_path).map_err(|error| {
                SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` failed loading: {error:#}",
                    peer.config_path.display()
                ))
            })?;
            validate_managed_peer_paths(&config, peer, peers.len())?;
            if config.chain_id != expected_chain
                || config.chain_discriminant != self.chain_discriminant
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` has the wrong chain or discriminant",
                    peer.config_path.display()
                )));
            }
            if config.genesis_public_key != *self.public_key()
                || config.genesis_expected_hash != expected_hash
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` has a different genesis key or hash",
                    peer.config_path.display()
                )));
            }
            if validated.block().da_proof_policies() != Some(&config.da_proof_policies) {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` DA proof policy differs from the signed genesis header",
                    peer.config_path.display()
                )));
            }
            let signed_confidential_policy = validated
                .block()
                .header()
                .confidential_features()
                .and_then(|digest| digest.zk_policy_hash);
            if signed_confidential_policy != Some(config.genesis_confidential_policy_hash) {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` confidential policy differs from the signed genesis header",
                    peer.config_path.display()
                )));
            }
            if fs::canonicalize(&config.genesis_block_path)? != canonical_block
                || fs::canonicalize(&config.genesis_manifest_path)? != canonical_manifest
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` selects genesis outside its generation",
                    peer.config_path.display()
                )));
            }
            if config.trusted_peer_pops != expected_roster
                || config.local_public_key != peer.keys.public_key
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` identity or PoP roster differs from signed genesis",
                    peer.config_path.display()
                )));
            }
        }
        Ok(())
    }
    fn generate_manifest(
        binaries: &mut BinaryPaths,
        request: GenesisManifestRequest<'_>,
    ) -> Result<RawGenesisTransaction> {
        let GenesisManifestRequest {
            genesis_dir,
            chain_id,
            genesis_public_key,
            consensus_mode,
            genesis_profile,
            vrf_seed_hex,
            kagemusha_mint_finality,
        } = request;
        validate_genesis_profile_inputs(genesis_profile, vrf_seed_hex)?;
        let kagami = binaries.ensure_kagami_ready()?;
        let kagemusha_mint_finality_file = TemporaryKagemushaMintFinalityParametersFile::create(
            genesis_dir,
            kagemusha_mint_finality,
        )?;
        let mut command = Command::new(kagami);
        command
            .current_dir(genesis_dir)
            .arg("genesis")
            .arg("generate")
            .arg("--ivm-dir")
            .arg(".")
            .arg("--genesis-public-key")
            .arg(genesis_public_key.to_string())
            .arg("--chain-id")
            .arg(chain_id)
            .arg("--kagemusha-mint-finality-parameters")
            .arg(kagemusha_mint_finality_file.path());
        if let Some(profile) = genesis_profile {
            command.arg("--profile").arg(profile.as_kagami_arg());
        }
        command.arg("--consensus-mode").arg(match consensus_mode {
            SumeragiConsensusMode::Permissioned => "permissioned",
            SumeragiConsensusMode::Npos => "npos",
        });
        if let Some(seed) = vrf_seed_hex {
            command.arg("--vrf-seed-hex").arg(seed);
        }
        command
            .arg("default")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = command.output().map_err(|err| {
            SupervisorError::KagamiInvocation(format!("failed to invoke `kagami`: {err}"))
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami` exited with status {}: {stderr}",
                output.status
            )));
        }
        if genesis_profile.is_some() && !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }
        if output.stdout.is_empty() {
            return Err(SupervisorError::KagamiInvocation(
                "`kagami` did not emit genesis JSON".into(),
            ));
        }
        let value: Value = norito::json::from_slice(&output.stdout).map_err(|err| {
            SupervisorError::KagamiInvocation(format!(
                "failed to parse `kagami` JSON output: {err}"
            ))
        })?;
        validate_kagami_manifest_chain(&value, chain_id)?;
        let manifest: RawGenesisTransaction = norito::json::from_value(value).map_err(|err| {
            SupervisorError::KagamiInvocation(format!(
                "failed to decode genesis manifest from `kagami` output: {err}"
            ))
        })?;
        Ok(manifest)
    }
    fn verify_manifest_with_kagami(
        binaries: &mut BinaryPaths,
        manifest_path: &Path,
        profile: GenesisProfile,
        vrf_seed_hex: Option<&str>,
    ) -> Result<()> {
        let kagami = binaries.ensure_kagami_ready()?;
        let mut command = Command::new(kagami);
        command
            .arg("verify")
            .arg("--profile")
            .arg(profile.as_kagami_arg())
            .arg("--genesis")
            .arg(manifest_path);
        if let Some(seed) = vrf_seed_hex {
            command.arg("--vrf-seed-hex").arg(seed);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let output = command.output().map_err(|err| {
            SupervisorError::KagamiInvocation(format!("failed to invoke `kagami verify`: {err}"))
        })?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami verify` exited with status {}: {stderr}",
                output.status
            )));
        }
        Ok(())
    }
    pub(super) fn public_key(&self) -> &PublicKey {
        self.key_pair.public_key()
    }
}

pub(super) fn read_generated_genesis_record(path: &Path, label: &str) -> io::Result<String> {
    read_generated_genesis_record_inner(path, label, || {})
}
pub(super) fn read_generated_genesis_record_inner(
    path: &Path,
    label: &str,
    before_open: impl FnOnce(),
) -> io::Result<String> {
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink() || !named.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` is not a regular file", path.display()),
        ));
    }
    let max_bytes = u64::try_from(GENERATED_GENESIS_RECORD_MAX_BYTES_V1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "generated genesis record byte limit does not fit u64",
        )
    })?;
    if named.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` exceeds its {}-byte limit",
                path.display(),
                GENERATED_GENESIS_RECORD_MAX_BYTES_V1
            ),
        ));
    }
    before_open();
    let mut file = open_existing_file_no_follow_nonblocking(path)?;
    let opened = file.metadata()?;
    if !generated_genesis_record_metadata_unchanged(&named, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` changed while it was opened", path.display()),
        ));
    }
    let read_limit = GENERATED_GENESIS_RECORD_MAX_BYTES_V1
        .checked_add(1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "generated genesis record read limit overflow",
            )
        })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(read_limit).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "generated genesis record allocation failed",
        )
    })?;
    Read::by_ref(&mut file)
        .take(u64::try_from(read_limit).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "generated genesis record read limit does not fit u64",
            )
        })?)
        .read_to_end(&mut bytes)?;
    if bytes.len() > GENERATED_GENESIS_RECORD_MAX_BYTES_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` exceeds its {}-byte limit",
                path.display(),
                GENERATED_GENESIS_RECORD_MAX_BYTES_V1
            ),
        ));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if named_after.file_type().is_symlink()
        || !generated_genesis_record_metadata_unchanged(&opened, &opened_after)
        || !generated_genesis_record_metadata_unchanged(&opened_after, &named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` changed while it was read", path.display()),
        ));
    }
    let record = String::from_utf8(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` is not UTF-8", path.display()),
        )
    })?;
    let payload = record.strip_suffix('\n').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` must contain exactly one LF-terminated record",
                path.display()
            ),
        )
    })?;
    if payload.is_empty()
        || payload.as_bytes().contains(&b'\r')
        || payload.as_bytes().contains(&b'\n')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` must contain exactly one LF-terminated record",
                path.display()
            ),
        ));
    }
    Ok(record)
}
fn generated_genesis_record_metadata_unchanged(
    expected: &fs::Metadata,
    observed: &fs::Metadata,
) -> bool {
    if !expected.is_file() || !observed.is_file() || expected.len() != observed.len() {
        return false;
    }
    #[cfg(unix)]
    {
        expected.dev() == observed.dev()
            && expected.ino() == observed.ino()
            && expected.mtime() == observed.mtime()
            && expected.mtime_nsec() == observed.mtime_nsec()
            && expected.ctime() == observed.ctime()
            && expected.ctime_nsec() == observed.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        expected.modified().ok() == observed.modified().ok()
    }
}

pub(super) fn validate_genesis_profile_inputs(
    genesis_profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<&str>,
) -> Result<()> {
    if let Some(profile) = genesis_profile
        && profile.requires_seed()
        && vrf_seed_hex.is_none()
    {
        return Err(SupervisorError::Config(format!(
            "genesis profile {profile:?} requires a 32-byte hexadecimal VRF seed"
        )));
    }
    if genesis_profile.is_none() && vrf_seed_hex.is_some() {
        return Err(SupervisorError::Config(
            "a VRF seed requires a genesis profile (NPoS mode)".to_owned(),
        ));
    }
    if let Some(seed) = vrf_seed_hex
        && (seed.len() != VRF_SEED_HEX_CHARS || !seed.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(SupervisorError::Config(
            "VRF seed must be exactly 32 hexadecimal bytes".to_owned(),
        ));
    }
    Ok(())
}
pub(super) fn validate_kagami_manifest_chain(value: &Value, expected_chain_id: &str) -> Result<()> {
    let object = value.as_object().ok_or_else(|| {
        SupervisorError::KagamiInvocation("`kagami` JSON payload must be an object".to_owned())
    })?;
    let raw_chain_id = object.get("chain").and_then(Value::as_str).ok_or_else(|| {
        SupervisorError::KagamiInvocation(
            "`kagami` JSON payload must contain a string `chain` field".to_owned(),
        )
    })?;
    let chain_id = raw_chain_id.parse::<ChainId>().map_err(|error| {
        SupervisorError::KagamiInvocation(format!(
            "`kagami` emitted invalid chain id `{raw_chain_id}`: {error}"
        ))
    })?;
    let canonical_chain_id = chain_id.to_string();
    if raw_chain_id != canonical_chain_id {
        return Err(SupervisorError::KagamiInvocation(format!(
            "`kagami` emitted non-canonical chain id `{raw_chain_id}`; expected `{canonical_chain_id}`"
        )));
    }
    if canonical_chain_id != expected_chain_id {
        return Err(SupervisorError::KagamiInvocation(format!(
            "`kagami` emitted chain id `{canonical_chain_id}` instead of requested `{expected_chain_id}`"
        )));
    }
    Ok(())
}
