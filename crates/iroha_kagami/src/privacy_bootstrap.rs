//! Native privacy catalog inspection and secret-free Taira release inputs.
use crate::{Outcome, RunArgs};
use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core_privacy::privacy_profiles::compiled_privacy_profile_catalog_v1;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write},
    path::{Component, Path, PathBuf},
};
mod release;
/// Inspect native profiles or prepare reviewed first-release Taira inputs.
#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the complete native catalog, including concrete unavailable implementations.
    Catalog,
    /// Validate a reviewed Taira NEVO genesis source template without creating release artifacts.
    #[command(name = "validate-taira-nevo-review-v1")]
    ValidateTairaNevoReviewV1(release::ValidateTairaNevoReviewV1Args),
    /// Compose a secret-free Taira release plan, config, and non-signable genesis source template.
    #[command(name = "render-taira-release-v1")]
    RenderTairaReleaseV1(Box<release::RenderTairaReleaseV1Args>),
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        match self.command {
            Command::Catalog => writer.write_all(&catalog_json_v1()?)?,
            Command::ValidateTairaNevoReviewV1(args) => {
                release::validate_taira_nevo_review_v1(&args, writer)?;
            }
            Command::RenderTairaReleaseV1(args) => {
                release::render_taira_release_v1(&args, writer)?;
            }
        }
        Ok(())
    }
}
fn catalog_json_v1() -> color_eyre::Result<Vec<u8>> {
    let catalog = compiled_privacy_profile_catalog_v1()
        .map_err(|source| eyre!("local compiled privacy catalog is invalid: {source}"))?;
    catalog
        .validate()
        .map_err(|source| eyre!("native catalog is invalid: {source}"))?;
    let mut json = norito::json::to_json(&catalog).wrap_err("cannot encode native catalog")?;
    json.push('\n');
    Ok(json.into_bytes())
}
fn read_bounded(path: &Path, max_bytes: u64, description: &str) -> color_eyre::Result<Vec<u8>> {
    let before =
        fs::symlink_metadata(path).wrap_err_with(|| format!("failed to inspect {description}"))?;
    if !before.is_file() || before.file_type().is_symlink() {
        bail!("{description} must be one non-symlink regular file");
    }
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::nlink(&before) != 1 {
        bail!("{description} must have exactly one filesystem link");
    }
    if before.len() > max_bytes {
        bail!("{description} exceeds the fixed {max_bytes}-byte limit");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open {description}"))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {description}"))?;
    if !crate::secure_fs::same_single_link_input_snapshot(&before, &opened) {
        bail!("{description} changed before its immutable snapshot was opened");
    }
    let mut bytes = Vec::new();
    (&file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read {description}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        bail!("{description} exceeds the fixed {max_bytes}-byte limit");
    }
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect opened {description}"))?;
    if !crate::secure_fs::same_single_link_input_snapshot(&opened, &after) {
        bail!("{description} changed while its immutable snapshot was read");
    }
    Ok(bytes)
}

fn create_new_file(path: &Path, description: &str) -> color_eyre::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    options
        .open(path)
        .wrap_err_with(|| format!("failed to create new {description} at `{}`", path.display()))
}
fn resolved_new_output_path_v1(path: &Path) -> color_eyre::Result<PathBuf> {
    let Some(Component::Normal(file_name)) = path.components().next_back() else {
        bail!("new artifact output must end in one normal file name");
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let resolved_parent = fs::canonicalize(parent).wrap_err_with(|| {
        format!(
            "failed to resolve artifact output parent `{}`",
            parent.display()
        )
    })?;
    if !resolved_parent.is_dir() {
        bail!("artifact output parent must be a directory");
    }
    Ok(resolved_parent.join(file_name))
}
fn remove_created_file_if_unchanged_v1(path: &Path, file: &File) {
    let Ok(named) = fs::symlink_metadata(path) else {
        return;
    };
    let Ok(opened) = file.metadata() else {
        return;
    };
    if crate::secure_fs::same_single_link_input_snapshot(&named, &opened) {
        let _ = fs::remove_file(path);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_preserves_actual_availability_and_canonical_order() {
        let bytes = catalog_json_v1().expect("inspect actual native catalog");
        let observed: iroha_data_model::privacy::PrivacyCompiledProfileCatalogV1 =
            norito::json::from_slice(&bytes).expect("catalog JSON");
        let expected = compiled_privacy_profile_catalog_v1().expect("actual native catalog");
        assert_eq!(observed, expected);
        observed
            .validate()
            .expect("complete canonical ordered catalog");
        assert_eq!(
            observed.protocols.len(),
            iroha_data_model::privacy::PrivacyProtocolIdV1::COUNT
        );
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_symlinks_hardlinks_and_oversized_inputs() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().expect("tempdir");
        let source = directory.path().join("source.json");
        let symlink_path = directory.path().join("symlink.json");
        let hardlink_path = directory.path().join("hardlink.json");
        fs::write(&source, b"{}\n").expect("write source");
        symlink(&source, &symlink_path).expect("create symlink");
        assert!(
            read_bounded(&symlink_path, 16, "symlinked test input")
                .expect_err("reject symlink")
                .to_string()
                .contains("non-symlink regular file")
        );
        fs::hard_link(&source, &hardlink_path).expect("create hardlink");
        assert!(
            read_bounded(&source, 16, "hardlinked test input")
                .expect_err("reject hardlink")
                .to_string()
                .contains("exactly one filesystem link")
        );
        fs::remove_file(&hardlink_path).expect("remove hardlink");
        assert!(
            read_bounded(&source, 2, "oversized test input")
                .expect_err("reject oversize")
                .to_string()
                .contains("fixed 2-byte limit")
        );
        assert_eq!(
            read_bounded(&source, 3, "bounded test input").expect("read exact bound"),
            b"{}\n"
        );
    }
    #[test]
    fn cleanup_never_removes_a_replacement_path() {
        let directory = tempfile::tempdir().expect("tempdir");
        let original = directory.path().join("artifact.json");
        let moved = directory.path().join("created-artifact.json");
        let created = create_new_file(&original, "test artifact").expect("create artifact");
        fs::rename(&original, &moved).expect("move opened artifact");
        fs::write(&original, b"replacement").expect("install replacement path");
        remove_created_file_if_unchanged_v1(&original, &created);
        assert_eq!(
            fs::read(&original).expect("read replacement"),
            b"replacement",
            "cleanup must never unlink a path that no longer names the created inode"
        );
        assert!(moved.exists());
    }
}
