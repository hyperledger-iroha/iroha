//! Fixed-descriptor runtime signer used by the first-release Taira launcher.
//!
//! Taira keeps each validator's Soracloud runtime key in its deployment
//! supervisor.  The supervisor opens the owner-only regular file and passes it
//! to the daemon as inherited descriptor 198.  No path, key, or alternate
//! descriptor is accepted through arguments, configuration, or environment.

use crate::{
    IrohaRuntimeDeps, IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderRegistryErrorV1,
    IrohaRuntimeProviderRegistryV1, IrohaRuntimeProviderSlotV1,
    soracloud_runtime_signer::{
        SoracloudRuntimeMutationSignerV1, SoracloudRuntimeSignerProbeErrorV1,
        SoracloudRuntimeSignerQualificationV1, SoracloudRuntimeSigningErrorV1,
    },
};
use iroha_config::parameters::{
    actual::{NexusStorageWeights, Root as Config, SoracloudRuntime},
    defaults::soracloud_runtime as soracloud_runtime_defaults,
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    account::AccountId,
    soracloud::{
        SoracloudRuntimeProvenancePurposeV1, validate_soracloud_runtime_provenance_preimage_v1,
    },
    transaction::{SignedTransaction, TransactionBuilder, TransactionPayload},
};
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

fn invocation_does_not_start_a_node(argument: &OsStr) -> bool {
    matches!(
        argument.to_str(),
        Some("--check-config" | "--help" | "-h" | "--version" | "-V")
    )
}

/// Fixed inherited descriptor containing the Taira runtime signer key.
pub const TAIRA_RUNTIME_SIGNER_FD_V1: RawFd = 198;
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
/// Exact first-release Taira artifact-hydration worker count.
pub const TAIRA_HYDRATION_CONCURRENCY_V1: usize = 4;
/// Exact first-release Taira idle prepared-runtime cache capacity.
pub const TAIRA_PREPARED_RUNTIME_CACHE_CAPACITY_V1: usize = 4;
/// Exact aggregate Nexus disk budget for one first-release Taira validator.
pub const TAIRA_NEXUS_STORAGE_BUDGET_BYTES_V1: u64 = 68_719_476_736;
/// Exact Kura share of the first-release Taira Nexus disk budget.
pub const TAIRA_NEXUS_KURA_BLOCKS_BPS_V1: u16 = 5_500;
/// Exact WSV snapshot share of the first-release Taira Nexus disk budget.
pub const TAIRA_NEXUS_WSV_SNAPSHOTS_BPS_V1: u16 = 2_000;
/// Exact `SoraFS` share of the first-release Taira Nexus disk budget.
pub const TAIRA_NEXUS_SORAFS_BPS_V1: u16 = 2_000;
/// Exact `SoraNet` spool share of the first-release Taira Nexus disk budget.
pub const TAIRA_NEXUS_SORANET_SPOOL_BPS_V1: u16 = 250;
/// Exact `SoraVPN` spool share of the first-release Taira Nexus disk budget.
pub const TAIRA_NEXUS_SORAVPN_SPOOL_BPS_V1: u16 = 250;
/// Exact effective `SoraFS` component cap derived for first-release Taira.
pub const TAIRA_SORAFS_STORAGE_CAP_BYTES_V1: u64 = 13_743_895_347;
/// Exact aggregate Inrou CPU ceiling for one first-release Taira validator.
pub const TAIRA_INROU_MAX_CPU_MILLIS_V1: u32 = 8_000;
/// Exact aggregate Inrou memory ceiling for one first-release Taira validator.
pub const TAIRA_INROU_MAX_MEMORY_BYTES_V1: u64 = 8 * 1024 * 1024 * 1024;
/// Exact aggregate Inrou writable-storage ceiling for one first-release Taira validator.
pub const TAIRA_INROU_MAX_STORAGE_BYTES_V1: u64 = 64 * 1024 * 1024 * 1024;
/// Exact immutable Inrou guest-image ceiling for one first-release Taira validator.
pub const TAIRA_INROU_GUEST_IMAGE_MAX_BYTES_V1: u64 = 10 * 1024 * 1024 * 1024;
/// Exact Inrou startup grace for one first-release Taira validator.
pub const TAIRA_INROU_START_GRACE_MS_V1: u64 = 30_000;
/// Exact Inrou shutdown grace for one first-release Taira validator.
pub const TAIRA_INROU_STOP_GRACE_MS_V1: u64 = 10_000;
/// Exact Inrou egress request budget for one first-release Taira validator.
pub const TAIRA_INROU_EGRESS_RATE_PER_MINUTE_V1: u32 = 600;
/// Exact Inrou egress byte budget for one first-release Taira validator.
pub const TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE_V1: u64 = 100 * 1024 * 1024;

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
    if runtime.hydration_concurrency.get() != TAIRA_HYDRATION_CONCURRENCY_V1
        || runtime.prepared_runtime_cache_capacity.get() != TAIRA_PREPARED_RUNTIME_CACHE_CAPACITY_V1
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
        || weights.soranet_spool_bps != TAIRA_NEXUS_SORANET_SPOOL_BPS_V1
        || weights.soravpn_spool_bps != TAIRA_NEXUS_SORAVPN_SPOOL_BPS_V1
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
    /// Descriptor 198 is absent or unreadable.
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

