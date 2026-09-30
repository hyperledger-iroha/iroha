//! Exact local Git source capture shared by public-reset export and admission.

use super::*;
use std::{
    io::BufReader,
    process::{Child, ChildStdin, ChildStdout, Stdio},
};

const SYMLINK_MODE: u16 = 0o120000;
const GITLINK_MODE: u16 = 0o160000;
const MAX_LINK_BYTES: usize = 4096;

// One process serves the whole source tree. Requests and responses are
// interleaved so neither pipe can fill while waiting for the other endpoint.
struct GitBlobReader {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    finished: bool,
}

impl GitBlobReader {
    fn new(root: &Path) -> Result<Self> {
        let mut child = git_command(root)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .wrap_err("failed to start fixed Git blob reader")?;
        let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(eyre!("Git blob reader pipes are unavailable"));
        };
        Ok(Self {
            child,
            input: Some(input),
            output: BufReader::new(output),
            finished: false,
        })
    }

    fn verify(&mut self, object: &str, size: u64, digest: &str) -> Result<()> {
        validate_lower_hex("requested Git blob SHA-1", object, 40)?;
        let input = self
            .input
            .as_mut()
            .ok_or_else(|| eyre!("Git blob reader is closed"))?;
        writeln!(input, "{object}").wrap_err("failed to request indexed Git blob")?;
        input.flush().wrap_err("failed to flush Git blob request")?;
        if read_blob_digest(&mut self.output, object, size)? != digest {
            return Err(eyre!("source bytes differ from the exact indexed Git blob"));
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        self.input.take();
        let mut extra = [0];
        if self.output.read(&mut extra)? != 0 {
            return Err(eyre!("Git blob reader returned unexpected trailing output"));
        }
        let status = self
            .child
            .wait()
            .wrap_err("failed to reap Git blob reader")?;
        self.finished = true;
        if !status.success() {
            return Err(eyre!("Git blob reader failed"));
        }
        Ok(())
    }
}

