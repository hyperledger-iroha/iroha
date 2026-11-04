//! Bounded native input acquisition for CAR publication without retaining payload bytes.
use crate::{
    CAR_DIRECTORY_MAX_ENTRIES, CAR_LOGICAL_PATH_MAX_COMPONENTS, CAR_PLAN_MAX_CHUNKS, CarBuildPlan,
    CarChunk, DirectoryPayload, FilePayload, FilePlan, PayloadSource, ensure_car_plan_profile,
    ensure_plan_file_count, validate_path,
};
use iroha_fs::{FileSnapshot, ReaderDirectory, RetainedFile};
use sorafs_chunker::{ChunkProfile, Chunker};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// Fixed on-disk publication input bound; the existing eager and metadata heap limits still apply.
pub const MAX_STREAMING_INPUT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const READ_BUFFER_BYTES: usize = 64 * 1024;

/// One native source, its canonical plan and file commitments, retained through publication.
pub struct PreparedCarInput {
    /// Canonical path/chunk inventory; payload bytes are not retained in this value.
    pub plan: CarBuildPlan,
    /// Seekable source that revalidates native identity and snapshots on every pass.
    pub payload: NativePayloadReader,
    /// BLAKE3 file digests in the exact order of `plan.files`.
    pub file_digests: Vec<[u8; 32]>,
    /// Whether the source is a directory rather than one raw file.
    pub is_directory: bool,
}

enum NativeSource {
    File(FilePayload),
    Directory(DirectoryPayload),
}
struct DirectorySnapshot {
    owner: ReaderDirectory,
    snapshot: FileSnapshot,
}
struct RecordedFile {
    path: PathBuf,
    snapshot: FileSnapshot,
}

/// Bounded seekable view over retained native payload authority, never a whole-payload allocation.
pub struct NativePayloadReader {
    source: NativeSource,
    directories: Vec<DirectorySnapshot>,
    files: Vec<RecordedFile>,
    length: u64,
    position: u64,
}
impl NativePayloadReader {
    /// Revalidate the exact source inventory, including empty files and directory membership.
    pub fn revalidate(&mut self) -> io::Result<()> {
        for directory in &self.directories {
            directory.owner.revalidate()?;
            if directory.owner.snapshot()? != directory.snapshot {
                return Err(io::Error::other(
                    "directory changed after publication inventory",
                ));
            }
        }
        for recorded in &self.files {
            let file = RetainedFile::open_regular(&recorded.path)?;
            if file.snapshot()? != recorded.snapshot {
                return Err(io::Error::other("file changed after publication inventory"));
            }
        }
        match &mut self.source {
            NativeSource::File(source) => source.ensure_exhausted(self.length),
            NativeSource::Directory(source) => source.ensure_exhausted(self.length),
        }
        .map_err(io::Error::other)
    }
}
impl Read for NativePayloadReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let count = usize::try_from((self.length - self.position).min(bytes.len() as u64))
            .map_err(io::Error::other)?;
        if count == 0 {
            self.revalidate()?;
            return Ok(0);
        }
        match &mut self.source {
            NativeSource::File(source) => source.read_exact(self.position, &mut bytes[..count]),
            NativeSource::Directory(source) => {
                source.read_exact(self.position, &mut bytes[..count])
            }
        }
        .map_err(io::Error::other)?;
        self.position += count as u64;
        Ok(count)
    }
}
impl Seek for NativePayloadReader {
    fn seek(&mut self, seek: SeekFrom) -> io::Result<u64> {
        let position = match seek {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::End(value) => i128::from(self.length) + i128::from(value),
            SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
        };
        if !(0..=i128::from(self.length)).contains(&position) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "payload seek outside exact input",
            ));
        }
        self.position = position as u64;
        Ok(self.position)
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
fn scan(
    owner: ReaderDirectory,
    parent: &[String],
    files: &mut Vec<FilePlan>,
    recorded: &mut Vec<RecordedFile>,
    directories: &mut Vec<DirectorySnapshot>,
    entries: &mut usize,
    length: &mut u64,
) -> io::Result<()> {
    let snapshot = owner.snapshot()?;
    let mut names = Vec::new();
    for entry in fs::read_dir(owner.path())? {
        *entries = entries
            .checked_add(1)
            .ok_or_else(|| invalid("directory entry overflow"))?;
        if *entries > CAR_DIRECTORY_MAX_ENTRIES {
            return Err(invalid("too many directory entries"));
        }
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF8 path"))?;
        names.push(name);
    }
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("duplicate directory entry"));
    }
    for name in names {
        let mut logical = parent.to_vec();
        logical.push(name.clone());
        validate_path(&logical).map_err(invalid)?;
        if logical.len() > CAR_LOGICAL_PATH_MAX_COMPONENTS {
            return Err(invalid("directory depth exceeded"));
        }
        let path = owner.path().join(&name);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("symbolic links are forbidden"));
        }
        if metadata.is_dir() {
            scan(
                owner.open_child(&name)?,
                &logical,
                files,
                recorded,
                directories,
                entries,
                length,
            )?;
        } else if metadata.is_file() {
            ensure_plan_file_count(files.len().saturating_add(1)).map_err(invalid)?;
            let file = owner.open_retained_regular(&name)?;
            let size = file.file().metadata()?.len();
            *length = length
                .checked_add(size)
                .ok_or_else(|| invalid("payload length overflow"))?;
            if *length > MAX_STREAMING_INPUT_BYTES {
                return Err(invalid("streaming payload exceeds 4 GiB limit"));
            }
            recorded.push(RecordedFile {
                path,
                snapshot: file.snapshot()?,
            });
            files.push(FilePlan {
                path: logical,
                first_chunk: 0,
                chunk_count: 0,
                size,
            });
        } else {
            return Err(invalid(
                "input must contain only regular files and directories",
            ));
        }
    }
    owner.revalidate()?;
    if owner.snapshot()? != snapshot {
        return Err(invalid("directory changed during inventory"));
    }
    directories.push(DirectorySnapshot { owner, snapshot });
    Ok(())
}

