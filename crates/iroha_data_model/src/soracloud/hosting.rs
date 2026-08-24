/// Backend family admitted for authoritative HF placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "backend_family", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfBackendFamilyV1 {
    /// Hugging Face Transformers-style local execution.
    Transformers,
    /// GGUF-backed local execution.
    Gguf,
}
/// Canonical weight/layout format admitted for authoritative HF placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "model_format", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfModelFormatV1 {
    /// Safetensors checkpoint layout.
    Safetensors,
    /// `PyTorch` `.bin` / `.pt` / `.pth` / `.ot` checkpoint layout.
    PyTorch,
    /// GGUF layout.
    Gguf,
}
/// Canonical GGUF weight-file suffixes admitted by HF profile derivation and import.
pub const SORA_HF_GGUF_WEIGHT_FILE_EXTENSIONS_V1: &[&str] = &[".gguf"];
/// Canonical SafeTensors weight-file suffixes admitted by HF profile derivation and import.
pub const SORA_HF_SAFETENSORS_WEIGHT_FILE_EXTENSIONS_V1: &[&str] = &[".safetensors"];
/// Canonical PyTorch-compatible weight-file suffixes admitted by HF profile derivation and import.
pub const SORA_HF_PYTORCH_WEIGHT_FILE_EXTENSIONS_V1: &[&str] = &[".bin", ".pt", ".pth", ".ot"];
/// Complete canonical HF weight-file suffix contract for the first release.
///
/// Every file matching one of these suffixes is model executable material and
/// therefore requires authenticated LFS SHA-256 and size metadata before it is
/// imported. Keep format-specific selection and runtime integrity enforcement
/// sourced from this table so a newly supported format cannot bypass either.
pub const SORA_HF_WEIGHT_FILE_EXTENSIONS_V1: &[&str] =
    &[".gguf", ".safetensors", ".bin", ".pt", ".pth", ".ot"];
