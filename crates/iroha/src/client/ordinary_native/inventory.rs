//! Shared first-release signed public-original inventory and actual descriptor custody.
//! The independently installed runtime authority is a mandatory Native owner input.

use super::*;
use iroha_core_zk::{
    kagemusha_v1_recursion::{KagemushaArtifactByteResolverV1, KagemushaArtifactErrorV1},
    kagemusha_v1_state::{
        KagemushaOrdinaryGovernedPolicyOriginalsV1, KagemushaOrdinaryNativeClockNodeV1,
        KagemushaOrdinaryNativeClockOriginalsV1, KagemushaOrdinaryNativeClockPolicyV1,
        KagemushaOrdinaryPreparationSelectedOriginalsV1,
    },
};
use iroha_data_model::{
    kagemusha::*,
    sumeragi_finality::{SumeragiFinalityCheckpoint, SumeragiFinalityVerifier},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    os::unix::fs::{FileExt as _, MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
};

const DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-native-installed-inventory\0";
const MAGIC: &[u8; 8] = b"KGMINV01";
const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_ORIGINAL: usize = 16 * 1024 * 1024;
const MAX_FILES: usize = 128;
const MAX_TOTAL: u64 = 16 * 1024 * 1024 * 1024;

// Structural DATA precheck shared by the existing FI and lineage transports. Only the
// independently installed issuer original chooses this lane preimage. This constructs no
// checked policy, Native session, live clock/FI loan or financial owner.
fn require_installed_owner_lane_data(
    issuer: &KagemushaOrdinaryEnrollmentIssuerPolicyV1,
    runtime: &KagemushaRetailEnrollmentRuntimeV1,
    owner: &KagemushaRetailEnrollmentOwnerV1,
) -> Result<()> {
    let lane = issuer
        .derive_enrollment_lane(&runtime.fi_id, &owner.account_id)
        .map_err(|_| eyre!("Native installed ordinary lane DATA rejected"))?;
    ensure!(
        issuer.network_id == *runtime.network_id.as_bytes()
            && owner.runtime == *runtime
            && owner.lane_id == lane,
        "Native request changed installed issuer/runtime/wallet lane"
    );
    Ok(())
}

/// One exact public original relative to the installed package directory; never a key file.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::KagemushaOrdinaryNativeOriginalDescriptorV1")]
pub struct KagemushaOrdinaryNativeOriginalDescriptorV1 {
    /// Bounded slash-separated relative path with no dot segments or symbolic links.
    pub path: String,
    /// SHA-256 of the complete exact file.
    pub sha256: [u8; 32],
    /// Complete file size, checked against the purpose's independent upper bound.
    pub byte_len: u64,
}

/// Four exact validator transport targets corresponding to the same ordered signed node pins.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::KagemushaOrdinaryNativeNodeTargetV1")]
pub struct KagemushaOrdinaryNativeNodeTargetV1 {
    /// Independently authorized current BLS/build/config identity.
    pub node: KagemushaOrdinaryNativeClockNodeV1,
    /// Explicit canonical HTTPS origin with no credentials, query, fragment or base path.
    pub endpoint: String,
}

/// Public original inventory, signed only by the already installed runtime authority.
/// Decoding this value neither admits a Native installation nor supplies current FI status.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::KagemushaOrdinaryNativeInventoryV1")]
pub struct KagemushaOrdinaryNativeInventoryV1 {
    /// Exactly one; there is no old-format fallback.
    pub version: u16,
    /// Complete independently authenticated online runtime manifest SHA-256.
    pub runtime_manifest_sha256: [u8; 32],
    /// Actual independently installed common Native SDK release identity.
    pub sdk_release_sha256: [u8; 32],
    /// Native owner-selected minimum/replay sequence; a downloaded package cannot select it.
    pub sequence: u64,
    /// Exact threshold-authenticated ordinary release identifier.
    pub release_id: [u8; 32],
    /// Exact enabled ordinary profile, distinct from OEM financial hardware profiles.
    pub profile_id: [u8; 32],
    /// Existing native Core public P256 authorization point, selected before any C/E/FI.
    pub core_public_key: KagemushaDevicePublicKeyV1,
    /// Independent actual compiled complete-World schema identity.
    pub world_schema_hash: Hash,
    /// Existing complete canonical checkpoint public original descriptor.
    pub checkpoint: KagemushaOrdinaryNativeOriginalDescriptorV1,
    /// Four ordered nodes/endpoints, never selected by an offered signed response.
    pub nodes: [KagemushaOrdinaryNativeNodeTargetV1; 4],
    /// Exact independently governed software-clock bounds.
    pub clock_policy: KagemushaOrdinaryNativeClockPolicyV1,
    /// Actual FI public native-control endpoint; this pin cannot assert non-revocation.
    pub fi_current_control_endpoint: String,
    /// Existing FI policy ID against which live native control must be joined.
    pub fi_issuer_policy_digest: [u8; 32],
    /// Mandatory independently signed ordinary threshold policy identity.
    pub ordinary_identity_policy_id: [u8; 32],
    /// Mandatory complete Core issuer policy digest, distinct from the FI retail policy.
    pub ordinary_core_issuer_policy_digest: [u8; 32],
    /// Mandatory independently installed account/FI lane namespace.
    pub ordinary_lane_namespace: [u8; 32],
    /// Exact integrity provider policy file when the selected ordinary trust requires PI.
    pub integrity_policy: Option<KagemushaOrdinaryNativeOriginalDescriptorV1>,
    /// Fixed complete set of public release/issuer/trust originals and release-bound artifact files.
    pub originals: Vec<KagemushaOrdinaryNativeOriginalDescriptorV1>,
}

/// Real retained public authority installed independently by the product's Native release owner.
/// There is no decoder, C/JNI constructor, reply-pin setter or public-key getter.
/// The expected digests/sequence must come from the existing app release admission, not the
/// downloaded inventory or managed settings. This explicit Rust boundary remains mandatory.
pub struct KagemushaNativeInstalledRuntimeAuthorityV1 {
    key_file: HeldFile,
    public_key: iroha_crypto::PublicKey,
    runtime_manifest_sha256: [u8; 32],
    sdk_release_sha256: [u8; 32],
    minimum_sequence: u64,
}
impl KagemushaNativeInstalledRuntimeAuthorityV1 {
    /// Retain the independently selected installed raw Ed32 authority and immutable release pins.
    /// This function checks custody/crypto relationships; it does not independently approve the
    /// supplied native owner pins. Shipping startup must obtain them from app release admission.
    /// # Errors
    /// Refuses an unsafe/moved/changed authority original or zero SDK/runtime/replay selection.
    pub fn from_independently_installed_original(
        key_path: &Path,
        expected_key_file_sha256: [u8; 32],
        runtime_manifest_sha256: [u8; 32],
        sdk_release_sha256: [u8; 32],
        minimum_sequence: u64,
    ) -> Result<Self> {
        ensure!(
            runtime_manifest_sha256 != [0; 32]
                && sdk_release_sha256 != [0; 32]
                && minimum_sequence != 0,
            "installed Native release selection rejected"
        );
        let key_file = HeldFile::open_exact(key_path.to_owned(), expected_key_file_sha256, 32, 32)?;
        let original = key_file.bytes(32)?;
        ensure!(
            original.iter().any(|byte| *byte != 0),
            "installed Native authority rejected"
        );
        let public_key =
            iroha_crypto::PublicKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &original)?;
        let this = Self {
            key_file,
            public_key,
            runtime_manifest_sha256,
            sdk_release_sha256,
            minimum_sequence,
        };
        this.recheck()?;
        Ok(this)
    }
    fn recheck(&self) -> Result<()> {
        self.key_file.bytes(32).map(|_| ())
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    links: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl Identity {
    fn of(value: &std::fs::Metadata) -> Self {
        Self {
            dev: value.dev(),
            ino: value.ino(),
            uid: value.uid(),
            gid: value.gid(),
            mode: value.mode(),
            links: value.nlink(),
            size: value.len(),
            modified: (value.mtime(), value.mtime_nsec()),
            changed: (value.ctime(), value.ctime_nsec()),
        }
    }
}
struct HeldFile {
    path: PathBuf,
    file: File,
    identity: Identity,
    sha256: [u8; 32],
    process: u32,
}
impl HeldFile {
    fn open_exact(path: PathBuf, sha256: [u8; 32], length: u64, maximum: u64) -> Result<Self> {
        ensure!(
            sha256 != [0; 32] && length > 0 && length <= maximum,
            "Native original bound rejected"
        );
        require_path(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32,
            )
            .open(&path)?;
        let meta = file.metadata()?;
        ensure!(
            meta.is_file()
                && meta.nlink() == 1
                && meta.len() == length
                && meta.mode() & 0o022 == 0
                && (meta.uid() == rustix::process::geteuid().as_raw() || meta.uid() == 0),
            "Native original custody rejected"
        );
        let this = Self {
            path,
            file,
            identity: Identity::of(&meta),
            sha256,
            process: std::process::id(),
        };
        this.stream_check()?;
        Ok(this)
    }
    fn metadata_stable(&self) -> Result<()> {
        ensure!(
            self.process == std::process::id(),
            "Native original process changed"
        );
        require_path(&self.path)?;
        let held = self.file.metadata()?;
        let named = std::fs::symlink_metadata(&self.path)?;
        ensure!(
            held.is_file()
                && named.is_file()
                && !named.file_type().is_symlink()
                && Identity::of(&held) == self.identity
                && Identity::of(&named) == self.identity,
            "Native original identity changed"
        );
        Ok(())
    }
    fn stream_check(&self) -> Result<()> {
        self.metadata_stable()?;
        let mut offset = 0_u64;
        let mut hash = sha2::Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        while offset < self.identity.size {
            let remaining =
                usize::try_from((self.identity.size - offset).min(buffer.len() as u64))?;
            let count = self.file.read_at(&mut buffer[..remaining], offset)?;
            ensure!(count != 0, "Native original truncated");
            hash.update(&buffer[..count]);
            offset = offset
                .checked_add(count as u64)
                .ok_or_else(|| eyre!("Native original overflow"))?;
        }
        ensure!(
            <[u8; 32]>::from(hash.finalize()) == self.sha256,
            "Native original digest changed"
        );
        self.metadata_stable()
    }
    fn bytes(&self, maximum: usize) -> Result<Vec<u8>> {
        self.metadata_stable()?;
        ensure!(
            self.identity.size <= maximum as u64,
            "Native original size rejected"
        );
        let mut bytes = vec![0; usize::try_from(self.identity.size)?];
        self.file.read_exact_at(&mut bytes, 0)?;
        ensure!(
            <[u8; 32]>::from(sha2::Sha256::digest(&bytes)) == self.sha256,
            "Native original digest changed"
        );
        self.metadata_stable()?;
        Ok(bytes)
    }
}
fn require_path(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute() && path.canonicalize()? == path,
        "Native original canonical path rejected"
    );
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        ensure!(
            !std::fs::symlink_metadata(&prefix)?.file_type().is_symlink(),
            "Native original symbolic path rejected"
        );
    }
    Ok(())
}
fn relative_path(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 512
            && value.is_ascii()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte))
            && value
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
            && value.split('/').count() <= 8,
        "Native inventory relative path rejected"
    );
    Ok(())
}

