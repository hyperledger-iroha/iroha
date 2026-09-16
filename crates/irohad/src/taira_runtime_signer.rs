//! Fixed-descriptor runtime signer used by the first-release Taira launcher.
//!
//! Taira keeps each validator's Soracloud runtime key in its deployment
//! supervisor.  The supervisor opens the owner-only regular file and passes it
//! to the daemon as inherited descriptor 198.  No path, key, or alternate
//! descriptor is accepted through arguments, configuration, or environment.
//! Independent mint-finality seed custody uses consumed descriptor 199, bound
//! only after the exact signed genesis has been authenticated. Optional global-beacon
//! custody consumes descriptor 200 only when the exact public provider binding is
//! configured; the canonical credential is bound to the expected genesis network.

use crate::{
    IrohaRuntimeDeps, IrohaRuntimeProviderBindingV1, IrohaRuntimeProviderBindingsV1,
    IrohaRuntimeProviderRegistryErrorV1, IrohaRuntimeProviderRegistryV1,
    IrohaRuntimeProviderSlotV1,
    soracloud_runtime_signer::{
        SoracloudRuntimeMutationSignerV1, SoracloudRuntimeSignerProbeErrorV1,
        SoracloudRuntimeSignerQualificationV1, SoracloudRuntimeSigningErrorV1,
    },
};
use iroha_config::parameters::{
    actual::{NexusStorageWeights, Root as Config, SoracloudRuntime},
    defaults::{
        soracloud_runtime as soracloud_runtime_defaults,
        taira::{
            INROU_EGRESS_MAX_BYTES_PER_MINUTE as TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE_V1,
            INROU_EGRESS_RATE_PER_MINUTE as TAIRA_INROU_EGRESS_RATE_PER_MINUTE_V1,
            NEXUS_KURA_BLOCKS_BPS as TAIRA_NEXUS_KURA_BLOCKS_BPS_V1,
            NEXUS_SORAFS_BPS as TAIRA_NEXUS_SORAFS_BPS_V1,
            NEXUS_STORAGE_BUDGET_BYTES as TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1,
            NEXUS_WSV_SNAPSHOTS_BPS as TAIRA_NEXUS_WSV_SNAPSHOTS_BPS_V1,
            SORAFS_STORAGE_CAP_BYTES as TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
        },
    },
};
use iroha_core::{
    sumeragi::GenesisV2Bootstrap, zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1,
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    isi::kagemusha_v1::KagemushaMintFinalityEpochRosterV1,
    soracloud::{
        SoracloudRuntimeProvenancePurposeV1, validate_soracloud_runtime_provenance_preimage_v1,
    },
    transaction::{SignedTransaction, TransactionBuilder, TransactionPayload},
};
use iroha_model_base::peer::PeerId;
use std::{
    ffi::OsStr,
    fmt,
    fs::{File, OpenOptions},
    io::{Read as _, Seek as _, Write as _},
    num::{NonZeroU32, NonZeroU64},
    os::{
        fd::{FromRawFd as _, RawFd},
        unix::fs::MetadataExt as _,
    },
    sync::Arc,
    time::Duration,
};

use zeroize::Zeroizing;

fn invocation_does_not_start_a_node(argument: &OsStr) -> bool {
    matches!(
        argument.to_str(),
        Some("--check-config" | "--help" | "-h" | "--version" | "-V")
    )
}

/// Fixed inherited descriptor containing the Taira runtime signer key.
pub const TAIRA_RUNTIME_SIGNER_FD_V1: RawFd = 198;
/// Fixed inherited descriptor containing the independent mint-finality seed.
pub const TAIRA_MINT_FINALITY_SEED_FD_V1: RawFd = 199;
/// Fixed inherited descriptor containing the configured global-beacon credential.
pub const TAIRA_GLOBAL_BEACON_CREDENTIAL_FD_V1: RawFd = 200;
/// Exact byte length of the raw independent mint-finality seed record.
pub const TAIRA_MINT_FINALITY_SEED_BYTES_V1: u64 = 32;
/// Exact adapter/public-policy revision of the first-release Taira signer.
pub const TAIRA_RUNTIME_SIGNER_REVISION_V1: u64 = 1;
/// Exact byte length of one canonical Ed25519 private multihash plus newline.
pub const TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1: u64 = 71;
/// Canonical public Taira chain identity accepted by this launcher.
pub const TAIRA_CHAIN_ID_V1: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
/// Canonical I105 address discriminant accepted by this launcher.
pub const TAIRA_CHAIN_DISCRIMINANT_V1: u16 = 369;
/// Exact first-release Taira validator count.
pub const TAIRA_VALIDATOR_COUNT_V1: usize = 4;
/// Exact aggregate Inrou CPU ceiling for one first-release Taira validator.
pub const TAIRA_INROU_MAX_CPU_MILLIS_V1: u32 =
    iroha_config::parameters::defaults::taira::INROU_MAX_CPU_MILLIS;
/// Exact aggregate Inrou memory ceiling for one first-release Taira validator.
pub const TAIRA_INROU_MAX_MEMORY_BYTES_V1: u64 =
    iroha_config::parameters::defaults::taira::INROU_MAX_MEMORY_BYTES;
/// Exact aggregate Inrou writable-storage ceiling for one first-release Taira validator.
pub const TAIRA_INROU_MAX_STORAGE_BYTES_V1: u64 =
    iroha_config::parameters::defaults::taira::INROU_MAX_STORAGE_BYTES;
/// Exact immutable Inrou guest-image ceiling for one first-release Taira validator.
pub const TAIRA_INROU_GUEST_IMAGE_MAX_BYTES_V1: u64 =
    iroha_config::parameters::defaults::taira::INROU_GUEST_IMAGE_MAX_BYTES;
/// Exact Inrou startup grace for one first-release Taira validator.
pub const TAIRA_INROU_START_GRACE_MS_V1: u64 = 30_000;
/// Exact Inrou shutdown grace for one first-release Taira validator.
pub const TAIRA_INROU_STOP_GRACE_MS_V1: u64 = 10_000;

const TAIRA_RUNTIME_SIGNER_HANDLE_PREFIX_V1: &str = "software://taira/inrou/";
const TAIRA_RUNTIME_SIGNER_POLICY_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.taira.runtime-signer.compiled-policy.digest.v1\0";
const TAIRA_RUNTIME_SIGNER_COMPILED_POLICY_V1: &[u8] = b"algorithm=ed25519;credential=inherited-fd-198-consumed-after-load;descriptor=stable-owner-euid-regular-mode-0600-nlink-1-size-71;key=canonical-private-multihash-plus-newline;handle=software://taira/inrou/<lowercase-raw-public-key-hex>;authority=account-id(public-key);transactions=exact-authority-payload;provenance=canonical-soracloud-v1-domain-version-purpose-preimage;qualification=active-nontest;";

fn taira_runtime_signer_policy_digest_v1() -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TAIRA_RUNTIME_SIGNER_POLICY_DIGEST_DOMAIN_V1);
    hasher.update(&TAIRA_RUNTIME_SIGNER_REVISION_V1.to_be_bytes());
    hasher.update(
        &u64::try_from(TAIRA_RUNTIME_SIGNER_COMPILED_POLICY_V1.len())
            .expect("compiled Taira signer policy length fits u64")
            .to_be_bytes(),
    );
    hasher.update(TAIRA_RUNTIME_SIGNER_COMPILED_POLICY_V1);
    *hasher.finalize().as_bytes()
}

