//! Independent consumers for the reviewed BOI/Taira publication tree formats.
//!
//! The byte domains and framing in this module intentionally match:
//! - `supervise_taira_distinct_uid_build.py`;
//! - `provision_taira_build_closure.py`.
//!
//! Do not change these algorithms without changing and independently reviewing
//! both the privileged producer and this unprivileged consumer.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs::{File, Metadata},
    io::{Read as _, Seek as _, SeekFrom},
    os::{
        fd::{AsRawFd as _, OwnedFd},
        unix::{ffi::OsStrExt as _, fs::MetadataExt as _},
    },
    path::{Component, Path, PathBuf},
};

#[cfg(target_os = "macos")]
use std::fs;

use color_eyre::eyre::{Result, WrapErr as _, bail, eyre};
use rustix::fs::{
    AtFlags, Dir, FileType as RustixFileType, Mode, OFlags, open, openat, readlinkat, statat,
};

pub(super) const CANDIDATE_ARTIFACT_DOMAIN: &[u8] =
    b"iroha.kagemusha.root-published-build-artifact.v1\0";
pub(super) const GENERATED_ARTIFACT_DOMAIN: &[u8] =
    b"iroha.kagemusha.root-published-generated-candidate.v1\0";
pub(super) const GENERATED_SUBTREE_DOMAIN: &[u8] =
    b"iroha.kagemusha.root-published-generated-subtree.v1\0";
pub(super) const PRODUCTION_CLOSURE_TREE_DOMAIN: &[u8] =
    b"iroha.kagemusha.production-build-closure.v1\0";
pub(super) const PRODUCTION_PROVENANCE_FILE_NAME: &[u8] = b"production-build-closure.json";

const MAX_TREE_ENTRIES: usize = 1_000_000;
const MAX_TREE_DEPTH: usize = 256;
const MAX_RELATIVE_PATH_BYTES: usize = 16 * 1024;

/// Streaming SHA-256 used so multi-gigabyte production artifacts are never
/// buffered in memory. This is the standard FIPS 180-4 compression function.
#[derive(Clone)]
pub(super) struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length_bytes: u64,
}