/// Actual signed inventory and held public original descriptors; data alone cannot construct it.
/// It neither creates FI current status nor a financial/money owner.
pub struct KagemushaAdmittedOrdinaryNativeInventoryV1 {
    authority: Arc<KagemushaNativeInstalledRuntimeAuthorityV1>,
    package: HeldFile,
    root: PathBuf,
    root_file: File,
    root_identity: Identity,
    body: KagemushaOrdinaryNativeInventoryV1,
    files: BTreeMap<String, HeldFile>,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    issuer: KagemushaRetailEnrollmentIssuerPolicyV1,
    lineage_issuer: KagemushaOrdinaryLineageIssuerPolicyV1,
    identity_data: OrdinaryIdentityInstalledDataV1,
    checkpoint: SumeragiFinalityCheckpoint,
}
impl KagemushaAdmittedOrdinaryNativeInventoryV1 {
    /// Authenticate the exact purpose-bound signature before interpreting its bounded inventory,
    /// then retain every named descriptor and threshold-authenticate the actual release originals.
    /// App code supplies the package location/digest as correlation data; it cannot select root.
    /// # Errors
    /// Rejects another installed root/SDK/runtime/sequence, unsafe descriptors or substituted originals.
    pub fn intake(
        authority: Arc<KagemushaNativeInstalledRuntimeAuthorityV1>,
        package_path: &Path,
        package_sha256: [u8; 32],
        root: &Path,
    ) -> Result<Self> {
        authority.recheck()?;
        require_path(root)?;
        let root_file = OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32,
            )
            .open(root)?;
        let root_meta = root_file.metadata()?;
        ensure!(
            root_meta.is_dir() && root_meta.mode() & 0o022 == 0,
            "Native inventory root custody rejected"
        );
        let length = std::fs::symlink_metadata(package_path)?.len();
        let package = HeldFile::open_exact(
            package_path.to_owned(),
            package_sha256,
            length,
            u64::try_from(MAX_BODY + 140)?,
        )?;
        let bytes = package.bytes(MAX_BODY + 140)?;
        let payload = verify_packet(&bytes, &authority.public_key)?;
        let body: KagemushaOrdinaryNativeInventoryV1 = canonical(payload)?;
        ensure!(
            bytes[12..44] == body.runtime_manifest_sha256
                && bytes[44..76] == body.sdk_release_sha256,
            "Native inventory signed header differs from original body"
        );
        ensure!(
            body.runtime_manifest_sha256 == authority.runtime_manifest_sha256
                && body.sdk_release_sha256 == authority.sdk_release_sha256
                && body.sequence >= authority.minimum_sequence,
            "Native inventory installed selection changed"
        );
        let (files, release, issuer, checkpoint, lineage_issuer) = admit_files(root, &body)?;
        let identity_data = OrdinaryIdentityInstalledDataV1::read(&body, &files)?;
        let this = Self {
            authority,
            package,
            root: root.to_owned(),
            root_file,
            root_identity: Identity::of(&root_meta),
            body,
            files,
            release,
            issuer,
            lineage_issuer,
            identity_data,
            checkpoint,
        };
        this.recheck()?;
        Ok(this)
    }
    /// Recheck actual installed authority, signed inventory and every public-original descriptor.
    /// # Errors
    /// Rejects descriptor/path/process drift, changed signature source or static model originals.
    pub fn recheck(&self) -> Result<()> {
        self.authority.recheck()?;
        self.package.stream_check()?;
        require_path(&self.root)?;
        let held = self.root_file.metadata()?;
        let named = std::fs::symlink_metadata(&self.root)?;
        ensure!(
            held.is_dir()
                && named.is_dir()
                && Identity::of(&held) == self.root_identity
                && Identity::of(&named) == self.root_identity,
            "Native inventory root changed"
        );
        for (name, file) in &self.files {
            if name.starts_with("artifacts/") {
                file.metadata_stable()?;
            } else {
                file.stream_check()?;
            }
        }
        self.lineage_issuer
            .validate_for_issuer(&self.issuer)
            .map_err(|_| eyre!("Native admitted CAS purpose changed"))?;
        self.issuer
            .validate()
            .map_err(|_| eyre!("Native issuer original rejected"))?;
        Ok(())
    }
    /// Borrow public CAS purpose data only from the independently signed inventory and its held
    /// exact descriptor. This does not admit a receipt, current DATA observation or monetary loan.
    /// # Errors
    /// Refuses changed installation, descriptor, issuer/key/runtime or disabled purpose.
    pub fn lineage_policy_original(&self) -> Result<Vec<u8>> {
        self.recheck()?;
        self.files["originals/ordinary-lineage-cas-policy.norito"].bytes(32 * 1024)
    }
    /// Same independently installed exact CAS purpose; raw decoding cannot create this inventory.
    /// The separate global receipt and current financial owner remain mandatory.
    /// # Errors
    /// Refuses changed installed policy custody.
    pub fn lineage_issuer_policy(&self) -> Result<&KagemushaOrdinaryLineageIssuerPolicyV1> {
        self.recheck()?;
        Ok(&self.lineage_issuer)
    }
    /// Build the actual independently installed clock selection from authenticated public originals.
    /// This does not answer a fresh clock read; Native transport must do so before startup.
    /// # Errors
    /// Rejects changed installation or another checkpoint/node/policy/network.
    pub fn clock_originals(&self) -> Result<Arc<KagemushaOrdinaryNativeClockOriginalsV1>> {
        self.recheck()?;
        Ok(Arc::new(
            KagemushaOrdinaryNativeClockOriginalsV1::from_selected_originals(
                self.checkpoint.clone(),
                self.issuer.runtime.network_id,
                self.checkpoint.chain_id().into(),
                std::array::from_fn(|index| self.body.nodes[index].node.clone()),
                self.body.clock_policy,
            )
            .map_err(|_| eyre!("Native installed clock originals rejected"))?,
        ))
    }
    /// Export the same complete public clock selection from this actual independently signed
    /// installed inventory. A Core release assembler may admit this exact file under its own
    /// existing signed runtime manifest; a decoded export itself grants no installed clock root.
    /// # Errors
    /// Refuses changed inventory/descriptors or canonical selection framing failure.
    pub fn clock_selection_original(&self) -> Result<Vec<u8>> {
        self.recheck()?;
        let raw = self
            .clock_originals()?
            .canonical_selection_original()
            .map_err(|_| eyre!("Native installed clock selection rejected"))?;
        self.recheck()?;
        Ok(raw)
    }
    /// Join exact certified S/W and real Native AccountClient to the installed ordinary policy,
    /// actual clock owner and caller-independent Core point; no selected DTO grants this custody.
    /// # Errors
    /// Rejects foreign key/account/membership/root/schema/clock/runtime or expired current evidence.
    pub fn select_account(
        self: &Arc<Self>,
        account: AccountClient,
        current: VerifiedEnrollmentWalletSignatoryV1,
        clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
    ) -> Result<(
        Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        KagemushaNativeAccountCustodyV1,
    )> {
        self.recheck()?;
        let verifier = clock
            .lock()
            .map_err(|_| eyre!("Native clock owner unavailable"))?
            .current_finality_verifier()
            .map_err(|_| eyre!("Native current finality custody rejected"))?;
        let nodes = std::array::from_fn(|index| {
            crate::participant_enrollment_request::SelectedEnrollmentReadNodeV1 {
                peer_id: self.body.nodes[index].node.peer_id.clone(),
                build_fingerprint: self.body.nodes[index].node.build_fingerprint,
                config_fingerprint: self.body.nodes[index].node.config_fingerprint,
            }
        });
        current.recheck_under_installed_finality(
            &verifier,
            &nodes,
            self.body.world_schema_hash,
            self.issuer.runtime.network_id,
        )?;
        let custody = KagemushaNativeAccountCustodyV1::from_current_wallet(account, current)?;
        let governed = KagemushaOrdinaryGovernedPolicyOriginalsV1::authenticate(
            self.release.clone(),
            self.body.profile_id,
            &self.files["originals/ordinary-trust.norito"].bytes(MAX_ORIGINAL)?,
            &self.files["originals/app-authority.bin"].bytes(MAX_ORIGINAL)?,
        )
        .map_err(|_| eyre!("Native governed ordinary originals rejected"))?;
        let expected = self.clock_originals()?;
        let temporary_digest = {
            let actual = clock
                .lock()
                .map_err(|_| eyre!("Native clock owner unavailable"))?;
            actual
                .installed_selection_digest()
                .map_err(|_| eyre!("Native clock custody rejected"))?
        };
        // The independently authenticated selection is reconstructed under a separate scratch
        // owner-free shape value; compare its public digest directly, without creating another WAL.
        ensure!(
            temporary_digest == expected.selection_digest(),
            "Native clock differs from installed original selection"
        );
        // Actual held signed-clock interval supplies both threshold-policy endpoints.
        // Intake above retained DATA only; no downloaded timestamp supplies this admission.
        let interval = clock
            .lock()
            .map_err(|_| eyre!("Native clock owner unavailable"))?
            .current_native_time_interval()
            .map_err(|_| eyre!("Native current ordinary policy interval unavailable"))?;
        let ordinary = Arc::new(self.identity_data.authenticate(
            &self.body,
            &self.release,
            &self.issuer,
            interval.lower_ms(),
        )?);
        ordinary
            .recheck_current(&self.release, interval.upper_ms())
            .map_err(|_| eyre!("Native ordinary threshold interval rejected"))?;
        let lane = ordinary
            .enrollment_lane(&self.release, custody.wallet(), interval.lower_ms())
            .map_err(|_| eyre!("Native ordinary account/FI lane rejected"))?;
        let owner = KagemushaRetailEnrollmentOwnerV1 {
            account_id: custody.wallet().clone(),
            runtime: self.issuer.runtime.clone(),
            lane_id: lane,
        };
        let selected=Arc::new(KagemushaOrdinaryPreparationSelectedOriginalsV1::from_governed_originals_with_native_clock(
            owner,governed,self.issuer.clone(),ordinary,&self.body.core_public_key,clock,temporary_digest,self.body.world_schema_hash,
        ).map_err(|_|eyre!("Native ordinary account selection rejected"))?);
        custody.recheck()?;
        self.recheck()?;
        Ok((selected, custody))
    }
    pub(super) fn require_current_control_request(
        &self,
        request: &iroha_data_model::kagemusha::KagemushaOrdinaryCurrentControlRequestV1,
    ) -> Result<()> {
        self.recheck()?;
        request
            .validate_shape()
            .map_err(|_| eyre!("Native current FI request shape rejected"))?;
        // Structural DATA join only. Actual Current FI admission separately retains its
        // genuine clock, short challenge and mandatory installed Native financial owner.
        require_installed_owner_lane_data(
            &self.identity_data.issuer,
            &self.issuer.runtime,
            &request.owner,
        )?;
        ensure!(
            request.issuer_policy_digest == self.body.fi_issuer_policy_digest,
            "Native current FI request changed installed issuer/runtime/wallet lane"
        );
        self.recheck()
    }
    pub(super) fn require_lineage_request(
        &self,
        request: &iroha_data_model::kagemusha::KagemushaOrdinaryLineageRequestV1,
    ) -> Result<()> {
        self.recheck()?;
        request
            .canonical_bytes()
            .map_err(|_| eyre!("Native lineage request shape rejected"))?;
        let owner = &request.operation.lineage().owner;
        require_installed_owner_lane_data(&self.identity_data.issuer, &self.issuer.runtime, owner)?;
        ensure!(
            request.issuer_policy_digest == self.lineage_issuer.issuer_policy_digest,
            "Native lineage request changed installed issuer/runtime/wallet lane"
        );
        self.recheck()
    }
    pub(super) fn membership_nodes(
        &self,
    ) -> [crate::participant_enrollment_request::SelectedEnrollmentReadNodeV1; 4] {
        std::array::from_fn(|index| {
            crate::participant_enrollment_request::SelectedEnrollmentReadNodeV1 {
                peer_id: self.body.nodes[index].node.peer_id.clone(),
                build_fingerprint: self.body.nodes[index].node.build_fingerprint,
                config_fingerprint: self.body.nodes[index].node.config_fingerprint,
            }
        })
    }
    pub(super) fn world_schema_hash(&self) -> Hash {
        self.body.world_schema_hash
    }
    pub(super) fn require_account_transport(&self, account: &AccountClient) -> Result<()> {
        self.recheck()?;
        ensure!(
            account.network_id() == &self.issuer.runtime.network_id
                && account.context.chain.to_string() == self.checkpoint.chain_id()
                && self
                    .body
                    .nodes
                    .iter()
                    .any(|node| Url::parse(&node.endpoint)
                        .is_ok_and(|url| &url == account.endpoint()))
                && account.signing_capability() == AccountSigningCapability::MultisigMember
                && account.context.key_pair.public_key().algorithm()
                    == iroha_crypto::Algorithm::Ed25519,
            "Native account context differs from installed network/transport/key"
        );
        Ok(())
    }
    /// Construct four actual shared HTTP contexts from the held account context and installed targets.
    /// No caller transport callback or copied mobile node pins enter this path.
    /// # Errors
    /// Refuses foreign account/transport/network/chain or changed original endpoint descriptors.
    pub fn clock_transport(
        &self,
        account: &AccountClient,
        clock: &Mutex<KagemushaOrdinaryNativeClockOwnerV1>,
    ) -> Result<KagemushaNativeClockTransportV1> {
        self.require_account_transport(account)?;
        let mut clients = Vec::with_capacity(4);
        for target in &self.body.nodes {
            let mut builder = account.context.to_builder();
            builder.torii_url = Url::parse(&target.endpoint)?;
            // These are public finality reads. The independently retained account key stays in
            // Native, and no original endpoint's bearer/default headers are forwarded elsewhere.
            builder.headers.clear();
            builder.operator_key_pair = None;
            clients.push(builder.build()?);
        }
        KagemushaNativeClockTransportV1::from_native_clients(
            clients
                .try_into()
                .map_err(|_| eyre!("Native node count rejected"))?,
            clock,
        )
    }

    /// Load the real shared recursive verifier using every exact descriptor-backed release key.
    /// This authenticates proof material only; it creates no current State or money capability.
    /// # Errors
    /// Rejects unavailable/substituted keys, incompatible released protocols or changed originals.
    pub fn load_recursive_verifier(
        self: &Arc<Self>,
        profile: iroha_core_zk::kagemusha_v1_recursion::KagemushaRecursiveVerifierProfileV1,
    ) -> Result<Arc<iroha_core_zk::kagemusha_v1_recursion::KagemushaAuthenticatedRecursiveVerifierV1>>
    {
        use iroha_core_zk::kagemusha_v1_recursion::{
            KagemushaAuthenticatedArtifactSetV1, KagemushaAuthenticatedRecursiveVerifierV1,
        };
        self.recheck()?;
        let artifacts = KagemushaAuthenticatedArtifactSetV1::new_canonical(
            &self.release,
            KagemushaOrdinaryNativeArtifactResolverV1(self.clone()),
        )?;
        let mut verifier = KagemushaAuthenticatedRecursiveVerifierV1::load(&artifacts, profile)?;
        verifier
            .authorize_ordinary_monetary_release(self.release.clone())
            .map_err(|_| eyre!("Native ordinary artifact family authorization rejected"))?;
        self.recheck()?;
        Ok(Arc::new(verifier))
    }

    /// Complete original optional PI policy for the actual Native source. This is policy data only.
    /// # Errors
    /// Rejects changed installed policy descriptor.
    pub fn integrity_policy_original(&self) -> Result<Option<Vec<u8>>> {
        self.recheck()?;
        self.body
            .integrity_policy
            .as_ref()
            .map(|entry| self.files[&entry.path].bytes(MAX_ORIGINAL))
            .transpose()
    }
    /// Exact current-control endpoint pin; reading it neither asserts live KYC nor non-revocation.
    #[must_use]
    pub fn fi_current_control_endpoint(&self) -> &str {
        &self.body.fi_current_control_endpoint
    }
}

