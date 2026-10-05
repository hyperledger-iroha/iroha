//! Read-only capability capture for an independently selected native interpreter.

use super::*;
use crate::taira_public_reset::validate_absolute_normal_path;
use host_pair::NativePythonRuntimeV1;

const SCHEMA: &str = "iroha.taira.public-reset.native-python-runtime.v1";
const MAX_IMAGE: u64 = 512 * 1024 * 1024;
const PROBE: &str = r#"import ctypes, hashlib, json, os, stat, sys
image, fd = json.loads(sys.argv[1]), int(sys.argv[2])
if sys.platform != 'darwin' or sys.version_info[:2] < (3, 11):
    raise RuntimeError('native_python_platform_version')
library = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
library.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
library.proc_pidpath.restype = ctypes.c_int
buffer = ctypes.create_string_buffer(4096)
length = library.proc_pidpath(os.getpid(), buffer, len(buffer))
if length <= 0 or buffer.value.decode() != image['file']['path']:
    raise RuntimeError('native_python_image_changed')
def identity(value):
    if not stat.S_ISREG(value.st_mode): raise RuntimeError('native_python_not_regular')
    return {'device':value.st_dev,'inode':value.st_ino,'uid':value.st_uid,
            'gid':value.st_gid,'mode':stat.S_IMODE(value.st_mode),'links':value.st_nlink,
            'size':value.st_size,'mtime_ns':value.st_mtime_ns,'ctime_ns':value.st_ctime_ns}
def verify():
    if identity(os.fstat(fd)) != image['file']['identity'] or identity(os.stat(image['file']['path'], follow_symlinks=False)) != image['file']['identity']:
        raise RuntimeError('native_python_identity_changed')
verify()
digest = hashlib.sha256()
offset = 0
while offset < image['file']['identity']['size']:
    body = os.pread(fd, min(65536, image['file']['identity']['size'] - offset), offset)
    if not body: raise RuntimeError('native_python_extent_changed')
    digest.update(body)
    offset += len(body)
if digest.hexdigest() != image['sha256']: raise RuntimeError('native_python_digest_changed')
verify()
sys.stdout.write(json.dumps(list(sys.version_info[:3]), separators=(',', ':')) + '\n')
"#;

#[derive(clap::Args, Debug)]
pub(in crate::taira_public_reset) struct CaptureNativePython {
    /// Absolute path to the actual Darwin main image, selected independently of PATH.
    #[arg(long, value_name = "PATH")]
    executable: PathBuf,
    /// Independent SHA-256 pin of the selected image's public bytes.
    #[arg(long, value_name = "SHA256")]
    expected_executable_sha256: String,
    /// Fresh private output directory for the typed public capability.
    #[arg(long, value_name = "DIR")]
    output: PathBuf,
}

#[derive(JsonSerialize)]
struct CaptureReceiptV1 {
    schema: String,
    capability: NativePublicFileV1,
}

fn capture_image(
    executable: &Path,
    expected: &str,
    owner_uid: u32,
) -> Result<(PublicPin, NativePythonRuntimeV1)> {
    validate_lower_hex("native Python independent image digest", expected, 64)?;
    let image = PublicPin::open(executable, MAX_IMAGE, false)?;
    if image.reference.sha256 != expected {
        return Err(eyre!(
            "native Python image differs from its independent pin"
        ));
    }
    let mut runtime = NativePythonRuntimeV1 {
        schema: SCHEMA.into(),
        executable: image.reference.clone(),
        // This validates image custody before executing the independently pinned file.
        version: [3, 11, 0],
    };
    runtime.validate(owner_uid)?;
    let file = image.retained.file().try_clone()?;
    image.revalidate()?;
    let result = require_success(
        RealProcessRunner.run(&ProcessSpec {
            program: executable.into(),
            args: vec![
                "-B".into(),
                "-I".into(),
                "-c".into(),
                PROBE.into(),
                json::to_json(&image.reference)?.into(),
                file.as_raw_fd().to_string().into(),
            ],
            stdin_prefix: Vec::new(),
            stdin_file: None,
            stdin_files: Vec::new(),
            inherited_files: vec![file],
            deadline: Instant::now() + Duration::from_secs(20),
        })?,
        "native Python capability capture",
    )?;
    image.revalidate()?;
    if result.len() > 128 {
        return Err(eyre!(
            "native Python version exceeds its bounded public result"
        ));
    }
    runtime.version = json::from_slice::<Vec<u16>>(&result)?
        .try_into()
        .map_err(|_| eyre!("native Python version requires exactly three numeric components"))?;
    runtime.validate(owner_uid)?;
    Ok((image, runtime))
}

pub(in crate::taira_public_reset) fn capture_native_python(
    args: &CaptureNativePython,
    mut output: impl Write,
) -> Result<()> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err(eyre!("native Python capture requires Darwin/AArch64"));
    }
    let (uid, _, _, _) = native_account()?;
    validate_absolute_normal_path(&args.output, "native Python capability output")?;
    let parent = PrivateDirectory::open(
        args.output
            .parent()
            .ok_or_else(|| eyre!("native Python output parent missing"))?,
    )?;
    require_path_absent(&args.output, "native Python capability output")?;
    let name = args
        .output
        .file_name()
        .ok_or_else(|| eyre!("native Python output basename missing"))?;
    let (image, runtime) = capture_image(&args.executable, &args.expected_executable_sha256, uid)?;
    let body = json::to_json(&runtime)?.into_bytes();
    image.revalidate()?;
    parent.revalidate()?;
    let stage = parent.create_child(format!(
        ".native-python-{}",
        hex::encode(rand::random::<[u8; 16]>())
    ))?;
    stage.write_atomic("native-python.json", &body, PublishMode::CreateNew)?;
    stage.sync()?;
    image.revalidate()?;
    parent.revalidate()?;
    let published = stage.rename_to_sibling(name, PublishMode::CreateNew)?;
    let capability = PublicPin::private_child(&published, "native-python.json", 16 * 1024)?;
    image.revalidate()?;
    capability.revalidate()?;
    writeln!(
        output,
        "{}",
        json::to_json(&CaptureReceiptV1 {
            schema: "iroha.taira.public-reset.native-python-capture.v1".into(),
            capability: capability.reference.clone(),
        })?
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[test]
    fn capability_capture_requires_explicit_image_pin_and_fresh_output() {
        let command = [
            "iroha",
            "taira",
            "public-reset",
            "capture-native-python",
            "--executable",
            "/opt/native/Python",
            "--expected-executable-sha256",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "--output",
            "/Users/operator/capability",
        ];
        assert!(crate::Args::try_parse_from(command).is_ok());
        for range in [4..6, 6..8, 8..10] {
            let missing: Vec<_> = command
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (!range.contains(&index)).then_some(*value))
                .collect();
            assert!(crate::Args::try_parse_from(missing).is_err());
        }
    }

    #[test]
    fn selected_image_digest_is_checked_before_any_probe() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempfile::Builder::new()
            .prefix(".native-python-pin-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        let parent = PrivateDirectory::open(directory.path()).unwrap();
        parent
            .write_atomic(
                "image",
                b"public interpreter fixture",
                PublishMode::CreateNew,
            )
            .unwrap();
        let error = capture_image(
            &directory.path().join("image"),
            &"0".repeat(64),
            rustix::process::geteuid().as_raw(),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("independent pin"));
        assert_eq!(
            parent.read("image", 1024).unwrap().as_slice(),
            b"public interpreter fixture"
        );
    }
}