impl PreparedCarInput {
    /// Acquire a canonical native file/directory plan with bounded buffers and retained custody.
    ///
    /// Rejects links, changed files/directories, nonportable paths, inputs above 4 GiB, and all
    /// existing plan/ingest resource limits. Every file is chunked independently, exactly as the
    /// eager planner. No caller-selected resource limit can weaken these bounds.
    pub fn open(path: &Path, profile: ChunkProfile) -> io::Result<Self> {
        ensure_car_plan_profile(profile).map_err(invalid)?;
        let metadata = fs::symlink_metadata(path)?;
        let is_directory = metadata.is_dir();
        let mut files = Vec::new();
        let mut recorded = Vec::new();
        let mut directories = Vec::new();
        let mut length = 0;
        let source = if is_directory {
            scan(
                ReaderDirectory::open(path)?,
                &[],
                &mut files,
                &mut recorded,
                &mut directories,
                &mut 0,
                &mut length,
            )?;
            files.sort_by(|left, right| left.path.cmp(&right.path));
            if files.is_empty() {
                return Err(invalid("empty directory input"));
            }
            NativeSource::Directory(DirectoryPayload::new(path, &files)?)
        } else {
            let file = RetainedFile::open_regular(path)?;
            length = file.file().metadata()?.len();
            if length == 0 || length > MAX_STREAMING_INPUT_BYTES {
                return Err(invalid("file payload must be nonempty and at most 4 GiB"));
            }
            recorded.push(RecordedFile {
                path: path.to_path_buf(),
                snapshot: file.snapshot()?,
            });
            files.push(FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: 0,
                size: length,
            });
            NativeSource::File(FilePayload::open(path)?)
        };
        let maximum_chunks = files.iter().try_fold(0usize, |count, file| {
            let count = count
                .checked_add(
                    usize::try_from(file.size)
                        .map_err(invalid)?
                        .div_ceil(profile.min_size),
                )
                .ok_or_else(|| invalid("chunk count overflow"))?;
            if count > CAR_PLAN_MAX_CHUNKS {
                return Err(invalid("too many publication chunks"));
            }
            Ok(count)
        })?;
        let mut payload = NativePayloadReader {
            source,
            directories,
            files: recorded,
            length,
            position: 0,
        };
        payload.revalidate()?;
        let mut chunks = Vec::new();
        chunks.try_reserve_exact(maximum_chunks).map_err(invalid)?;
        let mut buffer = [0u8; READ_BUFFER_BYTES];
        let mut payload_digest = blake3::Hasher::new();
        let mut file_digests = Vec::with_capacity(files.len());
        let mut base = 0u64;
        for file in &mut files {
            file.first_chunk = chunks.len();
            let mut chunker = Chunker::try_with_profile(profile).map_err(invalid)?;
            let mut file_digest = blake3::Hasher::new();
            let mut chunk_digest = blake3::Hasher::new();
            let mut consumed = 0u64;
            while consumed < file.size {
                let count = usize::try_from((file.size - consumed).min(buffer.len() as u64))
                    .map_err(invalid)?;
                payload.read_exact(&mut buffer[..count])?;
                payload_digest.update(&buffer[..count]);
                file_digest.update(&buffer[..count]);
                let mut cursor = 0usize;
                chunker.feed(&buffer[..count], |boundary| {
                    let end = (boundary.offset + boundary.length) - consumed as usize;
                    chunk_digest.update(&buffer[cursor..end]);
                    chunks.push(CarChunk {
                        offset: base + boundary.offset as u64,
                        length: boundary.length as u32,
                        digest: chunk_digest.finalize().into(),
                    });
                    chunk_digest = blake3::Hasher::new();
                    cursor = end;
                });
                chunk_digest.update(&buffer[cursor..count]);
                consumed += count as u64;
            }
            chunker.finish(|boundary| {
                if boundary.length != 0 {
                    chunks.push(CarChunk {
                        offset: base + boundary.offset as u64,
                        length: boundary.length as u32,
                        digest: chunk_digest.finalize().into(),
                    });
                }
            });
            file.chunk_count = chunks.len() - file.first_chunk;
            file_digests.push(file_digest.finalize().into());
            base += file.size;
        }
        payload.revalidate()?;
        payload.rewind()?;
        let plan = CarBuildPlan {
            chunk_profile: profile,
            payload_digest: payload_digest.finalize(),
            content_length: length,
            chunks,
            files,
        };
        plan.validate_for_ingest().map_err(invalid)?;
        Ok(Self {
            plan,
            payload,
            file_digests,
            is_directory,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CarStreamingWriter, CarWriter};

    #[test]
    fn streaming_scan_rejects_entry_overflow_before_retaining_payload() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("payload"), b"not retained").unwrap();
        for initial_entries in [CAR_DIRECTORY_MAX_ENTRIES, usize::MAX] {
            let mut entries = initial_entries;
            let mut files = Vec::new();
            let mut recorded = Vec::new();
            let mut directories = Vec::new();
            let mut length = 0;
            let error = scan(
                ReaderDirectory::open(directory.path()).unwrap(),
                &[],
                &mut files,
                &mut recorded,
                &mut directories,
                &mut entries,
                &mut length,
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            let expected = if initial_entries == usize::MAX {
                "directory entry overflow"
            } else {
                "too many directory entries"
            };
            assert_eq!(error.to_string(), expected);
            // A propagated inventory error must retain its classification and
            // diagnostic when an outer input-acquisition boundary wraps it.
            let propagated = invalid(error);
            assert_eq!(propagated.kind(), io::ErrorKind::InvalidData);
            assert_eq!(propagated.to_string(), expected);
            assert!(files.is_empty());
            assert!(recorded.is_empty());
            assert!(directories.is_empty());
            assert_eq!(length, 0);
        }
    }

