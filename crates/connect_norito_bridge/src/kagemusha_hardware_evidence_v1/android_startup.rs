//! Shipping Android startup: compiled public root, fixed package assets, measured code and
//! genuine nonce-bound signed Native clock. No managed pins, timestamps or wallet are accepted.
use super::*;
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaHardwareBootstrapArtifactMeasurementsV1, KagemushaOrdinaryNativeClockOwnerV1,
    KagemushaOrdinaryNativeClockSelectionOriginalV1,
};
use iroha_data_model::kagemusha::{
    KagemushaHardwareEvidenceCompiledBindingOriginalV1, KagemushaSignedHardwareBootstrapReleaseV1,
};
use jni::{JNIEnv, objects::JObject};
use std::sync::Mutex;

const COMPILED: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/hardware-evidence-compiled-binding.norito"
));
const RELEASE_ASSET: &str = "kagemusha/hardware-evidence-bootstrap-release.norito";
const CLOCK_ASSET: &str = "kagemusha/hardware-evidence-native-clock-selection.norito";
static STARTUP: Mutex<Option<Startup>> = Mutex::new(None);

#[path = "android_package.rs"]
pub(crate) mod android_package;
use android_package::*;

struct Startup {
    package: HeldPackage,
    splits: Vec<HeldPackage>,
    root: PathBuf,
    signed_release: Vec<u8>,
    compiled: KagemushaHardwareEvidenceCompiledBindingOriginalV1,
    package_name: String,
    version: u64,
    cert: [u8; 32],
    dex: [u8; 32],
    library: [u8; 32],
    clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
    transport: iroha::client::KagemushaNativeClockTransportV1,
}
impl Startup {
    fn recheck_packages(&self) -> Result<()> {
        self.package.recheck()?;
        for split in &self.splits {
            split.recheck()?;
        }
        Ok(())
    }
}