fn load_key_pair_from_file(mut file: File) -> Result<KeyPair, TairaRuntimeSignerErrorV1> {
    let before_metadata = file
        .metadata()
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    let before = DescriptorIdentityV1::from_metadata(&before_metadata);
    let effective_uid = rustix::process::geteuid().as_raw();
    if !before_metadata.is_file()
        || before.owner != effective_uid
        || before.mode & 0o7777 != 0o600
        || before.links != 1
        || before.length != TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1
    {
        return Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor);
    }

    let capacity = usize::try_from(TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1)
        .expect("fixed Taira key length fits usize");
    let mut bytes = Vec::with_capacity(capacity + 1);
    std::io::Read::by_ref(&mut file)
        .take(TAIRA_RUNTIME_SIGNER_KEY_FILE_BYTES_V1 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    let after = DescriptorIdentityV1::from_metadata(
        &file
            .metadata()
            .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?,
    );
    if after != before || bytes.len() != capacity {
        bytes.fill(0);
        return Err(TairaRuntimeSignerErrorV1::UntrustedDescriptor);
    }

    let parsed = (|| {
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
    })();
    bytes.fill(0);
    consume_trusted_key_file(&mut file, &before, &bytes)?;
    parsed
}

#[allow(
    unsafe_code,
    reason = "the Taira launcher contract transfers unique ownership of inherited FD 198"
)]
fn load_inherited_key_pair() -> Result<KeyPair, TairaRuntimeSignerErrorV1> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let descriptor_path = format!("/proc/self/fd/{TAIRA_RUNTIME_SIGNER_FD_V1}");
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let descriptor_path = format!("/dev/fd/{TAIRA_RUNTIME_SIGNER_FD_V1}");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(descriptor_path)
        .map_err(|_| TairaRuntimeSignerErrorV1::DescriptorUnavailable)?;
    // SAFETY: the deployment supervisor transfers FD 198 to this launcher as
    // its only owner. Opening its kernel descriptor path above proves that it
    // is live before ownership is constructed. Dropping this value closes the
    // inherited descriptor; the owned duplicate is consumed before startup.
    let inherited = unsafe { File::from_raw_fd(TAIRA_RUNTIME_SIGNER_FD_V1) };
    drop(inherited);
    load_key_pair_from_file(file)
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

impl IrohaRuntimeProviderRegistryV1 for TairaRuntimeProviderRegistryV1 {
    fn resolve(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
        if bindings.len() != 1 {
            return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution);
        }
        let requested = bindings
            .iter()
            .next()
            .expect("one requested Taira runtime provider");
        if requested.slot() != IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner {
            return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution);
        }
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
        Ok(IrohaRuntimeDeps::default().with_soracloud_runtime_mutation_signer(signer))
    }
}

/// Run the Taira daemon with the exact signer inherited at descriptor 198.
///
/// Config validation, help, and version introspection remain offline and do not
/// read the descriptor. Every node-starting invocation resolves the one
/// configured signer through [`crate::run_with_runtime_provider_registry`].
pub fn main_entry() {
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
    ) {
        eprintln!("{report:?}");
        std::process::exit(1);
    }
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
        runtime.hydration_concurrency = NonZeroUsize::new(TAIRA_HYDRATION_CONCURRENCY_V1)
            .expect("nonzero Taira hydration-worker count");
        runtime.prepared_runtime_cache_capacity =
            NonZeroUsize::new(TAIRA_PREPARED_RUNTIME_CACHE_CAPACITY_V1)
                .expect("nonzero Taira prepared-runtime capacity");
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
        changed.hydration_concurrency = NonZeroUsize::new(TAIRA_HYDRATION_CONCURRENCY_V1 + 1)
            .expect("changed hydration-worker count is nonzero");
        assert_rejected(&changed);

        let mut changed = runtime.clone();
        changed.prepared_runtime_cache_capacity =
            NonZeroUsize::new(TAIRA_PREPARED_RUNTIME_CACHE_CAPACITY_V1 + 1)
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
            soranet_spool_bps: TAIRA_NEXUS_SORANET_SPOOL_BPS_V1,
            soravpn_spool_bps: TAIRA_NEXUS_SORAVPN_SPOOL_BPS_V1,
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
            KeyPair::try_from_seed(vec![0x33; 32], Algorithm::Ed25519).expect("Ed25519 key pair");
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
}
