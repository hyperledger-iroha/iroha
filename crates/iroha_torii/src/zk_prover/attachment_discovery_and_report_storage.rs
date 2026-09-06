#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AttachmentLocation {
    tenant_key: String,
    id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AttachmentDiscoveryGeometry {
    max_locations: usize,
    max_work_items: u64,
}
impl AttachmentDiscoveryGeometry {
    fn from_scan_bytes(max_scan_bytes: u64) -> Self {
        let max_locations_u64 = max_scan_bytes
            .div_ceil(ATTACHMENT_DISCOVERY_BYTES_PER_LOCATION)
            .min(ATTACHMENT_DISCOVERY_MAX_LOCATIONS);
        let max_locations = usize::try_from(max_locations_u64)
            .expect("the hard attachment discovery cap fits usize");
        let max_work_items = max_locations_u64
            .saturating_mul(ATTACHMENT_DISCOVERY_WORK_PER_LOCATION)
            .min(ATTACHMENT_DISCOVERY_MAX_WORK_ITEMS);
        Self {
            max_locations,
            max_work_items,
        }
    }
}
#[cfg(unix)]
struct TrackedDirectoryEntries {
    entries: rustix::fs::Dir,
}
#[cfg(unix)]
impl TrackedDirectoryEntries {
    fn open(_path: &Path, pinned: &fs::File) -> std::io::Result<Self> {
        Ok(Self {
            entries: rustix::fs::Dir::read_from(pinned).map_err(std::io::Error::from)?,
        })
    }

    fn next_name(&mut self) -> std::io::Result<Option<String>> {
        loop {
            let Some(entry) = self.entries.next() else {
                return Ok(None);
            };
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if matches!(bytes, b"." | b"..") {
                continue;
            }
            return std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map(Some)
                .map_err(|_| {
                    IoError::new(
                        IoErrorKind::InvalidData,
                        "ZK attachment directory contains a non-UTF-8 entry",
                    )
                });
        }
    }
}
#[cfg(windows)]
struct TrackedDirectoryEntries {
    entries: crate::secure_file_metadata::DirectDirectoryEntryStream,
}
#[cfg(windows)]
impl TrackedDirectoryEntries {
    fn open(path: &Path, _pinned: &fs::File) -> std::io::Result<Self> {
        Ok(Self {
            entries: crate::secure_file_metadata::DirectDirectoryEntryStream::open(path)?,
        })
    }

    fn next_name(&mut self) -> std::io::Result<Option<String>> {
        self.entries
            .next_name()?
            .map(|name| {
                name.into_string().map_err(|_| {
                    IoError::new(
                        IoErrorKind::InvalidData,
                        "ZK attachment directory contains a non-UTF-8 entry",
                    )
                })
            })
            .transpose()
    }
}
#[cfg(not(any(unix, windows)))]
struct TrackedDirectoryEntries;
#[cfg(not(any(unix, windows)))]
impl TrackedDirectoryEntries {
    fn open(_path: &Path, _pinned: &fs::File) -> std::io::Result<Self> {
        Err(IoError::new(
            IoErrorKind::Unsupported,
            "secure attachment directory enumeration is unsupported on this platform",
        ))
    }