/// Read the actual package Context only. Context is transport to the Android framework, not
/// an authority DTO: root/source pins are compiled, library bytes come from this loaded symbol,
/// and Core independently authenticates app identity in raw KeyMint and Play Integrity originals.
pub(super) fn initialize_if_packaged<'a>(
    env: &mut JNIEnv<'a>,
    application: &JObject<'a>,
    offered_root: &Path,
) -> Result<bool> {
    if COMPILED.is_empty() {
        return Ok(false);
    }
    let mut state = STARTUP.lock().map_err(|_| Error::Custody)?;
    if let Some(startup) = state.as_ref() {
        startup.recheck_packages()?;
        if startup.root != offered_root {
            return Err(Error::Rejected);
        }
        if SOURCE.get().is_some() {
            return Ok(true);
        }
    } else {
        if !env
            .is_instance_of(application, "android/app/Application")
            .map_err(|_| Error::Rejected)?
        {
            return Err(Error::Rejected);
        }
        let actual = object(
            env,
            application,
            "getApplicationContext",
            "()Landroid/content/Context;",
            &[],
        )?;
        if !env
            .is_same_object(application, &actual)
            .map_err(|_| Error::Rejected)?
        {
            return Err(Error::Rejected);
        }
        let backup = object(
            env,
            application,
            "getNoBackupFilesDir",
            "()Ljava/io/File;",
            &[],
        )?;
        let parent = PathBuf::from(string_method(env, &backup, "getAbsolutePath")?);
        let root = parent.join("kagemusha-first-device-hardware-v1");
        validate_path(&parent)?;
        if root != offered_root {
            return Err(Error::Rejected);
        }
        let compiled: KagemushaHardwareEvidenceCompiledBindingOriginalV1 =
            hardware_bootstrap_decode_v1(COMPILED).map_err(|_| Error::Rejected)?;
        let authority = compiled.validate().map_err(|_| Error::Rejected)?;
        if compiled.native_abi != crate::CONNECT_NORITO_BRIDGE_ABI_VERSION {
            return Err(Error::Rejected);
        }
        let assets = object(
            env,
            application,
            "getAssets",
            "()Landroid/content/res/AssetManager;",
            &[],
        )?;
        let release = asset(env, &assets, RELEASE_ASSET, frame::MAX_FIELD)?;
        let signed: KagemushaSignedHardwareBootstrapReleaseV1 =
            hardware_bootstrap_decode_v1(&release).map_err(|_| Error::Rejected)?;
        signed
            .authenticate(&authority)
            .map_err(|_| Error::Rejected)?;
        if signed.manifest.app_source_sha256 != compiled.app_source_sha256
            || signed.manifest.sdk_source_sha256 != compiled.sdk_source_sha256
            || signed.manifest.native_abi != compiled.native_abi
        {
            return Err(Error::Rejected);
        }
        let package_name = string_method(env, application, "getPackageName")?;
        let app_info = object(
            env,
            application,
            "getApplicationInfo",
            "()Landroid/content/pm/ApplicationInfo;",
            &[],
        )?;
        let apk = env
            .get_field(&app_info, "sourceDir", "Ljava/lang/String;")
            .map_err(|_| Error::Rejected)?
            .l()
            .map_err(|_| Error::Rejected)?;
        let apk = string(env, apk)?;
        let package = HeldPackage::open(PathBuf::from(&apk))?;
        let split_paths = installed_split_paths(env, &app_info)?;
        let mut splits = Vec::new();
        for path in &split_paths {
            if path == &apk {
                return Err(Error::Rejected);
            }
            let held = HeldPackage::open(PathBuf::from(path))?;
            let split = zip_file(env, path)?;
            let split = env.auto_local(split);
            let accepted = require_resource_split(env, &split);
            let closed = env
                .call_method(&split, "close", "()V", &[])
                .map_err(|_| Error::Custody);
            accepted?;
            closed?;
            held.recheck()?;
            splits.push(held);
        }
        let (version, cert) = package_identity(env, application, &package_name)?;
        let zip = zip_file(env, &apk)?;
        let measured: Result<([u8; 32], [u8; 32])> = (|| {
            let dex = dex_digest(env, &zip)?;
            let library = loaded_library_digest(env, &zip, &apk, &split_paths)?;
            Ok((dex, library))
        })();
        let closed = env
            .call_method(&zip, "close", "()V", &[])
            .map_err(|_| Error::Custody);
        let (dex, library) = measured?;
        closed?;
        package.recheck()?;
        let m = &signed.manifest;
        let actual_abi = android_abi()?;
        if package_name != m.app_package
            || version != m.app_version_code
            || cert != m.app_signing_identity_digest
            || dex != m.app_code_sha256
            || m.jni_artifacts
                .binary_search_by(|a| a.android_abi.as_str().cmp(actual_abi))
                .ok()
                .is_none_or(|index| library != m.jni_artifacts[index].sha256)
        {
            return Err(Error::Rejected);
        }
        let clock_original = asset(env, &assets, CLOCK_ASSET, 16 * 1024 * 1024 + 4096)?;
        let clock_data =
            KagemushaOrdinaryNativeClockSelectionOriginalV1::decode_original(&clock_original)
                .map_err(|_| Error::Rejected)?;
        let selected = Arc::new(
            clock_data
                .selected_originals()
                .map_err(|_| Error::Rejected)?,
        );
        if selected.selection_digest() != m.native_clock_selection_digest
            || *clock_data.network().as_bytes() != m.network_id
        {
            return Err(Error::Rejected);
        }
        package.recheck()?;
        // The trusted no-backup parent already exists. The clock purpose owns its original
        // native-clock child directly; evidence alias/WAL has a separate fixed sibling directory.
        let clock = if parent
            .join("native-clock")
            .try_exists()
            .map_err(|_| Error::Custody)?
        {
            KagemushaOrdinaryNativeClockOwnerV1::open_existing(&parent, selected)
        } else {
            KagemushaOrdinaryNativeClockOwnerV1::create(&parent, selected)
        }
        .map_err(|_| Error::Custody)?;
        let clock = Arc::new(Mutex::new(clock));
        let transport = iroha::client::KagemushaNativeClockTransportV1::from_public_node_base_urls(
            m.native_clock_base_urls.clone(),
            clock_data.network(),
            &clock,
        )
        .map_err(|_| Error::Custody)?;
        *state = Some(Startup {
            package,
            splits,
            root,
            signed_release: release,
            compiled,
            package_name,
            version,
            cert,
            dex,
            library,
            clock,
            transport,
        });
    }
    let startup = state.as_ref().ok_or(Error::Custody)?;
    startup.recheck_packages()?;
    // The original actual software clock is refreshed after cold recovery; stored samples never
    // become a caller time, renewed evidence deadline or a fake hardware monotonic clock.
    startup
        .transport
        .refresh_current_clock(&startup.clock)
        .map_err(|_| Error::Custody)?;
    startup.recheck_packages()?;
    let measured =
        KagemushaHardwareBootstrapArtifactMeasurementsV1::from_native_startup_measurement(
            startup.package_name.clone(),
            startup.version,
            startup.cert,
            startup.compiled.app_source_sha256,
            startup.dex,
            startup.compiled.sdk_source_sha256,
            android_abi()?.to_owned(),
            startup.library,
            crate::CONNECT_NORITO_BRIDGE_ABI_VERSION,
        )?;
    let authority = startup.compiled.validate().map_err(|_| Error::Rejected)?;
    // This is the actual shipping installer callsite, reached from dedicated JNI open on its
    // retained worker. No account, financial reservation, runtime catalog or OEM service enters.
    bootstrap_kagemusha_native_hardware_evidence_v1(
        startup.root.clone(),
        &startup.signed_release,
        &startup.compiled.authority_policy_original,
        authority.canonical_digest().map_err(|_| Error::Rejected)?,
        measured,
        startup.clock.clone(),
    )?;
    Ok(true)
}
pub(super) fn recheck_installed_package() -> Result<()> {
    STARTUP
        .lock()
        .map_err(|_| Error::Custody)?
        .as_ref()
        .ok_or(Error::Custody)?
        .recheck_packages()
}
