//! Explicit destinations for finished records and the harness hand-off.
//!
//! A session delivers its record to the sink it was given and to nothing
//! else. Instrumented code receives no value back from the recorder, so a
//! measurement cannot steer the measured computation.

use std::{
    fs::OpenOptions,
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use crate::schema::{HARNESS_CONTEXT_FILE, MeasurementRecord, RunContext, SchemaError};

/// Largest hand-off context file a diagnostic adapter will read.
pub const MAX_CONTEXT_BYTES: u64 = 64 * 1024;
/// Highest sequence number tried before a directory sink gives up.
const MAX_SEQUENCE: u32 = 9_999;

/// Destination of one finished record.
pub trait RecordSink: Send {
    /// Take ownership of the finished record. The session keeps no copy.
    fn accept(&mut self, record: MeasurementRecord);
}

/// In-memory sink for tests and diagnostic adapters.
#[derive(Clone, Debug, Default)]
pub struct CollectingSink {
    records: Arc<Mutex<Vec<MeasurementRecord>>>,
}

impl CollectingSink {
    /// An empty collector. Clones share the same storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove and return every record delivered so far, in delivery order.
    pub fn take(&self) -> Vec<MeasurementRecord> {
        core::mem::take(&mut *self.records.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl RecordSink for CollectingSink {
    fn accept(&mut self, record: MeasurementRecord) {
        self.records
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(record);
    }
}

/// Sink that hands every record to two sinks, the first before the second.
///
/// A diagnostic adapter uses it to keep a record in memory and also give it
/// to a [`DirectorySink`], so that a run which fails, is abandoned on an
/// error return or unwinds is still written for the harness by the session
/// itself, not by code that runs only after a successful return.
pub struct TeeSink {
    first: Box<dyn RecordSink>,
    second: Box<dyn RecordSink>,
}

impl TeeSink {
    /// Deliver each record to `first` and then to `second`.
    pub fn new(first: Box<dyn RecordSink>, second: Box<dyn RecordSink>) -> Self {
        Self { first, second }
    }
}

impl RecordSink for TeeSink {
    fn accept(&mut self, record: MeasurementRecord) {
        self.first.accept(record.clone());
        self.second.accept(record);
    }
}

#[derive(Debug, Default)]
struct DirectoryLog {
    written: Vec<PathBuf>,
    partial: Vec<PathBuf>,
    errors: Vec<io::ErrorKind>,
}

/// What a [`DirectorySink`] wrote and which writes failed.
#[derive(Clone, Debug, Default)]
pub struct DirectoryReport {
    log: Arc<Mutex<DirectoryLog>>,
}

impl DirectoryReport {
    /// Paths of the Norito files written so far; each has a `.json` sibling.
    pub fn written(&self) -> Vec<PathBuf> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .written
            .clone()
    }

    /// Norito files whose `.json` view could not be written. Such a file is
    /// left in place, because nothing is ever removed, and the harness
    /// reports it as a record without its view.
    pub fn partial(&self) -> Vec<PathBuf> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .partial
            .clone()
    }

    /// Kinds of the I/O errors that prevented a record from being written.
    pub fn errors(&self) -> Vec<io::ErrorKind> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .errors
            .clone()
    }
}

/// Sink that writes `<stem>-<pid>-<sequence>.norito` and its `.json` view.
///
/// Files are created exclusively: an existing record, including an earlier
/// failure, is never overwritten or removed. The two files of a record are
/// not written atomically: when the `.json` view fails after the `.norito`
/// file exists, the error and the partial file are reported through
/// [`DirectoryReport`].
#[derive(Debug)]
pub struct DirectorySink {
    directory: PathBuf,
    stem: &'static str,
    max_sequence: u32,
    report: DirectoryReport,
}

/// A record could not be written, in whole or in part.
struct WriteFailure {
    error: io::Error,
    /// The Norito file that exists without its JSON view, if any.
    partial: Option<PathBuf>,
}

impl From<io::Error> for WriteFailure {
    fn from(error: io::Error) -> Self {
        Self {
            error,
            partial: None,
        }
    }
}

impl DirectorySink {
    /// Write records into `directory` under the public label `stem`.
    pub fn new(directory: PathBuf, stem: &'static str) -> Self {
        Self {
            directory,
            stem: crate::text::public_or_invalid(stem).0,
            max_sequence: MAX_SEQUENCE,
            report: DirectoryReport::default(),
        }
    }

    /// Handle through which a diagnostic adapter learns what was written.
    pub fn report(&self) -> DirectoryReport {
        self.report.clone()
    }

    fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn write_record(&self, record: &MeasurementRecord) -> Result<PathBuf, WriteFailure> {
        let bytes = record
            .to_norito_bytes()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut view = record.to_json_view();
        view.push('\n');
        let process = std::process::id();
        for sequence in 0..=self.max_sequence {
            let base = format!("{}-{process}-{sequence:04}", self.stem);
            let binary = self.directory.join(format!("{base}.norito"));
            match Self::write_new(&binary, &bytes) {
                Ok(()) => {
                    return match Self::write_new(
                        &self.directory.join(format!("{base}.json")),
                        view.as_bytes(),
                    ) {
                        Ok(()) => Ok(binary),
                        Err(error) => Err(WriteFailure {
                            error,
                            partial: Some(binary),
                        }),
                    };
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "every record sequence number is taken",
        )
        .into())
    }
}

impl RecordSink for DirectorySink {
    fn accept(&mut self, record: MeasurementRecord) {
        let result = self.write_record(&record);
        let mut log = self
            .report
            .log
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match result {
            Ok(path) => log.written.push(path),
            Err(failure) => {
                log.errors.push(failure.error.kind());
                log.partial.extend(failure.partial);
            }
        }
    }
}

/// The harness hand-off context could not be read.
#[derive(Debug)]
pub enum HandoffError {
    /// The context file is absent, unreadable, not regular or too large.
    Io(io::Error),
    /// The context file does not follow the context schema.
    Schema(SchemaError),
}

impl core::fmt::Display for HandoffError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "harness context is unreadable: {error}"),
            Self::Schema(error) => write!(f, "harness context is malformed: {error}"),
        }
    }
}