/// Descriptor-backed resolver which keeps the actual admitted shared inventory alive.
/// Release consumers independently check the exact role/length/hash before decoding proof keys.
#[derive(Clone)]
pub struct KagemushaOrdinaryNativeArtifactResolverV1(
    pub Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
);
impl KagemushaArtifactByteResolverV1 for KagemushaOrdinaryNativeArtifactResolverV1 {
    fn resolve_bytes(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> std::result::Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        let get = || -> Result<Arc<[u8]>> {
            self.0.recheck()?;
            let file = self
                .0
                .files
                .get(&format!("artifacts/{}", hex::encode(binding.sha256)))
                .ok_or_else(|| eyre!("Native artifact absent"))?;
            ensure!(
                file.sha256 == binding.sha256 && file.identity.size == binding.byte_len,
                "Native artifact binding changed"
            );
            Ok(file.bytes(usize::try_from(binding.byte_len)?)?.into())
        };
        get().map_err(|_| KagemushaArtifactErrorV1::Missing(binding.role))
    }
}

fn canonical<T>(raw: &[u8]) -> Result<T>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    ensure!(
        !raw.is_empty() && raw.len() <= MAX_ORIGINAL,
        "Native original frame rejected"
    );
    let value: T =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))?;
    ensure!(
        norito::encode_canonical(&value)? == raw,
        "Native original canonical bytes changed"
    );
    Ok(value)
}
fn verify_packet<'a>(bytes: &'a [u8], key: &iroha_crypto::PublicKey) -> Result<&'a [u8]> {
    ensure!(
        bytes.len() >= 140 && bytes.len() <= MAX_BODY + 140 && &bytes[..8] == MAGIC,
        "Native inventory framing rejected"
    );
    let length = usize::try_from(u32::from_le_bytes(bytes[8..12].try_into()?))?;
    ensure!(
        length > 0 && length <= MAX_BODY && bytes.len() == 76 + length + 64,
        "Native inventory length rejected"
    );
    let mut message = Vec::with_capacity(DOMAIN.len() + 76 + length);
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(&bytes[..76 + length]);
    iroha_crypto::Signature::from_bytes(&bytes[76 + length..]).verify(key, &message)?;
    Ok(&bytes[76..76 + length])
}
// This object retains descriptor-authenticated public DATA. It grants no policy/current
// admission until select_account obtains the actual held Native signed-clock interval.
struct OrdinaryIdentityInstalledDataV1 {
    roots: KagemushaOrdinaryAppIdentityAuthorityPolicyV1,
    signed: KagemushaSignedOrdinaryAppIdentityPolicyV1,
    issuer: KagemushaOrdinaryEnrollmentIssuerPolicyV1,
}
impl OrdinaryIdentityInstalledDataV1 {
    fn read(
        body: &KagemushaOrdinaryNativeInventoryV1,
        files: &BTreeMap<String, HeldFile>,
    ) -> Result<Self> {
        let original = |name: &str| -> Result<Vec<u8>> {
            files
                .get(name)
                .ok_or_else(|| eyre!("Native mandatory ordinary policy original absent"))?
                .bytes(KAGEMUSHA_ORDINARY_APP_IDENTITY_POLICY_MAX_BYTES_V1)
        };
        let roots: KagemushaOrdinaryAppIdentityAuthorityPolicyV1 =
            canonical(&original("originals/ordinary-identity-authority.norito")?)?;
        roots
            .validate()
            .map_err(|_| eyre!("Native ordinary threshold roots DATA invalid"))?;
        let signed = KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(&original(
            "originals/ordinary-identity-policy.norito",
        )?)
        .map_err(|_| eyre!("Native ordinary threshold policy DATA invalid"))?;
        let issuer = KagemushaOrdinaryEnrollmentIssuerPolicyV1::decode_canonical_exact(&original(
            "originals/ordinary-core-issuer-policy.norito",
        )?)
        .map_err(|_| eyre!("Native ordinary Core issuer DATA invalid"))?;
        ensure!(
            roots.expected_identity_policy_id == body.ordinary_identity_policy_id
                && signed
                    .policy
                    .canonical_digest()
                    .map_err(|_| eyre!("Native ordinary policy DATA digest invalid"))?
                    == body.ordinary_identity_policy_id
                && issuer
                    .canonical_digest()
                    .map_err(|_| eyre!("Native ordinary issuer DATA digest invalid"))?
                    == body.ordinary_core_issuer_policy_digest
                && issuer.lane_namespace_id == body.ordinary_lane_namespace
                && signed.policy.enrollment_issuer_policy_digest
                    == body.ordinary_core_issuer_policy_digest
                && signed.policy.profile.planned_release_id == body.release_id
                && signed.policy.profile.planned_hardware_profile_id == body.profile_id,
            "Native independently signed ordinary policy DATA pins differ"
        );
        Ok(Self {
            roots,
            signed,
            issuer,
        })
    }
    fn authenticate(
        &self,
        body: &KagemushaOrdinaryNativeInventoryV1,
        release: &KagemushaAuthenticatedReleaseV1,
        retail: &KagemushaRetailEnrollmentIssuerPolicyV1,
        native_interval_lower_ms: u64,
    ) -> Result<KagemushaOrdinaryRetailIdentityPolicyOriginalsV1> {
        let policy = Arc::new(
            self.signed
                .authenticate(&self.roots, native_interval_lower_ms)
                .map_err(|_| eyre!("Native genuine threshold policy admission rejected"))?,
        );
        let issuer = Arc::new(
            self.issuer
                .authenticate_under_policy(
                    &policy,
                    body.ordinary_lane_namespace,
                    native_interval_lower_ms,
                )
                .map_err(|_| eyre!("Native genuine complete Core issuer admission rejected"))?,
        );
        KagemushaOrdinaryRetailIdentityPolicyOriginalsV1::authenticate(
            policy,
            issuer,
            body.ordinary_lane_namespace,
            retail.clone(),
            release,
            body.profile_id,
            native_interval_lower_ms,
        )
        .map_err(|_| eyre!("Native distinct Core/FI policy originals join rejected"))
    }
}
fn admit_files(
    root: &Path,
    body: &KagemushaOrdinaryNativeInventoryV1,
) -> Result<(
    BTreeMap<String, HeldFile>,
    Arc<KagemushaAuthenticatedReleaseV1>,
    KagemushaRetailEnrollmentIssuerPolicyV1,
    SumeragiFinalityCheckpoint,
    KagemushaOrdinaryLineageIssuerPolicyV1,
)> {
    ensure!(
        body.version == 1
            && body.sequence != 0
            && body.runtime_manifest_sha256 != [0; 32]
            && body.sdk_release_sha256 != [0; 32]
            && body.release_id != [0; 32]
            && body.profile_id != [0; 32]
            && body.ordinary_identity_policy_id != [0; 32]
            && body.ordinary_core_issuer_policy_digest != [0; 32]
            && body.ordinary_lane_namespace != [0; 32]
            && body.originals.len() >= 6
            && body.originals.len() <= MAX_FILES,
        "Native inventory shape rejected"
    );
    body.core_public_key
        .validate()
        .map_err(|_| eyre!("Native Core point rejected"))?;
    super::endpoint::require_https_directory_base(&body.fi_current_control_endpoint)?;
    for node in &body.nodes {
        super::endpoint::require_https_directory_base(&node.endpoint)?;
    }
    ensure!(
        body.checkpoint.path == "originals/finality-checkpoint.norito"
            && body
                .integrity_policy
                .as_ref()
                .is_none_or(|entry| entry.path == "originals/integrity-provider-policy.norito"),
        "Native inventory original role path rejected"
    );
    let mut files = BTreeMap::new();
    let mut total = 0_u64;
    let mut names = BTreeSet::new();
    for descriptor in body
        .originals
        .iter()
        .chain(std::iter::once(&body.checkpoint))
        .chain(body.integrity_policy.iter())
    {
        relative_path(&descriptor.path)?;
        let public_original = matches!(
            descriptor.path.as_str(),
            "originals/authority-policy.norito"
                | "originals/release-manifest.norito"
                | "originals/validation-receipt.norito"
                | "originals/release-attestation.norito"
                | "originals/issuer-policy.norito"
                | "originals/ordinary-identity-authority.norito"
                | "originals/ordinary-identity-policy.norito"
                | "originals/ordinary-core-issuer-policy.norito"
                | "originals/ordinary-lineage-cas-policy.norito"
                | "originals/ordinary-trust.norito"
                | "originals/app-authority.bin"
                | "originals/finality-checkpoint.norito"
                | "originals/integrity-provider-policy.norito"
        );
        let artifact = descriptor
            .path
            .strip_prefix("artifacts/")
            .is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            });
        ensure!(
            public_original || artifact,
            "Native inventory private or unknown original rejected"
        );
        let mode = std::fs::symlink_metadata(root.join(&descriptor.path))?.mode() & 0o777;
        ensure!(
            mode == 0o644 || mode == 0o444,
            "Native inventory public-original permissions rejected"
        );
        ensure!(
            names.insert(descriptor.path.clone()),
            "Native inventory duplicate original"
        );
        total = total
            .checked_add(descriptor.byte_len)
            .ok_or_else(|| eyre!("Native inventory overflow"))?;
        ensure!(total <= MAX_TOTAL, "Native inventory total rejected");
        let maximum = if descriptor.path.starts_with("artifacts/") {
            MAX_TOTAL
        } else {
            u64::try_from(MAX_ORIGINAL)?
        };
        files.insert(
            descriptor.path.clone(),
            HeldFile::open_exact(
                root.join(&descriptor.path),
                descriptor.sha256,
                descriptor.byte_len,
                maximum,
            )?,
        );
    }
    let raw = |name: &str| -> Result<Vec<u8>> {
        files
            .get(name)
            .ok_or_else(|| eyre!("Native required public original absent"))?
            .bytes(MAX_ORIGINAL)
    };
    let authority = KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&raw(
        "originals/authority-policy.norito",
    )?)
    .map_err(|_| eyre!("Native release authority rejected"))?;
    let manifest = KagemushaReleaseManifestV1::decode_canonical_exact(&raw(
        "originals/release-manifest.norito",
    )?)
    .map_err(|_| eyre!("Native release manifest rejected"))?;
    let receipt = KagemushaInternalValidationReceiptV1::decode_canonical_exact(&raw(
        "originals/validation-receipt.norito",
    )?)
    .map_err(|_| eyre!("Native release receipt rejected"))?;
    let attestation = KagemushaReleaseAttestationV1::decode_canonical_exact(&raw(
        "originals/release-attestation.norito",
    )?)
    .map_err(|_| eyre!("Native release attestation rejected"))?;
    let release = Arc::new(
        manifest
            .authenticate(&receipt, &authority, &attestation)
            .map_err(|_| eyre!("Native threshold release authentication failed"))?,
    );
    let issuer = KagemushaRetailEnrollmentIssuerPolicyV1::decode_canonical_exact(&raw(
        "originals/issuer-policy.norito",
    )?)
    .map_err(|_| eyre!("Native issuer policy rejected"))?;
    issuer
        .validate()
        .map_err(|_| eyre!("Native issuer policy shape rejected"))?;
    ensure!(
        release.purpose() == KagemushaReleasePurposeV1::Production
            && release.release_id() == body.release_id
            && release.network_id() == issuer.runtime.network_id
            && kagemusha_ordinary_retail_issuer_policy_digest_v1(&issuer)
                .map_err(|_| eyre!("Native issuer digest rejected"))?
                == body.fi_issuer_policy_digest,
        "Native release/runtime/issuer scope changed"
    );
    let lineage_issuer = decode_lineage_policy(
        &raw("originals/ordinary-lineage-cas-policy.norito")?,
        &issuer,
    )?;
    let governed = KagemushaOrdinaryGovernedPolicyOriginalsV1::authenticate(
        release.clone(),
        body.profile_id,
        &raw("originals/ordinary-trust.norito")?,
        &raw("originals/app-authority.bin")?,
    )
    .map_err(|_| eyre!("Native ordinary policy authentication failed"))?;
    let admitted_trust: KagemushaOrdinaryAppTrustPolicyV1 =
        canonical(governed.original_trust_policy_bytes())?;
    let expected_integrity = admitted_trust
        .play_integrity_policy
        .as_ref()
        .map(|policy| policy.policy_digest);
    ensure!(
        body.integrity_policy.as_ref().map(|entry| entry.sha256) == expected_integrity,
        "Native inventory integrity policy differs from governed original"
    );
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(
        &files[&body.checkpoint.path].bytes(MAX_ORIGINAL)?,
    )?;
    KagemushaOrdinaryNativeClockOriginalsV1::from_selected_originals(
        checkpoint.clone(),
        issuer.runtime.network_id,
        checkpoint.chain_id().into(),
        std::array::from_fn(|index| body.nodes[index].node.clone()),
        body.clock_policy,
    )
    .map_err(|_| eyre!("Native installed clock selection rejected"))?;
    for role in KagemushaArtifactRoleV1::ALL {
        let binding = release.artifact(role);
        let path = format!("artifacts/{}", hex::encode(binding.sha256));
        let file = files
            .get(&path)
            .ok_or_else(|| eyre!("Native release artifact descriptor absent"))?;
        ensure!(
            file.sha256 == binding.sha256 && file.identity.size == binding.byte_len,
            "Native release artifact original changed"
        );
    }
    // Extra files can hide a private key or provide a second ambiguous purpose. The signed
    // inventory permits only these public original roles and exact released content addresses.
    for name in files.keys() {
        ensure!(
            matches!(
                name.as_str(),
                "originals/authority-policy.norito"
                    | "originals/release-manifest.norito"
                    | "originals/validation-receipt.norito"
                    | "originals/release-attestation.norito"
                    | "originals/issuer-policy.norito"
                    | "originals/ordinary-identity-authority.norito"
                    | "originals/ordinary-identity-policy.norito"
                    | "originals/ordinary-core-issuer-policy.norito"
                    | "originals/ordinary-lineage-cas-policy.norito"
                    | "originals/ordinary-trust.norito"
                    | "originals/app-authority.bin"
            ) || *name == body.checkpoint.path
                || body
                    .integrity_policy
                    .as_ref()
                    .is_some_and(|item| item.path == *name)
                || KagemushaArtifactRoleV1::ALL.iter().any(|role| *name
                    == format!("artifacts/{}", hex::encode(release.artifact(*role).sha256))),
            "Native inventory unrecognized original role"
        );
    }
    Ok((files, release, issuer, checkpoint, lineage_issuer))
}
fn decode_lineage_policy(
    raw: &[u8],
    issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
) -> Result<KagemushaOrdinaryLineageIssuerPolicyV1> {
    ensure!(
        !raw.is_empty() && raw.len() <= 32 * 1024,
        "Native CAS purpose original bound rejected"
    );
    let policy: KagemushaOrdinaryLineageIssuerPolicyV1 = canonical(raw)?;
    policy
        .validate_for_issuer(issuer)
        .map_err(|_| eyre!("Native CAS purpose differs from held issuer"))?;
    Ok(policy)
}
/// Emit the exact full public clock selection from the same genuine held public release
/// inputs as inventory assembly. The Core manifest must independently authenticate this file
/// before it can verify carried clock samples. The emitted shape grants no selected root.
/// # Errors
/// Refuses missing/changed original custody or checkpoint/network/node/policy inconsistencies.
pub fn assemble_kagemusha_ordinary_native_clock_selection_v1(
    root: &Path,
    body: &KagemushaOrdinaryNativeInventoryV1,
) -> Result<Vec<u8>> {
    require_path(root)?;
    let (files, _, issuer, checkpoint, _) = admit_files(root, body)?;
    let selected = KagemushaOrdinaryNativeClockOriginalsV1::from_selected_originals(
        checkpoint.clone(),
        issuer.runtime.network_id,
        checkpoint.chain_id().into(),
        std::array::from_fn(|index| body.nodes[index].node.clone()),
        body.clock_policy,
    )
    .map_err(|_| eyre!("Native release clock selection rejected"))?;
    let raw = selected
        .canonical_selection_original()
        .map_err(|_| eyre!("Native release clock selection rejected"))?;
    for file in files.values() {
        file.stream_check()?;
    }
    Ok(raw)
}