    #[test]
    fn streaming_plan_and_car_match_eager_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("empty"), []).unwrap();
        fs::write(
            dir.path().join("nested/file"),
            (0..700_123).map(|i| (i % 251) as u8).collect::<Vec<_>>(),
        )
        .unwrap();
        let (eager, bytes) = CarBuildPlan::from_directory(dir.path()).unwrap();
        let mut stream = PreparedCarInput::open(dir.path(), ChunkProfile::DEFAULT).unwrap();
        assert_eq!(stream.plan, eager);
        let mut expected = Vec::new();
        CarWriter::new(&eager, &bytes)
            .unwrap()
            .write_to(&mut expected)
            .unwrap();
        let mut actual = Vec::new();
        CarStreamingWriter::new(&stream.plan)
            .write_from_reader(&mut stream.payload, &mut actual)
            .unwrap();
        assert_eq!(actual, expected);
        fs::write(dir.path().join("empty"), "mutation").unwrap();
        assert!(stream.payload.revalidate().is_err());
    }

    #[test]
    fn streaming_plan_and_car_match_eager_raw_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        let payload = (0..700_123).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        fs::write(&path, &payload).unwrap();
        let eager = CarBuildPlan::single_file(&payload).unwrap();
        let mut stream = PreparedCarInput::open(&path, ChunkProfile::DEFAULT).unwrap();
        assert!(!stream.is_directory);
        assert_eq!(stream.plan, eager);
        assert_eq!(
            stream.file_digests,
            vec![*blake3::hash(&payload).as_bytes()]
        );
        let mut expected = Vec::new();
        CarWriter::new(&eager, &payload)
            .unwrap()
            .write_to(&mut expected)
            .unwrap();
        let mut actual = Vec::new();
        CarStreamingWriter::new(&stream.plan)
            .write_from_reader(&mut stream.payload, &mut actual)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            stream.payload.seek(SeekFrom::End(0)).unwrap(),
            payload.len() as u64
        );
        assert!(stream.payload.seek(SeekFrom::End(1)).is_err());
        assert!(stream.payload.seek(SeekFrom::Start(u64::MAX)).is_err());
        fs::write(&path, &payload[..payload.len() - 1]).unwrap();
        assert!(stream.payload.revalidate().is_err());
    }

    #[test]
    fn streaming_input_rejects_oversized_inventory_before_reading() {
        let dir = tempfile::tempdir().unwrap();
        fs::File::create(dir.path().join("huge"))
            .unwrap()
            .set_len(MAX_STREAMING_INPUT_BYTES + 1)
            .unwrap();
        assert!(PreparedCarInput::open(dir.path(), ChunkProfile::DEFAULT).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn streaming_input_rejects_links_and_directory_changes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("source"), "bytes").unwrap();
        std::os::unix::fs::symlink("source", dir.path().join("link")).unwrap();
        assert!(PreparedCarInput::open(dir.path(), ChunkProfile::DEFAULT).is_err());
        fs::remove_file(dir.path().join("link")).unwrap();
        fs::hard_link(dir.path().join("source"), dir.path().join("link")).unwrap();
        assert!(PreparedCarInput::open(dir.path(), ChunkProfile::DEFAULT).is_err());
        fs::remove_file(dir.path().join("link")).unwrap();
        let mut input = PreparedCarInput::open(dir.path(), ChunkProfile::DEFAULT).unwrap();
        fs::write(dir.path().join("new"), "new").unwrap();
        assert!(input.payload.revalidate().is_err());
    }

    #[test]
    #[ignore = "release qualification streams more than the eager 512 MiB bound"]
    fn streaming_input_crosses_eager_bound() {
        let dir = tempfile::tempdir().unwrap();
        let size = 513 * 1024 * 1024;
        fs::File::create(dir.path().join("large"))
            .unwrap()
            .set_len(size)
            .unwrap();
        assert!(CarBuildPlan::from_directory(dir.path()).is_err());
        let mut input = PreparedCarInput::open(dir.path(), ChunkProfile::DEFAULT).unwrap();
        assert_eq!(input.plan.content_length, size);
        let stats = CarStreamingWriter::new(&input.plan)
            .write_from_reader(&mut input.payload, io::sink())
            .unwrap();
        assert!(stats.car_size > size);
    }
}