/// Maximum number of provider-controlled sibling entries decoded from one HF model-info response.
pub const SORA_HF_MODEL_INFO_MAX_SIBLINGS_V1: usize = 4_096;
/// Maximum byte length of one provider-controlled HF model-info string used as a file path.
pub const SORA_HF_MODEL_INFO_MAX_STRING_BYTES_V1: usize = 4 * 1_024;
/// Maximum number of `/`-separated components in one selected HF weight-file path.
pub const SORA_HF_WEIGHT_PATH_MAX_COMPONENTS_V1: usize = 64;
/// One immutable, authenticated weight shard selected from HF model-info metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoraHfRequiredWeightFileV1 {
    /// Exact repository-relative sibling path.
    pub path: String,
    /// Positive byte length authenticated by the Hub LFS record.
    pub content_length: u64,
    /// Canonical lowercase LFS SHA-256 digest.
    pub lfs_sha256: String,
}
/// Complete precedence-selected immutable HF weight contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoraHfWeightSelectionV1 {
    /// Runtime backend family implied by the selected format.
    pub backend_family: SoraHfBackendFamilyV1,
    /// Highest-precedence weight format present in the sibling set.
    pub model_format: SoraHfModelFormatV1,
    /// Exact sorted and deduplicated selected shard set.
    pub required_weight_files: Vec<SoraHfRequiredWeightFileV1>,
    /// Checked sum of all selected authenticated LFS sizes.
    pub required_model_bytes: u64,
    /// Domain-separated commitment to format, paths, sizes, and LFS digests.
    pub weight_selection_commitment: Hash,
}
#[cfg(feature = "json")]
fn hf_model_format_for_path_v1(path: &str) -> Option<SoraHfModelFormatV1> {
    let has_extension = |extensions: &[&str]| {
        extensions.iter().any(|extension| {
            path.get(path.len().saturating_sub(extension.len())..)
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(extension))
        })
    };
    if has_extension(SORA_HF_GGUF_WEIGHT_FILE_EXTENSIONS_V1) {
        Some(SoraHfModelFormatV1::Gguf)
    } else if has_extension(SORA_HF_SAFETENSORS_WEIGHT_FILE_EXTENSIONS_V1) {
        Some(SoraHfModelFormatV1::Safetensors)
    } else if has_extension(SORA_HF_PYTORCH_WEIGHT_FILE_EXTENSIONS_V1) {
        Some(SoraHfModelFormatV1::PyTorch)
    } else {
        None
    }
}
#[cfg(feature = "json")]
fn hf_weight_path_is_canonical_v1(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= SORA_HF_MODEL_INFO_MAX_STRING_BYTES_V1
        && path.trim() == path
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').count() <= SORA_HF_WEIGHT_PATH_MAX_COMPONENTS_V1
        && path
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}
/// Derive the exact first-release weight contract from one HF model-info response.
///
/// Selection precedence is GGUF, SafeTensors, then `PyTorch`. The complete provider sibling array
/// is bounded before processing; duplicate paths are sorted and coalesced only when their LFS
/// metadata is identical. Every selected shard must carry a positive LFS size and canonical
/// lowercase SHA-256 digest and must fit the configured per-file and aggregate importer budgets.
///
/// # Errors
/// Returns [`SoracloudManifestError`] when provider metadata is malformed, ambiguous, unbounded,
/// unauthenticated, or exceeds the supplied import limits.
#[cfg(feature = "json")]
pub fn derive_hf_weight_selection_v1(
    model_info: &Value,
    maximum_files: u32,
    maximum_file_bytes: u64,
    maximum_total_bytes: u64,
) -> Result<Option<SoraHfWeightSelectionV1>, SoracloudManifestError> {
    let manifest = "Hugging Face model-info";
    if maximum_files == 0 || maximum_file_bytes == 0 || maximum_total_bytes == 0 {
        return Err(invalid_field(
            manifest,
            "import_limits",
            "file count, per-file bytes, and aggregate bytes must all be greater than zero",
        ));
    }
    let Some(siblings) = model_info.get("siblings").and_then(Value::as_array) else {
        return Ok(None);
    };
    if siblings.len() > SORA_HF_MODEL_INFO_MAX_SIBLINGS_V1 {
        return Err(invalid_field(
            manifest,
            "siblings",
            format!(
                "contains {} entries, exceeding the {}-entry limit",
                siblings.len(),
                SORA_HF_MODEL_INFO_MAX_SIBLINGS_V1
            ),
        ));
    }
    let mut records = BTreeMap::<String, Option<(String, u64)>>::new();
    for entry in siblings {
        let Some(path) = entry.get("rfilename").and_then(Value::as_str) else {
            continue;
        };
        if path.len() > SORA_HF_MODEL_INFO_MAX_STRING_BYTES_V1 {
            return Err(invalid_field(
                manifest,
                "siblings",
                format!(
                    "path exceeds the {}-byte limit",
                    SORA_HF_MODEL_INFO_MAX_STRING_BYTES_V1
                ),
            ));
        }
        let lfs = match entry.get("lfs") {
            None | Some(Value::Null) => None,
            Some(lfs) => {
                let lfs = lfs.as_object().ok_or_else(|| {
                    invalid_field(
                        manifest,
                        "siblings",
                        format!("sibling `{path}` has non-object LFS metadata"),
                    )
                })?;
                let sha256 = lfs.get("sha256").and_then(Value::as_str).ok_or_else(|| {
                    invalid_field(
                        manifest,
                        "siblings",
                        format!("sibling `{path}` omits its LFS SHA-256 digest"),
                    )
                })?;
                if sha256.len() != 64
                    || !sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(invalid_field(
                        manifest,
                        "siblings",
                        format!("sibling `{path}` has a noncanonical LFS SHA-256 digest"),
                    ));
                }
                let size = lfs
                    .get("size")
                    .and_then(Value::as_u64)
                    .filter(|size| *size > 0)
                    .ok_or_else(|| {
                        invalid_field(
                            manifest,
                            "siblings",
                            format!("sibling `{path}` lacks a positive integer LFS size"),
                        )
                    })?;
                Some((sha256.to_owned(), size))
            }
        };
        if let Some(previous) = records.insert(path.to_owned(), lfs.clone())
            && previous != lfs
        {
            return Err(invalid_field(
                manifest,
                "siblings",
                format!("sibling `{path}` has conflicting integrity metadata"),
            ));
        }
    }
    let selected_format = [
        SoraHfModelFormatV1::Gguf,
        SoraHfModelFormatV1::Safetensors,
        SoraHfModelFormatV1::PyTorch,
    ]
    .into_iter()
    .find(|format| {
        records
            .keys()
            .any(|path| hf_model_format_for_path_v1(path) == Some(*format))
    });
    let Some(model_format) = selected_format else {
        return Ok(None);
    };
    let maximum_files = usize::try_from(maximum_files).map_err(|error| {
        invalid_field(
            manifest,
            "import_limits",
            format!("file-count limit does not fit this host: {error}"),
        )
    })?;
    let selected_count = records
        .keys()
        .filter(|path| hf_model_format_for_path_v1(path) == Some(model_format))
        .count();
    if selected_count == 0 || selected_count > maximum_files {
        return Err(invalid_field(
            manifest,
            "siblings",
            format!(
                "selected weight set has {selected_count} files, outside 1..={maximum_files}"
            ),
        ));
    }
    let mut required_weight_files = Vec::new();
    required_weight_files
        .try_reserve_exact(selected_count)
        .map_err(|error| {
            invalid_field(
                manifest,
                "siblings",
                format!("failed to reserve the bounded selected weight set: {error}"),
            )
        })?;
    let mut required_model_bytes = 0_u64;
    for (path, lfs) in records
        .into_iter()
        .filter(|(path, _)| hf_model_format_for_path_v1(path) == Some(model_format))
    {
        if !hf_weight_path_is_canonical_v1(&path) {
            return Err(invalid_field(
                manifest,
                "siblings",
                format!("selected weight path `{path}` is not canonical"),
            ));
        }
        let (lfs_sha256, content_length) = lfs.ok_or_else(|| {
            invalid_field(
                manifest,
                "siblings",
                format!("selected weight `{path}` lacks authenticated LFS metadata"),
            )
        })?;
        if content_length > maximum_file_bytes {
            return Err(invalid_field(
                manifest,
                "siblings",
                format!(
                    "selected weight `{path}` has {content_length} bytes, exceeding the {maximum_file_bytes}-byte per-file limit"
                ),
            ));
        }
        required_model_bytes = required_model_bytes
            .checked_add(content_length)
            .ok_or_else(|| {
                invalid_field(manifest, "siblings", "selected weight byte total overflow")
            })?;
        if required_model_bytes > maximum_total_bytes {
            return Err(invalid_field(
                manifest,
                "siblings",
                format!(
                    "selected weight total {required_model_bytes} exceeds the {maximum_total_bytes}-byte aggregate limit"
                ),
            ));
        }
        required_weight_files.push(SoraHfRequiredWeightFileV1 {
            path,
            content_length,
            lfs_sha256,
        });
    }
    let format_label = match model_format {
        SoraHfModelFormatV1::Gguf => "gguf",
        SoraHfModelFormatV1::Safetensors => "safetensors",
        SoraHfModelFormatV1::PyTorch => "pytorch",
    };
    let commitment_records = required_weight_files
        .iter()
        .map(|weight| {
            (
                weight.path.as_str(),
                weight.content_length,
                weight.lfs_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let weight_selection_commitment = Hash::new(Encode::encode(&(
        "soracloud:hf-weight-selection:v1",
        format_label,
        commitment_records,
    )));
    let backend_family = match model_format {
        SoraHfModelFormatV1::Gguf => SoraHfBackendFamilyV1::Gguf,
        SoraHfModelFormatV1::Safetensors | SoraHfModelFormatV1::PyTorch => {
            SoraHfBackendFamilyV1::Transformers
        }
    };
    Ok(Some(SoraHfWeightSelectionV1 {
        backend_family,
        model_format,
        required_weight_files,
        required_model_bytes,
        weight_selection_commitment,
    }))
}
/// Deterministic size bucket used for adaptive placement and tariff lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "size_bucket", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfModelSizeBucketV1 {
    /// Models up to and including 2 GiB.
    Small,
    /// Models above 2 GiB and up to and including 8 GiB.
    Medium,
    /// Models above 8 GiB.
    Large,
}
/// Canonical resource profile derived from HF source/import metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfResourceProfileV1 {
    /// Canonical model bytes that must fit on a single assigned host.
    pub required_model_bytes: u64,
    /// Backend family required to execute the source locally.
    pub backend_family: SoraHfBackendFamilyV1,
    /// Weight/layout format required to execute the source locally.
    pub model_format: SoraHfModelFormatV1,
    /// Exact count of authenticated shards committed by `weight_selection_commitment`.
    pub selected_weight_file_count: u32,
    /// Domain-separated commitment to the exact selected paths, sizes, and LFS digests.
    pub weight_selection_commitment: Hash,
    /// Minimum on-host disk cache bytes required to keep the model resident.
    pub disk_cache_bytes_floor: u64,
    /// Minimum system RAM bytes required to run the model.
    pub ram_bytes_floor: u64,
    /// Minimum accelerator VRAM bytes required to run the model.
    pub vram_bytes_floor: u64,
}
impl SoraHfResourceProfileV1 {
    /// Validate the derived HF resource profile.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when any required capacity floor is zero or
    /// inconsistent with the canonical model size.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.required_model_bytes == 0 {
            return Err(invalid_field(
                "sora hf resource profile",
                "required_model_bytes",
                "must be greater than zero",
            ));
        }
        if self.disk_cache_bytes_floor < self.required_model_bytes {
            return Err(invalid_field(
                "sora hf resource profile",
                "disk_cache_bytes_floor",
                "must be greater than or equal to required_model_bytes",
            ));
        }
        if self.selected_weight_file_count == 0 {
            return Err(invalid_field(
                "sora hf resource profile",
                "selected_weight_file_count",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora hf resource profile",
            "weight_selection_commitment",
            self.weight_selection_commitment,
        )?;
        if self.ram_bytes_floor == 0 && self.vram_bytes_floor == 0 {
            return Err(invalid_field(
                "sora hf resource profile",
                "ram_bytes_floor",
                "either ram_bytes_floor or vram_bytes_floor must be greater than zero",
            ));
        }
        Ok(())
    }
    /// Return the deterministic model-size bucket for this profile.
    #[must_use]
    pub fn size_bucket(&self) -> SoraHfModelSizeBucketV1 {
        const TWO_GIB: u64 = 2 * 1024 * 1024 * 1024;
        const EIGHT_GIB: u64 = 8 * 1024 * 1024 * 1024;
        if self.required_model_bytes <= TWO_GIB {
            SoraHfModelSizeBucketV1::Small
        } else if self.required_model_bytes <= EIGHT_GIB {
            SoraHfModelSizeBucketV1::Medium
        } else {
            SoraHfModelSizeBucketV1::Large
        }
    }
}
/// Return the exact first-release maximum compute reservation charge for an HF shared-lease window.
///
/// Host reservation tariffs are nominal per-window charges in V1, so the amount does not scale with
/// `lease_term_ms`. The lease term is nevertheless part of this function's contract so callers
/// cannot accidentally quote a zero-duration window and so a future version cannot silently change
/// the signed arithmetic.
///
/// The cap is the adaptive placement target multiplied by the greatest
/// permitted V1 host-class tariff for the profile's model-size bucket:
///
/// - small: 3 hosts × 0.0000025 XOR;
/// - medium: 2 hosts × 0.000004 XOR;
/// - large: 2 hosts × 0.000006 XOR.
///
/// # Errors
/// Returns [`SoracloudManifestError`] when the profile is invalid or the lease term is zero.
pub fn hf_shared_lease_max_compute_reservation_fee_v1(
    resource_profile: &SoraHfResourceProfileV1,
    lease_term_ms: u64,
) -> Result<Quantity, SoracloudManifestError> {
    resource_profile.validate()?;
    if lease_term_ms == 0 {
        return Err(invalid_field(
            "sora hf shared lease compute reservation cap",
            "lease_term_ms",
            "must be greater than zero",
        ));
    }
    let nanos: u128 = match resource_profile.size_bucket() {
        SoraHfModelSizeBucketV1::Small => 7_500,
        SoraHfModelSizeBucketV1::Medium => 8_000,
        SoraHfModelSizeBucketV1::Large => 12_000,
    };
    Ok(xor_quantity_from_nanos(nanos))
}
/// Active opt-in validator host capability advert for authoritative HF placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelHostCapabilityRecordV1 {
    /// Schema version; must equal [`SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Validator account that owns this host advert.
    pub validator_account_id: AccountId,
    /// Peer identifier used for Soracloud routing.
    pub peer_id: String,
    /// Supported backend families.
    pub supported_backends: BTreeSet<SoraHfBackendFamilyV1>,
    /// Supported weight/layout formats.
    pub supported_formats: BTreeSet<SoraHfModelFormatV1>,
    /// Maximum canonical model bytes accepted by this host.
    pub max_model_bytes: u64,
    /// Maximum disk cache bytes reserved for resident models.
    pub max_disk_cache_bytes: u64,
    /// Maximum system RAM bytes reserved for resident models.
    pub max_ram_bytes: u64,
    /// Maximum accelerator VRAM bytes reserved for resident models.
    pub max_vram_bytes: u64,
    /// Maximum concurrent resident-model slots.
    pub max_concurrent_resident_models: u16,
    /// Governance-defined host class used for compute tariff lookup.
    pub host_class: String,
    /// Timestamp when the advert was last refreshed.
    pub advertised_at_ms: u64,
    /// Timestamp after which the advert is no longer eligible without a heartbeat.
    pub heartbeat_expires_at_ms: u64,
}
impl SoraModelHostCapabilityRecordV1 {
    /// Validate the authoritative model-host capability advert.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when any required field is empty or invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model host capability record",
            self.schema_version,
            SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora model host capability record",
            "peer_id",
            &self.peer_id,
        )?;
        if self.supported_backends.is_empty() {
            return Err(invalid_field(
                "sora model host capability record",
                "supported_backends",
                "must not be empty",
            ));
        }
        if self.supported_formats.is_empty() {
            return Err(invalid_field(
                "sora model host capability record",
                "supported_formats",
                "must not be empty",
            ));
        }
        for (field, value) in [
            ("max_model_bytes", self.max_model_bytes),
            ("max_disk_cache_bytes", self.max_disk_cache_bytes),
            ("max_ram_bytes", self.max_ram_bytes),
        ] {
            if value == 0 {
                return Err(invalid_field(
                    "sora model host capability record",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        if self.max_concurrent_resident_models == 0 {
            return Err(invalid_field(
                "sora model host capability record",
                "max_concurrent_resident_models",
                "must be greater than zero",
            ));
        }
        validate_nonblank_field(
            "sora model host capability record",
            "host_class",
            &self.host_class,
        )?;
        if self.advertised_at_ms == 0 || self.heartbeat_expires_at_ms == 0 {
            return Err(invalid_field(
                "sora model host capability record",
                "advertised_at_ms",
                "advertised_at_ms and heartbeat_expires_at_ms must be greater than zero",
            ));
        }
        if self.heartbeat_expires_at_ms <= self.advertised_at_ms {
            return Err(invalid_field(
                "sora model host capability record",
                "heartbeat_expires_at_ms",
                "must be greater than advertised_at_ms",
            ));
        }
        Ok(())
    }
    /// Return whether the advert remains eligible at the supplied timestamp.
    #[must_use]
    pub fn is_active_at(&self, now_ms: u64) -> bool {
        self.heartbeat_expires_at_ms > now_ms
    }
}
/// Active opt-in validator host capability advert for authoritative Inrou placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouHostCapabilityRecordV1 {
    /// Schema version; must equal [`SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Validator account that owns this host advert.
    pub validator_account_id: AccountId,
    /// Peer identifier used for Torii routing.
    pub peer_id: String,
    /// Supported guest ISAs for locally materialized replicas.
    pub supported_guest_isas: BTreeSet<SoraInrouGuestIsaV1>,
    /// Maximum number of concurrently hosted placed replicas.
    pub max_hosted_replica_capacity: u16,
    /// Maximum aggregate hosted CPU budget in millicores.
    pub max_cpu_millis: u32,
    /// Maximum aggregate hosted memory budget in bytes.
    pub max_memory_bytes: u64,
    /// Maximum aggregate hosted writable storage budget in bytes.
    pub max_storage_bytes: u64,
    /// Optional geography labels advertised by the host or derived by telemetry.
    pub geography_tags: BTreeSet<String>,
    /// Optional observed latency hint used when exact geography is unavailable.
    #[norito(required)]
    pub observed_latency_ms: Option<u32>,
    /// Timestamp when the advert was last refreshed.
    pub advertised_at_ms: u64,
    /// Timestamp after which the advert is no longer eligible without a refresh.
    pub heartbeat_expires_at_ms: u64,
}
impl SoraInrouHostCapabilityRecordV1 {
    /// Validate the authoritative Inrou host advert.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when required fields are empty or capacity
    /// invariants are inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora inrou host capability record",
            self.schema_version,
            SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
        )?;
        validate_peer_id_field("sora inrou host capability record", &self.peer_id)?;
        if self.supported_guest_isas.len() != 1 {
            return Err(invalid_field(
                "sora inrou host capability record",
                "supported_guest_isas",
                "must contain exactly the qualified host-native guest ISA",
            ));
        }
        for tag in &self.geography_tags {
            validate_distribution_geography_tag(
                "sora inrou host capability record",
                "geography_tags",
                tag,
            )?;
        }
        if self.observed_latency_ms == Some(0) {
            return Err(invalid_field(
                "sora inrou host capability record",
                "observed_latency_ms",
                "must be greater than zero when provided",
            ));
        }
        if self.advertised_at_ms == 0 || self.heartbeat_expires_at_ms == 0 {
            return Err(invalid_field(
                "sora inrou host capability record",
                "advertised_at_ms",
                "advertised_at_ms and heartbeat_expires_at_ms must be greater than zero",
            ));
        }
        if self.heartbeat_expires_at_ms <= self.advertised_at_ms {
            return Err(invalid_field(
                "sora inrou host capability record",
                "heartbeat_expires_at_ms",
                "must be greater than advertised_at_ms",
            ));
        }
        if self.max_hosted_replica_capacity != SORA_INROU_HOSTED_REPLICA_CAPACITY_V1 {
            return Err(invalid_field(
                "sora inrou host capability record",
                "max_hosted_replica_capacity",
                "must equal the first-release capacity of one",
            ));
        }
        for (field, value) in [
            ("max_cpu_millis", u64::from(self.max_cpu_millis)),
            ("max_memory_bytes", self.max_memory_bytes),
            ("max_storage_bytes", self.max_storage_bytes),
        ] {
            if value == 0 {
                return Err(invalid_field(
                    "sora inrou host capability record",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        Ok(())
    }
    /// Return whether the advert remains eligible at the supplied timestamp.
    #[must_use]
    pub fn is_active_at(&self, now_ms: u64) -> bool {
        self.heartbeat_expires_at_ms > now_ms
    }
    /// Return whether the advert may host placed replicas at the supplied timestamp.
    #[must_use]
    pub fn can_host_replicas_at(&self, now_ms: u64) -> bool {
        self.validate().is_ok() && self.is_active_at(now_ms)
    }
}
/// Authoritative host assignment for one placed Inrou replica slot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouReplicaPlacementV1 {
    /// One-based replica slot within the selected service revision.
    pub replica_slot: u16,
    /// Validator assigned to materialize the replica.
    pub validator_account_id: AccountId,
    /// Peer identifier used for Torii proxy routing.
    pub peer_id: String,
    /// Guest ISA selected locally on the assigned host.
    pub selected_guest_isa: SoraInrouGuestIsaV1,
    /// Geography tag that matched the requested distribution policy, when known.
    #[norito(required)]
    pub selected_geography_tag: Option<String>,
    /// Latency observation used by placement/hydration, when available.
    #[norito(required)]
    pub selection_latency_ms: Option<u32>,
}
impl SoraInrouReplicaPlacementV1 {
    /// Validate one placed Inrou replica assignment.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when required routing metadata is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.replica_slot == 0 {
            return Err(invalid_field(
                "sora inrou replica placement",
                "replica_slot",
                "must be greater than zero",
            ));
        }
        validate_peer_id_field("sora inrou replica placement", &self.peer_id)?;
        if let Some(tag) = self.selected_geography_tag.as_ref() {
            validate_distribution_geography_tag(
                "sora inrou replica placement",
                "selected_geography_tag",
                tag,
            )?;
        }
        if self.selection_latency_ms == Some(0) {
            return Err(invalid_field(
                "sora inrou replica placement",
                "selection_latency_ms",
                "must be greater than zero when provided",
            ));
        }
        Ok(())
    }
}
/// Authoritative per-revision placement record for hosted Inrou replicas.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouServicePlacementRecordV1 {
    /// Schema version; must equal [`SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose placed replicas are being tracked.
    pub service_name: Name,
    /// Service revision/version covered by this record.
    pub service_version: String,
    /// Desired replica count declared by the admitted service manifest.
    pub desired_replica_count: u16,
    /// Number of currently active eligible validators considered during reconciliation.
    pub eligible_validator_count: u32,
    /// Assigned replica placements in deterministic slot order.
    pub placements: Vec<SoraInrouReplicaPlacementV1>,
    /// Timestamp of the last placement reconciliation.
    pub reconciled_at_ms: u64,
    /// Latest placement error, when not all requested slots could be assigned.
    #[norito(required)]
    pub last_error: Option<String>,
}
impl SoraInrouServicePlacementRecordV1 {
    /// Validate the authoritative Inrou placement record.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or slot
    /// assignments are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora inrou service placement record",
            self.schema_version,
            SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora inrou service placement record",
            "service_version",
            &self.service_version,
        )?;
        if self.desired_replica_count == 0 {
            return Err(invalid_field(
                "sora inrou service placement record",
                "desired_replica_count",
                "must be greater than zero",
            ));
        }
        if self.reconciled_at_ms == 0 {
            return Err(invalid_field(
                "sora inrou service placement record",
                "reconciled_at_ms",
                "must be greater than zero",
            ));
        }
        if self.placements.len() > usize::from(self.desired_replica_count) {
            return Err(invalid_field(
                "sora inrou service placement record",
                "placements",
                "placement count must not exceed desired_replica_count",
            ));
        }
        if u32::try_from(self.placements.len())
            .expect("placement count was bounded by desired_replica_count")
            > self.eligible_validator_count
        {
            return Err(invalid_field(
                "sora inrou service placement record",
                "placements",
                "placement count must not exceed eligible_validator_count",
            ));
        }
        let mut seen_validators = BTreeSet::new();
        let mut seen_peer_ids = BTreeSet::new();
        for (index, placement) in self.placements.iter().enumerate() {
            placement.validate()?;
            let expected_slot = u16::try_from(index + 1)
                .expect("placement count was bounded by desired_replica_count");
            if placement.replica_slot != expected_slot {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora inrou service placement record",
                    field: "placements",
                    reason: format!(
                        "placements must use the sorted contiguous slot prefix 1..=len; expected replica_slot {expected_slot}, found {}",
                        placement.replica_slot
                    ),
                });
            }
            if !seen_validators.insert(placement.validator_account_id.clone()) {
                return Err(invalid_field(
                    "sora inrou service placement record",
                    "placements",
                    "each placement must use a distinct validator account",
                ));
            }
            if !seen_peer_ids.insert(placement.peer_id.clone()) {
                return Err(invalid_field(
                    "sora inrou service placement record",
                    "placements",
                    "each placement must use a distinct peer ID",
                ));
            }
        }
        Ok(())
    }
}
/// Placement lifecycle state for the active HF compute reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfPlacementStatusV1 {
    /// The placement is being selected.
    Selecting,
    /// Hosts are assigned but none are warm yet.
    Warming,
    /// The primary is warm and the target host set is healthy.
    Ready,
    /// The primary is warm but the placement has lost a replica or has warming replicas.
    Degraded,
    /// No assigned host is currently warm.
    Unavailable,
    /// The placement was retired alongside the lease window.
    Retired,
}
/// Assigned role of a validator within a placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "role", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfPlacementHostRoleV1 {
    /// Primary execution host.
    Primary,
    /// Warm or warming failover replica.
    Replica,
}
/// Current placement status for an assigned validator host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfPlacementHostStatusV1 {
    /// Slot is reserved and the host is warming the model.
    Warming,
    /// Host is warm and can execute inference.
    Warm,
    /// Host lost eligibility or heartbeat and is unavailable.
    Unavailable,
    /// Host slot was retired from the placement.
    Retired,
}
/// Host assignment persisted on the authoritative placement record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfPlacementHostAssignmentV1 {
    /// Validator assigned to the slot.
    pub validator_account_id: AccountId,
    /// Peer identifier used for routing.
    pub peer_id: String,
    /// Current role of the host.
    pub role: SoraHfPlacementHostRoleV1,
    /// Current health/warmness state of the slot.
    pub status: SoraHfPlacementHostStatusV1,
    /// Host class used for compute tariff lookup.
    pub host_class: String,
}
impl SoraHfPlacementHostAssignmentV1 {
    /// Validate an authoritative placement host assignment.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the routing metadata is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field(
            "sora hf placement host assignment",
            "peer_id",
            &self.peer_id,
        )?;
        validate_nonblank_field(
            "sora hf placement host assignment",
            "host_class",
            &self.host_class,
        )?;
        Ok(())
    }
}
/// Authoritative placement record attached to the active HF lease window.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfPlacementRecordV1 {
    /// Schema version; must equal [`SORA_HF_PLACEMENT_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Stable placement identifier for the current lease window.
    pub placement_id: Hash,
    /// Canonical imported source identifier.
    pub source_id: Hash,
    /// Shared-lease pool this placement belongs to.
    pub pool_id: Hash,
    /// Current placement lifecycle state.
    pub status: SoraHfPlacementStatusV1,
    /// Deterministic seed hash used when ranking eligible validators.
    pub selection_seed_hash: Hash,
    /// Resource profile used for eligibility checks and tariff lookup.
    pub resource_profile: SoraHfResourceProfileV1,
    /// Number of eligible validators considered for the current window.
    pub eligible_validator_count: u32,
    /// Target assigned host count for the current model-size bucket.
    pub adaptive_target_host_count: u16,
    /// Assigned validator hosts in deterministic rank order.
    pub assigned_hosts: Vec<SoraHfPlacementHostAssignmentV1>,
    /// Total nominal compute reservation fee charged for the current window.
    pub total_reservation_fee: Quantity,
    /// Timestamp of the last placement rebalance.
    pub last_rebalance_at_ms: u64,
    /// Latest placement/runtime error.
    #[norito(required)]
    pub last_error: Option<String>,
}
impl SoraHfPlacementRecordV1 {
    /// Validate the authoritative HF placement record.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or assignments are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf placement record",
            self.schema_version,
            SORA_HF_PLACEMENT_RECORD_VERSION_V1,
        )?;
        for (field, digest) in [
            ("placement_id", self.placement_id),
            ("source_id", self.source_id),
            ("pool_id", self.pool_id),
            ("selection_seed_hash", self.selection_seed_hash),
        ] {
            validate_soracloud_digest_hash("sora hf placement record", field, digest)?;
        }
        self.resource_profile.validate()?;
        if self.adaptive_target_host_count == 0 {
            return Err(invalid_field(
                "sora hf placement record",
                "adaptive_target_host_count",
                "must be greater than zero",
            ));
        }
        if self.last_rebalance_at_ms == 0 {
            return Err(invalid_field(
                "sora hf placement record",
                "last_rebalance_at_ms",
                "must be greater than zero",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut primary_count = 0_u8;
        for assignment in &self.assigned_hosts {
            assignment.validate()?;
            if !seen.insert(assignment.validator_account_id.clone()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora hf placement record",
                    field: "assigned_hosts",
                    reason: format!(
                        "duplicate validator assignment `{}`",
                        assignment.validator_account_id
                    ),
                });
            }
            if matches!(assignment.role, SoraHfPlacementHostRoleV1::Primary) {
                primary_count = primary_count.saturating_add(1);
            }
        }
        if !self.assigned_hosts.is_empty() && primary_count != 1 {
            return Err(invalid_field(
                "sora hf placement record",
                "assigned_hosts",
                "non-empty placements must contain exactly one primary",
            ));
        }
        if self
            .last_error
            .as_ref()
            .is_some_and(|error| error.trim().is_empty())
        {
            return Err(invalid_field(
                "sora hf placement record",
                "last_error",
                "must not be empty when provided",
            ));
        }
        Ok(())
    }
    /// Count the currently warm assigned hosts.
    #[must_use]
    pub fn warm_host_count(&self) -> u32 {
        u32::try_from(
            self.assigned_hosts
                .iter()
                .filter(|assignment| matches!(assignment.status, SoraHfPlacementHostStatusV1::Warm))
                .count(),
        )
        .unwrap_or(u32::MAX)
    }
}
/// Canonical Soracloud model-host violation kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraModelHostViolationKindV1 {
    /// The host was assigned to warm a model but never became ready before its advert expired.
    WarmupNoShow,
    /// The host was already assigned and warm but later lost its assigned-host heartbeat.
    AssignedHeartbeatMiss,
    /// The host advert was provably self-contradictory.
    AdvertContradiction,
}
/// Authoritative evidence record for a Soracloud model-host violation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelHostViolationEvidenceRecordV1 {
    /// Schema version; must equal [`SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Stable evidence identifier.
    pub evidence_id: Hash,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Validator responsible for the violation.
    pub validator_account_id: AccountId,
    /// Violation class.
    pub kind: SoraModelHostViolationKindV1,
    /// Placement implicated in the violation when applicable.
    #[norito(required)]
    pub placement_id: Option<Hash>,
    /// HF lease pool implicated in the violation when applicable.
    #[norito(required)]
    pub pool_id: Option<Hash>,
    /// Canonical HF source implicated in the violation when applicable.
    #[norito(required)]
    pub source_id: Option<Hash>,
    /// Reservation-window start timestamp used for strike counting when applicable.
    #[norito(required)]
    pub window_started_at_ms: Option<u64>,
    /// Block timestamp when the evidence was recorded.
    pub observed_at_ms: u64,
    /// Optional explanatory detail attached to the evidence.
    #[norito(required)]
    pub detail: Option<String>,
    /// Strike count for repeated heartbeat misses within one reservation window.
    pub strike_count: u32,
    /// Whether the corresponding validator penalty was already applied.
    pub penalty_applied: bool,
    /// Whether the host advert was evicted from future placement eligibility.
    pub host_evicted: bool,
    /// Slash identifier applied through the public-lane validator slash path, if any.
    #[norito(required)]
    pub slash_id: Option<Hash>,
}
impl SoraModelHostViolationEvidenceRecordV1 {
    /// Validate the authoritative host-violation evidence record.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the version, sequence, timestamps, or
    /// strike/penalty fields are inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model host violation evidence record",
            self.schema_version,
            SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1,
        )?;
        validate_soracloud_digest_hash(
            "sora model host violation evidence record",
            "evidence_id",
            self.evidence_id,
        )?;
        for (field, digest) in [
            ("placement_id", self.placement_id),
            ("pool_id", self.pool_id),
            ("source_id", self.source_id),
            ("slash_id", self.slash_id),
        ] {
            if let Some(digest) = digest {
                validate_soracloud_digest_hash(
                    "sora model host violation evidence record",
                    field,
                    digest,
                )?;
            }
        }
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora model host violation evidence record",
                "sequence",
                "must be greater than zero",
            ));
        }
        if self.observed_at_ms == 0 {
            return Err(invalid_field(
                "sora model host violation evidence record",
                "observed_at_ms",
                "must be greater than zero",
            ));
        }
        if matches!(
            self.kind,
            SoraModelHostViolationKindV1::WarmupNoShow
                | SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        ) {
            if self.placement_id.is_none() || self.pool_id.is_none() || self.source_id.is_none() {
                return Err(invalid_field(
                    "sora model host violation evidence record",
                    "placement_id",
                    "placement-scoped violations must include placement_id, pool_id, and source_id",
                ));
            }
            if self.window_started_at_ms.is_none() {
                return Err(invalid_field(
                    "sora model host violation evidence record",
                    "window_started_at_ms",
                    "placement-scoped violations must include the reservation-window start",
                ));
            }
        }
        if self
            .detail
            .as_ref()
            .is_some_and(|detail| detail.trim().is_empty())
        {
            return Err(invalid_field(
                "sora model host violation evidence record",
                "detail",
                "must not be empty when provided",
            ));
        }
        if self.penalty_applied && self.slash_id.is_none() {
            return Err(invalid_field(
                "sora model host violation evidence record",
                "slash_id",
                "must be present when penalty_applied is true",
            ));
        }
        if !self.penalty_applied && self.slash_id.is_some() {
            return Err(invalid_field(
                "sora model host violation evidence record",
                "slash_id",
                "must be absent when penalty_applied is false",
            ));
        }
        if self.kind != SoraModelHostViolationKindV1::AssignedHeartbeatMiss && self.strike_count > 1
        {
            return Err(invalid_field(
                "sora model host violation evidence record",
                "strike_count",
                "only assigned heartbeat misses may accumulate multiple strikes",
            ));
        }
        if self.kind == SoraModelHostViolationKindV1::AssignedHeartbeatMiss
            && self.strike_count == 0
        {
            return Err(invalid_field(
                "sora model host violation evidence record",
                "strike_count",
                "assigned heartbeat misses must record a strike count",
            ));
        }
        Ok(())
    }
}
/// Import lifecycle state for a canonical Hugging Face source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfSourceStatusV1 {
    /// Metadata has been admitted and the import worker still needs to hydrate bytes.
    PendingImport,
    /// Canonical import metadata is ready for shared leasing.
    Ready,
    /// The source failed import and requires operator intervention.
    Failed,
    /// The canonical source was retired and should no longer accept new joins.
    Retired,
}
/// Authoritative canonical Hugging Face import metadata.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSourceRecordV1 {
    /// Schema version; must equal [`SORA_HF_SOURCE_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Stable canonical source identifier.
    pub source_id: Hash,
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision used for this canonical source.
    pub resolved_revision: String,
    /// Normalized model name used for Soracloud surfaces.
    pub model_name: String,
    /// Adapter identifier that will serve this source.
    pub adapter_id: String,
    /// Hash of the normalized runtime artifact layout.
    pub normalized_runtime_hash: Hash,
    /// Canonical resource profile derived from HF metadata when available.
    #[norito(required)]
    pub resource_profile: Option<SoraHfResourceProfileV1>,
    /// Source lifecycle status.
    pub status: SoraHfSourceStatusV1,
    /// Block timestamp when the source was first admitted.
    pub created_at_ms: u64,
    /// Block timestamp of the last lifecycle mutation.
    pub updated_at_ms: u64,
    /// Latest import/runtime error when status is [`SoraHfSourceStatusV1::Failed`].
    #[norito(required)]
    pub last_error: Option<String>,
}
impl SoraHfSourceRecordV1 {
    /// Validate canonical Hugging Face source metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf source record",
            self.schema_version,
            SORA_HF_SOURCE_RECORD_VERSION_V1,
        )?;
        validate_soracloud_digest_hash("sora hf source record", "source_id", self.source_id)?;
        validate_soracloud_digest_hash(
            "sora hf source record",
            "normalized_runtime_hash",
            self.normalized_runtime_hash,
        )?;
        for (field, value) in [
            ("repo_id", self.repo_id.as_str()),
            ("model_name", self.model_name.as_str()),
            ("adapter_id", self.adapter_id.as_str()),
        ] {
            validate_nonblank_field("sora hf source record", field, value)?;
        }
        if !is_canonical_hf_commit_oid_v1(&self.resolved_revision) {
            return Err(invalid_field(
                "sora hf source record",
                "resolved_revision",
                "must be the full 40-character lowercase hexadecimal commit OID",
            ));
        }
        if let Some(resource_profile) = &self.resource_profile {
            resource_profile.validate()?;
        }
        if self.created_at_ms == 0 || self.updated_at_ms == 0 {
            return Err(invalid_field(
                "sora hf source record",
                "created_at_ms",
                "created_at_ms and updated_at_ms must be greater than zero",
            ));
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(invalid_field(
                "sora hf source record",
                "updated_at_ms",
                "must be >= created_at_ms",
            ));
        }
        if self
            .last_error
            .as_ref()
            .is_some_and(|error| error.trim().is_empty())
        {
            return Err(invalid_field(
                "sora hf source record",
                "last_error",
                "must not be empty when provided",
            ));
        }
        Ok(())
    }
}
/// Shared-lease pool lifecycle state for a canonical HF source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfSharedLeaseStatusV1 {
    /// The pool is accepting joins against the current window.
    Active,
    /// The pool is draining after the last member left.
    Draining,
    /// The current window expired and requires a new sponsor.
    Expired,
    /// The pool was explicitly retired.
    Retired,
}
/// Shared-lease membership lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfSharedLeaseMemberStatusV1 {
    /// The account actively participates in the current window.
    Active,
    /// The account left the pool or was expired out of the current window.
    Left,
}
/// Audit action recorded for shared Hugging Face lease windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfSharedLeaseActionV1 {
    /// A brand new window was opened by a sponsor.
    CreateWindow,
    /// An account joined an active window.
    Join,
    /// An active membership left the current window.
    Leave,
    /// A future or fresh window was sponsored.
    Renew,
    /// The current window was retired early.
    Retire,
}
/// Queued next-window sponsorship metadata for a shared Hugging Face lease pool.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeaseQueuedWindowV1 {
    /// Account that fronted the full charge for the queued window.
    pub sponsor_account_id: AccountId,
    /// Model label to adopt once the queued window becomes active.
    pub model_name: String,
    /// Settlement asset definition for the queued window.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window nominal price charged to the sponsor.
    pub base_fee: Quantity,
    /// Full-window nominal compute reservation fee charged to the sponsor.
    pub compute_reservation_fee: Quantity,
    /// Planned placement to activate when the queued window becomes current.
    pub planned_placement: SoraHfPlacementRecordV1,
    /// Timestamp when the queued sponsorship was recorded.
    pub sponsored_at_ms: u64,
    /// Planned start timestamp for the queued window.
    pub window_started_at_ms: u64,
    /// Planned expiry timestamp for the queued window.
    pub window_expires_at_ms: u64,
    /// Service binding that should be preserved for the queued sponsor.
    pub service_name: Name,
    /// Optional apartment binding that should be preserved for the queued sponsor.
    #[norito(required)]
    pub apartment_name: Option<Name>,
}
impl SoraHfSharedLeaseQueuedWindowV1 {
    /// Validate queued shared-lease sponsorship metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when timestamps, prices, or names are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.model_name.trim().is_empty() {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "model_name",
                "must not be empty",
            ));
        }
        if self.base_fee.is_zero() {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "base_fee",
                "must be greater than zero",
            ));
        }
        if self.compute_reservation_fee.is_zero() {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "compute_reservation_fee",
                "must be greater than zero",
            ));
        }
        self.planned_placement.validate()?;
        if self.planned_placement.total_reservation_fee != self.compute_reservation_fee {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "planned_placement.total_reservation_fee",
                "must match compute_reservation_fee",
            ));
        }
        if self.sponsored_at_ms == 0
            || self.window_started_at_ms == 0
            || self.window_expires_at_ms == 0
        {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "sponsored_at_ms",
                "queued-window timestamps must be greater than zero",
            ));
        }
        if self.window_started_at_ms < self.sponsored_at_ms {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "window_started_at_ms",
                "must be greater than or equal to sponsored_at_ms",
            ));
        }
        if self.window_expires_at_ms <= self.window_started_at_ms {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "window_expires_at_ms",
                "must be greater than window_started_at_ms",
            ));
        }
        Ok(())
    }
}
/// Shared-lease pool metadata keyed by canonical import and pricing dimensions.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeasePoolV1 {
    /// Schema version; must equal [`SORA_HF_SHARED_LEASE_POOL_VERSION_V1`].
    pub schema_version: u16,
    /// Stable pool identifier.
    pub pool_id: Hash,
    /// Canonical imported source identifier.
    pub source_id: Hash,
    /// Storage class used by the shared lease.
    pub storage_class: StorageClass,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window nominal price in `lease_asset_definition_id`.
    pub base_fee: Quantity,
    /// Shared window length in milliseconds.
    pub lease_term_ms: u64,
    /// Start timestamp for the active window.
    pub window_started_at_ms: u64,
    /// Expiry timestamp for the active window.
    pub window_expires_at_ms: u64,
    /// Number of currently active members.
    pub active_member_count: u32,
    /// Pool lifecycle status.
    pub status: SoraHfSharedLeaseStatusV1,
    /// Optional sponsorship for the next window after the current one expires.
    #[norito(required)]
    pub queued_next_window: Option<SoraHfSharedLeaseQueuedWindowV1>,
}
impl SoraHfSharedLeasePoolV1 {
    /// Validate shared-lease pool metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// time/price fields are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf shared lease pool",
            self.schema_version,
            SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
        )?;
        validate_soracloud_digest_hash("sora hf shared lease pool", "pool_id", self.pool_id)?;
        validate_soracloud_digest_hash("sora hf shared lease pool", "source_id", self.source_id)?;
        if self.base_fee.is_zero() {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "base_fee",
                "must be greater than zero",
            ));
        }
        if self.lease_term_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "lease_term_ms",
                "must be greater than zero",
            ));
        }
        if self.window_started_at_ms == 0 || self.window_expires_at_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "window_started_at_ms",
                "window timestamps must be greater than zero",
            ));
        }
        if self.window_expires_at_ms <= self.window_started_at_ms {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "window_expires_at_ms",
                "must be greater than window_started_at_ms",
            ));
        }
        if let Some(next_window) = &self.queued_next_window {
            next_window.validate()?;
            if self.status != SoraHfSharedLeaseStatusV1::Active {
                return Err(invalid_field(
                    "sora hf shared lease pool",
                    "queued_next_window",
                    "may only be set while the current window is active",
                ));
            }
            if next_window.window_started_at_ms != self.window_expires_at_ms {
                return Err(invalid_field(
                    "sora hf shared lease pool",
                    "queued_next_window.window_started_at_ms",
                    "must match window_expires_at_ms",
                ));
            }
            if next_window.window_expires_at_ms
                != next_window
                    .window_started_at_ms
                    .saturating_add(self.lease_term_ms)
            {
                return Err(invalid_field(
                    "sora hf shared lease pool",
                    "queued_next_window.window_expires_at_ms",
                    "must equal queued window_started_at_ms + lease_term_ms",
                ));
            }
        }
        Ok(())
    }
}
/// Account-scoped shared-lease membership plus free service/apartment bindings.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeaseMemberV1 {
    /// Schema version; must equal [`SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1`].
    pub schema_version: u16,
    /// Pool this membership belongs to.
    pub pool_id: Hash,
    /// Canonical imported source identifier.
    pub source_id: Hash,
    /// Member account.
    pub account_id: AccountId,
    /// Membership lifecycle status.
    pub status: SoraHfSharedLeaseMemberStatusV1,
    /// Timestamp when the account first joined the current or previous windows.
    pub joined_at_ms: u64,
    /// Timestamp of the last mutation to this membership.
    pub updated_at_ms: u64,
    /// Total nominal amount charged to this member across joins/renewals.
    pub total_paid: Quantity,
    /// Total nominal amount refunded to this member by later joiners.
    pub total_refunded: Quantity,
    /// Most recent nominal direct charge applied to this member.
    pub last_charge: Quantity,
    /// Total nominal compute reservation amount charged across joins/renewals.
    pub total_compute_paid: Quantity,
    /// Total nominal compute reservation amount refunded by later joiners.
    pub total_compute_refunded: Quantity,
    /// Most recent nominal direct compute reservation charge applied to this member.
    pub last_compute_charge: Quantity,
    /// Bound Soracloud services that reuse this membership.
    pub service_bindings: BTreeSet<String>,
    /// Bound Soracloud agent apartments that reuse this membership.
    pub apartment_bindings: BTreeSet<String>,
}
impl SoraHfSharedLeaseMemberV1 {
    /// Validate shared-lease membership metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// bindings contain invalid names.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf shared lease member",
            self.schema_version,
            SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
        )?;
        validate_soracloud_digest_hash("sora hf shared lease member", "pool_id", self.pool_id)?;
        validate_soracloud_digest_hash("sora hf shared lease member", "source_id", self.source_id)?;
        if self.joined_at_ms == 0 || self.updated_at_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease member",
                "joined_at_ms",
                "joined_at_ms and updated_at_ms must be greater than zero",
            ));
        }
        if self.updated_at_ms < self.joined_at_ms {
            return Err(invalid_field(
                "sora hf shared lease member",
                "updated_at_ms",
                "must be >= joined_at_ms",
            ));
        }
        for service_name in &self.service_bindings {
            if service_name.trim().is_empty() {
                return Err(invalid_field(
                    "sora hf shared lease member",
                    "service_bindings",
                    "service bindings must not contain empty names",
                ));
            }
        }
        for apartment_name in &self.apartment_bindings {
            if apartment_name.trim().is_empty() {
                return Err(invalid_field(
                    "sora hf shared lease member",
                    "apartment_bindings",
                    "apartment bindings must not contain empty names",
                ));
            }
        }
        Ok(())
    }
}
/// Audit record for shared Hugging Face lease lifecycle changes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeaseAuditEventV1 {
    /// Schema version; must equal [`SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Audit action that produced the event.
    pub action: SoraHfSharedLeaseActionV1,
    /// Pool affected by the event.
    pub pool_id: Hash,
    /// Canonical imported source identifier.
    pub source_id: Hash,
    /// Account responsible for the lifecycle mutation.
    pub account_id: AccountId,
    /// Block timestamp of the mutation.
    pub occurred_at_ms: u64,
    /// Number of active members after the mutation.
    pub active_member_count: u32,
    /// Direct nominal amount charged to the acting account.
    pub charged: Quantity,
    /// Direct nominal refund amount recorded for the acting account.
    pub refunded: Quantity,
    /// Current pool expiry after the mutation.
    pub lease_expires_at_ms: u64,
    /// Optional service binding touched by the mutation.
    #[norito(required)]
    pub service_name: Option<String>,
    /// Optional apartment binding touched by the mutation.
    #[norito(required)]
    pub apartment_name: Option<String>,
}
impl SoraHfSharedLeaseAuditEventV1 {
    /// Validate shared-lease audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// optional bindings contain empty names.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf shared lease audit event",
            self.schema_version,
            SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
        )?;
        validate_soracloud_digest_hash(
            "sora hf shared lease audit event",
            "pool_id",
            self.pool_id,
        )?;
        validate_soracloud_digest_hash(
            "sora hf shared lease audit event",
            "source_id",
            self.source_id,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora hf shared lease audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        if self.occurred_at_ms == 0 || self.lease_expires_at_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease audit event",
                "occurred_at_ms",
                "occurred_at_ms and lease_expires_at_ms must be greater than zero",
            ));
        }
        if self
            .service_name
            .as_ref()
            .is_some_and(|service_name| service_name.trim().is_empty())
        {
            return Err(invalid_field(
                "sora hf shared lease audit event",
                "service_name",
                "must not be empty when provided",
            ));
        }
        if self
            .apartment_name
            .as_ref()
            .is_some_and(|apartment_name| apartment_name.trim().is_empty())
        {
            return Err(invalid_field(
                "sora hf shared lease audit event",
                "apartment_name",
                "must not be empty when provided",
            ));
        }
        Ok(())
    }
}
/// Audit record for model-artifact lifecycle changes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelArtifactAuditEventV1 {
    /// Schema version; must equal [`SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Model-artifact action that produced the event.
    pub action: SoraModelArtifactActionV1,
    /// Service that owns the artifact.
    pub service_name: Name,
    /// Active service revision when the event was emitted.
    pub service_version: String,
    /// Logical model name.
    pub model_name: String,
    /// Training job associated with the artifact.
    pub training_job_id: String,
    /// Model weight version that consumed the artifact, when any.
    #[norito(required)]
    pub consumed_by_version: Option<String>,
    /// Provenance signer that authorized the event.
    pub signer: PublicKey,
}
impl SoraModelArtifactAuditEventV1 {
    /// Validate model-artifact audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model artifact audit event",
            self.schema_version,
            SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora model artifact audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        for (field, value) in [
            ("service_version", self.service_version.as_str()),
            ("model_name", self.model_name.as_str()),
            ("training_job_id", self.training_job_id.as_str()),
        ] {
            validate_nonblank_field("sora model artifact audit event", field, value)?;
        }
        if self
            .consumed_by_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(invalid_field(
                "sora model artifact audit event",
                "consumed_by_version",
                "must not be empty when provided",
            ));
        }
        Ok(())
    }
}
/// Audit action recorded for authoritative Soracloud agent-apartment state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraAgentApartmentActionV1 {
    /// A new apartment was deployed.
    Deploy,
    /// An apartment lease was renewed.
    LeaseRenew,
    /// An apartment process was restarted.
    Restart,
    /// A wallet spend request was created but not yet approved.
    WalletSpendRequested,
    /// A wallet spend request was approved and applied.
    WalletSpendApproved,
    /// A policy capability was revoked.
    PolicyRevoked,
    /// A mailbox message was enqueued for delivery.
    MessageEnqueued,
    /// A mailbox message was acknowledged and consumed.
    MessageAcknowledged,
    /// An autonomy artifact was allowlisted.
    ArtifactAllowed,
    /// An autonomy run was approved and recorded.
    AutonomyRunApproved,
    /// An approved autonomy run completed execution and recorded a runtime outcome.
    AutonomyRunExecuted,
}
/// Runtime status of an authoritative Soracloud agent apartment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraAgentRuntimeStatusV1 {
    /// Apartment lease is active and the process is considered runnable.
    Running,
    /// Apartment lease expired and must be renewed before further work.
    LeaseExpired,
}
/// Pending wallet spend request tracked inside an authoritative apartment record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentWalletSpendRequestV1 {
    /// Deterministic request identifier.
    pub request_id: String,
    /// Asset definition constrained by the apartment policy.
    pub asset_definition: String,
    /// Requested nominal spend amount.
    pub amount: Quantity,
    /// Audit sequence that created the request.
    pub created_sequence: u64,
}
/// Daily wallet-spend aggregate for an asset/day bucket pair.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentWalletDailySpendEntryV1 {
    /// Asset definition constrained by the apartment policy.
    pub asset_definition: String,
    /// Deterministic day bucket.
    pub day_bucket: u64,
    /// Total nominal quantity spent in that bucket.
    pub spent: Quantity,
}
/// Mailbox message queued for deterministic apartment-to-apartment delivery.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentMailboxMessageV1 {
    /// Deterministic message identifier.
    pub message_id: String,
    /// Apartment that sent the message.
    pub from_apartment: String,
    /// Logical mailbox channel.
    pub channel: String,
    /// Message payload.
    pub payload: String,
    /// Canonical hash of the payload.
    pub payload_hash: Hash,
    /// Audit sequence that enqueued the message.
    pub enqueued_sequence: u64,
}
/// Allowlist entry authorizing an autonomy artifact for an apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentArtifactAllowRuleV1 {
    /// Artifact hash that was approved.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Audit sequence that added the rule.
    pub added_sequence: u64,
}
/// Historical autonomy-run approval recorded for an apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentAutonomyRunRecordV1 {
    /// Deterministic run identifier.
    pub run_id: String,
    /// Artifact hash executed by the run.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Approved budget units for the run.
    pub budget_units: u64,
    /// Human-readable run label.
    pub run_label: String,
    /// Optional canonical JSON body forwarded to the generated HF `/infer` handler.
    #[norito(required)]
    pub workflow_input_json: Option<String>,
    /// Apartment process generation active when the run was approved.
    pub approved_process_generation: u64,
    /// Deterministic runtime request commitment bound to the approved run.
    pub request_commitment: Hash,
    /// Audit sequence that approved the run.
    pub approved_sequence: u64,
}
/// Authoritative persistent-state accounting attached to an apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentPersistentStateV1 {
    /// Total bytes consumed by apartment-owned state.
    pub total_bytes: u64,
    /// Per-key size accounting for deterministic quota tracking.
    pub key_sizes: BTreeMap<String, u64>,
}
/// Authoritative runtime record for a Soracloud agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentApartmentRecordV1 {
    /// Schema version; must equal [`SORA_AGENT_APARTMENT_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Admitted apartment manifest.
    pub manifest: AgentApartmentManifestV1,
    /// Canonical hash of the manifest.
    pub manifest_hash: Hash,
    /// Current runtime status.
    pub status: SoraAgentRuntimeStatusV1,
    /// Audit sequence that deployed the apartment.
    pub deployed_sequence: u64,
    /// Audit sequence when the current lease began.
    pub lease_started_sequence: u64,
    /// Audit sequence when the lease expires.
    pub lease_expires_sequence: u64,
    /// Audit sequence of the latest lease renewal.
    pub last_renewed_sequence: u64,
    /// Deterministic restart count.
    pub restart_count: u32,
    /// Audit sequence of the last restart, when any.
    #[norito(required)]
    pub last_restart_sequence: Option<u64>,
    /// Human-readable reason for the last restart, when any.
    #[norito(required)]
    pub last_restart_reason: Option<String>,
    /// Monotonic local-process generation.
    pub process_generation: u64,
    /// Audit sequence that started the current process generation.
    pub process_started_sequence: u64,
    /// Audit sequence of the most recent activity.
    pub last_active_sequence: u64,
    /// Audit sequence of the latest checkpoint, when any.
    #[norito(required)]
    pub last_checkpoint_sequence: Option<u64>,
    /// Number of recorded checkpoints.
    pub checkpoint_count: u32,
    /// Deterministic persistent-state accounting.
    pub persistent_state: SoraAgentPersistentStateV1,
    /// Revoked policy capabilities.
    pub revoked_policy_capabilities: BTreeSet<String>,
    /// Pending wallet requests keyed by request id.
    pub pending_wallet_requests: BTreeMap<String, SoraAgentWalletSpendRequestV1>,
    /// Daily wallet-spend aggregates keyed by `<asset>:<day_bucket>`.
    pub wallet_daily_spend: BTreeMap<String, SoraAgentWalletDailySpendEntryV1>,
    /// Pending mailbox queue for the apartment.
    pub mailbox_queue: Vec<SoraAgentMailboxMessageV1>,
    /// Total autonomy budget allocated to the apartment.
    pub autonomy_budget_ceiling_units: u64,
    /// Remaining autonomy budget units.
    pub autonomy_budget_remaining_units: u64,
    /// Approved artifact allowlist keyed by artifact hash.
    pub artifact_allowlist: BTreeMap<String, SoraAgentArtifactAllowRuleV1>,
    /// Historical autonomy-run approvals.
    pub autonomy_run_history: Vec<SoraAgentAutonomyRunRecordV1>,
}
impl SoraAgentApartmentRecordV1 {
    /// Validate apartment lifecycle and deterministic-accounting invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, the embedded manifest is
    /// invalid, or the recorded lifecycle/accounting state is inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_required_fields()?;
        self.validate_restart_fields()?;
        self.validate_budget_fields()?;
        self.validate_collection_fields()
    }
    fn validate_required_fields(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora agent apartment record",
            self.schema_version,
            SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
        )?;
        self.manifest.validate()?;
        validate_soracloud_digest_hash(
            "sora agent apartment record",
            "manifest_hash",
            self.manifest_hash,
        )?;
        if self.manifest_hash != self.manifest.manifest_hash() {
            return Err(invalid_field(
                "sora agent apartment record",
                "manifest_hash",
                "must match the canonical apartment manifest hash",
            ));
        }
        for (field, value) in [
            ("process_generation", self.process_generation),
            ("deployed_sequence", self.deployed_sequence),
            ("lease_started_sequence", self.lease_started_sequence),
            ("lease_expires_sequence", self.lease_expires_sequence),
            ("last_renewed_sequence", self.last_renewed_sequence),
            ("process_started_sequence", self.process_started_sequence),
            ("last_active_sequence", self.last_active_sequence),
        ] {
            if value == 0 {
                return Err(invalid_field(
                    "sora agent apartment record",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        if self.lease_expires_sequence <= self.lease_started_sequence {
            return Err(invalid_field(
                "sora agent apartment record",
                "lease_expires_sequence",
                "must be greater than lease_started_sequence",
            ));
        }
        if self.last_renewed_sequence < self.lease_started_sequence {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_renewed_sequence",
                "must be >= lease_started_sequence",
            ));
        }
        Ok(())
    }
    fn validate_restart_fields(&self) -> Result<(), SoracloudManifestError> {
        if self
            .last_restart_reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_restart_reason",
                "must not be empty when provided",
            ));
        }
        if self.last_restart_sequence.is_some() != self.last_restart_reason.is_some() {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_restart",
                "last_restart_sequence and last_restart_reason must be populated together",
            ));
        }
        if self
            .last_checkpoint_sequence
            .is_some_and(|sequence| sequence == 0)
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_checkpoint_sequence",
                "must be greater than zero when provided",
            ));
        }
        Ok(())
    }
    fn validate_budget_fields(&self) -> Result<(), SoracloudManifestError> {
        if self.autonomy_budget_ceiling_units == 0 {
            return Err(invalid_field(
                "sora agent apartment record",
                "autonomy_budget_ceiling_units",
                "must be greater than zero",
            ));
        }
        if self.autonomy_budget_remaining_units > self.autonomy_budget_ceiling_units {
            return Err(invalid_field(
                "sora agent apartment record",
                "autonomy_budget_remaining_units",
                "must not exceed autonomy_budget_ceiling_units",
            ));
        }
        Ok(())
    }
    fn validate_collection_fields(&self) -> Result<(), SoracloudManifestError> {
        for revoked in &self.revoked_policy_capabilities {
            if revoked.trim().is_empty() {
                return Err(invalid_field(
                    "sora agent apartment record",
                    "revoked_policy_capabilities",
                    "entries must not be empty",
                ));
            }
        }
        for (request_id, request) in &self.pending_wallet_requests {
            Self::validate_pending_wallet_request(request_id, request)?;
        }
        for (key, entry) in &self.wallet_daily_spend {
            Self::validate_wallet_daily_spend_entry(key, entry)?;
        }
        for message in &self.mailbox_queue {
            Self::validate_mailbox_message(message)?;
        }
        for (artifact_hash, rule) in &self.artifact_allowlist {
            Self::validate_artifact_allowlist_rule(artifact_hash, rule)?;
        }
        for run in &self.autonomy_run_history {
            Self::validate_autonomy_run(run)?;
        }
        Ok(())
    }
    fn validate_pending_wallet_request(
        request_id: &str,
        request: &SoraAgentWalletSpendRequestV1,
    ) -> Result<(), SoracloudManifestError> {
        if request_id != request.request_id
            || request.request_id.trim().is_empty()
            || request.asset_definition.trim().is_empty()
            || request.created_sequence == 0
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "pending_wallet_requests",
                "wallet request entries must use non-empty matching request ids and valid metadata",
            ));
        }
        Ok(())
    }
    fn validate_wallet_daily_spend_entry(
        key: &str,
        entry: &SoraAgentWalletDailySpendEntryV1,
    ) -> Result<(), SoracloudManifestError> {
        if entry.asset_definition.trim().is_empty() || key.trim().is_empty() {
            return Err(invalid_field(
                "sora agent apartment record",
                "wallet_daily_spend",
                "wallet daily spend entries must use non-empty keys and asset definitions",
            ));
        }
        Ok(())
    }
    fn validate_mailbox_message(
        message: &SoraAgentMailboxMessageV1,
    ) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "sora agent apartment record",
            "mailbox_queue.payload_hash",
            message.payload_hash,
        )?;
        if message.payload_hash != Hash::new(message.payload.as_bytes()) {
            return Err(invalid_field(
                "sora agent apartment record",
                "mailbox_queue.payload_hash",
                "must match the canonical mailbox payload hash",
            ));
        }
        if message.message_id.trim().is_empty()
            || message.from_apartment.trim().is_empty()
            || message.channel.trim().is_empty()
            || message.enqueued_sequence == 0
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "mailbox_queue",
                "mailbox messages must use non-empty ids/origins/channels and valid sequences",
            ));
        }
        Ok(())
    }
    fn validate_artifact_allowlist_rule(
        artifact_hash: &str,
        rule: &SoraAgentArtifactAllowRuleV1,
    ) -> Result<(), SoracloudManifestError> {
        if artifact_hash != rule.artifact_hash
            || rule.artifact_hash.trim().is_empty()
            || rule
                .provenance_hash
                .as_ref()
                .is_some_and(|hash| hash.trim().is_empty())
            || rule.added_sequence == 0
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "artifact_allowlist",
                "allowlist entries must use non-empty matching artifact hashes and valid metadata",
            ));
        }
        Ok(())
    }
    fn validate_autonomy_run(
        run: &SoraAgentAutonomyRunRecordV1,
    ) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "sora agent apartment record",
            "autonomy_run_history.request_commitment",
            run.request_commitment,
        )?;
        if run.run_id.trim().is_empty()
            || run.artifact_hash.trim().is_empty()
            || run.run_label.trim().is_empty()
            || run.budget_units == 0
            || run.approved_process_generation == 0
            || run.approved_sequence == 0
            || run
                .provenance_hash
                .as_ref()
                .is_some_and(|hash| hash.trim().is_empty())
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "autonomy_run_history",
                "autonomy run entries must use non-empty ids/hash/label plus positive budgets, process generations, and sequences",
            ));
        }
        if let Some(workflow_input_json) = run.workflow_input_json.as_deref() {
            if workflow_input_json.trim().is_empty() {
                return Err(invalid_field(
                    "sora agent apartment record",
                    "autonomy_run_history",
                    "workflow_input_json must not be empty when provided",
                ));
            }
            norito::json::from_str::<norito::json::Value>(workflow_input_json).map_err(
                |error| SoracloudManifestError::InvalidField {
                    manifest: "sora agent apartment record",
                    field: "autonomy_run_history",
                    reason: format!("workflow_input_json must be valid JSON: {error}"),
                },
            )?;
        }
        Ok(())
    }
}
/// Audit record for authoritative agent-apartment state transitions.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentApartmentAuditEventV1 {
    /// Schema version; must equal [`SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Agent-apartment action that produced the event.
    pub action: SoraAgentApartmentActionV1,
    /// Logical apartment identifier.
    pub apartment_name: Name,
    /// Resulting runtime status.
    pub status: SoraAgentRuntimeStatusV1,
    /// Lease-expiry sequence after the event.
    pub lease_expires_sequence: u64,
    /// Hash of the admitted apartment manifest.
    pub manifest_hash: Hash,
    /// Restart count after the event.
    pub restart_count: u32,
    /// Provenance signer that authorized the event.
    pub signer: PublicKey,
    /// Optional wallet request id associated with the event.
    #[norito(required)]
    pub request_id: Option<String>,
    /// Optional asset definition associated with the event.
    #[norito(required)]
    pub asset_definition: Option<String>,
    /// Optional nominal wallet amount associated with the event.
    #[norito(required)]
    pub amount: Option<Quantity>,
    /// Optional capability associated with the event.
    #[norito(required)]
    pub capability: Option<String>,
    /// Optional reason associated with the event.
    #[norito(required)]
    pub reason: Option<String>,
    /// Optional sender apartment associated with the event.
    #[norito(required)]
    pub from_apartment: Option<String>,
    /// Optional recipient apartment associated with the event.
    #[norito(required)]
    pub to_apartment: Option<String>,
    /// Optional mailbox channel associated with the event.
    #[norito(required)]
    pub channel: Option<String>,
    /// Optional payload hash associated with the event.
    #[norito(required)]
    pub payload_hash: Option<Hash>,
    /// Optional artifact hash associated with the event.
    #[norito(required)]
    pub artifact_hash: Option<String>,
    /// Optional provenance hash associated with the event.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Optional run id associated with the event.
    #[norito(required)]
    pub run_id: Option<String>,
    /// Optional run label associated with the event.
    #[norito(required)]
    pub run_label: Option<String>,
    /// Optional run budget associated with the event.
    #[norito(required)]
    pub budget_units: Option<u64>,
    /// Optional generated service name used for execution.
    #[norito(required)]
    pub service_name: Option<String>,
    /// Optional generated service version used for execution.
    #[norito(required)]
    pub service_version: Option<String>,
    /// Optional service handler used for execution.
    #[norito(required)]
    pub handler_name: Option<String>,
    /// Optional execution result commitment recorded for the run.
    #[norito(required)]
    pub result_commitment: Option<Hash>,
    /// Optional authoritative runtime receipt identifier recorded for the run.
    #[norito(required)]
    pub runtime_receipt_id: Option<Hash>,
    /// Optional node-local journal artifact hash recorded for the run.
    #[norito(required)]
    pub journal_artifact_hash: Option<Hash>,
    /// Optional checkpoint artifact hash recorded for the run.
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Optional success flag recorded for executed autonomy runs.
    #[norito(required)]
    pub succeeded: Option<bool>,
}
impl SoraAgentApartmentAuditEventV1 {
    /// Validate agent-apartment audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora agent apartment audit event",
            self.schema_version,
            SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora agent apartment audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        if self.lease_expires_sequence == 0 {
            return Err(invalid_field(
                "sora agent apartment audit event",
                "lease_expires_sequence",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora agent apartment audit event",
            "manifest_hash",
            self.manifest_hash,
        )?;
        for (field, digest) in [
            ("payload_hash", self.payload_hash),
            ("result_commitment", self.result_commitment),
            ("runtime_receipt_id", self.runtime_receipt_id),
            ("journal_artifact_hash", self.journal_artifact_hash),
            ("checkpoint_artifact_hash", self.checkpoint_artifact_hash),
        ] {
            if let Some(digest) = digest {
                validate_soracloud_digest_hash("sora agent apartment audit event", field, digest)?;
            }
        }
        for (field, value) in [
            ("request_id", self.request_id.as_deref()),
            ("asset_definition", self.asset_definition.as_deref()),
            ("capability", self.capability.as_deref()),
            ("reason", self.reason.as_deref()),
            ("from_apartment", self.from_apartment.as_deref()),
            ("to_apartment", self.to_apartment.as_deref()),
            ("channel", self.channel.as_deref()),
            ("artifact_hash", self.artifact_hash.as_deref()),
            ("provenance_hash", self.provenance_hash.as_deref()),
            ("run_id", self.run_id.as_deref()),
            ("run_label", self.run_label.as_deref()),
            ("service_name", self.service_name.as_deref()),
            ("service_version", self.service_version.as_deref()),
            ("handler_name", self.handler_name.as_deref()),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    field,
                    "must not be empty when provided",
                ));
            }
        }
        if self.budget_units.is_some_and(|budget| budget == 0) {
            return Err(invalid_field(
                "sora agent apartment audit event",
                "budget_units",
                "must be greater than zero when provided",
            ));
        }
        if self.action == SoraAgentApartmentActionV1::AutonomyRunExecuted {
            if self.run_id.is_none() {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "run_id",
                    "autonomy execution events require a run_id",
                ));
            }
            if self.result_commitment.is_none() {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "result_commitment",
                    "autonomy execution events require a result_commitment",
                ));
            }
            if self.succeeded.is_none() {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "succeeded",
                    "autonomy execution events require a success flag",
                ));
            }
        }
        Ok(())
    }
}
fn validate_public_url(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    let trimmed = value.trim();
    validate_nonblank_field(manifest, field, trimmed)?;
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err(invalid_field(
            manifest,
            field,
            "must start with http:// or https://",
        ));
    }
    Ok(())
}
fn validate_absolute_path(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonblank_field(manifest, field, value)?;
    if !value.starts_with('/') {
        return Err(invalid_field(manifest, field, "must start with `/`"));
    }
    Ok(())
}
/// Lifecycle action recorded for an app-level Soracloud topology transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraAppInfraActionV1 {
    /// First-time admission of an app topology.
    Deploy,
    /// Upgrade of an already admitted app topology.
    Upgrade,
}
/// Static-site binding attached to a Soracloud app.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppStaticSiteBindingV1 {
    /// Schema version; must equal [`SORA_APP_STATIC_SITE_BINDING_VERSION_V1`].
    pub schema_version: u16,
    /// Public URL exposed to browsers.
    pub public_url: String,
    /// Optional content-addressed site CID.
    #[norito(required)]
    pub content_cid: Option<String>,
    /// Optional static-site manifest digest.
    #[norito(required)]
    pub manifest_digest_hex: Option<String>,
    /// URL path where static content is mounted.
    pub mount_path: String,
    /// Optional API base path consumed by the static frontend.
    #[norito(required)]
    pub api_base_path: Option<String>,
}
impl SoraAppStaticSiteBindingV1 {
    /// Validate static-site binding fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version or URL/path fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app static site binding",
            self.schema_version,
            SORA_APP_STATIC_SITE_BINDING_VERSION_V1,
        )?;
        validate_public_url(
            "sora app static site binding",
            "public_url",
            &self.public_url,
        )?;
        validate_absolute_path(
            "sora app static site binding",
            "mount_path",
            &self.mount_path,
        )?;
        if let Some(api_base_path) = self.api_base_path.as_ref() {
            validate_absolute_path(
                "sora app static site binding",
                "api_base_path",
                api_base_path,
            )?;
        }
        validate_optional_nonempty(
            "sora app static site binding",
            "content_cid",
            self.content_cid.as_deref(),
        )?;
        validate_optional_nonempty(
            "sora app static site binding",
            "manifest_digest_hex",
            self.manifest_digest_hex.as_deref(),
        )
    }
}
/// Route projection exposed by a service inside an app topology.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppRouteProjectionV1 {
    /// Schema version; must equal [`SORA_APP_ROUTE_PROJECTION_VERSION_V1`].
    pub schema_version: u16,
    /// Public host name when the route is externally reachable.
    #[norito(required)]
    pub public_host: Option<String>,
    /// Public or internal path prefix.
    pub path_prefix: String,
    /// Optional internal service URL routed by Soracloud.
    #[norito(required)]
    pub internal_url: Option<String>,
}
impl SoraAppRouteProjectionV1 {
    /// Validate app route projection fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version or route fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app route projection",
            self.schema_version,
            SORA_APP_ROUTE_PROJECTION_VERSION_V1,
        )?;
        validate_absolute_path(
            "sora app route projection",
            "path_prefix",
            &self.path_prefix,
        )?;
        validate_optional_nonempty(
            "sora app route projection",
            "public_host",
            self.public_host.as_deref(),
        )?;
        validate_optional_nonempty(
            "sora app route projection",
            "internal_url",
            self.internal_url.as_deref(),
        )
    }
}
/// Reference to an admitted Soracloud service revision in an app topology.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraServiceRefV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_SERVICE_REF_VERSION_V1`].
    pub schema_version: u16,
    /// Service name.
    pub service_name: Name,
    /// Admitted service revision.
    pub service_version: String,
    /// Hash of the service manifest for the referenced revision.
    pub service_manifest_hash: Hash,
    /// Hash of the container manifest for the referenced revision.
    pub container_manifest_hash: Hash,
    /// Expected execution plane for the referenced service.
    pub execution_plane: SoraServiceExecutionPlaneV1,
    /// Expected container runtime for the referenced service.
    pub runtime: SoraContainerRuntimeV1,
    /// Routes projected from this service into the app.
    pub routes: Vec<SoraAppRouteProjectionV1>,
    /// Optional persistent lease-volume names the app topology depends on.
    pub lease_volumes: Vec<Name>,
    /// Optional shard identifier for horizontally sharded app services.
    #[norito(required)]
    pub shard: Option<String>,
}
impl SoraAppInfraServiceRefV1 {
    /// Validate service reference fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version, revision, or routes are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra service ref",
            self.schema_version,
            SORA_APP_INFRA_SERVICE_REF_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora app infra service ref",
            "service_version",
            &self.service_version,
        )?;
        validate_soracloud_digest_hash(
            "sora app infra service ref",
            "service_manifest_hash",
            self.service_manifest_hash,
        )?;
        validate_soracloud_digest_hash(
            "sora app infra service ref",
            "container_manifest_hash",
            self.container_manifest_hash,
        )?;
        validate_optional_nonempty("sora app infra service ref", "shard", self.shard.as_deref())?;
        let mut route_paths = BTreeSet::new();
        for route in &self.routes {
            route.validate()?;
            if !route_paths.insert((route.public_host.clone(), route.path_prefix.clone())) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora app infra service ref",
                    field: "routes",
                    reason: format!(
                        "duplicate route projection for host {:?} and path `{}`",
                        route.public_host, route.path_prefix
                    ),
                });
            }
        }
        let mut volumes = BTreeSet::new();
        for volume in &self.lease_volumes {
            if !volumes.insert(volume.clone()) {
                return Err(SoracloudManifestError::DuplicateLeaseVolume {
                    volume: volume.clone(),
                });
            }
        }
        Ok(())
    }
}
/// Canonical app-level Soracloud infrastructure manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraManifestV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Stable application name.
    pub app_name: Name,
    /// Application topology revision.
    pub app_version: String,
    /// Public URL for the application.
    pub public_url: String,
    /// Optional static-site binding.
    #[norito(required)]
    pub static_site: Option<SoraAppStaticSiteBindingV1>,
    /// Authoritative service topology.
    pub services: Vec<SoraAppInfraServiceRefV1>,
}
impl SoraAppInfraManifestV1 {
    /// Compute the canonical app-infra manifest hash.
    #[must_use]
    pub fn manifest_hash(&self) -> Hash {
        Hash::new(Encode::encode(self))
    }
    /// Validate app topology invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the topology is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra manifest",
            self.schema_version,
            SORA_APP_INFRA_MANIFEST_VERSION_V1,
        )?;
        validate_nonblank_field("sora app infra manifest", "app_version", &self.app_version)?;
        validate_public_url("sora app infra manifest", "public_url", &self.public_url)?;
        if self.services.is_empty() {
            return Err(invalid_field(
                "sora app infra manifest",
                "services",
                "at least one service reference is required",
            ));
        }
        if let Some(static_site) = self.static_site.as_ref() {
            static_site.validate()?;
        }
        let mut service_names = BTreeSet::new();
        for service in &self.services {
            service.validate()?;
            if !service_names.insert(service.service_name.clone()) {
                return Err(SoracloudManifestError::DuplicateAppService {
                    service: service.service_name.clone(),
                });
            }
        }
        Ok(())
    }
}
/// Authoritative app-level Soracloud infrastructure state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraStateV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Stable application name.
    pub app_name: Name,
    /// Active app topology revision.
    pub current_app_version: String,
    /// Hash of the active app topology.
    pub current_manifest_hash: Hash,
    /// Number of admitted app topology revisions.
    pub revision_count: u32,
    /// Audit sequence that deployed this topology.
    pub deployed_sequence: u64,
    /// Audit sequence that last updated this topology.
    pub updated_sequence: u64,
    /// Active app topology manifest.
    pub manifest: SoraAppInfraManifestV1,
}
impl SoraAppInfraStateV1 {
    /// Validate app topology state.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the state is inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra state",
            self.schema_version,
            SORA_APP_INFRA_STATE_VERSION_V1,
        )?;
        self.manifest.validate()?;
        if self.app_name != self.manifest.app_name {
            return Err(invalid_field(
                "sora app infra state",
                "app_name",
                "must match embedded manifest app_name",
            ));
        }
        if self.current_app_version != self.manifest.app_version {
            return Err(invalid_field(
                "sora app infra state",
                "current_app_version",
                "must match embedded manifest app_version",
            ));
        }
        if self.current_manifest_hash != self.manifest.manifest_hash() {
            return Err(invalid_field(
                "sora app infra state",
                "current_manifest_hash",
                "must match embedded manifest hash",
            ));
        }
        if self.revision_count == 0 || self.deployed_sequence == 0 || self.updated_sequence == 0 {
            return Err(invalid_field(
                "sora app infra state",
                "sequence",
                "revision_count and audit sequences must be greater than zero",
            ));
        }
        Ok(())
    }
}
/// Audit record for an authoritative Soracloud app topology event.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraAuditEventV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Monotonic audit/event sequence.
    pub sequence: u64,
    /// App topology action.
    pub action: SoraAppInfraActionV1,
    /// Application affected by the transition.
    pub app_name: Name,
    /// Previous active app version, when applicable.
    #[norito(required)]
    pub from_version: Option<String>,
    /// Resulting active app version.
    pub to_version: String,
    /// Hash of the admitted app topology.
    pub app_manifest_hash: Hash,
    /// Number of service refs in the topology.
    pub service_count: u32,
    /// Provenance signer that authorized the topology transition.
    pub signer: PublicKey,
}
impl SoraAppInfraAuditEventV1 {
    /// Validate app topology audit records.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when event fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra audit event",
            self.schema_version,
            SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora app infra audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        validate_nonblank_field("sora app infra audit event", "to_version", &self.to_version)?;
        validate_soracloud_digest_hash(
            "sora app infra audit event",
            "app_manifest_hash",
            self.app_manifest_hash,
        )?;
        if self.service_count == 0 {
            return Err(invalid_field(
                "sora app infra audit event",
                "service_count",
                "must be greater than zero",
            ));
        }
        validate_optional_nonempty(
            "sora app infra audit event",
            "from_version",
            self.from_version.as_deref(),
        )
    }
}
/// Audit record for an authoritative Soracloud lifecycle event.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceAuditEventV1 {
    /// Schema version; must equal [`SORA_SERVICE_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Monotonic audit/event sequence.
    pub sequence: u64,
    /// Lifecycle action that produced the event.
    pub action: SoraServiceLifecycleActionV1,
    /// Service affected by the transition.
    pub service_name: Name,
    /// Previous active version; the wire key is explicitly null when not applicable.
    #[norito(required)]
    pub from_version: Option<String>,
    /// Resulting active version after the transition.
    pub to_version: String,
    /// Service manifest hash bound to the transition.
    pub service_manifest_hash: Hash,
    /// Container manifest hash bound to the transition.
    pub container_manifest_hash: Hash,
    /// Governance transaction hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub governance_tx_hash: Option<Hash>,
    /// Binding associated with this event; the wire key is explicitly null when absent.
    #[norito(required)]
    pub binding_name: Option<Name>,
    /// State key associated with this event; the wire key is explicitly null when absent.
    #[norito(required)]
    pub state_key: Option<String>,
    /// Service config entry; the wire key is explicitly null when absent.
    #[norito(required)]
    pub config_name: Option<String>,
    /// Service secret entry; the wire key is explicitly null when absent.
    #[norito(required)]
    pub secret_name: Option<String>,
    /// Rollout handle; the wire key is explicitly null when absent.
    #[norito(required)]
    pub rollout_handle: Option<String>,
    /// Decryption policy name; the wire key is explicitly null when absent.
    #[norito(required)]
    pub policy_name: Option<Name>,
    /// Snapshotted policy hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub policy_snapshot_hash: Option<Hash>,
    /// Jurisdiction/compliance tag; the wire key is explicitly null when absent.
    #[norito(required)]
    pub jurisdiction_tag: Option<String>,
    /// Consent-evidence commitment; the wire key is explicitly null when absent.
    #[norito(required)]
    pub consent_evidence_hash: Option<Hash>,
    /// Break-glass flag; the wire key is explicitly null when not applicable.
    #[norito(required)]
    pub break_glass: Option<bool>,
    /// Break-glass justification; the wire key is explicitly null when absent.
    #[norito(required)]
    pub break_glass_reason: Option<String>,
    /// Provenance signer that authorized the lifecycle action.
    pub signer: PublicKey,
}
impl SoraServiceAuditEventV1 {
    /// Validate Soracloud lifecycle audit records.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when event sequencing or version fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_required_fields()?;
        self.validate_optional_fields()?;
        self.validate_break_glass_fields()
    }
    fn validate_required_fields(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service audit event",
            self.schema_version,
            SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora service audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        if self
            .from_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(invalid_field(
                "sora service audit event",
                "from_version",
                "must not be empty when provided",
            ));
        }
        validate_nonblank_field("sora service audit event", "to_version", &self.to_version)?;
        validate_soracloud_digest_hash(
            "sora service audit event",
            "service_manifest_hash",
            self.service_manifest_hash,
        )?;
        validate_soracloud_digest_hash(
            "sora service audit event",
            "container_manifest_hash",
            self.container_manifest_hash,
        )?;
        Ok(())
    }
    fn validate_optional_fields(&self) -> Result<(), SoracloudManifestError> {
        if self
            .rollout_handle
            .as_ref()
            .is_some_and(|handle| handle.trim().is_empty())
        {
            return Err(invalid_field(
                "sora service audit event",
                "rollout_handle",
                "must not be empty when provided",
            ));
        }
        if self
            .state_key
            .as_ref()
            .is_some_and(|state_key| state_key.trim().is_empty())
        {
            return Err(invalid_field(
                "sora service audit event",
                "state_key",
                "must not be empty when provided",
            ));
        }
        if self
            .state_key
            .as_ref()
            .is_some_and(|state_key| !state_key.starts_with('/'))
        {
            return Err(invalid_field(
                "sora service audit event",
                "state_key",
                "must start with '/' when provided",
            ));
        }
        if let Some(config_name) = self.config_name.as_deref() {
            validate_service_material_name("sora service audit event", "config_name", config_name)?;
        }
        if let Some(secret_name) = self.secret_name.as_deref() {
            validate_service_material_name("sora service audit event", "secret_name", secret_name)?;
        }
        if self
            .jurisdiction_tag
            .as_ref()
            .is_some_and(|tag| tag.trim().is_empty())
        {
            return Err(invalid_field(
                "sora service audit event",
                "jurisdiction_tag",
                "must not be empty when provided",
            ));
        }
        if let Some(governance_tx_hash) = self.governance_tx_hash {
            validate_soracloud_digest_hash(
                "sora service audit event",
                "governance_tx_hash",
                governance_tx_hash,
            )?;
        }
        if let Some(policy_snapshot_hash) = self.policy_snapshot_hash {
            validate_soracloud_digest_hash(
                "sora service audit event",
                "policy_snapshot_hash",
                policy_snapshot_hash,
            )?;
        }
        if let Some(consent_evidence_hash) = self.consent_evidence_hash {
            validate_soracloud_digest_hash(
                "sora service audit event",
                "consent_evidence_hash",
                consent_evidence_hash,
            )?;
        }
        Ok(())
    }
    fn validate_break_glass_fields(&self) -> Result<(), SoracloudManifestError> {
        if self
            .break_glass_reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(invalid_field(
                "sora service audit event",
                "break_glass_reason",
                "must not be empty when provided",
            ));
        }
        if self.break_glass == Some(false) && self.break_glass_reason.is_some() {
            return Err(invalid_field(
                "sora service audit event",
                "break_glass_reason",
                "must be null when break_glass=false",
            ));
        }
        if self.break_glass == Some(true) && self.break_glass_reason.is_none() {
            return Err(invalid_field(
                "sora service audit event",
                "break_glass_reason",
                "must be provided when break_glass=true",
            ));
        }
        Ok(())
    }
}
/// Runtime health status observed for a materialized service revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "health_status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceHealthStatusV1 {
    /// Revision is still hydrating bundles or replay state.
    Hydrating,
    /// Revision is serving normally.
    #[default]
    Healthy,
    /// Revision is serving but under elevated failure/load pressure.
    Degraded,
    /// Revision is not fit to serve traffic.
    Unavailable,
}
/// Authoritative runtime state for the active revision of a service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceRuntimeStateV1 {
    /// Schema version; must equal [`SORA_SERVICE_RUNTIME_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose runtime state is being tracked.
    pub service_name: Name,
    /// Active service revision/version on this node/ledger view.
    pub active_service_version: String,
    /// Current health classification.
    pub health_status: SoraServiceHealthStatusV1,
    /// Load factor in basis points (`0..=10_000`).
    pub load_factor_bps: u16,
    /// Active materialized bundle hash.
    pub materialized_bundle_hash: Hash,
    /// Active rollout handle; the wire key is explicitly null when absent.
    #[norito(required)]
    pub rollout_handle: Option<String>,
    /// Pending ordered mailbox messages for the service.
    pub pending_mailbox_message_count: u32,
    /// Last emitted runtime receipt identifier; the wire key is explicitly null when absent.
    #[norito(required)]
    pub last_receipt_id: Option<Hash>,
}
impl SoraServiceRuntimeStateV1 {
    /// Validate runtime-state bounds and formatting.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// runtime-state fields are out of range.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service runtime state",
            self.schema_version,
            SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora service runtime state",
            "active_service_version",
            &self.active_service_version,
        )?;
        if self.load_factor_bps > 10_000 {
            return Err(invalid_field(
                "sora service runtime state",
                "load_factor_bps",
                "must be within 0..=10_000",
            ));
        }
        if let Some(handle) = self.rollout_handle.as_ref()
            && handle.trim().is_empty()
        {
            return Err(invalid_field(
                "sora service runtime state",
                "rollout_handle",
                "must not be empty when provided",
            ));
        }
        validate_soracloud_digest_hash(
            "sora service runtime state",
            "materialized_bundle_hash",
            self.materialized_bundle_hash,
        )?;
        if let Some(last_receipt_id) = self.last_receipt_id {
            validate_soracloud_digest_hash(
                "sora service runtime state",
                "last_receipt_id",
                last_receipt_id,
            )?;
        }
        Ok(())
    }
}
/// Authoritative runtime state for one placed Inrou replica slot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouReplicaRuntimeStateV1 {
    /// Schema version; must equal [`SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose placed replica is being tracked.
    pub service_name: Name,
    /// Service revision currently materialized for the placed replica.
    pub service_version: String,
    /// One-based placed replica slot.
    pub replica_slot: u16,
    /// Validator currently hosting the replica.
    pub validator_account_id: AccountId,
    /// Peer identifier currently hosting the replica.
    pub peer_id: String,
    /// Guest ISA currently materializing the replica.
    pub selected_guest_isa: SoraInrouGuestIsaV1,
    /// Current health classification for the placed replica.
    pub health_status: SoraServiceHealthStatusV1,
    /// Load factor in basis points (`0..=10_000`) for the placed replica.
    pub load_factor_bps: u16,
    /// Active materialized bundle hash.
    pub materialized_bundle_hash: Hash,
    /// Total authoritative egress bytes accounted for the placed replica so far.
    pub accounted_egress_bytes: u64,
    /// Pending ordered mailbox messages projected for the placed replica.
    pub pending_mailbox_message_count: u32,
    /// Last emitted runtime receipt identifier; the wire key is explicitly null when absent.
    #[norito(required)]
    pub last_receipt_id: Option<Hash>,
    /// Timestamp when this replica state was last refreshed.
    pub updated_at_ms: u64,
    /// Human-readable last runtime error, when present.
    #[norito(required)]
    pub last_error: Option<String>,
}
impl SoraInrouReplicaRuntimeStateV1 {
    /// Validate per-replica runtime-state bounds and formatting.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// runtime-state fields are out of range.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora inrou replica runtime state",
            self.schema_version,
            SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora inrou replica runtime state",
            "service_version",
            &self.service_version,
        )?;
        if self.replica_slot == 0 {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "replica_slot",
                "must be greater than zero",
            ));
        }
        validate_peer_id_field("sora inrou replica runtime state", &self.peer_id)?;
        if self.load_factor_bps > 10_000 {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "load_factor_bps",
                "must be within 0..=10_000",
            ));
        }
        if self.updated_at_ms == 0 {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "updated_at_ms",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora inrou replica runtime state",
            "materialized_bundle_hash",
            self.materialized_bundle_hash,
        )?;
        if let Some(last_receipt_id) = self.last_receipt_id {
            validate_soracloud_digest_hash(
                "sora inrou replica runtime state",
                "last_receipt_id",
                last_receipt_id,
            )?;
        }
        Ok(())
    }
}
/// Ordered asynchronous mailbox message used for replicated cross-service calls.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceMailboxMessageV1 {
    /// Schema version; must equal [`SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic message identifier.
    pub message_id: Hash,
    /// Source service name.
    pub from_service: Name,
    /// Source handler name.
    pub from_handler: Name,
    /// Destination service name.
    pub to_service: Name,
    /// Destination handler name.
    pub to_handler: Name,
    /// Opaque mailbox payload bytes replicated through authoritative state.
    pub payload_bytes: Vec<u8>,
    /// Commitment over the opaque message payload.
    pub payload_commitment: Hash,
    /// Ordered sequence at which the message was enqueued.
    pub enqueue_sequence: u64,
    /// Earliest sequence at which the message may execute.
    pub available_after_sequence: u64,
    /// Expiry sequence; the wire key is explicitly null when the message does not expire.
    #[norito(required)]
    pub expires_at_sequence: Option<u64>,
}
impl SoraServiceMailboxMessageV1 {
    /// Validate deterministic mailbox-message ordering constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// availability/expiry sequences are inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service mailbox message",
            self.schema_version,
            SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        )?;
        validate_soracloud_digest_hash(
            "sora service mailbox message",
            "message_id",
            self.message_id,
        )?;
        validate_soracloud_digest_hash(
            "sora service mailbox message",
            "payload_commitment",
            self.payload_commitment,
        )?;
        if self.available_after_sequence < self.enqueue_sequence {
            return Err(invalid_field(
                "sora service mailbox message",
                "available_after_sequence",
                "must be >= enqueue_sequence",
            ));
        }
        if Hash::new(self.payload_bytes.as_slice()) != self.payload_commitment {
            return Err(invalid_field(
                "sora service mailbox message",
                "payload_commitment",
                "must match payload_bytes",
            ));
        }
        if let Some(expires_at) = self.expires_at_sequence
            && expires_at <= self.available_after_sequence
        {
            return Err(invalid_field(
                "sora service mailbox message",
                "expires_at_sequence",
                "must be greater than available_after_sequence",
            ));
        }
        Ok(())
    }
}
/// Authoritative execution receipt emitted by the generic Soracloud runtime.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraRuntimeReceiptV1 {
    /// Schema version; must equal [`SORA_RUNTIME_RECEIPT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic receipt identifier.
    pub receipt_id: Hash,
    /// Service that emitted the receipt.
    pub service_name: Name,
    /// Service revision that produced the receipt.
    pub service_version: String,
    /// Handler that executed the workload.
    pub handler_name: Name,
    /// Execution class for the receipt.
    pub handler_class: SoraServiceHandlerClassV1,
    /// Commitment over the request envelope.
    pub request_commitment: Hash,
    /// Commitment over the result envelope.
    pub result_commitment: Hash,
    /// Certification mode used for the response or audit record.
    pub certified_by: SoraCertifiedResponsePolicyV1,
    /// Ordered sequence that emitted the receipt.
    pub emitted_sequence: u64,
    /// Authoritative HF placement; the wire key is explicitly null when absent.
    #[norito(required)]
    pub placement_id: Option<Hash>,
    /// Executing validator account; the wire key is explicitly null when absent.
    #[norito(required)]
    pub selected_validator_account_id: Option<AccountId>,
    /// Executing Soracloud peer; the wire key is explicitly null when absent.
    #[norito(required)]
    pub selected_peer_id: Option<String>,
    /// Triggering mailbox message; the wire key is explicitly null when absent.
    #[norito(required)]
    pub mailbox_message_id: Option<Hash>,
    /// Journal artifact hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub journal_artifact_hash: Option<Hash>,
    /// Checkpoint artifact hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
}
impl SoraRuntimeReceiptV1 {
    /// Validate runtime-receipt classification and certification rules.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// handler-class/certification invariants are violated.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora runtime receipt",
            self.schema_version,
            SORA_RUNTIME_RECEIPT_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora runtime receipt",
            "service_version",
            &self.service_version,
        )?;
        validate_soracloud_digest_hash("sora runtime receipt", "receipt_id", self.receipt_id)?;
        validate_soracloud_digest_hash(
            "sora runtime receipt",
            "request_commitment",
            self.request_commitment,
        )?;
        validate_soracloud_digest_hash(
            "sora runtime receipt",
            "result_commitment",
            self.result_commitment,
        )?;
        if let Some(placement_id) = self.placement_id {
            validate_soracloud_digest_hash("sora runtime receipt", "placement_id", placement_id)?;
        }
        if let Some(mailbox_message_id) = self.mailbox_message_id {
            validate_soracloud_digest_hash(
                "sora runtime receipt",
                "mailbox_message_id",
                mailbox_message_id,
            )?;
        }
        if let Some(journal_artifact_hash) = self.journal_artifact_hash {
            validate_soracloud_digest_hash(
                "sora runtime receipt",
                "journal_artifact_hash",
                journal_artifact_hash,
            )?;
        }
        if let Some(checkpoint_artifact_hash) = self.checkpoint_artifact_hash {
            validate_soracloud_digest_hash(
                "sora runtime receipt",
                "checkpoint_artifact_hash",
                checkpoint_artifact_hash,
            )?;
        }
        validate_optional_nonempty(
            "sora runtime receipt",
            "selected_peer_id",
            self.selected_peer_id.as_deref(),
        )?;
        let placement_field_count = usize::from(self.placement_id.is_some())
            + usize::from(self.selected_validator_account_id.is_some())
            + usize::from(self.selected_peer_id.is_some());
        if placement_field_count != 0 && placement_field_count != 3 {
            return Err(invalid_field(
                "sora runtime receipt",
                "placement_id",
                "placement attribution must provide placement_id, selected_validator_account_id, and selected_peer_id together",
            ));
        }
        match self.handler_class {
            SoraServiceHandlerClassV1::Asset | SoraServiceHandlerClassV1::Query => {
                if self.certified_by == SoraCertifiedResponsePolicyV1::None {
                    return Err(invalid_field(
                        "sora runtime receipt",
                        "certified_by",
                        "asset/query receipts must remain certified",
                    ));
                }
                if self.mailbox_message_id.is_some() {
                    return Err(invalid_field(
                        "sora runtime receipt",
                        "mailbox_message_id",
                        "asset/query receipts must not originate from mailbox delivery",
                    ));
                }
            }
            SoraServiceHandlerClassV1::Update | SoraServiceHandlerClassV1::PrivateUpdate => {
                if self.certified_by != SoraCertifiedResponsePolicyV1::None {
                    return Err(invalid_field(
                        "sora runtime receipt",
                        "certified_by",
                        "update/private_update receipts use ordered mailbox execution instead of certified fast-path responses",
                    ));
                }
            }
        }
        Ok(())
    }
}