impl Sha256 {
    const ROUND_CONSTANTS: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];

    pub(super) fn new() -> Self {
        Self {
            state: [
                0x6a09_e667,
                0xbb67_ae85,
                0x3c6e_f372,
                0xa54f_f53a,
                0x510e_527f,
                0x9b05_688c,
                0x1f83_d9ab,
                0x5be0_cd19,
            ],
            buffer: [0; 64],
            buffered: 0,
            length_bytes: 0,
        }
    }

    pub(super) fn update(&mut self, mut bytes: &[u8]) {
        self.length_bytes = self
            .length_bytes
            .checked_add(u64::try_from(bytes.len()).expect("slice length fits u64"))
            .expect("SHA-256 input length exceeds u64 bytes");
        if self.buffered != 0 {
            let needed = 64 - self.buffered;
            let copied = needed.min(bytes.len());
            self.buffer[self.buffered..self.buffered + copied].copy_from_slice(&bytes[..copied]);
            self.buffered += copied;
            bytes = &bytes[copied..];
            if self.buffered < 64 {
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffered = 0;
        }
        let mut chunks = bytes.chunks_exact(64);
        for chunk in &mut chunks {
            let block: &[u8; 64] = chunk.try_into().expect("exact SHA-256 block");
            self.compress(block);
        }
        let remainder = chunks.remainder();
        self.buffer[..remainder.len()].copy_from_slice(remainder);
        self.buffered = remainder.len();
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut schedule = [0_u32; 64];
        for (word, bytes) in schedule[..16].iter_mut().zip(block.chunks_exact(4)) {
            *word = u32::from_be_bytes(bytes.try_into().expect("four-byte SHA-256 word"));
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(Self::ROUND_CONSTANTS[index])
                .wrapping_add(schedule[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(value);
        }
    }

    pub(super) fn finalize(mut self) -> [u8; 32] {
        let bit_length = self
            .length_bytes
            .checked_mul(8)
            .expect("SHA-256 input length exceeds its bit-length field");
        let mut padding = [0_u8; 128];
        padding[0] = 0x80;
        let padding_length = if self.buffered < 56 {
            56 - self.buffered
        } else {
            120 - self.buffered
        };
        self.update(&padding[..padding_length]);
        self.update(&bit_length.to_be_bytes());
        debug_assert_eq!(self.buffered, 0);
        let mut output = [0_u8; 32];
        for (slot, word) in output.chunks_exact_mut(4).zip(self.state) {
            slot.copy_from_slice(&word.to_be_bytes());
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum TreeEntryKind {
    Directory,
    File,
    Symlink,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TreeEntry {
    pub(super) relative: Vec<u8>,
    pub(super) kind: TreeEntryKind,
    pub(super) mode: u32,
    pub(super) size: u64,
    pub(super) sha256: [u8; 32],
    pub(super) symlink_target: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(super) struct HardenedTreePolicy {
    pub(super) trusted_uid: u32,
    pub(super) root_mode: u32,
    pub(super) directory_mode: u32,
    pub(super) allowed_file_modes: &'static [u32],
    pub(super) allow_internal_symlinks: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

impl FileIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Result<Self> {
        Ok(Self {
            device: u64::try_from(stat.st_dev)
                .map_err(|_| eyre!("filesystem device identifier does not fit u64"))?,
            inode: u64::try_from(stat.st_ino)
                .map_err(|_| eyre!("filesystem inode identifier does not fit u64"))?,
        })
    }
}

/// One path-admitted regular file retained as the exact authenticated inode.
///
/// Security-sensitive callers keep this handle alive through every consumer.
/// They never authenticate a pathname, close it, and later reopen that mutable
/// pathname as authority.
pub(super) struct StableFile {
    file: File,
    snapshot: Metadata,
    sha256: String,
    size: u64,
}

/// One deliberately inheritable duplicate of an authenticated file.
///
/// `rustix::io::dup` leaves `FD_CLOEXEC` clear. Keeping this value alive until
/// `Command::spawn` therefore gives the child an exact descriptor capability.
pub(super) struct InheritedFileDescriptor {
    _descriptor: OwnedFd,
    alias_path: PathBuf,
}

impl InheritedFileDescriptor {
    pub(super) fn alias_path(&self) -> &Path {
        &self.alias_path
    }
}

impl StableFile {
    pub(super) fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Return the exact authenticated bytes from the retained descriptor.
    pub(super) fn read_bytes(&mut self, maximum_bytes: u64) -> Result<Vec<u8>> {
        self.verify_snapshot("before reading exact bytes")?;
        if self.size > maximum_bytes {
            bail!("authenticated file exceeds its byte capture bound");
        }
        self.file
            .seek(SeekFrom::Start(0))
            .wrap_err("rewind authenticated file before byte capture")?;
        let expected_size = usize::try_from(self.size)
            .map_err(|_| eyre!("authenticated file byte length does not fit memory"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(expected_size)
            .map_err(|_| eyre!("unable to reserve authenticated file byte buffer"))?;
        self.file
            .read_to_end(&mut bytes)
            .wrap_err("read authenticated file bytes")?;
        self.verify_snapshot("after reading exact bytes")?;
        if bytes.len() != expected_size {
            bail!("authenticated file length changed while captured");
        }
        let mut digest = Sha256::new();
        digest.update(&bytes);
        if hex::encode(digest.finalize()) != self.sha256 {
            bail!("authenticated file bytes changed after admission");
        }
        self.file
            .seek(SeekFrom::Start(0))
            .wrap_err("rewind authenticated file after byte capture")?;
        Ok(bytes)
    }

    /// Rehash the retained descriptor without consulting its original path.
    pub(super) fn verify_unchanged(&mut self) -> Result<()> {
        self.verify_snapshot("before descriptor rehash")?;
        self.file
            .seek(SeekFrom::Start(0))
            .wrap_err("rewind authenticated file before descriptor rehash")?;
        let mut digest = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            let read = self
                .file
                .read(&mut buffer)
                .wrap_err("rehash authenticated file descriptor")?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
            size = size
                .checked_add(u64::try_from(read).expect("buffer length fits u64"))
                .ok_or_else(|| eyre!("authenticated file length overflow"))?;
            if size > self.size {
                bail!("authenticated file grew while descriptor-bound");
            }
        }
        self.verify_snapshot("after descriptor rehash")?;
        self.file
            .seek(SeekFrom::Start(0))
            .wrap_err("rewind authenticated file after descriptor rehash")?;
        if size != self.size || hex::encode(digest.finalize()) != self.sha256 {
            bail!("authenticated file descriptor bytes changed after admission");
        }
        Ok(())
    }

    /// Duplicate this exact inode into a descriptor inherited by a child.
    pub(super) fn inherited_descriptor(&mut self) -> Result<InheritedFileDescriptor> {
        self.verify_unchanged()?;
        let descriptor =
            rustix::io::dup(&self.file).wrap_err("duplicate authenticated file for child")?;
        let raw = descriptor.as_raw_fd();
        if raw < 3 {
            bail!("authenticated child descriptor aliases a standard stream");
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let alias_path = PathBuf::from(format!("/proc/self/fd/{raw}"));
        #[cfg(target_os = "macos")]
        let alias_path = PathBuf::from(format!("/dev/fd/{raw}"));
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            let _ = raw;
            bail!("descriptor-bound child inputs are unsupported on this POSIX platform");
        }
        Ok(InheritedFileDescriptor {
            _descriptor: descriptor,
            alias_path,
        })
    }

    /// Return an execution name that resolves this exact authenticated inode.
    pub(super) fn descriptor_bound_executable_path(
        &self,
        inherited: &InheritedFileDescriptor,
    ) -> Result<PathBuf> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            Ok(inherited.alias_path.clone())
        }
        #[cfg(target_os = "macos")]
        {
            let path = Path::new("/.vol")
                .join(self.snapshot.dev().to_string())
                .join(self.snapshot.ino().to_string());
            let observed = path
                .metadata()
                .wrap_err("resolve authenticated executable through macOS inode namespace")?;
            if FileIdentity::from_metadata(&observed) != FileIdentity::from_metadata(&self.snapshot)
            {
                bail!("macOS executable inode namespace resolved a different file");
            }
            let _ = inherited;
            Ok(path)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            let _ = inherited;
            bail!("descriptor-bound executable invocation is unsupported on this POSIX platform");
        }
    }

    fn verify_snapshot(&self, phase: &str) -> Result<()> {
        let observed = self
            .file
            .metadata()
            .wrap_err_with(|| format!("inspect authenticated file {phase}"))?;
        if !same_metadata(&self.snapshot, &observed) {
            bail!("authenticated file metadata changed {phase}");
        }
        Ok(())
    }
}

struct DirectoryLink {
    parent: File,
    name: OsString,
    identity: FileIdentity,
}

struct OpenTreeRoot {
    root: File,
    ancestry: Vec<DirectoryLink>,
}

impl OpenTreeRoot {
    fn verify_ancestry(&self, path: &Path) -> Result<()> {
        for link in &self.ancestry {
            let observed = statat(&link.parent, &link.name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)
                .wrap_err_with(|| format!("reinspect tree ancestry for {}", path.display()))?;
            if RustixFileType::from_raw_mode(observed.st_mode) != RustixFileType::Directory
                || FileIdentity::from_stat(&observed)? != link.identity
            {
                bail!(
                    "tree ancestry changed while inventoried: {}",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.size() == right.size()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn open_tree_root(path: &Path) -> Result<OpenTreeRoot> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        bail!("tree root must be one normalized absolute path");
    }
    let mut current = File::from(
        open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)
        .wrap_err("open filesystem root")?,
    );
    let mut ancestry = Vec::new();
    for component in path.components() {
        let name = match component {
            Component::RootDir => continue,
            Component::Normal(name) => name,
            Component::Prefix(_) | Component::CurDir | Component::ParentDir => {
                bail!("tree root contains an ambiguous component")
            }
        };
        let before = statat(&current, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .wrap_err_with(|| format!("inspect tree root component {}", path.display()))?;
        if RustixFileType::from_raw_mode(before.st_mode) != RustixFileType::Directory {
            bail!(
                "tree root contains a symlink or non-directory component: {}",
                path.display()
            );
        }
        let identity = FileIdentity::from_stat(&before)?;
        let child = File::from(
            openat(
                &current,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .wrap_err_with(|| format!("open tree root component {}", path.display()))?,
        );
        let opened = child
            .metadata()
            .wrap_err_with(|| format!("inspect opened tree root {}", path.display()))?;
        let after = statat(&current, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .wrap_err_with(|| format!("reinspect tree root component {}", path.display()))?;
        if !opened.is_dir()
            || FileIdentity::from_metadata(&opened) != identity
            || RustixFileType::from_raw_mode(after.st_mode) != RustixFileType::Directory
            || FileIdentity::from_stat(&after)? != identity
        {
            bail!("tree root changed while opening: {}", path.display());
        }
        ancestry.push(DirectoryLink {
            parent: current,
            name: name.to_os_string(),
            identity,
        });
        current = child;
    }
    let opened = OpenTreeRoot {
        root: current,
        ancestry,
    };
    opened.verify_ancestry(path)?;
    Ok(opened)
}

fn validate_relative(relative: &[u8]) -> Result<()> {
    if relative.is_empty()
        || relative.starts_with(b"/")
        || relative.contains(&0)
        || relative.len() > MAX_RELATIVE_PATH_BYTES
        || relative
            .split(|byte| *byte == b'/')
            .any(|component| matches!(component, b"" | b"." | b".."))
    {
        bail!("tree contains a malformed or oversized relative path");
    }
    Ok(())
}

fn validate_internal_symlink(relative: &[u8], target: &[u8]) -> Result<()> {
    if target.is_empty() || target.starts_with(b"/") || target.contains(&0) {
        bail!(
            "tree symlink is empty or absolute: {}",
            String::from_utf8_lossy(relative)
        );
    }
    let mut parts = relative
        .split(|byte| *byte == b'/')
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    parts.pop();
    for component in target.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                if parts.pop().is_none() {
                    bail!(
                        "tree symlink escapes its root: {}",
                        String::from_utf8_lossy(relative)
                    );
                }
            }
            value => parts.push(value.to_vec()),
        }
    }
    Ok(())
}

fn stable_file_sha256(
    parent: &File,
    name: &OsStr,
    before: &rustix::fs::Stat,
    maximum_bytes: Option<u64>,
) -> Result<(File, [u8; 32], u64, Metadata)> {
    let mut file = File::from(
        openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)
        .wrap_err("open tree file without following links")?,
    );
    let opened_before = file.metadata().wrap_err("inspect opened tree file")?;
    if !opened_before.is_file()
        || FileIdentity::from_metadata(&opened_before) != FileIdentity::from_stat(before)?
        || opened_before.nlink() != 1
        || maximum_bytes.is_some_and(|maximum| opened_before.size() > maximum)
    {
        bail!("tree file changed while opening");
    }
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).wrap_err("read tree file")?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        size = size
            .checked_add(u64::try_from(read).expect("buffer length fits u64"))
            .ok_or_else(|| eyre!("tree file length overflow"))?;
        if maximum_bytes.is_some_and(|maximum| size > maximum) {
            bail!("tree file exceeded its byte bound while hashed");
        }
    }
    let opened_after = file.metadata().wrap_err("reinspect opened tree file")?;
    let parent_after = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(std::io::Error::from)
        .wrap_err("reinspect tree file through its parent")?;
    if !same_metadata(&opened_before, &opened_after)
        || FileIdentity::from_metadata(&opened_after) != FileIdentity::from_stat(&parent_after)?
        || parent_after.st_nlink != 1
        || size != opened_after.size()
    {
        bail!("tree file changed while hashed");
    }
    file.seek(SeekFrom::Start(0))
        .wrap_err("rewind stable tree file after hashing")?;
    Ok((file, digest.finalize(), size, opened_after))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn inventory_directory(
    directory: &File,
    relative_parent: &[u8],
    depth: usize,
    policy: HardenedTreePolicy,
    excluded_relative: Option<&[u8]>,
    entries: &mut Vec<TreeEntry>,
    regular_inodes: &mut BTreeSet<(u64, u64)>,
    entries_seen: &mut usize,
) -> Result<()> {
    if depth > MAX_TREE_DEPTH {
        bail!("tree exceeds its depth bound");
    }
    let directory_before = directory.metadata().wrap_err("inspect tree directory")?;
    let mut stream = Dir::read_from(directory)
        .map_err(std::io::Error::from)
        .wrap_err("enumerate tree directory")?;
    let mut names = Vec::new();
    for entry in &mut stream {
        let entry = entry
            .map_err(std::io::Error::from)
            .wrap_err("read tree directory entry")?;
        let name = entry.file_name().to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        if name.is_empty() || name.contains(&b'/') || name.contains(&0) {
            bail!("tree contains an unsafe entry name");
        }
        names.push(name.to_vec());
    }
    names.sort();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        bail!("tree directory returned a duplicate entry");
    }
    for name in names {
        let relative = if relative_parent.is_empty() {
            name.clone()
        } else {
            [relative_parent, b"/", &name].concat()
        };
        validate_relative(&relative)?;
        if excluded_relative.is_some_and(|excluded| excluded == relative) {
            continue;
        }
        *entries_seen = entries_seen
            .checked_add(1)
            .ok_or_else(|| eyre!("tree entry count overflow"))?;
        if *entries_seen > MAX_TREE_ENTRIES {
            bail!("tree exceeds its entry bound");
        }
        let name = OsStr::from_bytes(&name);
        let before = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .wrap_err("inspect tree entry")?;
        if before.st_uid != policy.trusted_uid || u32::from(before.st_mode) & 0o7000 != 0 {
            bail!(
                "tree entry has untrusted ownership or set-id/sticky mode: {}",
                String::from_utf8_lossy(&relative)
            );
        }
        let mode = u32::from(before.st_mode) & 0o777;
        match RustixFileType::from_raw_mode(before.st_mode) {
            RustixFileType::Directory => {
                if mode != policy.directory_mode {
                    bail!("tree directory mode differs");
                }
                let identity = FileIdentity::from_stat(&before)?;
                let child = File::from(
                    openat(
                        directory,
                        name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(std::io::Error::from)
                    .wrap_err("open tree directory entry")?,
                );
                let opened = child.metadata().wrap_err("inspect opened tree directory")?;
                if !opened.is_dir()
                    || FileIdentity::from_metadata(&opened) != identity
                    || opened.uid() != policy.trusted_uid
                    || opened.mode() & 0o777 != policy.directory_mode
                {
                    bail!("tree directory changed while opening");
                }
                entries.push(TreeEntry {
                    relative: relative.clone(),
                    kind: TreeEntryKind::Directory,
                    mode,
                    size: 0,
                    sha256: [0; 32],
                    symlink_target: Vec::new(),
                });
                inventory_directory(
                    &child,
                    &relative,
                    depth + 1,
                    policy,
                    excluded_relative,
                    entries,
                    regular_inodes,
                    entries_seen,
                )?;
                let after = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(std::io::Error::from)
                    .wrap_err("reinspect tree directory entry")?;
                if RustixFileType::from_raw_mode(after.st_mode) != RustixFileType::Directory
                    || FileIdentity::from_stat(&after)? != identity
                {
                    bail!("tree directory changed while inventoried");
                }
            }
            RustixFileType::RegularFile => {
                if before.st_nlink != 1 || !policy.allowed_file_modes.contains(&mode) {
                    bail!("tree file mode or link count differs");
                }
                let inode = (
                    u64::try_from(before.st_dev)
                        .map_err(|_| eyre!("tree device id does not fit u64"))?,
                    u64::try_from(before.st_ino)
                        .map_err(|_| eyre!("tree inode id does not fit u64"))?,
                );
                if !regular_inodes.insert(inode) {
                    bail!("tree reuses one regular-file inode");
                }
                let (_file, sha256, size, opened) =
                    stable_file_sha256(directory, name, &before, None)?;
                if opened.uid() != policy.trusted_uid || opened.mode() & 0o777 != mode {
                    bail!("tree file ownership or mode changed while hashing");
                }
                entries.push(TreeEntry {
                    relative,
                    kind: TreeEntryKind::File,
                    mode,
                    size,
                    sha256,
                    symlink_target: Vec::new(),
                });
            }
            RustixFileType::Symlink if policy.allow_internal_symlinks => {
                let target = readlinkat(directory, name, Vec::new())
                    .map_err(std::io::Error::from)
                    .wrap_err("read tree symlink")?
                    .into_bytes();
                validate_internal_symlink(&relative, &target)?;
                let after = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(std::io::Error::from)
                    .wrap_err("reinspect tree symlink")?;
                if FileIdentity::from_stat(&before)? != FileIdentity::from_stat(&after)?
                    || before.st_mode != after.st_mode
                    || before.st_nlink != after.st_nlink
                    || before.st_mtime != after.st_mtime
                    || before.st_ctime != after.st_ctime
                {
                    bail!("tree symlink changed while inventoried");
                }
                entries.push(TreeEntry {
                    relative,
                    kind: TreeEntryKind::Symlink,
                    mode,
                    size: 0,
                    sha256: [0; 32],
                    symlink_target: target,
                });
            }
            _ => bail!("tree contains a symlink, special, or unsupported entry"),
        }
    }
    let directory_after = directory.metadata().wrap_err("reinspect tree directory")?;
    if !same_metadata(&directory_before, &directory_after) {
        bail!("tree directory changed while its children were inventoried");
    }
    Ok(())
}

pub(super) fn inventory_hardened_tree(
    root: &Path,
    policy: HardenedTreePolicy,
    excluded_relative: Option<&[u8]>,
) -> Result<Vec<TreeEntry>> {
    let opened = open_tree_root(root)?;
    let root_before = opened
        .root
        .metadata()
        .wrap_err("inspect opened tree root")?;
    if !root_before.is_dir()
        || root_before.uid() != policy.trusted_uid
        || root_before.mode() & 0o777 != policy.root_mode
        || root_before.mode() & 0o7000 != 0
    {
        bail!("tree root ownership or mode differs");
    }
    let mut entries = Vec::new();
    let mut regular_inodes = BTreeSet::new();
    let mut entries_seen = 0;
    inventory_directory(
        &opened.root,
        b"",
        0,
        policy,
        excluded_relative,
        &mut entries,
        &mut regular_inodes,
        &mut entries_seen,
    )?;
    let root_after = opened
        .root
        .metadata()
        .wrap_err("reinspect opened tree root")?;
    if !same_metadata(&root_before, &root_after) {
        bail!("tree root changed while inventoried");
    }
    opened.verify_ancestry(root)?;
    entries.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(entries)
}

pub(super) fn validate_trusted_parent_chain(path: &Path, trusted_uid: u32) -> Result<()> {
    let opened = open_tree_root(path)?;
    let filesystem_root = opened
        .ancestry
        .first()
        .map(|link| &link.parent)
        .unwrap_or(&opened.root);
    let filesystem_root_metadata = filesystem_root
        .metadata()
        .wrap_err("inspect filesystem root")?;
    if filesystem_root_metadata.uid() != trusted_uid || filesystem_root_metadata.mode() & 0o022 != 0
    {
        bail!("publication parent chain does not begin at a trusted read-only filesystem root");
    }
    for link in opened
        .ancestry
        .iter()
        .take(opened.ancestry.len().saturating_sub(1))
    {
        let observed = statat(&link.parent, &link.name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .wrap_err("inspect publication parent-chain component")?;
        if RustixFileType::from_raw_mode(observed.st_mode) != RustixFileType::Directory
            || observed.st_uid != trusted_uid
            || u32::from(observed.st_mode) & 0o022 != 0
        {
            bail!("publication parent chain is not trusted-owner and non-group/world-writable");
        }
    }
    opened.verify_ancestry(path)?;
    Ok(())
}

fn frame(digest: &mut Sha256, value: &[u8]) {
    digest.update(
        &u64::try_from(value.len())
            .expect("slice length fits u64")
            .to_be_bytes(),
    );
    digest.update(value);
}

fn split_parent_name(relative: &[u8]) -> (&[u8], &[u8]) {
    relative
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or((b"", relative), |index| {
            (&relative[..index], &relative[index + 1..])
        })
}

pub(super) fn flat_tree_digest(entries: &[TreeEntry], domain: &[u8]) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(domain);
    let mut sorted = entries.to_vec();
    sorted.sort_by(|left, right| left.relative.cmp(&right.relative));
    for entry in sorted {
        match entry.kind {
            TreeEntryKind::Directory => {
                digest.update(b"D");
                frame(&mut digest, &entry.relative);
                digest.update(&entry.mode.to_be_bytes());
            }
            TreeEntryKind::File => {
                digest.update(b"F");
                frame(&mut digest, &entry.relative);
                digest.update(&entry.mode.to_be_bytes());
                digest.update(&entry.size.to_be_bytes());
                digest.update(&entry.sha256);
            }
            TreeEntryKind::Symlink => bail!("flat generated trees do not admit symlinks"),
        }
    }
    Ok(hex::encode(digest.finalize()))
}

pub(super) fn candidate_artifact_digest(entries: &[TreeEntry]) -> Result<String> {
    if entries
        .iter()
        .any(|entry| entry.kind != TreeEntryKind::File)
    {
        bail!("candidate artifact digest accepts files only");
    }
    flat_tree_digest(entries, CANDIDATE_ARTIFACT_DOMAIN)
}

pub(super) fn production_closure_digest(entries: &[TreeEntry]) -> Result<String> {
    let mut by_relative = BTreeMap::new();
    let mut children: BTreeMap<Vec<u8>, Vec<&TreeEntry>> = BTreeMap::new();
    children.insert(Vec::new(), Vec::new());
    for entry in entries {
        validate_relative(&entry.relative)?;
        if by_relative.insert(entry.relative.clone(), entry).is_some() {
            bail!("production closure contains a duplicate entry");
        }
    }
    for entry in entries {
        let (parent, _) = split_parent_name(&entry.relative);
        if !parent.is_empty()
            && by_relative
                .get(parent)
                .is_none_or(|entry| entry.kind != TreeEntryKind::Directory)
        {
            bail!("production closure entry lacks its directory parent");
        }
        children.entry(parent.to_vec()).or_default().push(entry);
        if entry.kind == TreeEntryKind::Directory {
            children.entry(entry.relative.clone()).or_default();
        }
    }
    let mut digest = Sha256::new();
    digest.update(PRODUCTION_CLOSURE_TREE_DOMAIN);
    let mut excluded_seen = false;
    let mut stack = vec![Vec::new()];
    let mut entries_seen = 0_usize;
    while let Some(directory) = stack.pop() {
        let mut directory_children = children.get(&directory).cloned().unwrap_or_default();
        directory_children.sort_by(|left, right| {
            let (_, left_name) = split_parent_name(&left.relative);
            let (_, right_name) = split_parent_name(&right.relative);
            right_name.cmp(left_name)
        });
        for entry in directory_children {
            entries_seen = entries_seen
                .checked_add(1)
                .ok_or_else(|| eyre!("production closure entry count overflow"))?;
            if entries_seen > MAX_TREE_ENTRIES {
                bail!("production closure exceeds its entry bound");
            }
            if entry.relative == PRODUCTION_PROVENANCE_FILE_NAME {
                excluded_seen = true;
                continue;
            }
            match entry.kind {
                TreeEntryKind::Directory => {
                    digest.update(b"D");
                    frame(&mut digest, &entry.relative);
                    digest.update(&entry.mode.to_be_bytes());
                    stack.push(entry.relative.clone());
                }
                TreeEntryKind::File => {
                    digest.update(b"F");
                    frame(&mut digest, &entry.relative);
                    digest.update(&entry.mode.to_be_bytes());
                    digest.update(&entry.size.to_be_bytes());
                    digest.update(&entry.sha256);
                }
                TreeEntryKind::Symlink => {
                    validate_internal_symlink(&entry.relative, &entry.symlink_target)?;
                    digest.update(b"L");
                    frame(&mut digest, &entry.relative);
                    frame(&mut digest, &entry.symlink_target);
                }
            }
        }
    }
    if !excluded_seen {
        bail!("production closure provenance is not its one excluded tree entry");
    }
    Ok(hex::encode(digest.finalize()))
}

pub(super) fn subtree_entries(entries: &[TreeEntry], prefix: &[u8]) -> Vec<TreeEntry> {
    let mut needle = prefix.to_vec();
    needle.push(b'/');
    entries
        .iter()
        .filter_map(|entry| {
            entry
                .relative
                .strip_prefix(needle.as_slice())
                .map(|relative| {
                    let mut entry = entry.clone();
                    entry.relative = relative.to_vec();
                    entry
                })
        })
        .collect()
}

pub(super) fn exact_top_level(entries: &[TreeEntry]) -> BTreeSet<Vec<u8>> {
    entries
        .iter()
        .filter(|entry| !entry.relative.contains(&b'/'))
        .map(|entry| entry.relative.clone())
        .collect()
}

#[cfg(test)]
pub(super) fn stable_file_digest(path: &Path, maximum_bytes: u64) -> Result<(String, u64)> {
    let stable = stable_file(path, maximum_bytes)?;
    Ok((stable.sha256, stable.size))
}

/// Open, authenticate, hash, and retain one exact regular-file descriptor.
pub(super) fn stable_file(path: &Path, maximum_bytes: u64) -> Result<StableFile> {
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("file path has no parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("file path has no filename"))?;
    let opened_parent = open_tree_root(parent)?;
    let before = statat(&opened_parent.root, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(std::io::Error::from)
        .wrap_err("inspect stable file")?;
    if RustixFileType::from_raw_mode(before.st_mode) != RustixFileType::RegularFile
        || before.st_nlink != 1
        || before.st_size < 0
        || u64::try_from(before.st_size).unwrap_or(u64::MAX) > maximum_bytes
    {
        bail!("stable file is not one bounded singly-linked regular file");
    }
    let (file, digest, size, metadata) =
        stable_file_sha256(&opened_parent.root, name, &before, Some(maximum_bytes))?;
    opened_parent.verify_ancestry(parent)?;
    Ok(StableFile {
        file,
        snapshot: metadata,
        sha256: hex::encode(digest),
        size,
    })
}

/// Retain the exact running executable image where the kernel exposes an
/// inode-bound process handle.
///
/// `/proc/self/exe` is a kernel magic link to the image backing this process,
/// not a caller-controlled pathname lookup. Platforms without an equivalent
/// exact descriptor fail closed instead of reporting a hash of a reopenable
/// executable pathname.
pub(super) fn stable_executing_image(maximum_bytes: u64) -> Result<StableFile> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let file = File::open("/proc/self/exe")
            .wrap_err("open exact running image through /proc/self/exe")?;
        stable_opened_descriptor(file, maximum_bytes, "exact /proc running image")
    }
    #[cfg(target_os = "macos")]
    {
        const EXECUTING_IMAGE_FD_ENV: &str = "IROHA_KAGEMUSHA_EXECUTABLE_FD";
        let raw_text = std::env::var(EXECUTING_IMAGE_FD_ENV)
            .wrap_err("missing launcher-inherited exact executable descriptor")?;
        let raw = raw_text
            .parse::<i32>()
            .wrap_err("launcher executable descriptor is not a canonical integer")?;
        if raw < 3 || raw_text != raw.to_string() {
            bail!("launcher executable descriptor must be one canonical descriptor >= 3");
        }
        let descriptor_path = PathBuf::from(format!("/dev/fd/{raw}"));
        let file = File::open(&descriptor_path)
            .wrap_err("duplicate launcher executable through its inherited descriptor")?;
        let flags = rustix::fs::fcntl_getfl(&file)
            .wrap_err("inspect launcher executable descriptor access mode")?;
        if flags.intersects(OFlags::WRONLY | OFlags::RDWR) {
            bail!("launcher executable descriptor must be read-only");
        }
        let metadata = file
            .metadata()
            .wrap_err("inspect launcher executable descriptor identity")?;
        if !metadata.is_file() || metadata.mode() & 0o111 == 0 {
            bail!("launcher executable descriptor is not an executable regular file");
        }
        let running_path =
            std::env::current_exe().wrap_err("inspect launcher executable invocation name")?;
        let running_metadata = fs::symlink_metadata(&running_path)
            .wrap_err("inspect launcher executable invocation identity")?;
        let running_parent = running_path
            .parent()
            .ok_or_else(|| eyre!("launcher executable invocation has no parent"))?;
        let parent_metadata = fs::symlink_metadata(running_parent)
            .wrap_err("inspect launcher executable private directory")?;
        if running_metadata.file_type().is_symlink()
            || FileIdentity::from_metadata(&running_metadata)
                != FileIdentity::from_metadata(&metadata)
            || running_metadata.nlink() != 1
            || running_metadata.mode() & 0o777 != 0o500
            || !parent_metadata.is_dir()
            || parent_metadata.mode() & 0o777 != 0o500
            || running_metadata.uid() != rustix::process::getuid().as_raw()
            || parent_metadata.uid() != rustix::process::getuid().as_raw()
        {
            bail!(
                "macOS Kagami must execute the exact private 0500 image retained by \
                 IROHA_KAGEMUSHA_EXECUTABLE_FD"
            );
        }
        stable_opened_descriptor(file, maximum_bytes, "launcher-bound running image")
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    {
        let _ = maximum_bytes;
        bail!("this platform exposes no supported exact running-image descriptor")
    }
}

fn stable_opened_descriptor(mut file: File, maximum_bytes: u64, label: &str) -> Result<StableFile> {
    let before = file
        .metadata()
        .wrap_err_with(|| format!("inspect {label} descriptor"))?;
    if !before.is_file() || before.size() > maximum_bytes {
        bail!("{label} is not one bounded regular file");
    }
    file.seek(SeekFrom::Start(0))
        .wrap_err_with(|| format!("rewind {label} before hashing"))?;
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .wrap_err_with(|| format!("hash {label} descriptor"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        size = size
            .checked_add(u64::try_from(read).expect("buffer length fits u64"))
            .ok_or_else(|| eyre!("{label} length overflow"))?;
        if size > maximum_bytes {
            bail!("{label} exceeded its byte bound while hashed");
        }
    }
    let after = file
        .metadata()
        .wrap_err_with(|| format!("reinspect {label} descriptor"))?;
    if !same_metadata(&before, &after) || size != after.size() {
        bail!("{label} changed while hashed");
    }
    file.seek(SeekFrom::Start(0))
        .wrap_err_with(|| format!("rewind {label} descriptor"))?;
    Ok(StableFile {
        file,
        snapshot: after,
        sha256: hex::encode(digest.finalize()),
        size,
    })
}

/// Read one bounded regular file through a stable descriptor and return the
/// exact authenticated bytes together with their SHA-256.
///
/// Callers that parse security-sensitive descriptors must parse this returned
/// byte vector rather than close and reopen `path` after authenticating it.
pub(super) fn stable_file_bytes(path: &Path, maximum_bytes: u64) -> Result<(Vec<u8>, String)> {
    let mut stable = stable_file(path, maximum_bytes)?;
    let digest = stable.sha256.clone();
    let bytes = stable.read_bytes(maximum_bytes)?;
    Ok((bytes, digest))
}

pub(super) fn validate_absolute_normalized(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        bail!("{label} must be one normalized absolute path");
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(bytes);
        digest.finalize()
    }

    #[test]
    fn sha256_matches_fips_vectors() {
        assert_eq!(
            hex::encode(sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex::encode(sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_boundary_lengths_and_chunked_updates_match_reviewed_vectors() {
        const VECTORS: &[(usize, &str)] = &[
            (
                0,
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                1,
                "6922e93e3827642ce4b883c756b31abf80036649d3614bf5fcb3adda43b8ea32",
            ),
            (
                55,
                "26ee0116778740a66fe2ba10ea063748b27306acc99188ec812746d4e8d70083",
            ),
            (
                56,
                "4cf71e2b0aa0fcc0c271f68353026a77b8e50153632a8e4a73833cd64080e92e",
            ),
            (
                63,
                "a1942663a5b8b93dffc9c4ff5f62c71a1c021d1fcc1e470dd46172abace1bca5",
            ),
            (
                64,
                "bb626e5577021df95ea17eb6339e75904855b80087e40660931c4a89b302f74a",
            ),
            (
                65,
                "667f84020d981fcedce2816e4e9969a02d5c317a0aef56a6c588175820f82a81",
            ),
        ];
        for &(length, expected) in VECTORS {
            let input = vec![0xA5; length];
            assert_eq!(hex::encode(sha256(&input)), expected, "length {length}");
            for chunk_size in [1, 2, 7, 55, 64, 65] {
                let mut digest = Sha256::new();
                digest.update(&[]);
                for chunk in input.chunks(chunk_size) {
                    digest.update(chunk);
                    digest.update(&[]);
                }
                assert_eq!(
                    hex::encode(digest.finalize()),
                    expected,
                    "length {length}, chunk size {chunk_size}",
                );
            }
        }
    }

    #[test]
    fn production_closure_digest_matches_coordinated_python_golden() {
        let entries = vec![
            TreeEntry {
                relative: b"a".to_vec(),
                kind: TreeEntryKind::Directory,
                mode: 0o555,
                size: 0,
                sha256: [0; 32],
                symlink_target: Vec::new(),
            },
            TreeEntry {
                relative: b"a/f".to_vec(),
                kind: TreeEntryKind::File,
                mode: 0o444,
                size: 7,
                sha256: sha256(b"payload"),
                symlink_target: Vec::new(),
            },
            TreeEntry {
                relative: b"link".to_vec(),
                kind: TreeEntryKind::Symlink,
                mode: 0o777,
                size: 0,
                sha256: [0; 32],
                symlink_target: b"a/f".to_vec(),
            },
            TreeEntry {
                relative: PRODUCTION_PROVENANCE_FILE_NAME.to_vec(),
                kind: TreeEntryKind::File,
                mode: 0o444,
                size: 3,
                sha256: sha256(b"{}\n"),
                symlink_target: Vec::new(),
            },
        ];
        assert_eq!(
            production_closure_digest(&entries).expect("valid golden closure"),
            "8bacdea35042a2ec398ea74b2a73c4e3531569b2abce16d96567de248817ad24"
        );
    }

    #[cfg(unix)]
    #[test]
    fn inherited_input_descriptors_preserve_bytes_and_detect_path_replacement() {
        let directory = tempdir().expect("temporary directory");
        let directory_root = directory
            .path()
            .canonicalize()
            .expect("canonical temporary directory");
        let signature_path = directory_root.join("trust.sig");
        let key_path = directory_root.join("trust.gpg");
        std::fs::write(&signature_path, b"original signature").expect("write signature");
        std::fs::write(&key_path, b"original public key").expect("write key");
        let mut signature = stable_file(&signature_path, 1024).expect("capture signature");
        let mut key = stable_file(&key_path, 1024).expect("capture key");
        let signature_fd = signature
            .inherited_descriptor()
            .expect("inherit signature descriptor");
        let key_fd = key.inherited_descriptor().expect("inherit key descriptor");

        std::fs::rename(&signature_path, directory_root.join("original.sig"))
            .expect("hold original signature inode");
        std::fs::rename(&key_path, directory_root.join("original.gpg"))
            .expect("hold original key inode");
        std::fs::write(&signature_path, b"replacement signature").expect("replace signature path");
        std::fs::write(&key_path, b"replacement public key").expect("replace key path");

        let output = std::process::Command::new("/bin/cat")
            .arg(signature_fd.alias_path())
            .arg(key_fd.alias_path())
            .output()
            .expect("read inherited descriptors");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"original signatureoriginal public key");
        assert!(
            signature.verify_unchanged().is_err(),
            "renaming the authenticated signature inode must fail the post-use metadata check"
        );
        assert!(
            key.verify_unchanged().is_err(),
            "renaming the authenticated public-key inode must fail the post-use metadata check"
        );
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_bound_executable_preserves_bytes_and_detects_path_replacement() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempdir().expect("temporary directory");
        let directory_root = directory
            .path()
            .canonicalize()
            .expect("canonical temporary directory");
        let verifier_path = directory_root.join("gpgv");
        std::fs::write(&verifier_path, b"#!/bin/sh\nprintf original\n")
            .expect("write original verifier");
        std::fs::set_permissions(&verifier_path, std::fs::Permissions::from_mode(0o755))
            .expect("make original verifier executable");
        let mut verifier = stable_file(&verifier_path, 1024).expect("capture verifier");
        let descriptor = verifier
            .inherited_descriptor()
            .expect("inherit verifier descriptor");
        let executable = verifier
            .descriptor_bound_executable_path(&descriptor)
            .expect("resolve descriptor-bound executable");

        std::fs::rename(&verifier_path, directory_root.join("original-gpgv"))
            .expect("hold original verifier inode");
        std::fs::write(&verifier_path, b"#!/bin/sh\nprintf replacement\n")
            .expect("write replacement verifier");
        std::fs::set_permissions(&verifier_path, std::fs::Permissions::from_mode(0o755))
            .expect("make replacement verifier executable");

        let output = std::process::Command::new(executable)
            .output()
            .expect("execute retained verifier inode");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"original");
        assert!(
            verifier.verify_unchanged().is_err(),
            "renaming the authenticated verifier inode must fail the post-use metadata check"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn running_image_digest_comes_from_proc_descriptor() {
        let mut image =
            stable_executing_image(1024 * 1024 * 1024).expect("capture exact running image");
        assert!(is_lower_hex_sha256(image.sha256()));
        image
            .verify_unchanged()
            .expect("running image descriptor remains exact");
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn is_lower_hex_sha256(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
}
