//! Copy the independently approved public hardware compile original into its measured JNI.
//! The external signed release contains the resulting JNI SHA and is never embedded here.
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
use std::{env, fs, io::Read as _, path::PathBuf};
#[cfg(unix)]
fn same_original(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.uid() == b.uid()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
#[cfg(not(unix))]
fn same_original(_: &fs::Metadata, _: &fs::Metadata) -> bool {
    false
}
fn main() {
    println!("cargo:rerun-if-env-changed=MOBILE_SDK_HARDWARE_BOOTSTRAP_COMPILED_BINDING_FILE");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let bytes = match env::var_os("MOBILE_SDK_HARDWARE_BOOTSTRAP_COMPILED_BINDING_FILE") {
        None => Vec::new(), // Explicitly uninstalled, no default root or software readiness.
        Some(input) => {
            let path = PathBuf::from(input);
            assert!(
                path.is_absolute(),
                "hardware compile original must be an absolute public file"
            );
            let meta = fs::symlink_metadata(&path).expect("public hardware compile original");
            assert!(
                meta.is_file()
                    && !meta.file_type().is_symlink()
                    && meta.len() > 0
                    && meta.len() <= 192 * 1024,
                "public hardware compile original shape"
            );
            let canonical = fs::canonicalize(&path).expect("public hardware compile original path");
            assert_eq!(
                canonical, path,
                "public compile original may not traverse symlinks/dot segments"
            );
            println!("cargo:rerun-if-changed={}", path.display());
            let mut held =
                fs::File::open(&path).expect("public hardware compile original descriptor");
            // Check inode identity before reading: a replaced public path may never redirect
            // this build intake to another original, including through a new symlink.
            assert!(
                same_original(&meta, &held.metadata().expect("public compile descriptor")),
                "public compile original descriptor changed"
            );
            let mut bytes = Vec::new();
            (&mut held)
                .take(192 * 1024 + 1)
                .read_to_end(&mut bytes)
                .expect("public hardware compile original bytes");
            assert_eq!(
                bytes.len() as u64,
                meta.len(),
                "public compile original size changed"
            );
            let current = fs::symlink_metadata(&path).expect("public compile original recheck");
            assert!(
                current.is_file()
                    && !current.file_type().is_symlink()
                    && same_original(&meta, &current)
                    && same_original(
                        &meta,
                        &held.metadata().expect("public compile descriptor recheck")
                    ),
                "public compile original changed"
            );
            bytes
        }
    };
    fs::write(out.join("hardware-evidence-compiled-binding.norito"), bytes)
        .expect("compiled public original output");
    println!("cargo:rerun-if-env-changed=MOBILE_SDK_ORDINARY_CONTEXT_COMPILED_BINDING_FILE");
    let ordinary = match env::var_os("MOBILE_SDK_ORDINARY_CONTEXT_COMPILED_BINDING_FILE") {
        None => Vec::new(),
        Some(input) => {
            let path = PathBuf::from(input);
            assert!(
                path.is_absolute(),
                "ordinary compile original absolute path"
            );
            let meta = fs::symlink_metadata(&path).expect("ordinary public compile original");
            assert!(
                meta.is_file() && !meta.file_type().is_symlink() && meta.len() == 76,
                "ordinary exact public compile original"
            );
            assert_eq!(
                fs::canonicalize(&path).expect("ordinary canonical compile original"),
                path,
                "ordinary compile symbolic path rejected"
            );
            #[cfg(unix)]
            assert!(
                meta.nlink() == 1 && meta.mode() & 0o022 == 0,
                "ordinary public compile custody"
            );
            println!("cargo:rerun-if-changed={}", path.display());
            let mut file = fs::File::open(&path).expect("ordinary compile original FD");
            assert!(
                same_original(&meta, &file.metadata().expect("ordinary held original")),
                "ordinary compile original replaced before read"
            );
            let mut bytes = Vec::new();
            (&mut file)
                .take(77)
                .read_to_end(&mut bytes)
                .expect("ordinary public compile bytes");
            assert!(
                bytes.len() == 76
                    && &bytes[..8] == b"KGMROOT1"
                    && bytes[8..40].iter().any(|b| *b != 0)
                    && bytes[40..72].iter().any(|b| *b != 0)
                    && u32::from_le_bytes(bytes[72..76].try_into().expect("ordinary ABI width"))
                        > 0,
                "ordinary public compile shape"
            );
            assert!(
                same_original(&meta, &file.metadata().expect("ordinary held recheck"))
                    && same_original(
                        &meta,
                        &fs::symlink_metadata(&path).expect("ordinary named recheck")
                    ),
                "ordinary compile original changed"
            );
            bytes
        }
    };
    fs::write(out.join("ordinary-context-compiled-binding.bin"), ordinary)
        .expect("ordinary compiled public original");
}
