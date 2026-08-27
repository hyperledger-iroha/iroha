//! Atomic publication for non-secret Kagami output files.

use color_eyre::eyre::{Result, WrapErr as _};
#[cfg(unix)]
use std::fs::File;
use std::{
    fs,
    io::{BufWriter, ErrorKind, Write as _},
    path::{Path, PathBuf},
};

pub(crate) fn resolve_output_file(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent)
        .wrap_err_with(|| format!("resolve output directory {}", parent.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| color_eyre::eyre::eyre!("output must name a file: {}", path.display()))?;
    let resolved = parent.join(file_name);
    reject_final_symlink(&resolved)?;
    Ok(resolved)
}

fn reject_final_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(color_eyre::eyre::eyre!(
            "refusing to replace symbolic-link output: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).wrap_err_with(|| format!("inspect output destination {}", path.display()))
        }
    }
}

/// Resolve an output file outside a protected directory without following a final symlink.
pub(crate) fn resolve_outside_directory(
    protected_directory: &Path,
    output: &Path,
    protected_label: &str,
) -> Result<PathBuf> {
    let protected_directory = fs::canonicalize(protected_directory).wrap_err_with(|| {
        format!(
            "resolve protected {protected_label} {}",
            protected_directory.display()
        )
    })?;
    let resolved = resolve_output_file(output)?;
    if resolved.starts_with(&protected_directory) {
        return Err(color_eyre::eyre::eyre!(
            "refusing to write output inside the {protected_label}: {}",
            resolved.display()
        ));
    }
    Ok(resolved)
}

/// Render a file beside its destination, synchronize it, and atomically publish it.
pub(crate) fn write_file(
    path: &Path,
    temporary_prefix: &str,
    render: impl FnOnce(&mut dyn std::io::Write) -> Result<()>,
) -> Result<()> {
    let path = resolve_output_file(path)?;
    let parent = path
        .parent()
        .expect("resolved output always has its canonical parent");
    let mut temporary = tempfile::Builder::new()
        .prefix(temporary_prefix)
        .tempfile_in(parent)
        .wrap_err_with(|| format!("stage output beside {}", path.display()))?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        render(&mut writer)?;
        writer
            .flush()
            .wrap_err_with(|| format!("flush staged output for {}", path.display()))?;
    }
    temporary
        .as_file()
        .sync_all()
        .wrap_err_with(|| format!("sync staged output for {}", path.display()))?;
    // Recheck immediately before publication so a final symlink introduced
    // while rendering also fails closed. Atomic rename never follows the final
    // component, but replacing an operator-managed symlink would still be an
    // unexpected state change.
    reject_final_symlink(&path)?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .wrap_err_with(|| format!("publish output atomically to {}", path.display()))?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .wrap_err_with(|| format!("sync output directory {}", parent.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::eyre::eyre;

    #[test]
    fn failed_render_preserves_existing_output() {
        let directory = tempfile::tempdir().expect("create atomic-output directory");
        let output = directory.path().join("output.txt");
        fs::write(&output, b"previous").expect("seed output");
        let _error = write_file(&output, ".kagami-test-", |_writer| {
            Err(eyre!("synthetic render failure"))
        })
        .expect_err("failed rendering must not publish");
        assert_eq!(
            fs::read(output).expect("read preserved output"),
            b"previous"
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("list output directory")
                .count(),
            1
        );
    }

    #[test]
    fn protected_directory_output_is_rejected() {
        let protected = tempfile::tempdir().expect("create protected directory");
        let output = protected.path().join("output.txt");
        let error = resolve_outside_directory(protected.path(), &output, "test directory")
            .expect_err("protected-directory output must fail");
        assert!(error.to_string().contains("inside the test directory"));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_output_is_rejected_without_changing_its_target() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("create symbolic-link output directory");
        let target = directory.path().join("target.txt");
        let output = directory.path().join("output.txt");
        fs::write(&target, b"target contents").expect("seed symlink target");
        symlink(&target, &output).expect("create output symlink");

        let error = write_file(&output, ".kagami-test-", |writer| {
            writer.write_all(b"replacement").map_err(Into::into)
        })
        .expect_err("symbolic-link output must fail closed");
        assert!(error.to_string().contains("symbolic-link output"));
        assert_eq!(
            fs::read(&target).expect("read preserved symlink target"),
            b"target contents"
        );
        assert!(
            fs::symlink_metadata(&output)
                .expect("inspect preserved output symlink")
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_created_during_render_is_rejected_before_publication() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("create raced symbolic-link output directory");
        let target = directory.path().join("target.txt");
        let output = directory.path().join("output.txt");
        fs::write(&target, b"target contents").expect("seed raced symlink target");

        let error = write_file(&output, ".kagami-test-", |writer| {
            symlink(&target, &output)?;
            writer.write_all(b"replacement").map_err(Into::into)
        })
        .expect_err("symbolic link introduced during rendering must fail closed");
        assert!(error.to_string().contains("symbolic-link output"));
        assert_eq!(
            fs::read(&target).expect("read preserved raced symlink target"),
            b"target contents"
        );
        assert!(
            fs::symlink_metadata(&output)
                .expect("inspect raced output symlink")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("list raced output directory")
                .count(),
            2,
            "failed publication must remove its staging file"
        );
    }
}