    fn next_name(&mut self) -> std::io::Result<Option<String>> {
        Err(IoError::new(
            IoErrorKind::Unsupported,
            "secure attachment directory enumeration is unsupported on this platform",
        ))
    }
}
struct AttachmentDirectoryStream {
    root: PathBuf,
    root_identity: crate::secure_file_metadata::SecureMetadata,
    tenant_entries: TrackedDirectoryEntries,
    current_tenant: Option<(
        String,
        crate::secure_file_metadata::SecureMetadata,
        TrackedDirectoryEntries,
    )>,
}
struct AttachmentDiscoveryState {
    root: PathBuf,
    stream: Option<AttachmentDirectoryStream>,
    // Locations discovered before a byte/time stop are retried ahead of new
    // directory work. The same hard location cap bounds this queue.
    retry_locations: Vec<AttachmentLocation>,
}
enum AttachmentDirectoryStep {
    Advanced,
    Location(AttachmentLocation),
    Complete,
}
impl AttachmentDirectoryStream {
    fn open(root: PathBuf) -> std::io::Result<Self> {
        let root_pin = super::zk_attachments::open_pinned_direct_directory(&root)?
            .ok_or_else(|| IoError::new(IoErrorKind::NotFound, "ZK attachment root is missing"))?;
        let root_identity = crate::secure_file_metadata::from_path(&root)?;
        verify_tracked_prover_directory(&root, &root_identity, &root_pin)?;
        let tenant_entries = TrackedDirectoryEntries::open(&root, &root_pin)?;
        verify_tracked_prover_directory(&root, &root_identity, &root_pin)?;
        drop(root_pin);
        Ok(Self {
            root,
            root_identity,
            tenant_entries,
            current_tenant: None,
        })
    }
    /// Advance by at most one directory-iterator operation.
    ///
    /// Keeping the iterator open across scan cycles ensures a bounded work
    /// window eventually reaches later entries instead of restarting at the
    /// beginning of an oversized namespace on every cycle.
    fn step(&mut self) -> std::io::Result<AttachmentDirectoryStep> {
        let root_pin = super::zk_attachments::open_pinned_direct_directory(&self.root)?
            .ok_or_else(|| IoError::new(IoErrorKind::NotFound, "ZK attachment root is missing"))?;
        verify_tracked_prover_directory(&self.root, &self.root_identity, &root_pin)?;
        if self.current_tenant.is_some() {
            let tenant_path = self
                .root
                .join(&self.current_tenant.as_ref().expect("checked above").0);
            let tenant_pin = match super::zk_attachments::open_pinned_direct_directory(&tenant_path)
            {
                Ok(Some(pin)) => pin,
                Ok(None) => {
                    self.current_tenant = None;
                    return Ok(AttachmentDirectoryStep::Advanced);
                }
                Err(error) => {
                    self.current_tenant = None;
                    return Err(error);
                }
            };
            let tenant_identity = &self.current_tenant.as_ref().expect("checked above").1;
            if let Err(error) =
                verify_tracked_prover_directory(&tenant_path, tenant_identity, &tenant_pin)
            {
                self.current_tenant = None;
                return Err(error);
            }
            let next = self
                .current_tenant
                .as_mut()
                .expect("checked above")
                .2
                .next_name()?;
            let Some(name) = next else {
                let validation = verify_tracked_prover_directory(
                    &tenant_path,
                    &self.current_tenant.as_ref().expect("checked above").1,
                    &tenant_pin,
                );
                self.current_tenant = None;
                validation?;
                return Ok(AttachmentDirectoryStep::Advanced);
            };
            let entry_path = tenant_path.join(&name);
            let opened = match super::zk_attachments::open_attachment_regular_file(&entry_path) {
                Ok(opened) => opened,
                Err(error) if error.kind() == IoErrorKind::NotFound => {
                    return Ok(AttachmentDirectoryStep::Advanced);
                }
                Err(error) => return Err(error),
            };
            if is_prover_persistence_temp_name(&name) {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
            let (raw_id, is_metadata) = if let Some(id) = name.strip_suffix(".json") {
                (id, true)
            } else if let Some(id) = name.strip_suffix(".bin") {
                (id, false)
            } else {
                return Err(IoError::new(
                    IoErrorKind::InvalidData,
                    format!("ZK attachment directory contains an unexpected entry: {name}"),
                ));
            };
            let id = sanitize_attachment_id(raw_id)
                .filter(|id| id == raw_id)
                .ok_or_else(|| {
                    IoError::new(
                        IoErrorKind::InvalidData,
                        format!("ZK attachment directory has a non-canonical entry: {name}"),
                    )
                })?;
            drop(opened);
            if !is_metadata {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
            verify_tracked_prover_directory(
                &tenant_path,
                &self
                    .current_tenant
                    .as_ref()
                    .expect("tenant remains active")
                    .1,
                &tenant_pin,
            )?;
            let tenant_key = self
                .current_tenant
                .as_ref()
                .expect("tenant remains active")
                .0
                .clone();
            return Ok(AttachmentDirectoryStep::Location(AttachmentLocation {
                tenant_key,
                id,
            }));
        }
        let Some(name) = self.tenant_entries.next_name()? else {
            verify_tracked_prover_directory(&self.root, &self.root_identity, &root_pin)?;
            return Ok(AttachmentDirectoryStep::Complete);
        };
        let tenant_key = sanitize_tenant_key(&name)
            .filter(|tenant| tenant == &name)
            .ok_or_else(|| {
                IoError::new(
                    IoErrorKind::InvalidData,
                    format!("ZK attachment root has a non-canonical tenant entry: {name}"),
                )
            })?;
        let tenant_path = self.root.join(&tenant_key);
        let tenant_handle = match super::zk_attachments::open_pinned_direct_directory(&tenant_path)
        {
            Ok(Some(handle)) => handle,
            Ok(None) => {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
            Err(error) => return Err(error),
        };
        let tenant_identity = crate::secure_file_metadata::from_path(&tenant_path)?;
        verify_tracked_prover_directory(&tenant_path, &tenant_identity, &tenant_handle)?;
        let entries = TrackedDirectoryEntries::open(&tenant_path, &tenant_handle)?;
        verify_tracked_prover_directory(&tenant_path, &tenant_identity, &tenant_handle)?;
        drop(tenant_handle);
        self.current_tenant = Some((tenant_key, tenant_identity, entries));
        Ok(AttachmentDirectoryStep::Advanced)
    }
}
fn is_prover_persistence_temp_name(name: &str) -> bool {
    name.strip_prefix(".tmp").is_some_and(|suffix| {
        suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}
fn validate_prover_persistence_name(name: &str) -> std::io::Result<()> {
    let raw_id = name.strip_suffix(".json").ok_or_else(|| {
        IoError::new(
            IoErrorKind::InvalidData,
            format!("ZK prover persistence directory contains an unexpected entry: {name}"),
        )
    })?;
    sanitize_report_id(raw_id)
        .filter(|clean| clean == raw_id)
        .map(|_| ())
        .ok_or_else(|| {
            IoError::new(
                IoErrorKind::InvalidData,
                format!("ZK prover persistence directory contains a non-canonical entry: {name}"),
            )
        })
}
#[cfg(unix)]
fn remove_prover_writer_temps_in(directory: &Path, max_entries: u64) -> std::io::Result<()> {
    let pinned =
        super::zk_attachments::open_pinned_direct_directory(directory)?.ok_or_else(|| {
            IoError::new(
                IoErrorKind::NotFound,
                format!(
                    "ZK prover persistence directory is missing: {}",
                    directory.display()
                ),
            )
        })?;
    let names = super::zk_attachments::pinned_directory_names(&pinned, max_entries)?;
    for name in names {
        if is_prover_persistence_temp_name(&name) {
            if super::zk_attachments::open_pinned_direct_regular_file(&pinned, &name)?.is_none() {
                return Err(IoError::new(
                    IoErrorKind::InvalidData,
                    "ZK prover temporary entry disappeared during recovery",
                ));
            }
            super::zk_attachments::unlink_pinned_regular_file_if_present(&pinned, &name)?;
        } else {
            validate_prover_persistence_name(&name)?;
            if super::zk_attachments::open_pinned_direct_regular_file(&pinned, &name)?.is_none() {
                return Err(IoError::new(
                    IoErrorKind::InvalidData,
                    "ZK prover persistence entry disappeared during recovery",
                ));
            }
        }
    }
    super::zk_attachments::sync_open_directory(&pinned)
}
#[cfg(windows)]
fn remove_prover_writer_temps_in(directory: &Path, max_entries: u64) -> std::io::Result<()> {
    use crate::secure_file_metadata::{from_file, from_path, is_direct_directory, same_file};

    let pinned =
        super::zk_attachments::open_pinned_direct_directory(directory)?.ok_or_else(|| {
            IoError::new(
                IoErrorKind::NotFound,
                format!(
                    "ZK prover persistence directory is missing: {}",
                    directory.display()
                ),
            )
        })?;
    let opened = from_file(&pinned)?;
    let names = super::zk_attachments::pinned_directory_names_at(directory, &pinned, max_entries)?;
    for name in names {
        let path = directory.join(&name);
        let child = super::zk_attachments::open_direct_regular_file_in_pinned_directory(
            directory, &pinned, &name,
        )?
        .ok_or_else(|| {
            IoError::new(
                IoErrorKind::InvalidData,
                "ZK prover persistence entry disappeared during recovery",
            )
        })?;
        drop(child);
        if is_prover_persistence_temp_name(&name) {
            super::zk_attachments::remove_direct_regular_file_if_present(&path)?;
        } else {
            validate_prover_persistence_name(&name)?;
        }
    }
    crate::durable_fs::sync_direct_directory(directory)?;
    let named_after = from_path(directory)?;
    let opened_after = from_file(&pinned)?;
    if !is_direct_directory(&named_after)
        || !is_direct_directory(&opened_after)
        || !same_file(&opened, &opened_after)
        || !same_file(&opened_after, &named_after)
    {
        return Err(IoError::new(
            IoErrorKind::InvalidData,
            "ZK prover persistence directory changed during recovery",
        ));
    }
    Ok(())
}
#[cfg(not(any(unix, windows)))]
fn remove_prover_writer_temps_in(_directory: &Path, _max_entries: u64) -> std::io::Result<()> {
    Err(IoError::new(
        IoErrorKind::Unsupported,
        "secure ZK prover recovery is unsupported on this platform",
    ))
}
fn recover_prover_writer_temps() -> std::io::Result<()> {
    let max_entries = cfg_reports_max_count()
        .max(iroha_config::parameters::defaults::torii::ZK_PROVER_REPORTS_MAX_COUNT)
        .saturating_add(1_024);
    remove_prover_writer_temps_in(&reports_dir(), max_entries)?;
    remove_prover_writer_temps_in(&report_index_dir(), max_entries)
}
#[derive(Debug, Default)]
struct AttachmentDiscovery {
    locations: Vec<AttachmentLocation>,
    work_items: u64,
    sweep_complete: bool,
    work_exhausted: bool,
    time_exhausted: bool,
}
impl AttachmentDiscovery {
    fn budget_reason(&self) -> Option<&'static str> {
        if self.time_exhausted {
            Some("time")
        } else if self.work_exhausted {
            Some("work")
        } else {
            None
        }
    }
    fn pending_estimate(&self) -> u64 {
        // An incomplete sweep cannot know the exact undiscovered backlog. A
        // single sentinel avoids advertising an empty queue while retaining
        // constant memory; the next cursor window refines the estimate.
        u64::try_from(self.locations.len())
            .unwrap_or(u64::MAX)
            .saturating_add(u64::from(!self.sweep_complete))
    }
}
fn sanitize_attachment_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() != ATTACHMENT_ID_HEX_LEN {
        return None;
    }
    if trimmed.bytes().any(|b| !b.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}
fn sanitize_report_id(raw: &str) -> Option<String> {
    sanitize_attachment_id(raw)
}
fn sanitize_tenant_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() != TENANT_KEY_HEX_LEN {
        return None;
    }
    if trimmed.bytes().any(|b| !b.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}
fn attachments_root_dir() -> PathBuf {
    super::zk_attachments::base_dir().join("zk_attachments")
}
fn attachment_meta_path(tenant_key: &str, id: &str) -> PathBuf {
    attachments_root_dir()
        .join(tenant_key)
        .join(format!("{}.json", id))
}
fn attachment_bin_path(tenant_key: &str, id: &str) -> PathBuf {
    attachments_root_dir()
        .join(tenant_key)
        .join(format!("{}.bin", id))
}
fn report_path_from_sanitized(id: &str) -> PathBuf {
    reports_dir().join(format!("{}.json", id))
}
fn report_index_dir() -> PathBuf {
    prover_dir().join("report_index")
}
fn report_summary_path_from_sanitized(id: &str) -> PathBuf {
    report_index_dir().join(format!("{id}.json"))
}
fn report_summary_lock() -> &'static Mutex<()> {
    REPORT_SUMMARY_LOCK.get_or_init(|| Mutex::new(()))
}
fn bounded_summary_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let marker = "...";
    let mut end = max_bytes.saturating_sub(marker.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = value[..end].to_owned();
    bounded.push_str(marker);
    bounded
}
fn report_summary_from_report(report: &ProverReport) -> ProverReportSummary {
    let zk1_tags = report.zk1_tags.as_ref().and_then(|tags| {
        let mut bounded = Vec::new();
        for tag in tags.iter().take(ZK1_MAX_TLV_COUNT) {
            let tag = bounded_summary_text(tag, REPORT_SUMMARY_TAG_MAX_BYTES);
            if !bounded.contains(&tag) {
                bounded.push(tag);
            }
        }
        (!bounded.is_empty()).then_some(bounded)
    });
    ProverReportSummary {
        id: report.id.clone(),
        ok: report.ok,
        error: report
            .error
            .as_deref()
            .map(|error| bounded_summary_text(error, REPORT_SUMMARY_ERROR_MAX_BYTES)),
        content_type: bounded_summary_text(
            &report.content_type,
            REPORT_SUMMARY_CONTENT_TYPE_MAX_BYTES,
        ),
        processed_ms: report.processed_ms,
        zk1_tags,
    }
}
fn bound_persisted_report_summary(mut summary: ProverReportSummary) -> ProverReportSummary {
    summary.error = summary
        .error
        .as_deref()
        .map(|error| bounded_summary_text(error, REPORT_SUMMARY_ERROR_MAX_BYTES));
    summary.content_type =
        bounded_summary_text(&summary.content_type, REPORT_SUMMARY_CONTENT_TYPE_MAX_BYTES);
    summary.zk1_tags = summary.zk1_tags.take().and_then(|tags| {
        let mut bounded = Vec::new();
        for tag in tags.into_iter().take(ZK1_MAX_TLV_COUNT) {
            let tag = bounded_summary_text(&tag, REPORT_SUMMARY_TAG_MAX_BYTES);
            if !bounded.contains(&tag) {
                bounded.push(tag);
            }
        }
        (!bounded.is_empty()).then_some(bounded)
    });
    summary
}
#[cfg(test)]
fn normalize_report_summaries(raw: Vec<ProverReportSummary>) -> Vec<ProverReportSummary> {
    let mut by_id: BTreeMap<String, ProverReportSummary> = BTreeMap::new();
    for mut summary in raw {
        let Some(clean) = sanitize_report_id(&summary.id) else {
            continue;
        };
        summary.id = clean.clone();
        by_id.insert(clean, summary);
    }
    by_id.into_values().collect()
}
fn persist_report_summary_locked(summary: &ProverReportSummary) -> std::io::Result<()> {
    let Some(id) = sanitize_report_id(&summary.id).filter(|id| id == &summary.id) else {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "invalid prover report summary id",
        ));
    };
    ensure_dirs()?;
    let path = report_summary_path_from_sanitized(&id);
    let mut normalized = summary.clone();
    normalized.id = id;
    let body = norito::json::to_json(&normalized)
        .map_err(|error| IoError::new(IoErrorKind::InvalidData, error.to_string()))?;
    if body.len() as u64 > REPORT_SUMMARY_FILE_MAX_BYTES {
        return Err(IoError::new(
            IoErrorKind::InvalidData,
            "prover report summary exceeds the hard size limit",
        ));
    }
    super::zk_attachments::persist_bytes_atomically(&path, body.as_bytes(), ".tmp")
}
