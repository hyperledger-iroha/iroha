use std::{env, path::PathBuf, process::Command};

fn main() {
    emit_build_cfgs();

    if env::var_os("DOCS_RS").is_some() {
        return;
    }

    if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {
        return;
    }

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.clone());

    let git_dir = workspace_root.join(".git");
    if !git_dir.exists() {
        // Building from a packaged source; skip sync check.
        return;
    }

    let script_py = workspace_root
        .join("scripts")
        .join("check_norito_bindings_sync.py");
    let script_sh = workspace_root
        .join("scripts")
        .join("check_norito_bindings_sync.sh");
    if !script_py.exists() && !script_sh.exists() {
        // Script absent (possibly trimmed workspace); nothing to do.
        return;
    }

    if script_py.exists() {
        println!("cargo:rerun-if-changed={}", script_py.display());
    }
    if script_sh.exists() {
        println!("cargo:rerun-if-changed={}", script_sh.display());
    }

    if script_py.exists() {
        let interpreters = ["python3", "python"];
        for interpreter in interpreters {
            match Command::new(interpreter)
                .current_dir(&workspace_root)
                .arg(&script_py)
                .status()
            {
                Ok(status) => {
                    if status.success() {
                        return;
                    }
                    panic!(
                        "Norito bindings sync check failed. Please run scripts/check_norito_bindings_sync.py manually."
                    );
                }
                Err(_) => continue,
            }
        }
        // Fall through to shell wrapper if Python is unavailable.
    }

    if !script_sh.exists() {
        panic!(
            "Norito bindings sync check could not be executed because a Python interpreter was not found. \
             Install Python 3 and rerun scripts/check_norito_bindings_sync.py manually."
        );
    }

    let shells = ["sh", "bash"];
    for shell in shells {
        match Command::new(shell)
            .current_dir(&workspace_root)
            .arg(script_sh.as_os_str())
            .status()
        {
            Ok(status) => {
                if status.success() {
                    return;
                }
                panic!(
                    "Norito bindings sync check failed. Please run scripts/check_norito_bindings_sync.py manually."
                );
            }
            Err(_) => continue,
        }
    }

    panic!(
        "Norito bindings sync check could not be executed because no suitable shell or Python interpreter was found. \
         Install Python 3 and rerun scripts/check_norito_bindings_sync.py manually."
    );
}

fn emit_build_cfgs() {
    const CABAC_ENV: &str = "ENABLE_CABAC";
    const TRELLIS_ENV: &str = "ENABLE_TRELLIS";
    println!("cargo:rustc-check-cfg=cfg(norito_enable_cabac)");
    println!("cargo:rustc-check-cfg=cfg(norito_enable_trellis)");
    println!("cargo:rustc-check-cfg=cfg(norito_enable_rans_bundles)");
    println!("cargo:rerun-if-env-changed={CABAC_ENV}");
    println!("cargo:rerun-if-env-changed={TRELLIS_ENV}");

    if env_flag_enabled(CABAC_ENV) {
        println!("cargo:rustc-cfg=norito_enable_cabac");
    }
    if env_flag_enabled(TRELLIS_ENV) {
        println!("cargo:rustc-cfg=norito_enable_trellis");
    }
    println!("cargo:rustc-cfg=norito_enable_rans_bundles");
}

fn env_flag_enabled(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            !trimmed.is_empty() && trimmed != "0" && !trimmed.eq_ignore_ascii_case("false")
        }
        Err(_) => false,
    }
}