impl Drop for GitBlobReader {
    fn drop(&mut self) {
        if !self.finished {
            // Only this reader's child is owned here; failed capture must not
            // leave a Git process blocked on a pipe after the caller returns.
            self.input.take();
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn read_blob_digest(reader: &mut impl Read, object: &str, expected_size: u64) -> Result<String> {
    if expected_size > MAX_SOURCE_FILE_BYTES {
        return Err(eyre!("indexed Git blob exceeds the V1 source size bound"));
    }
    let mut header = Vec::with_capacity(128);
    for _ in 0..128 {
        let mut byte = [0];
        reader
            .read_exact(&mut byte)
            .wrap_err("truncated Git blob header")?;
        header.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    if header.last() != Some(&b'\n') {
        return Err(eyre!("Git blob header exceeds its bound"));
    }
    let text = std::str::from_utf8(&header[..header.len() - 1])
        .wrap_err("Git blob header is not UTF-8")?;
    let fields: Vec<&str> = text.split(' ').collect();
    if fields.len() != 3
        || fields[0] != object
        || fields[1] != "blob"
        || fields[2] != expected_size.to_string()
    {
        return Err(eyre!(
            "Git blob response has a different identity, type or size"
        ));
    }
    let mut digest = Sha256::new();
    let mut remaining = expected_size;
    let mut buffer = [0; 64 * 1024];
    while remaining != 0 {
        let count = usize::try_from(remaining.min(buffer.len() as u64))?;
        reader
            .read_exact(&mut buffer[..count])
            .wrap_err("truncated Git blob content")?;
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    let mut terminator = [0];
    reader
        .read_exact(&mut terminator)
        .wrap_err("missing Git blob terminator")?;
    if terminator != [b'\n'] {
        return Err(eyre!("Git blob response has an invalid terminator"));
    }
    Ok(hex::encode(digest.finalize()))
}

#[derive(Debug, PartialEq, Eq)]
struct IndexedSource {
    mode: String,
    object: String,
}

fn parse_index(bytes: &[u8]) -> Result<BTreeMap<String, IndexedSource>> {
    let mut entries = BTreeMap::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let record = std::str::from_utf8(record).wrap_err("Git index record is not UTF-8")?;
        let (metadata, path) = record
            .split_once('\t')
            .ok_or_else(|| eyre!("Git index record is malformed"))?;
        let fields: Vec<&str> = metadata.split(' ').collect();
        if fields.len() != 3
            || !matches!(fields[0], "100644" | "100755" | "120000" | "160000")
            || fields[2] != "0"
        {
            return Err(eyre!("source index has a merge stage or unsupported mode"));
        }
        validate_source_relative_path(path)?;
        validate_lower_hex("Git source object SHA-1", fields[1], 40)?;
        let entry = IndexedSource {
            mode: fields[0].to_owned(),
            object: fields[1].to_owned(),
        };
        if entries.insert(path.to_owned(), entry).is_some() {
            return Err(eyre!("Git index contains a duplicate source path"));
        }
        if entries.len() > MAX_SOURCE_FILES {
            return Err(eyre!("source index exceeds the V1 file-count bound"));
        }
    }
    if entries.is_empty() {
        return Err(eyre!("source index is empty"));
    }
    Ok(entries)
}

/// Read the exact clean branch, commit and tree without requiring a matching CLI build.
pub(super) fn clean_git_identity(root: &Path) -> Result<[String; 3]> {
    let metadata =
        fs::symlink_metadata(root.join(".git")).wrap_err("source .git directory is unavailable")?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(eyre!("source .git must be one direct directory"));
    }
    let branch = git_text(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let head = git_text(root, &["rev-parse", "--verify", "HEAD"])?;
    let tree = git_text(root, &["rev-parse", "--verify", "HEAD^{tree}"])?;
    validate_lower_hex("source HEAD", &head, 40)?;
    validate_lower_hex("source HEAD tree", &tree, 40)?;
    if branch != SOURCE_BRANCH
        || !git_output(root, &["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty()
    {
        return Err(eyre!(
            "source checkout must be one clean optimizations HEAD/tree"
        ));
    }
    Ok([branch, head, tree])
}

/// Capture every stage-zero entry; no tracked symlink or gitlink is omitted.
pub(super) fn capture_tree(root: &Path) -> Result<Vec<SourceFileV1>> {
    let index_bytes = git_output(root, &["ls-files", "--stage", "-z"])?;
    let indexed = parse_index(&index_bytes)?;
    let mut blobs = GitBlobReader::new(root)?;
    let mut entries = Vec::with_capacity(indexed.len());
    for (relative, indexed) in &indexed {
        let path = root.join(relative);
        let (mode, size, sha256) = match indexed.mode.as_str() {
            "100644" | "100755" => {
                let mode = if indexed.mode == "100755" {
                    0o755
                } else {
                    0o644
                };
                let (mut file, snapshot) = open_pinned_regular(&path, "source closure file")?;
                if snapshot.len > MAX_SOURCE_FILE_BYTES {
                    return Err(eyre!("source closure file exceeds its V1 size bound"));
                }
                #[cfg(unix)]
                if snapshot.mode & 0o7777 != u32::from(mode) {
                    return Err(eyre!("source closure file mode mismatch"));
                }
                let digest = sha256_reader(&mut file, &path)?;
                blobs.verify(&indexed.object, snapshot.len, &digest)?;
                ensure_pinned_unchanged(&path, "source closure file", &file, &snapshot)?;
                (mode, snapshot.len, digest)
            }
            "120000" => capture_symlink(root, relative, &indexed.object, &mut blobs)?,
            "160000" => capture_gitlink(&path, &indexed.object)?,
            _ => return Err(eyre!("source index contains an unsupported mode")),
        };
        entries.push(SourceFileV1 {
            path: relative.clone(),
            mode,
            size,
            git_blob_sha1: indexed.object.clone(),
            sha256,
        });
    }
    blobs.finish()?;
    if git_output(root, &["ls-files", "--stage", "-z"])? != index_bytes {
        return Err(eyre!("source index changed during capture"));
    }
    Ok(entries)
}

fn relative_link_target(relative: &str, target: &str) -> Result<PathBuf> {
    if target.is_empty()
        || target.len() > MAX_LINK_BYTES
        || target.contains('\\')
        || target.as_bytes().contains(&0)
    {
        return Err(eyre!("source symlink target is not bounded relative UTF-8"));
    }
    let mut normalized = Path::new(relative)
        .parent()
        .ok_or_else(|| eyre!("source symlink has no parent"))?
        .to_path_buf();
    for component in Path::new(target).components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => return Err(eyre!("source symlink target escapes the source root")),
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(eyre!("source symlink target is the source root"));
    }
    Ok(normalized)
}

fn require_absent_link_target(path: &Path) -> Result<()> {
    validate_no_symlink_ancestors(path, "source symlink target")?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).wrap_err("cannot inspect source symlink target"),
        Ok(_) => Err(eyre!("V1 source symlink referent must remain absent")),
    }
}

#[cfg(unix)]
fn capture_symlink(
    root: &Path,
    relative: &str,
    object: &str,
    blobs: &mut GitBlobReader,
) -> Result<(u16, u64, String)> {
    let path = root.join(relative);
    validate_no_symlink_ancestors(&path, "source symlink")?;
    let metadata = fs::symlink_metadata(&path).wrap_err("source symlink is unavailable")?;
    let before = file_snapshot(&metadata)?;
    let uid = rustix::process::geteuid().as_raw();
    if !metadata.file_type().is_symlink()
        || before.nlink != 1
        || (before.uid != 0 && before.uid != uid)
    {
        return Err(eyre!("source symlink has unsafe type, owner or link count"));
    }
    let target = fs::read_link(&path).wrap_err("cannot read source symlink")?;
    let text = target
        .to_str()
        .ok_or_else(|| eyre!("source symlink is not UTF-8"))?;
    let referent = root.join(relative_link_target(relative, text)?);
    // V1 admits this repository's unbuilt SDK alias without opening an ignored
    // or external artifact. Materializing its referent changes admission.
    require_absent_link_target(&referent)?;
    let bytes = text.as_bytes();
    let size = u64::try_from(bytes.len())?;
    let digest = sha256_hex(bytes);
    blobs.verify(object, size, &digest)?;
    if file_snapshot(&fs::symlink_metadata(&path)?)? != before || fs::read_link(&path)? != target {
        return Err(eyre!("source symlink changed during capture"));
    }
    require_absent_link_target(&referent)?;
    Ok((SYMLINK_MODE, size, digest))
}

#[cfg(not(unix))]
fn capture_symlink(
    _root: &Path,
    _relative: &str,
    _object: &str,
    _blobs: &mut GitBlobReader,
) -> Result<(u16, u64, String)> {
    Err(eyre!("public-reset source symlinks require Unix"))
}

#[cfg(unix)]
fn capture_gitlink(path: &Path, object: &str) -> Result<(u16, u64, String)> {
    validate_no_symlink_ancestors(path, "source gitlink")?;
    let metadata = fs::symlink_metadata(path).wrap_err("source gitlink is unavailable")?;
    let before = file_snapshot(&metadata)?;
    let uid = rustix::process::geteuid().as_raw();
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || before.mode & 0o7777 != 0o755
        || (before.uid != 0 && before.uid != uid)
    {
        return Err(eyre!(
            "source gitlink must be one direct owner-controlled 0755 directory"
        ));
    }
    let directory = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    if file_snapshot(&directory.metadata()?)? != before {
        return Err(eyre!("source gitlink changed before opening"));
    }
    // The indexed commit identifies the uninitialized submodule. Any local
    // checkout content would be an additional source input and is rejected.
    if fs::read_dir(path)?.next().transpose()?.is_some() {
        return Err(eyre!("V1 source gitlink directory must be empty"));
    }
    if file_snapshot(&directory.metadata()?)? != before
        || file_snapshot(&fs::symlink_metadata(path)?)? != before
    {
        return Err(eyre!("source gitlink changed during capture"));
    }
    Ok((
        GITLINK_MODE,
        u64::try_from(object.len())?,
        sha256_hex(object.as_bytes()),
    ))
}

#[cfg(not(unix))]
fn capture_gitlink(_path: &Path, _object: &str) -> Result<(u16, u64, String)> {
    Err(eyre!("public-reset source gitlinks require Unix"))
}

fn assemble_manifest(
    identity: [String; 3],
    entries: Vec<SourceFileV1>,
) -> Result<SourceManifestV1> {
    let lock = entries
        .iter()
        .find(|entry| entry.path == "Cargo.lock")
        .filter(|entry| entry.mode == 0o644)
        .ok_or_else(|| eyre!("source closure omits canonical Cargo.lock"))?;
    let [branch, head_commit_sha1, head_tree_sha1] = identity;
    let mut manifest = SourceManifestV1 {
        schema: SOURCE_MANIFEST_SCHEMA_V1.to_owned(),
        branch,
        head_commit_sha1,
        head_tree_sha1,
        cargo_lock_sha256: lock.sha256.clone(),
        tracked_files: entries,
        untracked_files: Vec::new(),
        closure_sha256: String::new(),
    };
    manifest.closure_sha256 = source_closure_sha256(&manifest);
    Ok(manifest)
}

/// Print the exact Norito type consumed by admission; this grants no release authority.
pub(super) fn export_manifest(root: &Path, output: &mut impl Write) -> Result<()> {
    validate_absolute_normal_path(root, "source root")?;
    validate_source_root(root)?;
    let identity = clean_git_identity(root)?;
    let entries = capture_tree(root)?;
    if clean_git_identity(root)? != identity {
        return Err(eyre!("source identity changed during manifest capture"));
    }
    let manifest = assemble_manifest(identity, entries)?;
    let rendered = json::to_json(&manifest).wrap_err("failed to encode source manifest")?;
    if u64::try_from(rendered.len())? + 1 > MAX_JSON_BYTES {
        return Err(eyre!("source manifest exceeds the V1 JSON bound"));
    }
    writeln!(output, "{rendered}").wrap_err("failed to write source manifest")
}

#[cfg(all(test, unix))]
#[path = "taira_public_reset_source_tests.rs"]
mod tests;
