use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    // Ensure the produced dynamic library inherits the correct Python linkage flags.
    pyo3_build_config::add_extension_module_link_args();

    println!("cargo:rerun-if-env-changed=IROHA_PYTHON_RUNTIME_PATH");
    println!("cargo:rerun-if-env-changed=IROHA_PYTHON_SKIP_RUNTIME_LINK");
    println!("cargo:rerun-if-env-changed=PYO3_PYTHON");
    println!("cargo:rerun-if-env-changed=PYTHON_SYS_EXECUTABLE");
    println!("cargo:rerun-if-env-changed=PYTHON");
    println!("cargo:rerun-if-changed=python-runtime-path");

    // Tests link as executables; on macOS they also require the dynamic lookup flag set.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-undefined");
        println!("cargo:rustc-link-arg=dynamic_lookup");
    }

    if env::var_os("IROHA_PYTHON_SKIP_RUNTIME_LINK").is_some() {
        return;
    }

    if let Some(runtime) = python_runtime_library_path() {
        emit_runtime_link_args(&runtime);
    } else {
        println!(
            "cargo:warning=iroha_python_rs: unable to locate CPython shared library; \
             set `IROHA_PYTHON_RUNTIME_PATH` or add `python-runtime-path` to override."
        );
    }
}

fn emit_runtime_link_args(path: &Path) {
    println!("cargo:rustc-link-arg={}", path.display());
    if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix")
        && let Some(rpath) =
            framework_rpath_root(path).or_else(|| path.parent().map(Path::to_path_buf))
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", rpath.display());
    }
}

fn python_runtime_library_path() -> Option<PathBuf> {
    runtime_path_override()
        .or_else(runtime_path_from_env)
        .or_else(python_runtime_via_interpreter)
        .and_then(|path| {
            if path.exists() {
                fs::canonicalize(&path).ok().or(Some(path))
            } else {
                None
            }
        })
}

fn framework_rpath_root(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent();
    while let Some(dir) = current {
        if let Some(name) = dir.file_name().and_then(|n| n.to_str())
            && name.ends_with(".framework")
        {
            return dir.parent().map(|parent| parent.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn runtime_path_override() -> Option<PathBuf> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")?;
    let override_file = PathBuf::from(manifest_dir).join("python-runtime-path");
    let contents = fs::read_to_string(override_file).ok()?;
    contents
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(PathBuf::from)
}

fn runtime_path_from_env() -> Option<PathBuf> {
    env::var_os("IROHA_PYTHON_RUNTIME_PATH").map(PathBuf::from)
}

fn python_runtime_via_interpreter() -> Option<PathBuf> {
    const DISCOVERY_SCRIPT: &str = r#"
import os
import sys
import sysconfig
from pathlib import Path

def candidates():
    seen = set()

    def emit(path):
        if not path:
            return
        path = os.path.abspath(path)
        if path in seen:
            return
        seen.add(path)
        yield path

    def emit_libdir_candidate(lib_dir, name):
        if not lib_dir or not name:
            return
        path = os.path.abspath(os.path.join(lib_dir, name))
        yield from emit(path)
        if os.path.exists(path):
            return
        # Debian/Ubuntu often install only the SONAME, while sysconfig reports
        # the unversioned .so name. Probe the versioned variants in the same dir.
        stem = Path(name).name
        if stem.endswith(".so"):
            prefix = stem + "."
            for child in sorted(Path(lib_dir).glob(prefix + "*")):
                yield from emit(str(child))

    lib_dir = sysconfig.get_config_var("LIBDIR") or sysconfig.get_config_var("LIBPL")
    for lib_name in (
        sysconfig.get_config_var("LDLIBRARY"),
        sysconfig.get_config_var("PY3LIBRARY"),
        sysconfig.get_config_var("INSTSONAME"),
        sysconfig.get_config_var("LIBRARY"),
    ):
        if lib_name and os.path.isabs(lib_name):
            yield from emit(lib_name)
        yield from emit_libdir_candidate(lib_dir, lib_name)

    framework = sysconfig.get_config_var("PYTHONFRAMEWORK")
    install_dir = sysconfig.get_config_var("PYTHONFRAMEWORKINSTALLDIR")
    version = sysconfig.get_config_var("VERSION")
    if framework and install_dir:
        versions_dir = os.path.join(install_dir, "Versions")
        if version:
            yield from emit(os.path.join(versions_dir, version, framework))
        yield from emit(os.path.join(versions_dir, "Current", framework))
    libpython = sysconfig.get_config_var("LIBPYTHON")
    if libpython:
        yield from emit(libpython)

for candidate in candidates():
    if candidate and os.path.exists(candidate):
        sys.stdout.write(os.path.realpath(candidate))
        break
"#;

    for interpreter in python_interpreter_candidates() {
        let output = Command::new(&interpreter)
            .args(["-c", DISCOVERY_SCRIPT])
            .output();
        let output = match output {
            Ok(output) if output.status.success() => output,
            _ => continue,
        };

        if let Ok(path) = String::from_utf8(output.stdout) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
    }

    None
}

fn python_interpreter_candidates() -> Vec<String> {
    let mut candidates = Vec::<String>::new();
    push_candidate(&mut candidates, env::var("PYO3_PYTHON"));
    push_candidate(&mut candidates, env::var("PYTHON_SYS_EXECUTABLE"));
    push_candidate(&mut candidates, env::var("PYTHON"));
    if candidates.is_empty() {
        candidates.push("python3".to_string());
        candidates.push("python".to_string());
    }
    candidates.dedup();
    candidates
}

fn push_candidate(list: &mut Vec<String>, value: Result<String, env::VarError>) {
    if let Ok(value) = value
        && !list.iter().any(|existing| existing == &value)
    {
        list.push(value);
    }
}