/// Validate real held public originals at release assembly and emit the exact unsigned canonical
/// inventory packet. The existing app-runtime signing process signs DOMAIN || this packet;
/// this function reads no signing key, FI secret, wallet key or platform private material.
/// # Errors
/// Refuses unqualified threshold release originals, any changed descriptor or missing artifact.
pub fn assemble_kagemusha_ordinary_native_inventory_v1(
    root: &Path,
    body: &KagemushaOrdinaryNativeInventoryV1,
) -> Result<Vec<u8>> {
    require_path(root)?;
    let (files, _, _, _, _) = admit_files(root, body)?;
    let payload = norito::encode_canonical(body)?;
    ensure!(
        !payload.is_empty() && payload.len() <= MAX_BODY,
        "Native inventory canonical body rejected"
    );
    let mut out = Vec::with_capacity(76 + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&u32::try_from(payload.len())?.to_le_bytes());
    out.extend_from_slice(&body.runtime_manifest_sha256);
    out.extend_from_slice(&body.sdk_release_sha256);
    out.extend_from_slice(&payload);
    for file in files.values() {
        file.stream_check()?;
    }
    Ok(out)
}

#[cfg(test)]
mod codec_tests {
    use super::*;
    use crate::participant_enrollment_request::NativeCustodyFixture;

