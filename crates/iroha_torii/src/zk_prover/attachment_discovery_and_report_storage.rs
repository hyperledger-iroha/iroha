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
    fn step(&mut self) -> AttachmentDirectoryStep {
        if self.current_tenant.is_some() {
            let next = self
                .current_tenant
                .as_mut()
                .expect("checked above")
                .1
                .next();
            let Some(entry) = next else {
                self.current_tenant = None;
                return AttachmentDirectoryStep::Advanced;
            };
            let Ok(entry) = entry else {
                return AttachmentDirectoryStep::Advanced;
            };
            if !entry.file_type().is_ok_and(|file_type| file_type.is_file()) {
                return AttachmentDirectoryStep::Advanced;
            }
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                return AttachmentDirectoryStep::Advanced;
            };
            let Some(id) = name.strip_suffix(".json") else {
                return AttachmentDirectoryStep::Advanced;
            };
            let Some(id) = sanitize_attachment_id(id) else {
                return AttachmentDirectoryStep::Advanced;
            };
            let tenant_key = self
                .current_tenant
                .as_ref()
                .expect("tenant remains active")
                .0
                .clone();
            return AttachmentDirectoryStep::Location(AttachmentLocation { tenant_key, id });
        }

        let Some(entry) = self.tenant_entries.next() else {
            return AttachmentDirectoryStep::Complete;
        };
        let Ok(entry) = entry else {
            return AttachmentDirectoryStep::Advanced;
        };
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            return AttachmentDirectoryStep::Advanced;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            return AttachmentDirectoryStep::Advanced;
        };
        let Some(tenant_key) = sanitize_tenant_key(name) else {
            return AttachmentDirectoryStep::Advanced;
        };
        if let Ok(entries) = fs::read_dir(self.root.join(&tenant_key)) {
            self.current_tenant = Some((tenant_key, entries));
        }
        AttachmentDirectoryStep::Advanced
    }
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
    let Some(id) = sanitize_report_id(&summary.id) else {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "invalid prover report summary id",
        ));
    };
    ensure_dirs();
    fs::create_dir_all(report_index_dir())?;
    let path = report_summary_path_from_sanitized(&id);
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
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
    use std::io::Write as _;
    tmp.write_all(body.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path).map(|_| ()).map_err(|e| e.error)
}