fn validate_taira_launcher_profile_v1(
    chain_id: &str,
    chain_discriminant: u16,
    trusted_peer_count: usize,
    validator_roster_len: usize,
    runtime: &SoracloudRuntime,
) -> Result<(), String> {
    if chain_id != TAIRA_CHAIN_ID_V1 {
        return Err(format!(
            "Taira launcher requires canonical chain id {TAIRA_CHAIN_ID_V1}"
        ));
    }
    if chain_discriminant != TAIRA_CHAIN_DISCRIMINANT_V1 {
        return Err(format!(
            "Taira launcher requires chain discriminant {TAIRA_CHAIN_DISCRIMINANT_V1}"
        ));
    }
    if trusted_peer_count != TAIRA_VALIDATOR_COUNT_V1
        || validator_roster_len != TAIRA_VALIDATOR_COUNT_V1
    {
        return Err(format!(
            "Taira launcher requires exactly {TAIRA_VALIDATOR_COUNT_V1} trusted validator peers"
        ));
    }
    if !runtime.production_mode {
        return Err("Taira launcher requires Soracloud production mode".to_owned());
    }
    if runtime.hydration_concurrency
        != std::num::NonZeroUsize::new(
            iroha_config::parameters::defaults::taira::HYDRATION_CONCURRENCY,
        )
        .expect("nonzero Taira worker capacity")
        || runtime.prepared_runtime_cache_capacity
            != std::num::NonZeroUsize::new(
                iroha_config::parameters::defaults::taira::PREPARED_RUNTIME_CACHE_CAPACITY,
            )
            .expect("nonzero Taira worker capacity")
    {
        return Err(
            "Taira launcher requires the exact V1 hydration-worker and prepared-runtime capacities"
                .to_owned(),
        );
    }
    let inrou = &runtime.inrou;
    if !inrou.enabled {
        return Err("Taira launcher requires enabled Inrou PortableVM V1 hosting".to_owned());
    }
    let uid = inrou
        .portable_vm_uid
        .ok_or_else(|| "enabled Taira Inrou hosting requires portable_vm_uid".to_owned())?
        .get();
    let gid = inrou
        .portable_vm_gid
        .ok_or_else(|| "enabled Taira Inrou hosting requires portable_vm_gid".to_owned())?
        .get();
    if soracloud_runtime_defaults::inrou_portable_vm_identity_slot(uid, gid).is_none() {
        return Err(format!(
            "Taira Inrou uid/gid must be one equal canonical slot pair in {}..{} (upper bound exclusive)",
            soracloud_runtime_defaults::INROU_PORTABLE_VM_ID_BASE,
            soracloud_runtime_defaults::INROU_PORTABLE_VM_ID_MAX_EXCLUSIVE,
        ));
    }
    if inrou.guest_image_max_bytes.get() != TAIRA_INROU_GUEST_IMAGE_MAX_BYTES_V1
        || inrou.max_cpu_millis.get() != TAIRA_INROU_MAX_CPU_MILLIS_V1
        || inrou.max_memory_bytes.get() != TAIRA_INROU_MAX_MEMORY_BYTES_V1
        || inrou.max_storage_bytes.get() != TAIRA_INROU_MAX_STORAGE_BYTES_V1
    {
        return Err("Taira launcher requires the exact V1 Inrou resource ceilings".to_owned());
    }
    if inrou.start_grace != Duration::from_millis(TAIRA_INROU_START_GRACE_MS_V1)
        || inrou.stop_grace != Duration::from_millis(TAIRA_INROU_STOP_GRACE_MS_V1)
    {
        return Err("Taira launcher requires the exact V1 Inrou lifecycle graces".to_owned());
    }
    let egress = &runtime.egress;
    if egress.default_allow
        || !egress.allowed_hosts.is_empty()
        || egress.rate_per_minute.map(NonZeroU32::get)
            != Some(TAIRA_INROU_EGRESS_RATE_PER_MINUTE_V1)
        || egress.max_bytes_per_minute.map(NonZeroU64::get)
            != Some(TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE_V1)
    {
        return Err(
            "Taira launcher requires the exact deny-by-default V1 Inrou egress profile".to_owned(),
        );
    }
    Ok(())
}

fn validate_taira_storage_profile_v1(
    local_budget_bytes: Option<u64>,
    effective_budget_bytes: Option<u64>,
    weights: NexusStorageWeights,
    configured_sorafs_capacity_bytes: Option<u64>,
    sorafs_provider_enabled: bool,
    sorafs_capacity_bytes: u64,
) -> Result<(), String> {
    if local_budget_bytes != Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1)
        || effective_budget_bytes != Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1)
    {
        return Err(format!(
            "Taira launcher requires the exact {TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1}-byte Nexus storage budget"
        ));
    }
    if weights.kura_blocks_bps != TAIRA_NEXUS_KURA_BLOCKS_BPS_V1
        || weights.wsv_snapshots_bps != TAIRA_NEXUS_WSV_SNAPSHOTS_BPS_V1
        || weights.sorafs_bps != TAIRA_NEXUS_SORAFS_BPS_V1
    {
        return Err("Taira launcher requires the exact V1 Nexus storage weights".to_owned());
    }
    if configured_sorafs_capacity_bytes != Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1) {
        return Err(format!(
            "Taira launcher requires an explicit {TAIRA_SORAFS_STORAGE_CAP_BYTES_V1}-byte SoraFS storage cap before Nexus clamping"
        ));
    }
    if sorafs_provider_enabled {
        return Err(
            "Taira launcher requires embedded SoraFS provider storage to be disabled".to_owned(),
        );
    }
    if sorafs_capacity_bytes != TAIRA_SORAFS_STORAGE_CAP_BYTES_V1 {
        return Err(format!(
            "Taira launcher requires the exact {TAIRA_SORAFS_STORAGE_CAP_BYTES_V1}-byte effective SoraFS storage cap"
        ));
    }
    Ok(())
}

fn validate_taira_launcher_config_v1(config: &Config) -> Result<(), String> {
    if config.nexus.storage.max_wsv_memory_bytes.get()
        != iroha_config::parameters::defaults::taira::NEXUS_MAX_WSV_MEMORY_BYTES
    {
        return Err("Taira launcher requires the exact bounded WSV memory budget".to_owned());
    }
    let trusted_peers = config.common.trusted_peers.value();
    validate_taira_launcher_profile_v1(
        config.common.chain.as_ref(),
        *config.common.chain_discriminant.value(),
        trusted_peers.others.len().saturating_add(1),
        trusted_peers.validator_roster_len(),
        &config.soracloud_runtime,
    )?;
    validate_taira_storage_profile_v1(
        config
            .nexus
            .storage
            .local_budget_bytes
            .map(|bytes| bytes.get()),
        config
            .nexus
            .storage
            .effective_local_budget_bytes
            .map(|bytes| bytes.get()),
        config.nexus.storage.disk_budget_weights,
        config
            .nexus
            .storage
            .configured_sorafs_max_capacity_bytes()
            .map(|bytes| bytes.get()),
        config.torii.sorafs_storage.enabled,
        config.torii.sorafs_storage.max_capacity_bytes.get(),
    )
}

/// Payload-free fixed-descriptor signer startup failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TairaRuntimeSignerErrorV1 {
    /// A fixed runtime descriptor is absent or unreadable.
    DescriptorUnavailable,
    /// The descriptor does not identify one stable owner-only regular file.
    UntrustedDescriptor,
    /// The file does not contain exactly one canonical Ed25519 key record.
    InvalidKey,
}

impl fmt::Display for TairaRuntimeSignerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DescriptorUnavailable => "Taira runtime signer descriptor is unavailable",
            Self::UntrustedDescriptor => "Taira runtime signer descriptor is not trusted",
            Self::InvalidKey => "Taira runtime signer key record is invalid",
        })
    }
}

impl std::error::Error for TairaRuntimeSignerErrorV1 {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DescriptorIdentityV1 {
    device: u64,
    inode: u64,
    length: u64,
    owner: u32,
    mode: u32,
    links: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl DescriptorIdentityV1 {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            owner: metadata.uid(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn same_security_identity_after_consumption(&self, metadata: &std::fs::Metadata) -> bool {
        metadata.is_file()
            && metadata.dev() == self.device
            && metadata.ino() == self.inode
            && metadata.uid() == self.owner
            && metadata.mode() == self.mode
            && metadata.nlink() == self.links
            && metadata.len() == 0
    }
}

fn consume_trusted_key_file(
    file: &mut File,
    identity: &DescriptorIdentityV1,
    zeroed_key_record: &[u8],
) -> Result<(), TairaRuntimeSignerErrorV1> {
    file.seek(std::io::SeekFrom::Start(0))
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    file.write_all(zeroed_key_record)
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    file.sync_data()
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    file.set_len(0)
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    file.sync_data()
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    let consumed = file
        .metadata()
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    if !identity.same_security_identity_after_consumption(&consumed) {
        return Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor);
    }
    Ok(())
}

pub(crate) fn load_private_record_from_file<T>(
    mut file: File,
    record_bytes: u64,
    parse: impl FnOnce(&[u8]) -> Result<T, TairaRuntimeSignerErrorV1>,
) -> Result<T, TairaRuntimeSignerErrorV1> {
    let before_metadata = file
        .metadata()
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    let before = DescriptorIdentityV1::from_metadata(&before_metadata);
    let effective_uid = rustix::process::geteuid().as_raw();
    if !before_metadata.is_file()
        || before.owner != effective_uid
        || before.mode & 0o7777 != 0o600
        || before.links != 1
        || before.length != record_bytes
    {
        return Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor);
    }
    let capacity =
        usize::try_from(record_bytes).map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(capacity + 1));
    std::io::Read::by_ref(&mut file)
        .take(record_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    let after = DescriptorIdentityV1::from_metadata(
        &file
            .metadata()
            .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?,
    );
    if after != before || bytes.len() != capacity {
        return Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor);
    }
    let parsed = parse(&bytes);
    bytes.fill(0);
    consume_trusted_key_file(&mut file, &before, &bytes)?;
    parsed
}