    #[test]
    fn held_original_keeps_u64_budget_without_address_sized_narrowing() {
        use std::os::unix::fs::PermissionsExt as _;

        // Public-file custody only: no inventory, release or runtime authority is created.
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("public-original.bin");
        let original = b"data-only public original";
        std::fs::write(&path, original).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let digest = <[u8; 32]>::from(sha2::Sha256::digest(original));

        assert_eq!(MAX_TOTAL, 16_u64 * 1024 * 1024 * 1024);
        assert!(u32::try_from(MAX_TOTAL).is_err());
        let held = HeldFile::open_exact(
            path,
            digest,
            u64::try_from(original.len()).unwrap(),
            MAX_TOTAL,
        )
        .unwrap();
        assert_eq!(held.bytes(original.len()).unwrap(), original);
        held.stream_check().unwrap();
    }

    #[test]
    fn held_original_u64_budget_rejects_over_bound_and_named_replacement() {
        use std::os::unix::fs::PermissionsExt as _;

        // A real tiny file exercises bounds and descriptor identity without fake authority.
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("public-original.bin");
        let replacement = root.join("replacement.bin");
        let original = b"data-only public original";
        let length = u64::try_from(original.len()).unwrap();
        let digest = <[u8; 32]>::from(sha2::Sha256::digest(original));
        std::fs::write(&path, original).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let rejection = HeldFile::open_exact(path.clone(), digest, length, length - 1)
            .err()
            .expect("oversize rejected before descriptor open or hashing");
        assert_eq!(rejection.to_string(), "Native original bound rejected");
        let held = HeldFile::open_exact(path.clone(), digest, length, MAX_TOTAL).unwrap();
        assert!(held.bytes(original.len() - 1).is_err());
        std::fs::write(&replacement, original).unwrap();
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        assert!(held.stream_check().is_err());
        assert!(held.bytes(original.len()).is_err());
    }