impl std::error::Error for HandoffError {}

/// Read the [`RunContext`] the harness wrote into its output directory.
///
/// Only test and diagnostic adapters call this, with the directory they read
/// from [`crate::HARNESS_OUTPUT_DIR_ENV`].
///
/// # Errors
/// Fails when the file is absent, not a bounded regular file, or malformed.
pub fn read_harness_context(directory: &Path) -> Result<RunContext, HandoffError> {
    let path = directory.join(HARNESS_CONTEXT_FILE);
    let metadata = std::fs::symlink_metadata(&path).map_err(HandoffError::Io)?;
    if !metadata.is_file() || metadata.len() > MAX_CONTEXT_BYTES {
        return Err(HandoffError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "harness context is not a bounded regular file",
        )));
    }
    let mut text = String::new();
    std::fs::File::open(&path)
        .map_err(HandoffError::Io)?
        .take(MAX_CONTEXT_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(HandoffError::Io)?;
    RunContext::from_json_view(&text).map_err(HandoffError::Schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{sample_record, scratch_directory};

    #[test]
    fn collecting_sink_shares_storage_and_drains_in_delivery_order() {
        let sink = CollectingSink::new();
        let mut writer = sink.clone();
        let mut first = sample_record();
        first.identity.workload = "first".into();
        let mut second = sample_record();
        second.identity.workload = "second".into();
        writer.accept(first.clone());
        writer.accept(second.clone());
        assert_eq!(sink.take(), vec![first, second]);
        assert!(sink.take().is_empty());
    }

    #[test]
    fn directory_sink_writes_both_views_and_never_overwrites() {
        let directory = scratch_directory("directory-sink");
        let mut sink = DirectorySink::new(directory.clone(), "phase-tree");
        let report = sink.report();
        let record = sample_record();
        sink.accept(record.clone());
        sink.accept(record.clone());
        let written = report.written();
        assert_eq!(written.len(), 2);
        assert_ne!(written[0], written[1]);
        assert!(report.errors().is_empty());
        for path in &written {
            let bytes = std::fs::read(path).unwrap();
            assert_eq!(
                MeasurementRecord::from_norito_bytes(&bytes).unwrap(),
                record
            );
            let view = std::fs::read_to_string(path.with_extension("json")).unwrap();
            assert!(view.ends_with('\n'));
            assert_eq!(MeasurementRecord::from_json_view(&view).unwrap(), record);
        }
        let name = written[0].file_name().unwrap().to_str().unwrap().to_owned();
        assert_eq!(
            name,
            format!("phase-tree-{}-0000.norito", std::process::id())
        );
        let before = std::fs::read(&written[0]).unwrap();
        let mut changed = record;
        changed.identity.workload = "changed".into();
        sink.accept(changed);
        assert_eq!(std::fs::read(&written[0]).unwrap(), before);
        assert_eq!(report.written().len(), 3);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_sink_reports_a_record_left_without_its_json_view() {
        let directory = scratch_directory("directory-sink-partial");
        let mut sink = DirectorySink::new(directory.clone(), "phase-tree");
        let report = sink.report();
        // The JSON name of the first sequence number is already taken, so the
        // Norito file is created and its view cannot be.
        let base = format!("phase-tree-{}-0000", std::process::id());
        std::fs::write(directory.join(format!("{base}.json")), "occupied").unwrap();
        sink.accept(sample_record());
        assert!(report.written().is_empty());
        assert_eq!(report.errors(), vec![io::ErrorKind::AlreadyExists]);
        let partial = directory.join(format!("{base}.norito"));
        assert_eq!(report.partial(), vec![partial.clone()]);
        // Nothing is removed or overwritten: both files are still there.
        assert_eq!(
            MeasurementRecord::from_norito_bytes(&std::fs::read(&partial).unwrap()).unwrap(),
            sample_record()
        );
        assert_eq!(
            std::fs::read_to_string(directory.join(format!("{base}.json"))).unwrap(),
            "occupied"
        );
        // The next record takes the next sequence number and is complete.
        sink.accept(sample_record());
        assert_eq!(report.written().len(), 1);
        assert_eq!(report.partial().len(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_sink_gives_up_when_every_sequence_number_is_taken() {
        let directory = scratch_directory("directory-sink-exhausted");
        let mut sink = DirectorySink::new(directory.clone(), "phase-tree");
        sink.max_sequence = 1;
        let report = sink.report();
        for _ in 0..3 {
            sink.accept(sample_record());
        }
        // Sequence numbers 0 and 1 were written; the third record found none.
        assert_eq!(report.written().len(), 2);
        assert_eq!(report.errors(), vec![io::ErrorKind::AlreadyExists]);
        assert!(report.partial().is_empty());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 4);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn tee_sink_delivers_every_record_to_both_sinks_in_order() {
        let directory = scratch_directory("tee-sink");
        let memory = CollectingSink::new();
        let files = DirectorySink::new(directory.clone(), "phase-tree");
        let report = files.report();
        let mut tee = TeeSink::new(Box::new(memory.clone()), Box::new(files));
        let mut failed = sample_record();
        failed.identity.workload = "second".into();
        tee.accept(sample_record());
        tee.accept(failed.clone());
        assert_eq!(memory.take(), vec![sample_record(), failed.clone()]);
        let written = report.written();
        assert_eq!(written.len(), 2);
        assert_eq!(
            MeasurementRecord::from_norito_bytes(&std::fs::read(&written[1]).unwrap()).unwrap(),
            failed
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_sink_reports_a_missing_directory_instead_of_panicking() {
        let directory = scratch_directory("directory-sink-missing").join("absent");
        let mut sink = DirectorySink::new(directory, "not a label");
        assert_eq!(sink.stem, crate::text::INVALID_LABEL);
        let report = sink.report();
        sink.accept(sample_record());
        assert!(report.written().is_empty());
        assert_eq!(report.errors(), vec![io::ErrorKind::NotFound]);
    }

    #[test]
    fn harness_context_round_trips_and_rejects_missing_large_or_malformed_files() {
        let directory = scratch_directory("context");
        assert!(matches!(
            read_harness_context(&directory),
            Err(HandoffError::Io(_))
        ));
        let context = crate::test_support::bound_context();
        std::fs::write(directory.join(HARNESS_CONTEXT_FILE), context.to_json_view()).unwrap();
        assert_eq!(read_harness_context(&directory).unwrap(), context);
        std::fs::write(directory.join(HARNESS_CONTEXT_FILE), "{\"schema\":1}").unwrap();
        let malformed = read_harness_context(&directory).unwrap_err();
        assert!(matches!(malformed, HandoffError::Schema(_)));
        assert!(malformed.to_string().contains("malformed"));
        std::fs::write(
            directory.join(HARNESS_CONTEXT_FILE),
            vec![b' '; usize::try_from(MAX_CONTEXT_BYTES).unwrap() + 1],
        )
        .unwrap();
        let oversized = read_harness_context(&directory).unwrap_err();
        assert!(matches!(oversized, HandoffError::Io(_)));
        assert!(oversized.to_string().contains("unreadable"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