fn load_key_pair_from_file(file: File) -> Result<KeyPair, TairaRuntimeSignerErrorV1> {
    load_private_record_from_file(file, TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1, |bytes| {
        let record = bytes
            .strip_suffix(b"\n")
            .ok_or(TairaRuntimeSignerErrorV1::InvalidKey)?;
        let literal =
            std::str::from_utf8(record).map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)?;
        let exposed = literal
            .parse::<ExposedPrivateKey>()
            .map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)?;
        if exposed.0.algorithm() != Algorithm::Ed25519
            || exposed
                .try_to_multihash_string()
                .map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)?
                != literal
        {
            return Err(TairaRuntimeSignerErrorV1::InvalidKey);
        }
        KeyPair::from_private_key(exposed.0).map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)
    })
}

fn load_mint_finality_seed_from_file(
    file: File,
) -> Result<Zeroizing<[u8; 32]>, TairaRuntimeSignerErrorV1> {
    load_private_record_from_file(file, TAIRA_MINT_FINALITY_SEED_BYTES_V1, |bytes| {
        let seed: [u8; 32] = bytes
            .try_into()
            .map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)?;
        Ok(Zeroizing::new(seed))
    })
}

#[allow(
    unsafe_code,
    reason = "the Taira launcher contract transfers unique ownership of fixed inherited descriptors"
)]
pub(crate) fn take_inherited_private_file(
    descriptor: RawFd,
) -> Result<File, TairaRuntimeSignerErrorV1> {
    if !matches!(
        descriptor,
        TAIRA_RUNTIME_SIGNER_FD_V1
            | TAIRA_MINT_FINALITY_SEED_FD_V1
            | TAIRA_GLOBAL_BEACON_CREDENTIAL_FD_V1
    ) {
        return Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor);
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let descriptor_path = format!("/proc/self/fd/{descriptor}");
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let descriptor_path = format!("/dev/fd/{descriptor}");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(descriptor_path)
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    // SAFETY: the deployment supervisor transfers each fixed descriptor to this launcher as
    // its only owner. Opening its kernel descriptor path proves it is live before ownership
    // is constructed. Drop closes the inherited descriptor; the owned duplicate is consumed.
    let inherited = unsafe { File::from_raw_fd(descriptor) };
    drop(inherited);
    Ok(file)
}

fn load_global_beacon_signer_from_file(
    file: File,
    network_id: &NetworkId,
    configured: &IrohaRuntimeProviderBindingV1,
) -> Result<
    Arc<dyn iroha_core::beacon::GlobalThresholdBeaconPartialSignerV1>,
    TairaRuntimeSignerErrorV1,
> {
    let length = file
        .metadata()
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?
        .len();
    let maximum =
        u64::try_from(crate::external_software_signer::MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1)
            .map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)?;
    if length == 0 || length > maximum {
        return Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor);
    }
    load_private_record_from_file(file, length, |bytes| {
        crate::external_software_signer::decode_global_beacon_runtime_signer_v1(
            bytes, network_id, configured,
        )
        .map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)
    })
}

fn load_inherited_global_beacon_signer(
    network_id: &NetworkId,
    configured: &IrohaRuntimeProviderBindingV1,
) -> Result<
    Arc<dyn iroha_core::beacon::GlobalThresholdBeaconPartialSignerV1>,
    TairaRuntimeSignerErrorV1,
> {
    load_global_beacon_signer_from_file(
        take_inherited_private_file(TAIRA_GLOBAL_BEACON_CREDENTIAL_FD_V1)?,
        network_id,
        configured,
    )
}

fn load_inherited_key_pair() -> Result<KeyPair, TairaRuntimeSignerErrorV1> {
    load_key_pair_from_file(take_inherited_private_file(TAIRA_RUNTIME_SIGNER_FD_V1)?)
}

fn bind_taira_mint_finality_authority(
    network_id: NetworkId,
    local_validator: &PeerId,
    epoch: &KagemushaMintFinalityEpochRosterV1,
    seed: Zeroizing<[u8; 32]>,
) -> Result<KagemushaMintFinalityLocalAuthorityV1, String> {
    if epoch.network_id != network_id {
        return Err("Taira mint-finality roster does not match the configured network".to_owned());
    }
    let validator_index = epoch
        .validators
        .iter()
        .position(|entry| &entry.validator == local_validator)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| "Taira mint-finality roster has no exact local validator".to_owned())?;
    KagemushaMintFinalityLocalAuthorityV1::new(Arc::new(epoch.clone()), seed, validator_index)
        .map_err(|_| {
            "Taira mint-finality seed does not match its authenticated validator roster".to_owned()
        })
}

fn resolve_taira_mint_finality_runtime(
    config: &Config,
    authenticated_genesis: &GenesisV2Bootstrap,
    dependencies: IrohaRuntimeDeps,
) -> Result<IrohaRuntimeDeps, String> {
    if dependencies.kagemusha_mint_finality_authority.is_some() {
        return Err("Taira rejects a second mint-finality runtime authority".to_owned());
    }
    let context = authenticated_genesis.context();
    let network_id = NetworkId::from_genesis_hash(config.genesis.expected_hash);
    if context.network_id != network_id
        || config.common.peer.id.public_key() != config.common.key_pair.public_key()
    {
        return Err(
            "Taira mint-finality context does not match the configured local node".to_owned(),
        );
    }
    let seed = load_mint_finality_seed_from_file(
        take_inherited_private_file(TAIRA_MINT_FINALITY_SEED_FD_V1)
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let authority = bind_taira_mint_finality_authority(
        network_id,
        &config.common.peer.id,
        &context.kagemusha_mint_finality_epoch_roster,
        seed,
    )?;
    Ok(dependencies.with_kagemusha_mint_finality_authority(Arc::new(authority)))
}

fn signer_handle(public_key: &PublicKey) -> Result<String, TairaRuntimeSignerErrorV1> {
    let (algorithm, payload) = public_key
        .try_to_bytes()
        .map_err(|_| TairaRuntimeSignerErrorV1::InvalidKey)?;
    if algorithm != Algorithm::Ed25519 || payload.len() != 32 {
        return Err(TairaRuntimeSignerErrorV1::InvalidKey);
    }
    Ok(format!(
        "{TAIRA_RUNTIME_SIGNER_HANDLE_PREFIX_V1}{}",
        hex::encode(payload)
    ))
}

struct TairaRuntimeSignerV1 {
    handle: String,
    key_pair: KeyPair,
}

impl TairaRuntimeSignerV1 {
    fn from_key_pair(key_pair: KeyPair) -> Result<Self, TairaRuntimeSignerErrorV1> {
        let handle = signer_handle(key_pair.public_key())?;
        Ok(Self { handle, key_pair })
    }
}

impl SoracloudRuntimeMutationSignerV1 for TairaRuntimeSignerV1 {
    fn handle(&self) -> &str {
        &self.handle
    }

    fn authority(&self) -> AccountId {
        AccountId::new(self.key_pair.public_key().clone())
    }

    fn public_key(&self) -> Result<PublicKey, SoracloudRuntimeSignerProbeErrorV1> {
        Ok(self.key_pair.public_key().clone())
    }

    fn qualification(
        &self,
    ) -> Result<SoracloudRuntimeSignerQualificationV1, SoracloudRuntimeSignerProbeErrorV1> {
        Ok(SoracloudRuntimeSignerQualificationV1::new(
            TAIRA_RUNTIME_SIGNER_REVISION_V1,
            taira_runtime_signer_policy_digest_v1(),
            true,
            false,
        ))
    }

    fn sign_transaction(
        &self,
        payload: TransactionPayload,
    ) -> Result<SignedTransaction, SoracloudRuntimeSigningErrorV1> {
        if payload.authority() != &self.authority() {
            return Err(SoracloudRuntimeSigningErrorV1::InputAuthorityMismatch);
        }
        TransactionBuilder::from_payload(payload)
            .map_err(|_| SoracloudRuntimeSigningErrorV1::Refused)?
            .try_sign(self.key_pair.private_key())
            .map_err(|_| SoracloudRuntimeSigningErrorV1::Refused)
    }

    fn sign_provenance(
        &self,
        purpose: SoracloudRuntimeProvenancePurposeV1,
        preimage: &[u8],
    ) -> Result<Signature, SoracloudRuntimeSigningErrorV1> {
        validate_soracloud_runtime_provenance_preimage_v1(purpose, preimage)
            .map_err(|_| SoracloudRuntimeSigningErrorV1::InvalidProvenancePreimage)?;
        Signature::try_new(self.key_pair.private_key(), preimage)
            .map_err(|_| SoracloudRuntimeSigningErrorV1::Refused)
    }
}

struct TairaRuntimeProviderRegistryV1 {
    signer: Arc<TairaRuntimeSignerV1>,
}

impl TairaRuntimeProviderRegistryV1 {
    fn from_inherited_descriptor() -> Result<Self, TairaRuntimeSignerErrorV1> {
        Ok(Self {
            signer: Arc::new(TairaRuntimeSignerV1::from_key_pair(
                load_inherited_key_pair()?,
            )?),
        })
    }
}

fn select_taira_provider_bindings<'a>(
    bindings: impl IntoIterator<Item = &'a IrohaRuntimeProviderBindingV1>,
) -> Result<
    (
        &'a IrohaRuntimeProviderBindingV1,
        Option<&'a IrohaRuntimeProviderBindingV1>,
    ),
    IrohaRuntimeProviderRegistryErrorV1,
