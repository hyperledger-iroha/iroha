//! Actual Android framework/APK/loaded-JNI measurements. No caller measurement DTO or trust key.
use super::{Error, PathBuf, Result, validate_path};
use jni::{
    JNIEnv,
    objects::{JByteArray, JObject, JObjectArray, JString, JValue},
};
use sha2::{Digest as _, Sha256};
use std::{
    ffi::CStr,
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
};
const MAX_DEX_TOTAL: u64 = 256 * 1024 * 1024;
#[derive(Clone, PartialEq, Eq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
    len: u64,
    uid: u32,
    mode: u32,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl FileIdentity {
    fn of(m: &std::fs::Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            len: m.len(),
            uid: m.uid(),
            mode: m.mode(),
            links: m.nlink(),
            modified: (m.mtime(), m.mtime_nsec()),
            changed: (m.ctime(), m.ctime_nsec()),
        }
    }
}
pub(super) struct HeldPackage {
    path: PathBuf,
    file: File,
    identity: FileIdentity,
    process: u32,
}
impl HeldPackage {
    pub(super) fn open(path: PathBuf) -> Result<Self> {
        validate_path(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| Error::Custody)?;
        let m = file.metadata().map_err(|_| Error::Custody)?;
        if !m.is_file() || m.len() == 0 || m.len() > 1024 * 1024 * 1024 || m.mode() & 0o022 != 0 {
            return Err(Error::Rejected);
        }
        let this = Self {
            path,
            file,
            identity: FileIdentity::of(&m),
            process: std::process::id(),
        };
        this.recheck()?;
        Ok(this)
    }
    pub(super) fn recheck(&self) -> Result<()> {
        let named = std::fs::symlink_metadata(&self.path).map_err(|_| Error::Custody)?;
        let held = self.file.metadata().map_err(|_| Error::Custody)?;
        if self.process != std::process::id()
            || !named.is_file()
            || named.file_type().is_symlink()
            || FileIdentity::of(&named) != self.identity
            || FileIdentity::of(&held) != self.identity
        {
            return Err(Error::Custody);
        }
        Ok(())
    }
}
pub(super) fn object<'a>(
    env: &mut JNIEnv<'a>,
    o: &JObject<'a>,
    name: &str,
    sig: &str,
    args: &[JValue<'a, '_>],
) -> Result<JObject<'a>> {
    env.call_method(o, name, sig, args)
        .map_err(|_| Error::Rejected)?
        .l()
        .map_err(|_| Error::Rejected)
}
pub(super) fn string<'a>(env: &mut JNIEnv<'a>, o: JObject<'a>) -> Result<String> {
    let value = env.auto_local(JString::from(o));
    let value = env.get_string(&value).map_err(|_| Error::Rejected)?;
    value
        .to_str()
        .map(str::to_owned)
        .map_err(|_| Error::Rejected)
}
pub(super) fn string_method<'a>(
    env: &mut JNIEnv<'a>,
    o: &JObject<'a>,
    method: &str,
) -> Result<String> {
    let raw = object(env, o, method, "()Ljava/lang/String;", &[])?;
    string(env, raw)
}
pub(super) fn asset<'a>(
    env: &mut JNIEnv<'a>,
    assets: &JObject<'a>,
    name: &str,
    maximum: usize,
) -> Result<Vec<u8>> {
    let name = env.new_string(name).map_err(|_| Error::Custody)?;
    let name = env.auto_local(name);
    let stream = object(
        env,
        assets,
        "open",
        "(Ljava/lang/String;)Ljava/io/InputStream;",
        &[JValue::Object(&name)],
    )?;
    let stream = env.auto_local(stream);
    read_stream(env, &stream, maximum)
}
fn read_stream<'a>(env: &mut JNIEnv<'a>, stream: &JObject<'a>, maximum: usize) -> Result<Vec<u8>> {
    let array = env.new_byte_array(8192).map_err(|_| Error::Custody)?;
    let array = env.auto_local(array);
    let mut out = Vec::new();
    let result = (|| {
        loop {
            let n = env
                .call_method(stream, "read", "([B)I", &[JValue::Object(&array)])
                .map_err(|_| Error::Custody)?
                .i()
                .map_err(|_| Error::Custody)?;
            if n == -1 {
                break;
            }
            if n <= 0
                || n > 8192
                || out
                    .len()
                    .checked_add(n as usize)
                    .is_none_or(|v| v > maximum)
            {
                return Err(Error::Rejected);
            }
            let chunk = env.convert_byte_array(&array).map_err(|_| Error::Custody)?;
            out.extend_from_slice(&chunk[..n as usize]);
        }
        if out.is_empty() {
            return Err(Error::Rejected);
        }
        Ok(out)
    })();
    let close = env
        .call_method(stream, "close", "()V", &[])
        .map_err(|_| Error::Custody);
    let bytes = result?;
    close?;
    Ok(bytes)
}
pub(super) fn zip_file<'a>(env: &mut JNIEnv<'a>, path: &str) -> Result<JObject<'a>> {
    let path = env.new_string(path).map_err(|_| Error::Custody)?;
    let path = env.auto_local(path);
    env.new_object(
        "java/util/zip/ZipFile",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&path)],
    )
    .map_err(|_| Error::Rejected)
}
fn zip_original<'a>(
    env: &mut JNIEnv<'a>,
    zip: &JObject<'a>,
    name: &str,
    maximum: usize,
) -> Result<Vec<u8>> {
    let name = env.new_string(name).map_err(|_| Error::Custody)?;
    let name = env.auto_local(name);
    let entry = object(
        env,
        zip,
        "getEntry",
        "(Ljava/lang/String;)Ljava/util/zip/ZipEntry;",
        &[JValue::Object(&name)],
    )?;
    let entry = env.auto_local(entry);
    if entry.is_null() {
        return Err(Error::Rejected);
    }
    let size = env
        .call_method(&entry, "getSize", "()J", &[])
        .map_err(|_| Error::Rejected)?
        .j()
        .map_err(|_| Error::Rejected)?;
    if size <= 0 || size as u64 > maximum as u64 {
        return Err(Error::Rejected);
    }
    let stream = object(
        env,
        zip,
        "getInputStream",
        "(Ljava/util/zip/ZipEntry;)Ljava/io/InputStream;",
        &[JValue::Object(&entry)],
    )?;
    let stream = env.auto_local(stream);
    let bytes = read_stream(env, &stream, maximum)?;
    if bytes.len() as i64 != size {
        return Err(Error::Rejected);
    }
    Ok(bytes)
}
fn zip_dex_names<'a>(env: &mut JNIEnv<'a>, zip: &JObject<'a>) -> Result<Vec<String>> {
    let entries = object(env, zip, "entries", "()Ljava/util/Enumeration;", &[])?;
    let entries = env.auto_local(entries);
    let mut names = Vec::new();
    let mut count = 0;
    while env
        .call_method(&entries, "hasMoreElements", "()Z", &[])
        .map_err(|_| Error::Rejected)?
        .z()
        .map_err(|_| Error::Rejected)?
    {
        count += 1;
        if count > 65536 {
            return Err(Error::Rejected);
        }
        let entry = object(env, &entries, "nextElement", "()Ljava/lang/Object;", &[])?;
        let entry = env.auto_local(entry);
        let name = string_method(env, &entry, "getName")?;
        if name.starts_with("classes") && name.ends_with(".dex") && !name.contains('/') {
            names.push(name);
        }
    }
    Ok(names)
}
pub(super) fn require_resource_split<'a>(env: &mut JNIEnv<'a>, zip: &JObject<'a>) -> Result<()> {
    // The signed first-release BPNG application has no code-bearing dynamic feature modules.
    // ABI/resource config splits are supported; they may not add code outside its measured DEX.
    if !zip_dex_names(env, zip)?.is_empty() {
        return Err(Error::Rejected);
    }
    Ok(())
}
pub(super) fn installed_split_paths<'a>(
    env: &mut JNIEnv<'a>,
    info: &JObject<'a>,
) -> Result<Vec<String>> {
    let raw = env
        .get_field(info, "splitSourceDirs", "[Ljava/lang/String;")
        .map_err(|_| Error::Rejected)?
        .l()
        .map_err(|_| Error::Rejected)?;
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let array = env.auto_local(JObjectArray::from(raw));
    let count = env.get_array_length(&*array).map_err(|_| Error::Rejected)?;
    if !(0..=256).contains(&count) {
        return Err(Error::Rejected);
    }
    let mut paths = Vec::new();
    for index in 0..count {
        let value = env
            .get_object_array_element(&array, index)
            .map_err(|_| Error::Rejected)?;
        if value.is_null() {
            return Err(Error::Rejected);
        }
        let path = string(env, value)?;
        if path.is_empty() || path.len() > 4096 || paths.contains(&path) {
            return Err(Error::Rejected);
        }
        paths.push(path);
    }
    Ok(paths)
}
pub(super) fn dex_digest<'a>(env: &mut JNIEnv<'a>, zip: &JObject<'a>) -> Result<[u8; 32]> {
    let names = canonical_dex_names(zip_dex_names(env, zip)?)?;
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:android-application-dex\0");
    hash.update((names.len() as u64).to_le_bytes());
    let mut total = 0u64;
    for name in &names {
        let bytes = zip_original(env, zip, name, MAX_DEX_TOTAL as usize)?;
        total = total
            .checked_add(bytes.len() as u64)
            .filter(|v| *v <= MAX_DEX_TOTAL)
            .ok_or(Error::Rejected)?;
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
    }
    Ok(hash.finalize().into())
}
pub(super) fn loaded_library_digest<'a>(
    env: &mut JNIEnv<'a>,
    zip: &JObject<'a>,
    apk: &str,
    splits: &[String],
) -> Result<[u8; 32]> {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::uninit();
    // SAFETY: dladdr receives this loaded function's address and a valid output structure.
    if unsafe {
        libc::dladdr(
            loaded_library_digest as *const () as *const libc::c_void,
            info.as_mut_ptr(),
        )
    } == 0
    {
        return Err(Error::Custody);
    }
    // SAFETY: successful dladdr initializes its output; the pathname is library-owned C text.
    let info = unsafe { info.assume_init() };
    if info.dli_fname.is_null() {
        return Err(Error::Custody);
    }
    let loaded = unsafe { CStr::from_ptr(info.dli_fname) }
        .to_str()
        .map_err(|_| Error::Rejected)?;
    let bytes = if let Some((container, entry)) = loaded.split_once("!/") {
        let abi = android_abi()?;
        if (container != apk && !splits.iter().any(|s| s == container))
            || entry != format!("lib/{abi}/libconnect_norito_bridge.so")
        {
            return Err(Error::Rejected);
        }
        if container == apk {
            zip_original(env, zip, entry, 512 * 1024 * 1024)?
        } else {
            let split = zip_file(env, container)?;
            let split = env.auto_local(split);
            let original = zip_original(env, &split, entry, 512 * 1024 * 1024);
            let closed = env
                .call_method(&split, "close", "()V", &[])
                .map_err(|_| Error::Custody);
            let bytes = original?;
            closed?;
            bytes
        }
    } else {
        let held = HeldPackage::open(PathBuf::from(loaded))?;
        if held.path.file_name().and_then(|v| v.to_str()) != Some("libconnect_norito_bridge.so") {
            return Err(Error::Rejected);
        }
        let mut data = Vec::new();
        (&held.file)
            .take(512 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(|_| Error::Custody)?;
        if data.len() != held.identity.len as usize || data.len() > 512 * 1024 * 1024 {
            return Err(Error::Rejected);
        }
        held.recheck()?;
        data
    };
    Ok(Sha256::digest(bytes).into())
}
pub(super) fn android_abi() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "aarch64" => Ok("arm64-v8a"),
        "x86_64" => Ok("x86_64"),
        "arm" => Ok("armeabi-v7a"),
        "x86" => Ok("x86"),
        _ => Err(Error::Rejected),
    }
}
pub(super) fn package_identity<'a>(
    env: &mut JNIEnv<'a>,
    application: &JObject<'a>,
    package: &str,
) -> Result<(u64, [u8; 32])> {
    let sdk = env
        .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")
        .map_err(|_| Error::Rejected)?
        .i()
        .map_err(|_| Error::Rejected)?;
    if sdk < 26 {
        return Err(Error::Rejected);
    }
    let pm = object(
        env,
        application,
        "getPackageManager",
        "()Landroid/content/pm/PackageManager;",
        &[],
    )?;
    let name = env.new_string(package).map_err(|_| Error::Custody)?;
    let info = object(
        env,
        &pm,
        "getPackageInfo",
        "(Ljava/lang/String;I)Landroid/content/pm/PackageInfo;",
        &[
            JValue::Object(&name),
            JValue::Int(if sdk >= 28 { 0x08000000 } else { 64 }),
        ],
    )?;
    let (version, signers) = if sdk >= 28 {
        let version = env
            .call_method(&info, "getLongVersionCode", "()J", &[])
            .map_err(|_| Error::Rejected)?
            .j()
            .map_err(|_| Error::Rejected)?;
        let signing = env
            .get_field(&info, "signingInfo", "Landroid/content/pm/SigningInfo;")
            .map_err(|_| Error::Rejected)?
            .l()
            .map_err(|_| Error::Rejected)?;
        (
            version,
            object(
                env,
                &signing,
                "getApkContentsSigners",
                "()[Landroid/content/pm/Signature;",
                &[],
            )?,
        )
    } else {
        (
            env.get_field(&info, "versionCode", "I")
                .map_err(|_| Error::Rejected)?
                .i()
                .map_err(|_| Error::Rejected)? as i64,
            env.get_field(&info, "signatures", "[Landroid/content/pm/Signature;")
                .map_err(|_| Error::Rejected)?
                .l()
                .map_err(|_| Error::Rejected)?,
        )
    };
    let signers = JObjectArray::from(signers);
    if version <= 0
        || env
            .get_array_length(&signers)
            .map_err(|_| Error::Rejected)?
            != 1
    {
        return Err(Error::Rejected);
    }
    let cert = env
        .get_object_array_element(&signers, 0)
        .map_err(|_| Error::Rejected)?;
    let raw = object(env, &cert, "toByteArray", "()[B", &[])?;
    let bytes = env
        .convert_byte_array(JByteArray::from(raw))
        .map_err(|_| Error::Rejected)?;
    if bytes.is_empty() || bytes.len() > 64 * 1024 {
        return Err(Error::Rejected);
    }
    Ok((version as u64, Sha256::digest(bytes).into()))
}
fn canonical_dex_names(mut names: Vec<String>) -> Result<Vec<String>> {
    if names.is_empty() || names.len() > 256 {
        return Err(Error::Rejected);
    }
    names.sort_by_key(|name| {
        if name == "classes.dex" {
            1
        } else {
            name.strip_prefix("classes")
                .and_then(|v| v.strip_suffix(".dex"))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0)
        }
    });
    for (index, name) in names.iter().enumerate() {
        let expected = if index == 0 {
            "classes.dex".to_owned()
        } else {
            format!("classes{}.dex", index + 1)
        };
        if name != &expected {
            return Err(Error::Rejected);
        }
    }
    Ok(names)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dex_selection_is_complete_contiguous_and_unambiguous() {
        assert_eq!(
            canonical_dex_names(vec![
                "classes3.dex".into(),
                "classes.dex".into(),
                "classes2.dex".into()
            ])
            .unwrap(),
            vec!["classes.dex", "classes2.dex", "classes3.dex"]
        );
        for names in [
            vec![],
            vec!["classes2.dex"],
            vec!["classes.dex", "classes3.dex"],
            vec!["classes.dex", "classes.dex"],
            vec!["classes.dex", "classes02.dex"],
            vec!["classesjunk.dex"],
        ] {
            assert!(canonical_dex_names(names.into_iter().map(str::to_owned).collect()).is_err());
        }
        assert!(canonical_dex_names(vec!["classes.dex".into(); 257]).is_err());
    }
    #[test]
    fn held_package_refuses_replaced_public_original() {
        use std::io::Write as _;
        let p = std::env::temp_dir().join(format!(
            "hardware-public-measurement-{}-replace",
            std::process::id()
        ));
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&p)
            .unwrap();
        f.write_all(b"known public fixture").unwrap();
        drop(f);
        let held = HeldPackage::open(p.clone()).unwrap();
        assert!(held.recheck().is_ok());
        let replacement = p.with_extension("replacement");
        std::fs::write(&replacement, b"new known public bytes").unwrap();
        std::fs::rename(replacement, &p).unwrap();
        assert!(held.recheck().is_err());
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn held_package_refuses_symlink_and_current_file_changes() {
        let p = std::env::temp_dir().join(format!(
            "hardware-public-measurement-{}-change",
            std::process::id()
        ));
        std::fs::write(&p, b"known public fixture").unwrap();
        let held = HeldPackage::open(p.clone()).unwrap();
        std::fs::write(&p, b"changed public fixture").unwrap();
        assert!(held.recheck().is_err());
        let link = p.with_extension("link");
        std::os::unix::fs::symlink(&p, &link).unwrap();
        assert!(HeldPackage::open(link.clone()).is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(p).unwrap();
    }
}
