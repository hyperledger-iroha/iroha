//! Tiny crate for common logic for finding and including CUDA.

use std::{
    env,
    path::{Path, PathBuf},
};

#[cfg(not(target_os = "windows"))]
fn append_if_cuda_driver_dir(paths: &mut Vec<PathBuf>, dir: PathBuf) {
    let has_cuda_soname = ["libcuda.so", "libcuda.so.1", "libcuda.so.1.1"]
        .iter()
        .any(|name| dir.join(name).is_file());
    if has_cuda_soname && !paths.iter().any(|existing| existing == &dir) {
        paths.push(dir);
    }
}

pub fn include_cuda() {
    if env::var("DOCS_RS").is_err() && !cfg!(doc) {
        let paths = find_cuda_lib_dirs();
        if paths.is_empty() {
            println!(
                "cargo:warning=find_cuda_helper: CUDA toolkit not found; skipping CUDA link setup"
            );
        } else {
            for path in paths {
                println!("cargo:rustc-link-search=native={}", path.display());
            }
            println!("cargo:rustc-link-lib=dylib=cuda");
        }

        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed=CUDA_LIBRARY_PATH");
        println!("cargo:rerun-if-env-changed=CUDA_ROOT");
        println!("cargo:rerun-if-env-changed=CUDA_PATH");
        println!("cargo:rerun-if-env-changed=CUDA_TOOLKIT_ROOT_DIR");
    }
}

// Returns true if the given path is a valid cuda installation
fn is_cuda_root_path<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().join("include").join("cuda.h").is_file()
}