> {
    let mut soracloud = None;
    let mut beacon = None;
    for binding in bindings {
        let previous = match binding.slot() {
            IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner => {
                soracloud.replace(binding)
            }
            IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner => beacon.replace(binding),
            _ => return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution),
        };
        if previous.is_some() {
            return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution);
        }
    }
    Ok((
        soracloud.ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?,
        beacon,
    ))
}

impl TairaRuntimeProviderRegistryV1 {
    fn resolve_with_beacon_loader(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
        load_beacon: impl FnOnce(
            &NetworkId,
            &IrohaRuntimeProviderBindingV1,
        ) -> Result<
            Arc<dyn iroha_core::beacon::GlobalThresholdBeaconPartialSignerV1>,
            TairaRuntimeSignerErrorV1,
        >,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
        let (requested, beacon) = select_taira_provider_bindings(bindings.iter())?;
        let exact = requested
            .soracloud_runtime_signer_binding()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
        let qualification = self
            .signer
            .qualification()
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
        if exact.handle() != self.signer.handle()
            || exact.authority() != &self.signer.authority()
            || exact.public_key() != self.signer.key_pair.public_key()
            || exact.qualification() != qualification
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
        }
        let signer: Arc<dyn SoracloudRuntimeMutationSignerV1> = self.signer.clone();
        let mut dependencies =
            IrohaRuntimeDeps::default().with_soracloud_runtime_mutation_signer(signer);
        if let Some(beacon) = beacon {
            let signer =
                load_beacon(bindings.network_id(), beacon).map_err(|error| match error {
                    TairaRuntimeSignerErrorV1::DescriptorUnavailable => {
                        IrohaRuntimeProviderRegistryErrorV1::Unavailable
                    }
                    TairaRuntimeSignerErrorV1::UntrustedDescriptor
                    | TairaRuntimeSignerErrorV1::InvalidKey => {
                        IrohaRuntimeProviderRegistryErrorV1::BindingMismatch
                    }
                })?;
            dependencies = dependencies.with_sumeragi_global_beacon_partial_signer(signer);
        }
        Ok(dependencies)
    }
}

impl IrohaRuntimeProviderRegistryV1 for TairaRuntimeProviderRegistryV1 {
    fn resolve(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
        self.resolve_with_beacon_loader(bindings, load_inherited_global_beacon_signer)
    }
}

/// Run Taira with consumed signer/seed descriptors 198/199 and optional beacon credential 200.
///
/// Config validation, help, and version introspection remain offline and do not
/// read the descriptors. Every node-starting invocation resolves exactly the
/// Soracloud signer and any explicitly configured global-beacon signer through [`crate::run_with_runtime_provider_registry`].
pub fn main_entry() {
    if crate::beacon_bootstrap::dispatch_if_requested() {
        return;
    }
    crate::soracloud_runtime::dispatch_inrou_internal_launcher_if_requested();
    if std::env::args_os().any(|argument| invocation_does_not_start_a_node(&argument)) {
        if let Err(report) = crate::run_with_config_guard(validate_taira_launcher_config_v1) {
            eprintln!("{report:?}");
            std::process::exit(1);
        }
        return;
    }
    let registry = match TairaRuntimeProviderRegistryV1::from_inherited_descriptor() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    if let Err(report) = crate::run_with_runtime_provider_registry_and_config_guard(
        &registry,
        validate_taira_launcher_config_v1,
        resolve_taira_mint_finality_runtime,
    ) {
        eprintln!("{report:?}");
        std::process::exit(1);
    }
}

