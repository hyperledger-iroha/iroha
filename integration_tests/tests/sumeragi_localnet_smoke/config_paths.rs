fn collect_config_paths(root: &Path, output: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_config_paths(&path, output);
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.contains("config")
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            output.push(path);
        }
    }
}

fn config_fingerprint(root: &Path) -> Result<Option<String>> {
    if !root.exists() {
        return Ok(None);
    }
    let mut paths = Vec::new();
    collect_config_paths(root, &mut paths);
    if paths.is_empty() {
        return Ok(None);
    }
    paths.sort();
    let mut hasher = Blake3Hasher::new();
    for path in paths {
        hasher.update(path.to_string_lossy().as_bytes());
        let contents = fs::read(&path).wrap_err_with(|| format!("read {}", path.display()))?;
        hasher.update(&contents);
    }
    Ok(Some(hasher.finalize().to_hex().to_string()))
}
