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
struct AttachmentDirectoryStream {
    root: PathBuf,
    tenant_entries: fs::ReadDir,
    current_tenant: Option<(String, fs::ReadDir)>,
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
        super::zk_attachments::verify_direct_directory(&root)?;
        let tenant_entries = fs::read_dir(&root)?;
        Ok(Self {
            root,
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
        if self.current_tenant.is_some() {
            let next = self
                .current_tenant
                .as_mut()
                .expect("checked above")
                .1
                .next();
            let Some(entry) = next else {
                let tenant_key = &self.current_tenant.as_ref().expect("checked above").0;
                match super::zk_attachments::verify_direct_directory(&self.root.join(tenant_key)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == IoErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
                self.current_tenant = None;
                return Ok(AttachmentDirectoryStep::Advanced);
            };
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if error.kind() == IoErrorKind::NotFound => {
                    return Ok(AttachmentDirectoryStep::Advanced);
                }
                Err(error) => return Err(error),
            };
            let file_name = entry.file_name();
            let name = file_name.to_str().ok_or_else(|| {
                IoError::new(
                    IoErrorKind::InvalidData,
                    "ZK attachment directory contains a non-UTF-8 entry",
                )
            })?;
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) if error.kind() == IoErrorKind::NotFound => {
                    return Ok(AttachmentDirectoryStep::Advanced);
                }
                Err(error) => return Err(error),
            };
            if is_prover_persistence_temp_name(name) {
                if !file_type.is_file() {
                    return Err(IoError::new(
                        IoErrorKind::InvalidData,
                        format!(
                            "ZK attachment temporary path is not a direct regular file: {}",
                            entry.path().display()
                        ),
                    ));
                }
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
            if !file_type.is_file() {
                return Err(IoError::new(
                    IoErrorKind::InvalidData,
                    format!(
                        "ZK attachment metadata path is not a direct regular file: {}",
                        entry.path().display()
                    ),
                ));
            }
            if !is_metadata {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
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
        let Some(entry) = self.tenant_entries.next() else {
            super::zk_attachments::verify_direct_directory(&self.root)?;
            return Ok(AttachmentDirectoryStep::Complete);
        };
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == IoErrorKind::NotFound => {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
            Err(error) => return Err(error),
        };
        let file_name = entry.file_name();
        let name = file_name.to_str().ok_or_else(|| {
            IoError::new(
                IoErrorKind::InvalidData,
                "ZK attachment root contains a non-UTF-8 entry",
            )
        })?;
        let tenant_key = sanitize_tenant_key(name)
            .filter(|tenant| tenant == name)
            .ok_or_else(|| {
                IoError::new(
                    IoErrorKind::InvalidData,
                    format!("ZK attachment root has a non-canonical tenant entry: {name}"),
                )
            })?;
        let tenant_path = self.root.join(&tenant_key);
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) if error.kind() == IoErrorKind::NotFound => {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
            Err(error) => return Err(error),
        };
        if !file_type.is_dir() {
            return Err(IoError::new(
                IoErrorKind::InvalidData,
                format!(
                    "ZK attachment tenant path is not a direct directory: {}",
                    tenant_path.display()
                ),
            ));
        }
        match super::zk_attachments::verify_direct_directory(&tenant_path) {
            Ok(()) => {}
            Err(error) if error.kind() == IoErrorKind::NotFound => {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
            Err(error) => return Err(error),
        }
        let entries = match fs::read_dir(&tenant_path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == IoErrorKind::NotFound => {
                return Ok(AttachmentDirectoryStep::Advanced);
            }
            Err(error) => return Err(error),
        };
        self.current_tenant = Some((tenant_key, entries));
        Ok(AttachmentDirectoryStep::Advanced)
    }
}
fn is_prover_persistence_temp_name(name: &str) -> bool {
    name.strip_prefix(".tmp").is_some_and(|suffix| {
        suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}
fn remove_prover_writer_temps_in(directory: &Path, max_entries: u64) -> std::io::Result<()> {
    super::zk_attachments::verify_direct_directory(directory)?;
    let mut scanned = 0_u64;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        scanned = scanned.saturating_add(1);
        if scanned > max_entries {
            return Err(IoError::new(
                IoErrorKind::InvalidData,
                format!(
                    "ZK prover temporary-file recovery exceeds {max_entries} entries in {}",
                    directory.display()
                ),
            ));
        }
        let file_name = entry.file_name();
        let name = file_name.to_str().ok_or_else(|| {
            IoError::new(
                IoErrorKind::InvalidData,
                "ZK prover persistence directory contains a non-UTF-8 entry",
            )
        })?;
        if !is_prover_persistence_temp_name(name) {
            let raw_id = name.strip_suffix(".json").ok_or_else(|| {
                IoError::new(
                    IoErrorKind::InvalidData,
                    format!("ZK prover persistence directory contains an unexpected entry: {name}"),
                )
            })?;
            sanitize_report_id(raw_id)
                .filter(|clean| clean == raw_id)
                .ok_or_else(|| {
                    IoError::new(
                        IoErrorKind::InvalidData,
                        format!(
                            "ZK prover persistence directory contains a non-canonical entry: {name}"
                        ),
                    )
                })?;
            if !entry.file_type()?.is_file() {
                return Err(IoError::new(
                    IoErrorKind::InvalidData,
                    format!(
                        "ZK prover persistence path is not a direct regular file: {}",
                        entry.path().display()
                    ),
                ));
            }
            drop(super::zk_attachments::open_attachment_regular_file(
                &entry.path(),
            )?);
            continue;
        }
        if !entry.file_type()?.is_file() {
            return Err(IoError::new(
                IoErrorKind::InvalidData,
                format!(
                    "ZK prover temporary path is not a direct regular file: {}",
                    entry.path().display()
                ),
            ));
        }
        drop(super::zk_attachments::open_attachment_regular_file(
            &entry.path(),
        )?);
        remove_file_if_present(&entry.path())?;
    }
    super::zk_attachments::verify_direct_directory(directory)
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
