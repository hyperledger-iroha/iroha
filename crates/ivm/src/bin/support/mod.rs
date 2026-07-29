use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[allow(dead_code)]
pub(crate) const EXPECTED_DOC_LOCALES: &[&str] = &[
    "am", "ar", "az", "ba", "dz", "es", "fr", "he", "hy", "ja", "ka", "kk", "mn", "my", "pt", "ru",
    "ur", "uz", "zh-hans", "zh-hant",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenerationMode {
    Write,
    Check,
}

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// One fully rendered generated file.
///
/// Construction reads and validates the existing destination, then renders its
/// replacement without publishing it. Callers must construct the complete
/// closed output set before passing it to [`sync_generated_outputs`].
pub(crate) struct GeneratedOutput {
    path: PathBuf,
    original: Option<Vec<u8>>,
    expected: Vec<u8>,
    permissions: Option<fs::Permissions>,
}

impl GeneratedOutput {
    #[allow(dead_code)]
    pub(crate) fn render(
        path: impl Into<PathBuf>,
        renderer: impl FnOnce(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let path = path.into();
        let (original, permissions) = read_regular_destination(&path)?;
        let original = original.ok_or_else(|| {
            format!(
                "cannot render marker-owned missing output {}",
                path.display()
            )
        })?;
        let text = std::str::from_utf8(&original)
            .map_err(|error| format!("read {} as UTF-8: {error}", path.display()))?;
        let expected =
            renderer(text).map_err(|error| format!("render {}: {error}", path.display()))?;
        Ok(Self {
            path,
            original: Some(original),
            expected: expected.into_bytes(),
            permissions,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn exact(
        path: impl Into<PathBuf>,
        expected: impl Into<String>,
    ) -> Result<Self, String> {
        let path = path.into();
        let (original, permissions) = read_regular_destination(&path)?;
        Ok(Self {
            path,
            original,
            expected: expected.into().into_bytes(),
            permissions,
        })
    }

    fn changed(&self) -> bool {
        self.original.as_deref() != Some(self.expected.as_slice())
    }
}

/// Validate every output before staging or publishing any of them, then either
/// check the complete set or publish each changed file by same-directory
/// rename. A validation or staging failure cannot mutate an output. Publication
/// is atomic per file; no cross-file power-loss atomicity is claimed.
pub(crate) fn sync_generated_outputs(
    outputs: &[GeneratedOutput],
    mode: GenerationMode,
    regenerate_command: &str,
) -> Result<Vec<PathBuf>, String> {
    validate_output_set(outputs)?;

    let changed = outputs
        .iter()
        .filter(|output| output.changed())
        .collect::<Vec<_>>();
    if mode == GenerationMode::Check {
        if changed.is_empty() {
            return Ok(Vec::new());
        }
        let stale = changed
            .iter()
            .map(|output| output.path.display().to_string())
            .collect::<Vec<_>>();
        return Err(format!(
            "generated outputs are stale: {stale:?}; run: {regenerate_command}"
        ));
    }
    if changed.is_empty() {
        return Ok(Vec::new());
    }

    // Stage every payload before the first rename. A later write, flush, sync,
    // or destination validation failure therefore leaves all outputs intact.
    let mut staged = Vec::with_capacity(changed.len());
    for output in &changed {
        staged.push(stage_output(output)?);
    }
    validate_output_set(outputs)?;

    let mut updated = Vec::with_capacity(staged.len());
    let mut parent_directories = BTreeSet::new();
    for mut pending in staged {
        fs::rename(&pending.temporary.path, &pending.destination).map_err(|error| {
            format!(
                "atomically publish {} from {}: {error}",
                pending.destination.display(),
                pending.temporary.path.display()
            )
        })?;
        pending.temporary.disarm();
        if let Some(parent) = output_parent(&pending.destination) {
            parent_directories.insert(parent.to_path_buf());
        }
        updated.push(pending.destination);
    }
    for directory in parent_directories {
        sync_directory(&directory)
            .map_err(|error| format!("sync output directory {}: {error}", directory.display()))?;
    }
    Ok(updated)
}

pub(crate) fn parse_generation_mode(
    args: impl IntoIterator<Item = String>,
) -> Result<GenerationMode, String> {
    let mut mode = None;
    for argument in args {
        let requested = match argument.as_str() {
            "--write" => GenerationMode::Write,
            "--check" => GenerationMode::Check,
            _ => {
                return Err(format!(
                    "unknown argument `{argument}`; usage: --write or --check"
                ));
            }
        };
        if mode.replace(requested).is_some() {
            return Err("select exactly one of --write or --check".to_owned());
        }
    }
    mode.ok_or_else(|| "select exactly one of --write or --check".to_owned())
}

struct TemporaryFile {
    path: PathBuf,
}

impl TemporaryFile {
    fn disarm(&mut self) {
        self.path = PathBuf::new();
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

struct StagedOutput {
    destination: PathBuf,
    temporary: TemporaryFile,
}

fn stage_output(output: &GeneratedOutput) -> Result<StagedOutput, String> {
    let parent = output_parent(&output.path)
        .ok_or_else(|| format!("output has no parent: {}", output.path.display()))?;
    let file_name = output
        .path
        .file_name()
        .ok_or_else(|| format!("output has no file name: {}", output.path.display()))?;
    let (mut file, temporary_path) = create_temporary(parent, file_name)?;
    let temporary = TemporaryFile {
        path: temporary_path,
    };
    file.write_all(&output.expected).map_err(|error| {
        format!(
            "write temporary output for {}: {error}",
            output.path.display()
        )
    })?;
    if let Some(permissions) = &output.permissions {
        file.set_permissions(permissions.clone()).map_err(|error| {
            format!(
                "set temporary output permissions for {}: {error}",
                output.path.display()
            )
        })?;
    }
    file.flush().map_err(|error| {
        format!(
            "flush temporary output for {}: {error}",
            output.path.display()
        )
    })?;
    file.sync_all().map_err(|error| {
        format!(
            "sync temporary output for {}: {error}",
            output.path.display()
        )
    })?;
    drop(file);
    Ok(StagedOutput {
        destination: output.path.clone(),
        temporary,
    })
}

fn create_temporary(parent: &Path, file_name: &OsStr) -> Result<(File, PathBuf), String> {
    for _ in 0..128 {
        let serial = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".{}.{}.tmp", std::process::id(), serial));
        let temporary_path = parent.join(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((file, temporary_path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "create temporary output under {}: {error}",
                    parent.display()
                ));
            }
        }
    }
    Err(format!(
        "could not reserve a unique temporary output under {}",
        parent.display()
    ))
}

fn validate_output_set(outputs: &[GeneratedOutput]) -> Result<(), String> {
    let mut paths = BTreeSet::new();
    for output in outputs {
        if !paths.insert(output.path.clone()) {
            return Err(format!(
                "duplicate generated output destination: {}",
                output.path.display()
            ));
        }
        let (current, _) = read_regular_destination(&output.path)?;
        if current != output.original {
            return Err(format!(
                "generated output changed after rendering: {}",
                output.path.display()
            ));
        }
    }
    Ok(())
}

fn read_regular_destination(
    path: &Path,
) -> Result<(Option<Vec<u8>>, Option<fs::Permissions>), String> {
    let parent =
        output_parent(path).ok_or_else(|| format!("output has no parent: {}", path.display()))?;
    validate_real_directory(parent)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
        Err(error) => {
            return Err(format!(
                "inspect generated output {}: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "generated output must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let contents = fs::read(path)
        .map_err(|error| format!("read generated output {}: {error}", path.display()))?;
    Ok((Some(contents), Some(metadata.permissions())))
}

fn output_parent(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .or_else(|| Some(Path::new(".")))
}

fn validate_real_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect output directory {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "output parent must be a real directory: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn exact_localized_markdown_paths(
    directory: &Path,
    stem: &str,
    include_canonical: bool,
    expected_locales: &[&str],
) -> Result<Vec<PathBuf>, String> {
    let canonical_name = format!("{stem}.md");
    let localized_prefix = format!("{stem}.");
    let mut expected_names = expected_locales
        .iter()
        .map(|locale| format!("{stem}.{locale}.md"))
        .collect::<BTreeSet<_>>();
    if include_canonical {
        expected_names.insert(canonical_name.clone());
    }

    let entries = fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?;
    let mut actual_names = BTreeSet::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("read entry in {}: {error}", directory.display()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("non-UTF-8 entry in {}", directory.display()))?;
        let is_candidate = name == canonical_name
            || name
                .strip_prefix(&localized_prefix)
                .and_then(|suffix| suffix.strip_suffix(".md"))
                .is_some_and(|locale| !locale.is_empty());
        if !is_candidate {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        if !file_type.is_file() {
            return Err(format!(
                "expected regular generated document, found non-file {}",
                entry.path().display()
            ));
        }
        actual_names.insert(name);
    }

    ensure_exact_inventory(
        directory,
        &format!("{stem} locale documents"),
        &expected_names,
        &actual_names,
    )?;
    Ok(expected_names
        .into_iter()
        .map(|name| directory.join(name))
        .collect())
}

#[allow(dead_code)]
pub(crate) fn exact_locale_child_paths(
    localized_root: &Path,
    child_name: &str,
    expected_locales: &[&str],
) -> Result<Vec<PathBuf>, String> {
    let expected_names = expected_locales
        .iter()
        .map(|locale| (*locale).to_owned())
        .collect::<BTreeSet<_>>();
    let entries = fs::read_dir(localized_root)
        .map_err(|error| format!("read {}: {error}", localized_root.display()))?;
    let mut actual_names = BTreeSet::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("read entry in {}: {error}", localized_root.display()))?;
        let locale = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("non-UTF-8 locale entry in {}", localized_root.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        if !file_type.is_dir() {
            return Err(format!(
                "expected real locale directory, found non-directory {}",
                entry.path().display()
            ));
        }
        actual_names.insert(locale);
    }

    ensure_exact_inventory(
        localized_root,
        &format!("locale directories containing {child_name}"),
        &expected_names,
        &actual_names,
    )?;
    expected_names
        .into_iter()
        .map(|locale| {
            let child = localized_root.join(locale).join(child_name);
            let metadata = fs::symlink_metadata(&child)
                .map_err(|error| format!("inspect {}: {error}", child.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "expected regular generated document, found non-file {}",
                    child.display()
                ));
            }
            Ok(child)
        })
        .collect()
}

fn ensure_exact_inventory(
    root: &Path,
    inventory_name: &str,
    expected: &BTreeSet<String>,
    actual: &BTreeSet<String>,
) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }
    let missing = expected.difference(actual).cloned().collect::<Vec<_>>();
    let unexpected = actual.difference(expected).cloned().collect::<Vec<_>>();
    Err(format!(
        "{inventory_name} under {} do not match the fixed release inventory; missing={missing:?}, unexpected={unexpected:?}",
        root.display()
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        EXPECTED_DOC_LOCALES, GeneratedOutput, GenerationMode, exact_locale_child_paths,
        exact_localized_markdown_paths, parse_generation_mode, sync_generated_outputs,
    };

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn temp_directory(label: &str) -> std::path::PathBuf {
        let serial = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ivm-generator-support-{}-{serial}-{label}",
            std::process::id()
        ))
    }

    #[test]
    fn generation_mode_is_exactly_one_known_argument() {
        assert_eq!(
            parse_generation_mode(["--check".to_owned()]),
            Ok(GenerationMode::Check)
        );
        assert_eq!(
            parse_generation_mode(["--write".to_owned()]),
            Ok(GenerationMode::Write)
        );
        assert!(parse_generation_mode(Vec::new()).is_err());
        assert!(parse_generation_mode(["--check".to_owned(), "--write".to_owned()]).is_err());
        assert!(parse_generation_mode(["--write".to_owned(), "--write".to_owned()]).is_err());
        assert!(parse_generation_mode(["--unknown".to_owned()]).is_err());
    }

    #[test]
    fn release_locale_inventory_is_sorted_unique_and_complete() {
        assert_eq!(EXPECTED_DOC_LOCALES.len(), 20);
        assert!(
            EXPECTED_DOC_LOCALES
                .windows(2)
                .all(|window| window[0] < window[1])
        );
    }

    #[test]
    fn complete_output_set_is_checked_then_published_atomically_per_file() {
        let directory = temp_directory("publish");
        fs::create_dir_all(&directory).expect("create test directory");
        let first = directory.join("first.md");
        let second = directory.join("second.md");
        fs::write(&first, "old first\n").expect("write first fixture");
        fs::write(&second, "old second\n").expect("write second fixture");
        let outputs = [
            GeneratedOutput::exact(&first, "new first\n").expect("prepare first output"),
            GeneratedOutput::exact(&second, "new second\n").expect("prepare second output"),
        ];

        assert!(
            sync_generated_outputs(&outputs, GenerationMode::Check, "generator --write").is_err()
        );
        assert_eq!(
            fs::read_to_string(&first).expect("read first after check"),
            "old first\n"
        );
        assert_eq!(
            fs::read_to_string(&second).expect("read second after check"),
            "old second\n"
        );

        let updated = sync_generated_outputs(&outputs, GenerationMode::Write, "generator --write")
            .expect("publish complete output set");
        assert_eq!(updated, [first.clone(), second.clone()]);
        assert_eq!(
            fs::read_to_string(&first).expect("read published first"),
            "new first\n"
        );
        assert_eq!(
            fs::read_to_string(&second).expect("read published second"),
            "new second\n"
        );
        assert!(
            fs::read_dir(&directory)
                .expect("read output directory")
                .all(|entry| !entry
                    .expect("read output entry")
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp"))
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn exact_output_can_atomically_create_a_missing_destination() {
        let directory = temp_directory("create");
        fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join("generated.rs");
        let outputs =
            [GeneratedOutput::exact(&path, "generated\n").expect("prepare missing output")];

        assert!(
            sync_generated_outputs(&outputs, GenerationMode::Check, "generator --write").is_err()
        );
        assert!(!path.exists());
        assert_eq!(
            sync_generated_outputs(&outputs, GenerationMode::Write, "generator --write")
                .expect("publish missing output"),
            [path.clone()]
        );
        assert_eq!(
            fs::read_to_string(&path).expect("read generated output"),
            "generated\n"
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn late_renderer_failure_leaves_earlier_output_unchanged() {
        let directory = temp_directory("late-renderer");
        fs::create_dir_all(&directory).expect("create test directory");
        let first = directory.join("first.md");
        let second = directory.join("second.md");
        fs::write(&first, "begin\nstale\nend\n").expect("write first fixture");
        fs::write(&second, "malformed\n").expect("write second fixture");
        let before = fs::read(&first).expect("snapshot first fixture");

        let first_output =
            GeneratedOutput::render(&first, |text| Ok(text.replace("stale", "current")))
                .expect("render first output");
        let late_error =
            GeneratedOutput::render(&second, |_| Err("required marker missing".to_owned()));
        assert!(late_error.is_err());
        drop(first_output);
        assert_eq!(
            fs::read(&first).expect("read first after late render failure"),
            before
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn late_destination_validation_failure_is_nonmutating() {
        use std::os::unix::fs::symlink;

        let directory = temp_directory("late-destination");
        fs::create_dir_all(&directory).expect("create test directory");
        let first = directory.join("first.md");
        let second = directory.join("second.md");
        let symlink_target = directory.join("target.md");
        fs::write(&first, "old first\n").expect("write first fixture");
        fs::write(&second, "old second\n").expect("write second fixture");
        fs::write(&symlink_target, "untouched\n").expect("write symlink target");
        let first_before = fs::read(&first).expect("snapshot first fixture");
        let outputs = [
            GeneratedOutput::exact(&first, "new first\n").expect("prepare first output"),
            GeneratedOutput::exact(&second, "new second\n").expect("prepare second output"),
        ];

        fs::remove_file(&second).expect("remove second output");
        symlink(&symlink_target, &second).expect("replace second output with symlink");
        assert!(
            sync_generated_outputs(&outputs, GenerationMode::Write, "generator --write").is_err()
        );
        assert_eq!(
            fs::read(&first).expect("read first after late validation failure"),
            first_before
        );
        assert_eq!(
            fs::read_to_string(&symlink_target).expect("read symlink target"),
            "untouched\n"
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn flat_locale_inventory_rejects_missing_and_unexpected_documents() {
        let directory = temp_directory("flat");
        fs::create_dir_all(&directory).expect("create test directory");
        for name in ["ivm_header.md", "ivm_header.am.md", "ivm_header.zh-hant.md"] {
            fs::write(directory.join(name), "fixture").expect("write fixture");
        }

        let paths =
            exact_localized_markdown_paths(&directory, "ivm_header", true, &["am", "zh-hant"])
                .expect("exact inventory");
        assert_eq!(
            paths,
            [
                directory.join("ivm_header.am.md"),
                directory.join("ivm_header.md"),
                directory.join("ivm_header.zh-hant.md"),
            ]
        );

        fs::remove_file(directory.join("ivm_header.am.md")).expect("remove expected fixture");
        assert!(
            exact_localized_markdown_paths(&directory, "ivm_header", true, &["am", "zh-hant"])
                .is_err()
        );
        fs::write(directory.join("ivm_header.am.md"), "fixture").expect("restore fixture");
        fs::write(directory.join("ivm_header.es.md"), "fixture").expect("write unexpected fixture");
        assert!(
            exact_localized_markdown_paths(&directory, "ivm_header", true, &["am", "zh-hant"])
                .is_err()
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn nested_locale_inventory_rejects_missing_and_unexpected_documents() {
        let root = temp_directory("nested");
        for locale in ["am", "zh-hant"] {
            let directory = root.join(locale);
            fs::create_dir_all(&directory).expect("create locale directory");
            fs::write(directory.join("ivm.md"), "fixture").expect("write fixture");
        }

        let paths =
            exact_locale_child_paths(&root, "ivm.md", &["am", "zh-hant"]).expect("exact inventory");
        assert_eq!(paths, [root.join("am/ivm.md"), root.join("zh-hant/ivm.md")]);

        fs::remove_file(root.join("am/ivm.md")).expect("remove expected fixture");
        assert!(exact_locale_child_paths(&root, "ivm.md", &["am", "zh-hant"]).is_err());
        fs::write(root.join("am/ivm.md"), "fixture").expect("restore fixture");
        fs::create_dir_all(root.join("es")).expect("create unexpected locale directory");
        assert!(exact_locale_child_paths(&root, "ivm.md", &["am", "zh-hant"]).is_err());

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn locale_inventory_rejects_symlinked_documents() {
        use std::os::unix::fs::symlink;

        let root = temp_directory("symlink");
        let directory = root.join("am");
        fs::create_dir_all(&directory).expect("create locale directory");
        let target = directory.join("target.md");
        fs::write(&target, "fixture").expect("write symlink target");
        symlink(&target, directory.join("ivm.md")).expect("create symlink");

        assert!(exact_locale_child_paths(&root, "ivm.md", &["am"]).is_err());
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