pub fn find_cuda_root() -> Option<PathBuf> {
    // search through the common environment variables first
    for path in ["CUDA_PATH", "CUDA_ROOT", "CUDA_TOOLKIT_ROOT_DIR"]
        .iter()
        .filter_map(|name| std::env::var(*name).ok())
    {
        if is_cuda_root_path(&path) {
            return Some(path.into());
        }
    }

    // If it wasn't specified by env var, try the default installation paths
    #[cfg(not(target_os = "windows"))]
    {
        for path in ["/usr/local/cuda", "/opt/cuda"] {
            if is_cuda_root_path(path) {
                return Some(path.into());
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::{cmp::Ordering, collections::HashSet, ffi::OsStr, fs};

        fn parse_version(candidate: &OsStr) -> Option<(u32, u32)> {
            let s = candidate
                .to_str()?
                .trim_start_matches(|c| matches!(c, 'v' | 'V'));
            if s.is_empty() {
                return None;
            }
            let mut parts = s.split(|c| c == '.' || c == '_');
            let major = parts.next()?.parse().ok()?;
            let minor = parts
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            Some((major, minor))
        }

        let mut base_roots = Vec::new();
        for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(value) = env::var_os(var) {
                let path = PathBuf::from(value);
                if path.is_dir() {
                    base_roots.push(path);
                }
            }
        }

        let mut candidates: Vec<(Option<(u32, u32)>, PathBuf)> = Vec::new();
        for base in base_roots {
            let toolkit_root = base.join("NVIDIA GPU Computing Toolkit").join("CUDA");
            if !toolkit_root.is_dir() {
                continue;
            }
            candidates.push((None, toolkit_root.clone()));
            if let Ok(entries) = fs::read_dir(&toolkit_root) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.file_type() {
                        if !metadata.is_dir() {
                            continue;
                        }
                    } else {
                        continue;
                    }
                    let version = parse_version(&entry.file_name());
                    candidates.push((version, entry.path()));
                }
            }
        }
        // Legacy fallback
        candidates.push((None, PathBuf::from("C:/CUDA")));

        candidates.sort_by(|a, b| match (&a.0, &b.0) {
            (Some(va), Some(vb)) => vb.cmp(va).then_with(|| b.1.cmp(&a.1)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => b.1.cmp(&a.1),
        });

        let mut seen = HashSet::new();
        for (_, path) in candidates {
            if !seen.insert(path.clone()) {
                continue;
            }
            if is_cuda_root_path(&path) {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
pub fn find_cuda_lib_dirs() -> Vec<PathBuf> {
    if let Some(root_path) = find_cuda_root() {
        // To do this the right way, we check to see which target we're building for.
        let target = env::var("TARGET")
            .expect("cargo did not set the TARGET environment variable as required.");

        // Targets use '-' separators. e.g. x86_64-pc-windows-msvc
        let target_components: Vec<_> = target.as_str().split('-').collect();

        // We check that we're building for Windows. This code assumes that the layout in
        // CUDA_PATH matches Windows.
        if target_components[2] != "windows" {
            panic!(
                "The CUDA_PATH variable is only used by cuda-sys on Windows. Your target is {}.",
                target
            );
        }

        // Sanity check that the second component of 'target' is "pc"
        debug_assert_eq!(
            "pc", target_components[1],
            "Expected a Windows target to have the second component be 'pc'. Target: {}",
            target
        );

        // x86_64 should use the libs in the "lib/x64" directory. If we ever support i686 (which
        // does not ship with cublas support), its libraries are in "lib/Win32".
        let lib_path = match target_components[0] {
            "x86_64" => "x64",
            "i686" => {
                // lib path would be "Win32" if we support i686. "cublas" is not present in the
                // 32-bit install.
                panic!("Rust cuda-sys does not currently support 32-bit Windows.");
            }
            _ => {
                panic!("Rust cuda-sys only supports the x86_64 Windows architecture.");
            }
        };

        let lib_dir = root_path.join("lib").join(lib_path);

        return if lib_dir.is_dir() {
            vec![lib_dir]
        } else {
            vec![]
        };
    }

    vec![]
}

pub fn read_env() -> Vec<PathBuf> {
    if let Ok(path) = env::var("CUDA_LIBRARY_PATH") {
        // The location of the libcuda, libcudart, and libcublas can be hardcoded with the
        // CUDA_LIBRARY_PATH environment variable.
        let split_char = if cfg!(target_os = "windows") {
            ";"
        } else {
            ":"
        };
        path.split(split_char).map(PathBuf::from).collect()
    } else {
        vec![]
    }
}

#[cfg(not(target_os = "windows"))]
pub fn find_cuda_lib_dirs() -> Vec<PathBuf> {
    let mut candidates = read_env();
    candidates.push(PathBuf::from("/opt/cuda"));
    candidates.push(PathBuf::from("/usr/local/cuda"));
    for e in glob::glob("/usr/local/cuda-*").unwrap().flatten() {
        candidates.push(e)
    }

    let mut valid_paths = vec![];
    for base in &candidates {
        let lib = PathBuf::from(base).join("lib64");
        if lib.is_dir() {
            valid_paths.push(lib.clone());
            valid_paths.push(lib.join("stubs"));
        }
        let base = base.join("targets/x86_64-linux");
        let header = base.join("include/cuda.h");
        if header.is_file() {
            valid_paths.push(base.join("lib"));
            valid_paths.push(base.join("lib/stubs"));
            continue;
        }
    }

    // Driver-only hosts (for example WSL GPU passthrough) may expose `libcuda.so`
    // without a full toolkit root such as `/usr/local/cuda`.
    for dir in [
        PathBuf::from("/usr/lib/wsl/lib"),
        PathBuf::from("/usr/lib64"),
        PathBuf::from("/usr/lib/x86_64-linux-gnu"),
        PathBuf::from("/usr/lib"),
        PathBuf::from("/lib64"),
        PathBuf::from("/lib/x86_64-linux-gnu"),
        PathBuf::from("/lib"),
    ] {
        append_if_cuda_driver_dir(&mut valid_paths, dir);
    }

    valid_paths
}

#[cfg(target_os = "windows")]
pub fn find_optix_root() -> Option<PathBuf> {
    // the optix SDK installer sets OPTIX_ROOT_DIR whenever it installs.
    // We also check OPTIX_ROOT first in case someone wants to override it without overriding
    // the SDK-set variable.

    env::var("OPTIX_ROOT")
        .ok()
        .or_else(|| env::var("OPTIX_ROOT_DIR").ok())
        .map(PathBuf::from)
}

#[cfg(target_family = "unix")]
pub fn find_optix_root() -> Option<PathBuf> {
    env::var("OPTIX_ROOT")
        .ok()
        .or_else(|| env::var("OPTIX_ROOT_DIR").ok())
        .map(PathBuf::from)
}

#[cfg(doc)]
pub fn find_libnvvm_bin_dir() -> String {
    String::new()
}

#[cfg(all(target_os = "windows", not(doc)))]
pub fn find_libnvvm_bin_dir() -> String {
    if env::var("DOCS_RS").is_ok() {
        return String::new();
    }
    match find_cuda_root() {
        Some(root) => root
            .join("nvvm")
            .join("lib")
            .join("x64")
            .to_string_lossy()
            .into_owned(),
        None => String::new(),
    }
}

#[cfg(all(target_os = "linux", not(doc)))]
pub fn find_libnvvm_bin_dir() -> String {
    if env::var("DOCS_RS").is_ok() {
        return String::new();
    }
    match find_cuda_root() {
        Some(root) => root
            .join("nvvm")
            .join("lib64")
            .to_string_lossy()
            .into_owned(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        sync::{
            Mutex,
            atomic::{AtomicU64, Ordering as AtomicOrdering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn finds_root_from_env_variable() {
        let _lock = ENV_MUTEX.lock().expect("env mutex poisoned");
        let temp_root = TempCudaRoot::new();
        let _path_guard = EnvVarGuard::set_path("CUDA_PATH", temp_root.path());
        let _root_guard = EnvVarGuard::unset("CUDA_ROOT");
        let _toolkit_guard = EnvVarGuard::unset("CUDA_TOOLKIT_ROOT_DIR");

        let detected = find_cuda_root();
        assert_eq!(detected.as_deref(), Some(temp_root.path()));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn prefers_highest_versioned_windows_install() {
        let _lock = ENV_MUTEX.lock().expect("env mutex poisoned");
        let base = TempCudaRoot::new();
        let toolkit_root = base
            .path()
            .join("NVIDIA GPU Computing Toolkit")
            .join("CUDA");
        let v11 = toolkit_root.join("v11.8");
        let v12 = toolkit_root.join("12.2");

        for path in [&toolkit_root, &v11, &v12] {
            fs::create_dir_all(path.join("include")).expect("create include");
            fs::write(path.join("include").join("cuda.h"), "// stub header").expect("write cuda.h");
        }

        let _unset_cuda_path = EnvVarGuard::unset("CUDA_PATH");
        let _unset_cuda_root = EnvVarGuard::unset("CUDA_ROOT");
        let _unset_toolkit_root = EnvVarGuard::unset("CUDA_TOOLKIT_ROOT_DIR");
        let _pf = EnvVarGuard::set_path("ProgramFiles", base.path());
        let _pf86 = EnvVarGuard::set_path("ProgramFiles(x86)", base.path());
        let _pf6432 = EnvVarGuard::set_path("ProgramW6432", base.path());

        let detected = find_cuda_root().expect("should detect versioned CUDA root");
        assert!(
            detected.ends_with("12.2"),
            "expected highest version directory, got {detected:?}"
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn appends_driver_dir_when_cuda_soname_exists() {
        let temp_root = TempDriverLibDir::new();
        let mut paths = Vec::new();
        append_if_cuda_driver_dir(&mut paths, temp_root.path().to_path_buf());
        assert_eq!(paths, vec![temp_root.path().to_path_buf()]);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn skips_driver_dir_without_cuda_soname() {
        let temp_root = TempEmptyDir::new();
        let mut paths = Vec::new();
        append_if_cuda_driver_dir(&mut paths, temp_root.path().to_path_buf());
        assert!(paths.is_empty());
    }

    struct TempCudaRoot {
        path: PathBuf,
    }

    impl TempCudaRoot {
        fn new() -> Self {
            let mut base = env::temp_dir();
            let nonce = NEXT_ID.fetch_add(1, AtomicOrdering::Relaxed);
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos();
            base.push(format!("find_cuda_helper_test_{ts}_{nonce}"));
            fs::create_dir_all(base.join("include")).expect("failed to create include dir");
            fs::write(base.join("include").join("cuda.h"), "// stub header")
                .expect("failed to create cuda.h");
            Self { path: base }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempCudaRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[cfg(not(target_os = "windows"))]
    struct TempDriverLibDir {
        path: PathBuf,
    }

    #[cfg(not(target_os = "windows"))]
    impl TempDriverLibDir {
        fn new() -> Self {
            let path = temp_dir("driver");
            fs::write(path.join("libcuda.so.1"), "").expect("failed to create libcuda stub");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    #[cfg(not(target_os = "windows"))]
    impl Drop for TempDriverLibDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[cfg(not(target_os = "windows"))]
    struct TempEmptyDir {
        path: PathBuf,
    }

    #[cfg(not(target_os = "windows"))]
    impl TempEmptyDir {
        fn new() -> Self {
            Self {
                path: temp_dir("empty"),
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    #[cfg(not(target_os = "windows"))]
    impl Drop for TempEmptyDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn temp_dir(tag: &str) -> PathBuf {
        let mut base = env::temp_dir();
        let nonce = NEXT_ID.fetch_add(1, AtomicOrdering::Relaxed);
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        base.push(format!("find_cuda_helper_test_{tag}_{ts}_{nonce}"));
        fs::create_dir_all(&base).expect("failed to create temp dir");
        base
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set_path(key: &'static str, path: &Path) -> Self {
            let original = env::var_os(key);
            // SAFETY: test suite serialises access via `ENV_MUTEX`, so modifying the
            // process environment is well-defined for the duration of the guard.
            unsafe {
                env::set_var(key, path);
            }
            Self { key, original }
        }

        fn unset(key: &'static str) -> Self {
            let original = env::var_os(key);
            // SAFETY: serialized via `ENV_MUTEX`, so removing the variable is safe.
            unsafe {
                env::remove_var(key);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = self.original.clone() {
                // SAFETY: restore previous value while `ENV_MUTEX` is held.
                unsafe {
                    env::set_var(self.key, value);
                }
            } else {
                // SAFETY: serialized mutation of process environment.
                unsafe {
                    env::remove_var(self.key);
                }
            }
        }
    }
}