    fn assert_exact_frame<T>(value: &T, nominal_name: &str)
    where
        T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSchema,
    {
        let raw = norito::encode_canonical(value).expect("data-only canonical public frame");
        assert!(!raw.is_empty() && raw.len() <= MAX_ORIGINAL);
        let header = norito::core::Header::read(std::io::Cursor::new(&raw)).unwrap();
        assert_eq!(T::nominal_name(), nominal_name);
        assert_eq!(
            header.schema,
            norito::core::schema_hash_for_name(nominal_name)
        );
        let decoded: T = canonical(&raw).expect("bounded canonical typed frame");
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), raw);
        assert!(canonical::<T>(&[]).is_err());
        for end in [
            0,
            norito::core::Header::SIZE - 1,
            norito::core::Header::SIZE,
            raw.len() - 1,
        ] {
            assert!(canonical::<T>(&raw[..end]).is_err(), "truncated at {end}");
        }
        let mut suffix = raw.clone();
        suffix.push(0);
        assert!(canonical::<T>(&suffix).is_err());
        let mut foreign_schema = raw;
        foreign_schema[6] ^= 1;
        assert!(canonical::<T>(&foreign_schema).is_err());
    }

    fn descriptor(path: &str, byte: u8) -> KagemushaOrdinaryNativeOriginalDescriptorV1 {
        KagemushaOrdinaryNativeOriginalDescriptorV1 {
            path: path.into(),
            sha256: [byte; 32],
            byte_len: u64::from(byte),
        }
    }

    fn node(index: usize) -> KagemushaOrdinaryNativeNodeTargetV1 {
        let key = iroha_crypto::KeyPair::from_seed(
            vec![u8::try_from(index + 1).unwrap(); 32],
            iroha_crypto::Algorithm::BlsNormal,
        );
        KagemushaOrdinaryNativeNodeTargetV1 {
            node: KagemushaOrdinaryNativeClockNodeV1 {
                peer_id: iroha_model_base::peer::PeerId::new(key.public_key().clone()),
                build_fingerprint: Hash::new([u8::try_from(index + 11).unwrap()]),
                config_fingerprint: Hash::new([u8::try_from(index + 21).unwrap()]),
            },
            endpoint: format!("https://node{index}.example.invalid/"),
        }
    }

    #[test]
    fn descriptor_framed_codec_rejects_retired_schema_truncation_and_oversize() {
        assert_exact_frame(
            &descriptor("originals/checkpoint.norito", 7),
            "iroha::client::KagemushaOrdinaryNativeOriginalDescriptorV1",
        );
        assert!(
            canonical::<KagemushaOrdinaryNativeOriginalDescriptorV1>(&vec![0; MAX_ORIGINAL + 1])
                .is_err()
        );
    }

    #[test]
    fn node_target_framed_codec_retains_exact_public_identity_and_endpoint() {
        assert_exact_frame(
            &node(0),
            "iroha::client::KagemushaOrdinaryNativeNodeTargetV1",
        );
    }

    #[test]
    fn inventory_framed_codec_retains_complete_public_pins_without_native_admission() {
        // These public data-only fields never call installation/release/native admission.
        let point = hex::decode(concat!(
            "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        ))
        .unwrap();
        let value = KagemushaOrdinaryNativeInventoryV1 {
            version: 1,
            runtime_manifest_sha256: [1; 32],
            sdk_release_sha256: [2; 32],
            sequence: 3,
            release_id: [4; 32],
            profile_id: [5; 32],
            core_public_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(&point).unwrap(),
            world_schema_hash: Hash::new(b"data-only codec schema"),
            checkpoint: descriptor("originals/checkpoint.norito", 7),
            nodes: std::array::from_fn(node),
            clock_policy: KagemushaOrdinaryNativeClockPolicyV1 {
                maximum_reply_age_ms: 10_000,
                maximum_node_skew_ms: 30_000,
                maximum_projection_age_ms: 86_400_000,
                maximum_persistence_age_ms: 5_000,
            },
            fi_current_control_endpoint: "https://fi.example.invalid/".into(),
            fi_issuer_policy_digest: [8; 32],
            ordinary_identity_policy_id: [11; 32],
            ordinary_core_issuer_policy_digest: [12; 32],
            ordinary_lane_namespace: [13; 32],
            integrity_policy: Some(descriptor("originals/integrity.norito", 9)),
            originals: vec![descriptor("originals/trust.norito", 10)],
        };
        assert_exact_frame(&value, "iroha::client::KagemushaOrdinaryNativeInventoryV1");
        // A complete canonical data shape cannot issue an installed clock selection.
        assert!(
            assemble_kagemusha_ordinary_native_clock_selection_v1(
                Path::new("offered-relative-root"),
                &value,
            )
            .is_err()
        );
        let empty = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(assemble_kagemusha_ordinary_native_clock_selection_v1(&empty, &value).is_err());
    }

    #[test]
    fn current_wallet_framed_codec_retains_original_public_values_without_native_admission() {
        // Reuse the maintained actual certificate fixture only as public sidecar data.
        // Repeating a public attestation here does not claim four-node admission/custody.
        let fixture = NativeCustodyFixture::new();
        let public = fixture.wallet_original([7; 32]);
        let value = KagemushaOrdinaryNativeCurrentWalletOriginalV1 {
            proof: norito::encode_canonical(&public.attestation.body.finality_proof).unwrap(),
            world_snapshot: public.world_snapshot,
            signatory_value: public.signatory_value,
            wallet_value: public.wallet_value,
            statements: std::array::from_fn(|_| public.attestation.clone()),
        };
        assert_exact_frame(
            &value,
            "iroha::client::KagemushaOrdinaryNativeCurrentWalletOriginalV1",
        );
    }
    #[test]
    fn installed_lineage_purpose_refuses_disabled_foreign_issuer_and_trailing_original() {
        use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
        let f = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let mut policy = KagemushaOrdinaryLineageIssuerPolicyV1 {
            version: 1,
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &f.issuer_policy,
            )
            .unwrap(),
            issuer_public_key: f.issuer_policy.issuer_public_key.clone(),
            runtime: f.issuer_policy.runtime.clone(),
            data_authority: KagemushaOrdinaryLineageDataAuthorityV1 {
                version: 1,
                liability_pool_id: kagemusha_liability_pool_id_v1(
                    &f.issuer_policy.runtime.network_id,
                    &f.issuer_policy.runtime.asset,
                    f.issuer_policy.runtime.asset_incarnation,
                )
                .unwrap(),
                service_identity_digest: [90; 32],
                data_incarnation_digest: [33; 32],
                dataspace: "mibank.bpng".into(),
                tenant: "mibank-core".into(),
                principal: "core-mibank".into(),
                collection: "retail_enrollments".into(),
            },
            purpose_domain_digest: KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            enabled: true,
        };
        let raw = norito::encode_canonical(&policy).unwrap();
        assert_eq!(
            decode_lineage_policy(&raw, &f.issuer_policy).unwrap(),
            policy
        );
        let mut trailing = raw;
        trailing.push(0);
        assert!(decode_lineage_policy(&trailing, &f.issuer_policy).is_err());
        policy.enabled = false;
        assert!(
            decode_lineage_policy(
                &norito::encode_canonical(&policy).unwrap(),
                &f.issuer_policy
            )
            .is_err()
        );
        policy.enabled = true;
        policy.issuer_policy_digest[0] ^= 1;
        assert!(
            decode_lineage_policy(
                &norito::encode_canonical(&policy).unwrap(),
                &f.issuer_policy
            )
            .is_err()
        );
    }

    #[test]
    fn current_and_lineage_lane_data_prechecks_refuse_retired_preimage_and_scope_substitution() {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{
            account::AccountId,
            testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture,
        };
        for apple in [false, true] {
            let f = Fixture::with_single_member_wallet(apple, false, [19; 32]);
            f.verify(300).unwrap();
            let issuer = f.ordinary_policy.issuer_policy().policy();
            let runtime = &f.issuer_policy.runtime;
            let owner = &f.selection.owner;
            require_installed_owner_lane_data(issuer, runtime, owner).unwrap();

            let mut retired = owner.clone();
            let mut hash = sha2::Sha256::new();
            hash.update(b"iroha:kagemusha:v1:ordinary-native-wallet-lane\0");
            hash.update(norito::encode_canonical(&owner.account_id).unwrap());
            hash.update(norito::encode_canonical(runtime).unwrap());
            retired.lane_id = hash.finalize().into();
            assert_ne!(retired.lane_id, owner.lane_id);
            assert!(require_installed_owner_lane_data(issuer, runtime, &retired).is_err());

            let mut foreign_namespace = issuer.clone();
            foreign_namespace.lane_namespace_id[0] ^= 1;
            assert!(require_installed_owner_lane_data(&foreign_namespace, runtime, owner).is_err());
            let mut foreign_network = issuer.clone();
            foreign_network.network_id[0] ^= 1;
            assert!(require_installed_owner_lane_data(&foreign_network, runtime, owner).is_err());

            let mut foreign_wallet = owner.clone();
            foreign_wallet.account_id = AccountId::new(
                KeyPair::from_seed(vec![47; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            );
            assert!(require_installed_owner_lane_data(issuer, runtime, &foreign_wallet).is_err());
            let mut foreign_runtime = owner.clone();
            foreign_runtime.runtime.scale += 1;
            assert!(require_installed_owner_lane_data(issuer, runtime, &foreign_runtime).is_err());
            // This uses the actual common DATA precheck only. No retained descriptor,
            // AccountClient, Native session, hardware owner or current FI loan is made.
        }
    }
}

#[path = "installed_context.rs"]
mod installed_context;
pub use installed_context::KagemushaNativeOrdinaryInstalledContextV1;