// This branch exists only in the existing feature-isolated native test daemon.
// It reuses the production registry, private codecs and authenticated mint factory;
// it does not attest Linux/Inrou hosting or alter the shipping launcher's guard.
#[cfg(any(test, feature = "test-network-message-control"))]
fn validate_production_beacon_fixture_profile(config: &Config) -> Result<(), String> {
    let peers = config.common.trusted_peers.value();
    if config.common.chain.as_ref() != TAIRA_CHAIN_ID_V1
        || *config.common.chain_discriminant.value() != TAIRA_CHAIN_DISCRIMINANT_V1
        || peers.others.len().saturating_add(1) != TAIRA_VALIDATOR_COUNT_V1
        || peers.validator_roster_len() != TAIRA_VALIDATOR_COUNT_V1
        || !config.soracloud_runtime.production_mode
        || config.soracloud_runtime.inrou.enabled
        || config.soracloud_runtime.inrou.portable_vm_uid.is_some()
        || config.soracloud_runtime.inrou.portable_vm_gid.is_some()
        || config
            .soracloud_runtime
            .inrou
            .trusted_guest_artifact
            .is_some()
    {
        return Err(
            "production beacon fixture requires an exact four-seat Taira Core-only profile".into(),
        );
    }
    Ok(())
}
#[cfg(feature = "test-network-message-control")]
pub(crate) fn dispatch_production_beacon_fixture_if_requested() -> bool {
    if !std::env::args_os().any(|arg| arg == "--test-network-production-beacon-custody") {
        return false;
    }
    // A binary combining deterministic Parliament providers cannot represent this fixture.
    if cfg!(feature = "test-network-parliament-signers") {
        eprintln!("production beacon custody fixture rejects deterministic Parliament providers");
        std::process::exit(1);
    }
    let result = if std::env::args_os().any(|arg| invocation_does_not_start_a_node(&arg)) {
        crate::run_with_config_guard(validate_production_beacon_fixture_profile)
    } else {
        let registry = match TairaRuntimeProviderRegistryV1::from_inherited_descriptor() {
            Ok(registry) => registry,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        };
        crate::run_with_runtime_provider_registry_and_config_guard(
            &registry,
            validate_production_beacon_fixture_profile,
            resolve_taira_mint_finality_runtime,
        )
    };
    if let Err(report) = result {
        eprintln!("{report:?}");
        std::process::exit(1);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        soracloud::encode_soracloud_runtime_provenance_preimage_v1,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use std::{
        fs,
        num::{NonZeroU32, NonZeroUsize},
        os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    };

    fn canonical_runtime_profile() -> SoracloudRuntime {
        let mut runtime = SoracloudRuntime::default();
        runtime.production_mode = true;
        runtime.hydration_concurrency = std::num::NonZeroUsize::new(
            iroha_config::parameters::defaults::taira::HYDRATION_CONCURRENCY,
        )
        .expect("nonzero Taira worker capacity");
        runtime.prepared_runtime_cache_capacity = std::num::NonZeroUsize::new(
            iroha_config::parameters::defaults::taira::PREPARED_RUNTIME_CACHE_CAPACITY,
        )
        .expect("nonzero Taira worker capacity");
        runtime.inrou.enabled = true;
        runtime.inrou.portable_vm_uid = NonZeroU32::new(70_000);
        runtime.inrou.portable_vm_gid = NonZeroU32::new(70_000);
        runtime.inrou.guest_image_max_bytes = NonZeroU64::new(TAIRA_INROU_GUEST_IMAGE_MAX_BYTES_V1)
            .expect("nonzero guest-image budget");
        runtime.inrou.max_cpu_millis =
            NonZeroU32::new(TAIRA_INROU_MAX_CPU_MILLIS_V1).expect("nonzero CPU budget");
        runtime.inrou.max_memory_bytes =
            NonZeroU64::new(TAIRA_INROU_MAX_MEMORY_BYTES_V1).expect("nonzero memory budget");
        runtime.inrou.max_storage_bytes =
            NonZeroU64::new(TAIRA_INROU_MAX_STORAGE_BYTES_V1).expect("nonzero storage budget");
        runtime.inrou.start_grace = Duration::from_millis(TAIRA_INROU_START_GRACE_MS_V1);
        runtime.inrou.stop_grace = Duration::from_millis(TAIRA_INROU_STOP_GRACE_MS_V1);
        runtime.egress.default_allow = false;
        runtime.egress.allowed_hosts.clear();
        runtime.egress.rate_per_minute = NonZeroU32::new(TAIRA_INROU_EGRESS_RATE_PER_MINUTE_V1);
        runtime.egress.max_bytes_per_minute =
            NonZeroU64::new(TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE_V1);
        runtime
    }

    #[test]
    fn offline_introspection_never_requires_the_runtime_signer() {
        for argument in ["--check-config", "--help", "-h", "--version", "-V"] {
            assert!(invocation_does_not_start_a_node(OsStr::new(argument)));
        }
        for argument in ["--config", "--sora", "--genesis-manifest-json"] {
            assert!(!invocation_does_not_start_a_node(OsStr::new(argument)));
        }
    }

    #[test]
    fn launcher_profile_is_exact_and_has_no_generic_network_fallback() {
        let runtime = canonical_runtime_profile();
        validate_taira_launcher_profile_v1(
            TAIRA_CHAIN_ID_V1,
            TAIRA_CHAIN_DISCRIMINANT_V1,
            TAIRA_VALIDATOR_COUNT_V1,
            TAIRA_VALIDATOR_COUNT_V1,
            &runtime,
        )
        .expect("canonical Taira profile");
        assert!(
            validate_taira_launcher_profile_v1(
                "iroha3-taira",
                TAIRA_CHAIN_DISCRIMINANT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                &runtime,
            )
            .is_err()
        );
        assert!(
            validate_taira_launcher_profile_v1(
                TAIRA_CHAIN_ID_V1,
                TAIRA_CHAIN_DISCRIMINANT_V1,
                TAIRA_VALIDATOR_COUNT_V1 - 1,
                TAIRA_VALIDATOR_COUNT_V1,
                &runtime,
            )
            .is_err()
        );
        let mut disabled_inrou = runtime.clone();
        disabled_inrou.inrou.enabled = false;
        disabled_inrou.inrou.portable_vm_uid = None;
        disabled_inrou.inrou.portable_vm_gid = None;
        assert!(
            validate_taira_launcher_profile_v1(
                TAIRA_CHAIN_ID_V1,
                TAIRA_CHAIN_DISCRIMINANT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                &disabled_inrou,
            )
            .is_err()
        );
        let mut exact_inrou = runtime.clone();
        for slot in 0..soracloud_runtime_defaults::INROU_PORTABLE_VM_ID_SLOT_COUNT {
            let id = soracloud_runtime_defaults::INROU_PORTABLE_VM_ID_BASE + slot;
            exact_inrou.inrou.portable_vm_uid = NonZeroU32::new(id);
            exact_inrou.inrou.portable_vm_gid = NonZeroU32::new(id);
            validate_taira_launcher_profile_v1(
                TAIRA_CHAIN_ID_V1,
                TAIRA_CHAIN_DISCRIMINANT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                &exact_inrou,
            )
            .expect("Taira accepts every canonical same-host PortableVM identity slot");
        }
        exact_inrou.inrou.portable_vm_uid = NonZeroU32::new(70_000);
        exact_inrou.inrou.portable_vm_gid = NonZeroU32::new(70_001);
        assert!(
            validate_taira_launcher_profile_v1(
                TAIRA_CHAIN_ID_V1,
                TAIRA_CHAIN_DISCRIMINANT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                &exact_inrou,
            )
            .is_err(),
            "Taira must reject mismatched Inrou uid/gid slots"
        );
        let mut configured_identity = runtime;
        configured_identity.inrou.portable_vm_gid = None;
        assert!(
            validate_taira_launcher_profile_v1(
                TAIRA_CHAIN_ID_V1,
                TAIRA_CHAIN_DISCRIMINANT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                TAIRA_VALIDATOR_COUNT_V1,
                &configured_identity,
            )
            .is_err()
        );
    }

    #[test]
    fn launcher_profile_rejects_noncanonical_inrou_resources_and_egress() {
        let runtime = canonical_runtime_profile();
        let assert_rejected = |runtime: &SoracloudRuntime| {
            assert!(
                validate_taira_launcher_profile_v1(
                    TAIRA_CHAIN_ID_V1,
                    TAIRA_CHAIN_DISCRIMINANT_V1,
                    TAIRA_VALIDATOR_COUNT_V1,
                    TAIRA_VALIDATOR_COUNT_V1,
                    runtime,
                )
                .is_err()
            );
        };

        let mut changed = runtime.clone();
        changed.hydration_concurrency = NonZeroUsize::new(
            std::num::NonZeroUsize::new(
                iroha_config::parameters::defaults::taira::HYDRATION_CONCURRENCY,
            )
            .expect("nonzero Taira worker capacity")
            .get()
                + 1,
        )
        .expect("changed hydration-worker count is nonzero");
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.prepared_runtime_cache_capacity = NonZeroUsize::new(
            std::num::NonZeroUsize::new(
                iroha_config::parameters::defaults::taira::PREPARED_RUNTIME_CACHE_CAPACITY,
            )
            .expect("nonzero Taira worker capacity")
            .get()
                + 1,
        )
        .expect("changed prepared-runtime capacity is nonzero");
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.inrou.guest_image_max_bytes =
            NonZeroU64::new(TAIRA_INROU_GUEST_IMAGE_MAX_BYTES_V1 + 1)
                .expect("changed guest-image budget is nonzero");
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.inrou.max_cpu_millis = NonZeroU32::new(TAIRA_INROU_MAX_CPU_MILLIS_V1 + 1)
            .expect("changed CPU budget is nonzero");
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.inrou.max_memory_bytes = NonZeroU64::new(TAIRA_INROU_MAX_MEMORY_BYTES_V1 + 1)
            .expect("changed memory budget is nonzero");
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.inrou.max_storage_bytes = NonZeroU64::new(TAIRA_INROU_MAX_STORAGE_BYTES_V1 + 1)
            .expect("changed storage budget is nonzero");
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.inrou.start_grace = Duration::from_millis(TAIRA_INROU_START_GRACE_MS_V1 + 1);
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.inrou.stop_grace = Duration::from_millis(TAIRA_INROU_STOP_GRACE_MS_V1 + 1);
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.egress.default_allow = true;
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed
            .egress
            .allowed_hosts
            .push("example.invalid".to_owned());
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.egress.rate_per_minute = NonZeroU32::new(TAIRA_INROU_EGRESS_RATE_PER_MINUTE_V1 + 1);
        assert_rejected(&changed);

        let mut changed = runtime;
        changed.egress.max_bytes_per_minute =
            NonZeroU64::new(TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE_V1 + 1);
        assert_rejected(&changed);
    }

    fn canonical_storage_weights() -> NexusStorageWeights {
        NexusStorageWeights {
            kura_blocks_bps: TAIRA_NEXUS_KURA_BLOCKS_BPS_V1,
            wsv_snapshots_bps: TAIRA_NEXUS_WSV_SNAPSHOTS_BPS_V1,
            sorafs_bps: TAIRA_NEXUS_SORAFS_BPS_V1,
        }
    }

    #[test]
    fn launcher_storage_profile_rejects_noncanonical_budgets_caps_and_provider() {
        assert_eq!(
            u64::try_from(
                u128::from(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1)
                    * u128::from(TAIRA_NEXUS_SORAFS_BPS_V1)
                    / 10_000,
            )
            .expect("Taira SoraFS cap fits u64"),
            TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
            "the compiled cap must be the exact floor of the canonical weighted budget"
        );
        let canonical = || {
            validate_taira_storage_profile_v1(
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                canonical_storage_weights(),
                Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1),
                false,
                TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
            )
        };
        canonical().expect("canonical Taira storage profile");
        assert_eq!(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1, 4 * 1024 * 1024 * 1024);
        assert_eq!(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1, 2560 * 1024 * 1024);
        assert!(
            validate_taira_storage_profile_v1(
                Some(1024 * 1024 * 1024),
                Some(1024 * 1024 * 1024),
                NexusStorageWeights {
                    kura_blocks_bps: 6_000,
                    wsv_snapshots_bps: 2_000,
                    sorafs_bps: 2_000,
                },
                Some(214_748_364),
                false,
                214_748_364,
            )
            .is_err(),
            "the undersized profile cannot admit the first-release guest"
        );

        assert!(
            validate_taira_storage_profile_v1(
                None,
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                canonical_storage_weights(),
                Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1),
                false,
                TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
            )
            .is_err()
        );
        assert!(
            validate_taira_storage_profile_v1(
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1 - 1),
                canonical_storage_weights(),
                Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1),
                false,
                TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
            )
            .is_err()
        );

        let mut wrong_weights = canonical_storage_weights();
        wrong_weights.kura_blocks_bps -= 1;
        wrong_weights.sorafs_bps += 1;
        assert!(
            validate_taira_storage_profile_v1(
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                wrong_weights,
                Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1),
                false,
                TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
            )
            .is_err()
        );
        assert!(
            validate_taira_storage_profile_v1(
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                canonical_storage_weights(),
                Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1),
                true,
                TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
            )
            .is_err()
        );
        assert!(
            validate_taira_storage_profile_v1(
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                canonical_storage_weights(),
                Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1),
                false,
                TAIRA_SORAFS_STORAGE_CAP_BYTES_V1 - 1,
            )
            .is_err()
        );

        for configured_cap in [
            None,
            Some(0),
            Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1 - 1),
            Some(TAIRA_SORAFS_STORAGE_CAP_BYTES_V1 + 1),
            Some(iroha_config::parameters::defaults::sorafs::storage::MAX_CAPACITY_BYTES.get()),
        ] {
            assert!(
                validate_taira_storage_profile_v1(
                    Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                    Some(TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1),
                    canonical_storage_weights(),
                    configured_cap,
                    false,
                    TAIRA_SORAFS_STORAGE_CAP_BYTES_V1,
                )
                .is_err(),
                "normalized noncanonical source cap {configured_cap:?} must be rejected"
            );
        }
    }

    fn key_file(key_pair: &KeyPair) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("temporary signer directory");
        let path = directory.path().join("runtime.private_key");
        let literal = ExposedPrivateKey(key_pair.private_key().clone())
            .try_to_multihash_string()
            .expect("canonical private key");
        assert_eq!(
            literal.len() + 1,
            usize::try_from(TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1)
                .expect("fixed Taira key length fits usize")
        );
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        writeln!(options.open(&path).expect("create signer key"), "{literal}")
            .expect("write signer key");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("protect signer key");
        (directory, path)
    }

    fn open_consumable_key_file(path: &std::path::Path) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("open consumable signer key")
    }

    fn mint_seed_file(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("temporary seed directory");
        let path = directory.path().join("mint-finality.fd199");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .expect("create seed file");
        file.write_all(bytes).expect("write seed file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("protect seed");
        (directory, path)
    }

    #[test]
    fn mint_seed_loader_consumes_exact_private_record_and_preserves_restart_source() {
        let (directory, source) = mint_seed_file(&[0x61; 32]);
        let launch = directory.path().join("launch.fd199");
        fs::copy(&source, &launch).expect("stage seed");
        let mut child = File::open(&launch).expect("inherited probe");
        let seed = load_mint_finality_seed_from_file(open_consumable_key_file(&launch))
            .expect("load independent seed");
        assert_eq!(*seed, [0x61; 32]);
        assert_eq!(fs::metadata(source).expect("restart metadata").len(), 32);
        assert_eq!(fs::metadata(launch).expect("launch metadata").len(), 0);
        let mut byte = [0_u8; 1];
        assert_eq!(child.read(&mut byte).expect("probe consumed inode"), 0);
    }

    #[test]
    fn mint_seed_loader_rejects_wrong_size_mode_links_and_read_only_descriptors() {
        for length in [0, 31, 33, 71] {
            let (_directory, path) = mint_seed_file(&vec![0x62; length]);
            assert!(matches!(
                load_mint_finality_seed_from_file(open_consumable_key_file(&path)),
                Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
            ));
        }
        let (directory, path) = mint_seed_file(&[0x62; 32]);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("weaken seed");
        assert!(matches!(
            load_mint_finality_seed_from_file(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("protect seed");
        let alias = directory.path().join("alias");
        fs::hard_link(&path, &alias).expect("alias seed");
        assert!(matches!(
            load_mint_finality_seed_from_file(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
        ));
        fs::remove_file(alias).expect("remove alias");
        assert!(matches!(
            load_mint_finality_seed_from_file(File::open(path).expect("read-only seed")),
            Err(TairaRuntimeSignerErrorV1::DescriptorUnavailable)
        ));
    }

    fn mint_runtime_roster() -> KagemushaMintFinalityEpochRosterV1 {
        let mut peers = (1_u8..=4)
            .map(|index| {
                PeerId::new(
                    KeyPair::try_from_seed(vec![index; 32], Algorithm::Ed25519)
                        .expect("fixture peer")
                        .public_key()
                        .clone(),
                )
            })
            .collect::<Vec<_>>();
        peers.sort();
        KagemushaMintFinalityEpochRosterV1 {
            version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
            network_id: NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"Taira mint seed admission fixture"))),
            epoch: 0,
            validators: peers.into_iter().enumerate().map(|(index, validator)| {
                iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                    &[0x70 + u8::try_from(index).expect("four validators"); 32], 0, validator)
                    .expect("derive private seed fixture public keys")
            }).collect(),
        }
    }

    #[test]
    fn mint_runtime_binds_only_exact_network_validator_and_private_seed() {
        let roster = mint_runtime_roster();
        let local = &roster.validators[1].validator;
        let authority = bind_taira_mint_finality_authority(
            roster.network_id,
            local,
            &roster,
            Zeroizing::new([0x71; 32]),
        )
        .expect("bind exact runtime seed");
        assert_eq!(authority.signer().validator_index(), 1);
        assert_eq!(authority.epoch(), &roster);
        assert!(
            bind_taira_mint_finality_authority(
                roster.network_id,
                local,
                &roster,
                Zeroizing::new([0x72; 32])
            )
            .is_err()
        );
        let foreign = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::new(
                b"another Taira network",
            )),
        );
        assert!(
            bind_taira_mint_finality_authority(foreign, local, &roster, Zeroizing::new([0x71; 32]))
                .is_err()
        );
        let absent = PeerId::new(
            KeyPair::try_from_seed(vec![99; 32], Algorithm::Ed25519)
                .expect("absent fixture peer")
                .public_key()
                .clone(),
        );
        assert!(
            bind_taira_mint_finality_authority(
                roster.network_id,
                &absent,
                &roster,
                Zeroizing::new([0x71; 32])
            )
            .is_err()
        );
        let mut wrong_epoch = roster.clone();
        wrong_epoch.epoch = 1;
        assert!(
            bind_taira_mint_finality_authority(
                roster.network_id,
                local,
                &wrong_epoch,
                Zeroizing::new([0x71; 32])
            )
            .is_err()
        );
    }

    #[test]
    fn descriptor_loader_accepts_only_canonical_owner_only_ed25519() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("Ed25519 key pair");
        let (_directory, path) = key_file(&key_pair);
        let loaded =
            load_key_pair_from_file(open_consumable_key_file(&path)).expect("load signer key");
        assert_eq!(loaded.public_key(), key_pair.public_key());
        assert_eq!(fs::metadata(path).expect("consumed key metadata").len(), 0);
    }

    #[test]
    fn consumption_preserves_restart_source_and_starves_child_descriptor() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x36; 32], Algorithm::Ed25519).expect("Ed25519 key pair");
        let (directory, source_path) = key_file(&key_pair);
        let launch_path = directory.path().join("runtime.fd198");
        fs::copy(&source_path, &launch_path).expect("stage consumable launch key");
        fs::set_permissions(&launch_path, fs::Permissions::from_mode(0o600))
            .expect("protect launch key");
        let child_descriptor = File::open(&launch_path).expect("open child descriptor probe");

        load_key_pair_from_file(open_consumable_key_file(&launch_path))
            .expect("consume staged launch key");
        assert_eq!(
            fs::metadata(&source_path)
                .expect("persistent restart source metadata")
                .len(),
            TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1
        );
        assert_eq!(
            fs::metadata(&launch_path)
                .expect("consumed launch metadata")
                .len(),
            0
        );
        let output = std::process::Command::new("/bin/cat")
            .stdin(std::process::Stdio::from(child_descriptor))
            .output()
            .expect("spawn child descriptor probe");
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn descriptor_loader_rejects_mode_and_link_substitution() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x32; 32], Algorithm::Ed25519).expect("Ed25519 key pair");
        let (directory, path) = key_file(&key_pair);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("weaken mode");
        assert!(matches!(
            load_key_pair_from_file(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("restore mode");
        fs::hard_link(&path, directory.path().join("linked.private_key"))
            .expect("create hard link");
        assert!(matches!(
            load_key_pair_from_file(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
        ));
    }

    #[test]
    fn descriptor_loader_rejects_noncanonical_or_wrong_algorithm_records() {
        let key_pair =
            KeyPair::try_from_seed(vec![0xAB; 32], Algorithm::Ed25519).expect("Ed25519 key pair");
        let (_directory, path) = key_file(&key_pair);
        let mut bytes = fs::read(&path).expect("read canonical key");
        let letter = bytes
            .iter_mut()
            .find(|byte| byte.is_ascii_hexdigit() && byte.is_ascii_alphabetic())
            .expect("canonical private key contains a hex letter");
        if letter.is_ascii_lowercase() {
            letter.make_ascii_uppercase();
        } else {
            letter.make_ascii_lowercase();
        }
        fs::write(&path, bytes).expect("replace key casing");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("protect key");
        assert!(matches!(
            load_key_pair_from_file(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::InvalidKey)
        ));
        assert_eq!(fs::metadata(path).expect("consumed key metadata").len(), 0);

        let wrong_algorithm = KeyPair::try_from_seed(vec![0xBC; 32], Algorithm::Secp256k1)
            .expect("non-Ed25519 fixture key");
        let (_directory, path) = key_file(&wrong_algorithm);
        assert_eq!(
            fs::metadata(&path).expect("wrong algorithm metadata").len(),
            TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1
        );
        assert!(matches!(
            load_key_pair_from_file(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::InvalidKey)
        ));
        assert_eq!(
            fs::metadata(path)
                .expect("consumed wrong algorithm metadata")
                .len(),
            0
        );
    }

    #[test]
    fn signer_binds_handle_authority_and_exact_payload() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x34; 32], Algorithm::Ed25519).expect("Ed25519 key pair");
        let signer = TairaRuntimeSignerV1::from_key_pair(key_pair).expect("Taira signer");
        assert!(
            signer
                .handle()
                .starts_with(TAIRA_RUNTIME_SIGNER_HANDLE_PREFIX_V1)
        );
        let payload = TransactionBuilder::new(
            NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                    iroha_crypto::Hash::prehashed([0x51; iroha_crypto::Hash::LENGTH]),
                ),
            ),
            signer.authority(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .into_payload()
        .expect("transaction payload");
        let signed_transaction = signer
            .sign_transaction(payload.clone())
            .expect("sign exact transaction");
        assert_eq!(signed_transaction.payload(), &payload);
        signed_transaction
            .verify_signature()
            .expect("valid signature");
    }

    #[test]
    fn signer_policy_digest_binds_the_compiled_policy() {
        let digest = taira_runtime_signer_policy_digest_v1();
        assert_ne!(digest, [0; 32]);
        let mut altered = TAIRA_RUNTIME_SIGNER_COMPILED_POLICY_V1.to_vec();
        altered.push(b'!');
        let mut hasher = blake3::Hasher::new();
        hasher.update(TAIRA_RUNTIME_SIGNER_POLICY_DIGEST_DOMAIN_V1);
        hasher.update(&TAIRA_RUNTIME_SIGNER_REVISION_V1.to_be_bytes());
        hasher.update(
            &u64::try_from(altered.len())
                .expect("altered policy length fits u64")
                .to_be_bytes(),
        );
        hasher.update(&altered);
        assert_ne!(digest, *hasher.finalize().as_bytes());
    }

    #[test]
    fn signer_rejects_cross_purpose_provenance() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x35; 32], Algorithm::Ed25519).expect("Ed25519 key pair");
        let signer = TairaRuntimeSignerV1::from_key_pair(key_pair).expect("Taira signer");
        let withdrawal = encode_soracloud_runtime_provenance_preimage_v1(
            SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw,
            b"canonical-withdrawal-payload",
        )
        .expect("encode withdrawal preimage");
        let signature = signer
            .sign_provenance(
                SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw,
                &withdrawal,
            )
            .expect("sign matching purpose");
        signature
            .verify(signer.key_pair.public_key(), &withdrawal)
            .expect("matching-purpose signature verifies");
        assert!(matches!(
            signer.sign_provenance(
                SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
                &withdrawal,
            ),
            Err(SoracloudRuntimeSigningErrorV1::InvalidProvenancePreimage)
        ));
        assert!(matches!(
            signer.sign_provenance(
                SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw,
                b"bare-account-id-payload",
            ),
            Err(SoracloudRuntimeSigningErrorV1::InvalidProvenancePreimage)
        ));
    }

    fn beacon_file(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("temporary beacon credential directory");
        let path = directory.path().join("beacon.fd200");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .expect("create consumable synthetic beacon credential");
        file.write_all(bytes)
            .expect("write synthetic beacon credential");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("protect credential");
        (directory, path)
    }

    fn beacon_config(
        registry: &TairaRuntimeProviderRegistryV1,
        network_id: NetworkId,
        beacon: Option<&IrohaRuntimeProviderBindingV1>,
    ) -> Config {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../defaults/kagami/iroha3-dev/peer0.toml");
        let source = fs::read_to_string(path).expect("read checked-in default config only");
        let mut table: toml::Table = toml::from_str(&source).expect("parse checked-in default");
        let genesis = table
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .expect("default genesis table");
        assert_eq!(
            genesis.remove("expected_hash_file"),
            Some(toml::Value::String("genesis.expected_hash".to_owned()))
        );
        assert!(
            genesis
                .insert(
                    "expected_hash".to_owned(),
                    toml::Value::String(network_id.to_string())
                )
                .is_none()
        );
        let mut config =
            Config::from_toml_source(iroha_config_base::toml::TomlSource::inline(table))
                .expect("parse public fixture trust without runtime identity files");
        let signer = &registry.signer;
        config.soracloud_runtime.submission.signer = Some(
            iroha_config::parameters::actual::SoracloudRuntimeMutationSignerBinding {
                handle: signer.handle().to_owned(),
                authority: signer.authority(),
                algorithm: Algorithm::Ed25519,
                public_key: signer.key_pair.public_key().clone(),
                revision: TAIRA_RUNTIME_SIGNER_REVISION_V1,
                policy_digest: taira_runtime_signer_policy_digest_v1(),
            },
        );
        config.sumeragi.global_beacon_partial_signer_provider_handle =
            beacon.map(|b| b.handle().to_owned());
        config
            .sumeragi
            .global_beacon_partial_signer_provider_revision = beacon.and_then(|b| b.revision());
        config
            .sumeragi
            .global_beacon_partial_signer_provider_policy_digest =
            beacon.and_then(|b| b.policy_digest());
        config
    }

    fn fixture_registry() -> TairaRuntimeProviderRegistryV1 {
        TairaRuntimeProviderRegistryV1 {
            signer: Arc::new(
                TairaRuntimeSignerV1::from_key_pair(
                    KeyPair::try_from_seed(vec![0x37; 32], Algorithm::Ed25519)
                        .expect("fixture Soracloud key"),
                )
                .expect("fixture Soracloud signer"),
            ),
        }
    }

    #[test]
    fn production_beacon_fixture_guard_keeps_exact_core_only_taira_identity() {
        let network = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::new(
                b"real-custody-fixture",
            )),
        );
        let mut config = beacon_config(&fixture_registry(), network, None);
        config.common.chain = TAIRA_CHAIN_ID_V1.into();
        config.common.chain_discriminant =
            iroha_config_base::WithOrigin::inline(TAIRA_CHAIN_DISCRIMINANT_V1);
        config.soracloud_runtime.production_mode = true;
        validate_production_beacon_fixture_profile(&config).expect("typed Core-only fixture");
        let mut bad = config.clone();
        bad.common.chain = "another-chain".into();
        assert!(validate_production_beacon_fixture_profile(&bad).is_err());
        let mut bad = config.clone();
        bad.common.chain_discriminant = iroha_config_base::WithOrigin::inline(1);
        assert!(validate_production_beacon_fixture_profile(&bad).is_err());
        let mut bad = config.clone();
        bad.soracloud_runtime.production_mode = false;
        assert!(validate_production_beacon_fixture_profile(&bad).is_err());
        let mut bad = config;
        bad.soracloud_runtime.inrou.enabled = true;
        assert!(validate_production_beacon_fixture_profile(&bad).is_err());
    }

    #[test]
    fn beacon_loader_consumes_exact_credential_and_verifies_native_signature() {
        let fixture =
            crate::external_software_signer::consensus_threshold_beacon_broker_test_fixture_v1();
        let binding = fixture
            .catalog
            .iter()
            .next()
            .expect("one exact beacon provider");
        let (directory, source) = beacon_file(&fixture.credential);
        let launch = directory.path().join("launch.fd200");
        fs::copy(&source, &launch).expect("stage launch credential");
        let mut child = File::open(&launch).expect("child descriptor probe");
        let signer = load_global_beacon_signer_from_file(
            open_consumable_key_file(&launch),
            fixture.catalog.network_id(),
            binding,
        )
        .expect("load exact native beacon credential");
        assert_eq!(
            fs::metadata(&source).expect("restart source").len(),
            u64::try_from(fixture.credential.len()).expect("credential length")
        );
        assert_eq!(fs::metadata(&launch).expect("consumed descriptor").len(), 0);
        assert_eq!(child.read(&mut [0_u8; 1]).expect("consumed child inode"), 0);
        let digest =
            crate::external_software_signer::global_beacon_partial_signer_public_inventory_digest_v1(
                *fixture.catalog.network_id(),
                &[(fixture.session.record().clone(), 1)],
            )
            .expect("derive public inventory without private components");
        assert_eq!(binding.policy_digest(), Some(digest));
        let anchor = iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1 {
            height: 40,
            block_hash: iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0x91; 32]),
            ),
        };
        let mut verifier = iroha_core::beacon::GlobalThresholdBeaconPulseAggregatorV1::new(
            fixture.session.clone(),
            41,
            anchor,
        )
        .expect("canonical pulse payload");
        let partial = signer
            .sign_partial(&fixture.session, verifier.payload())
            .expect("native custody signs exact pulse");
        assert!(
            verifier
                .accept_partial(partial)
                .expect("native cryptographic partial verification")
        );
    }

    #[test]
    fn beacon_loader_rejects_wrong_network_qualification_and_corruption() {
        let fixture =
            crate::external_software_signer::consensus_threshold_beacon_broker_test_fixture_v1();
        let exact = fixture.catalog.iter().next().expect("exact provider");
        let foreign_network = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0xD7; 32]),
            ),
        );
        assert_ne!(foreign_network, *fixture.catalog.network_id());
        let (_directory, path) = beacon_file(&fixture.credential);
        assert!(matches!(
            load_global_beacon_signer_from_file(
                open_consumable_key_file(&path),
                &foreign_network,
                exact
            ),
            Err(TairaRuntimeSignerErrorV1::InvalidKey)
        ));
        assert_eq!(
            fs::metadata(path)
                .expect("consumed foreign credential")
                .len(),
            0
        );
        for (handle, revision, digest) in [
            (
                "software://different-beacon-owner",
                exact.revision().expect("revision"),
                exact.policy_digest().expect("digest"),
            ),
            (
                exact.handle(),
                exact.revision().expect("revision") + 1,
                exact.policy_digest().expect("digest"),
            ),
            (
                exact.handle(),
                exact.revision().expect("revision"),
                [0xD8; 32],
            ),
        ] {
            let wrong = IrohaRuntimeProviderBindingsV1::qualified_for_test(
                "bound-credential-fixture",
                IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
                handle,
                revision,
                digest,
            )
            .with_network_id_for_test(*fixture.catalog.network_id());
            let (_directory, path) = beacon_file(&fixture.credential);
            assert!(matches!(
                load_global_beacon_signer_from_file(
                    open_consumable_key_file(&path),
                    fixture.catalog.network_id(),
                    wrong.iter().next().expect("substituted provider")
                ),
                Err(TairaRuntimeSignerErrorV1::InvalidKey)
            ));
            assert_eq!(
                fs::metadata(path)
                    .expect("consumed substituted credential")
                    .len(),
                0
            );
        }
        let mut corrupt = fixture.credential.clone();
        corrupt[0] ^= 1;
        let (_directory, path) = beacon_file(&corrupt);
        assert!(matches!(
            load_global_beacon_signer_from_file(
                open_consumable_key_file(&path),
                fixture.catalog.network_id(),
                exact
            ),
            Err(TairaRuntimeSignerErrorV1::InvalidKey)
        ));
        assert_eq!(
            fs::metadata(path)
                .expect("consumed malformed credential")
                .len(),
            0
        );
    }

    #[test]
    fn beacon_loader_rejects_untrusted_descriptor_and_size() {
        let fixture =
            crate::external_software_signer::consensus_threshold_beacon_broker_test_fixture_v1();
        let binding = fixture.catalog.iter().next().expect("exact provider");
        let load =
            |file| load_global_beacon_signer_from_file(file, fixture.catalog.network_id(), binding);
        for size in [
            0,
            u64::try_from(
                crate::external_software_signer::MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1,
            )
            .expect("max size")
                + 1,
        ] {
            let (_directory, path) = beacon_file(&[]);
            open_consumable_key_file(&path)
                .set_len(size)
                .expect("set bounded size fixture");
            assert!(matches!(
                load(open_consumable_key_file(&path)),
                Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
            ));
        }
        let (directory, path) = beacon_file(&fixture.credential);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("weaken credential");
        assert!(matches!(
            load(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("restore credential");
        let alias = directory.path().join("alias");
        fs::hard_link(&path, &alias).expect("create alias");
        assert!(matches!(
            load(open_consumable_key_file(&path)),
            Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
        ));
        fs::remove_file(alias).expect("remove alias");
        assert!(matches!(
            load(File::open(&path).expect("read-only credential")),
            Err(TairaRuntimeSignerErrorV1::DescriptorUnavailable)
        ));
        assert_ne!(
            fs::metadata(path).expect("unconsumable credential").len(),
            0
        );
        for fd in [-1, 0, 197, 201] {
            assert!(matches!(
                take_inherited_private_file(fd),
                Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor)
            ));
        }
    }

    #[test]
    fn registry_allows_bootstrap_without_beacon_and_rejects_extra_or_duplicate_slots() {
        let fixture =
            crate::external_software_signer::consensus_threshold_beacon_broker_test_fixture_v1();
        let registry = fixture_registry();
        let config = beacon_config(&registry, *fixture.catalog.network_id(), None);
        let bindings =
            IrohaRuntimeProviderBindingsV1::try_from_config(&config).expect("bootstrap catalog");
        let dependencies = registry
            .resolve_with_beacon_loader(&bindings, |_, _| panic!("bootstrap must not access FD200"))
            .expect("bootstrap Soracloud signer only");
        assert!(dependencies.soracloud_runtime_mutation_signer.is_some());
        assert!(dependencies.sumeragi_global_beacon_partial_signer.is_none());
        let soracloud = bindings.iter().next().expect("Soracloud binding");
        let beacon = fixture.catalog.iter().next().expect("beacon binding");
        for duplicated in [
            vec![soracloud, soracloud],
            vec![soracloud, beacon, beacon],
            vec![beacon],
        ] {
            assert!(matches!(
                select_taira_provider_bindings(duplicated),
                Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
            ));
        }
        let mut extra = config.clone();
        extra
            .gov
            .parliament_tle_partial_release_signer_provider_handle =
            Some("software://unexpected-tle".to_owned());
        extra
            .gov
            .parliament_tle_partial_release_signer_provider_revision = Some(1);
        extra
            .gov
            .parliament_tle_partial_release_signer_provider_policy_digest = Some([0xD3; 32]);
        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&extra)
            .expect("valid but unsupported public provider");
        assert!(matches!(
            registry.resolve_with_beacon_loader(&bindings, |_, _| panic!(
                "extra provider rejected before FD200"
            )),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));
    }

    #[test]
    fn registry_resolves_exact_configured_beacon_and_preserves_soracloud_binding() {
        let fixture =
            crate::external_software_signer::consensus_threshold_beacon_broker_test_fixture_v1();
        let registry = fixture_registry();
        let beacon = fixture.catalog.iter().next().expect("beacon provider");
        let config = beacon_config(&registry, *fixture.catalog.network_id(), Some(beacon));
        let bindings =
            IrohaRuntimeProviderBindingsV1::try_from_config(&config).expect("two exact providers");
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings.network_id(), fixture.catalog.network_id());
        let (_directory, path) = beacon_file(&fixture.credential);
        let dependencies = registry
            .resolve_with_beacon_loader(&bindings, |network_id, configured| {
                assert_eq!(network_id, fixture.catalog.network_id());
                assert_eq!(configured, beacon);
                load_global_beacon_signer_from_file(
                    open_consumable_key_file(&path),
                    network_id,
                    configured,
                )
            })
            .expect("resolve both exact native signers");
        assert!(dependencies.soracloud_runtime_mutation_signer.is_some());
        assert!(dependencies.sumeragi_global_beacon_partial_signer.is_some());
        assert_eq!(
            fs::metadata(path).expect("credential consumed once").len(),
            0
        );
        assert!(matches!(
            registry.resolve_with_beacon_loader(&bindings, |_, _| Err(
                TairaRuntimeSignerErrorV1::DescriptorUnavailable
            )),
            Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
        ));
        let mut wrong = config;
        wrong
            .soracloud_runtime
            .submission
            .signer
            .as_mut()
            .expect("configured Soracloud")
            .policy_digest[0] ^= 1;
        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&wrong)
            .expect("qualified substituted Soracloud binding");
        assert!(matches!(
            registry.resolve_with_beacon_loader(&bindings, |_, _| panic!(
                "Soracloud mismatch rejected before FD200"
            )),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));
    }
}
