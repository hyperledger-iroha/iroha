const SORA_STORAGE_BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
const SORA_NETWORK_BYTES_PER_MIB: u64 = 1024 * 1024;
const SORA_HTTP_SERVICE_QUOTA_CLASS_TAIRA_OPEN: &str = "taira-open";
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SoraHttpServiceQuotaClassPolicy {
    max_replicas: u16,
    max_cpu_millis: u32,
    max_memory_bytes: u64,
    max_ephemeral_storage_bytes: u64,
    max_open_files: u32,
    max_tasks: u16,
    max_total_lease_volume_bytes: u64,
}
const TAIRA_OPEN_HTTP_SERVICE_QUOTA_POLICY: SoraHttpServiceQuotaClassPolicy =
    SoraHttpServiceQuotaClassPolicy {
        max_replicas: 4,
        max_cpu_millis: 4_000,
        max_memory_bytes: 8 * SORA_STORAGE_BYTES_PER_GIB,
        max_ephemeral_storage_bytes: 16 * SORA_STORAGE_BYTES_PER_GIB,
        max_open_files: 8_192,
        max_tasks: 1_024,
        max_total_lease_volume_bytes: 512 * SORA_STORAGE_BYTES_PER_GIB,
    };
fn http_service_quota_class_policy(quota_class: &str) -> Option<SoraHttpServiceQuotaClassPolicy> {
    match quota_class {
        SORA_HTTP_SERVICE_QUOTA_CLASS_TAIRA_OPEN => Some(TAIRA_OPEN_HTTP_SERVICE_QUOTA_POLICY),
        _ => None,
    }
}
/// Validation errors returned by `Soracloud` manifest helpers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoracloudManifestError {
    /// The payload references an unsupported schema version.
    #[error("{manifest} schema version {found} is not supported (expected version {expected})")]
    UnsupportedVersion {
        /// Logical manifest type being validated.
        manifest: &'static str,
        /// Supported schema version.
        expected: u16,
        /// Encountered schema version.
        found: u16,
    },
    /// A required field was left empty.
    #[error("{manifest} field `{field}` must not be empty")]
    EmptyField {
        /// Logical manifest type being validated.
        manifest: &'static str,
        /// Name of the field that failed validation.
        field: &'static str,
    },
    /// A field violated deterministic or policy constraints.
    #[error("{manifest} field `{field}` is invalid: {reason}")]
    InvalidField {
        /// Logical manifest type being validated.
        manifest: &'static str,
        /// Name of the field that failed validation.
        field: &'static str,
        /// Human-readable reason.
        reason: String,
    },
    /// Service manifests cannot define duplicate state-binding names.
    #[error("sora service manifest includes duplicate state binding `{binding}`")]
    DuplicateStateBinding {
        /// Duplicate binding identifier.
        binding: Name,
    },
    /// Service manifests cannot define duplicate lease-volume names.
    #[error("sora service manifest includes duplicate lease volume `{volume}`")]
    DuplicateLeaseVolume {
        /// Duplicate lease-volume identifier.
        volume: Name,
    },
    /// App infrastructure manifests cannot reference the same service twice.
    #[error("sora app infra manifest includes duplicate service `{service}`")]
    DuplicateAppService {
        /// Duplicate service identifier.
        service: Name,
    },
    /// Service manifests cannot define duplicate handler names.
    #[error("sora service manifest includes duplicate handler `{handler}`")]
    DuplicateHandler {
        /// Duplicate handler identifier.
        handler: Name,
    },
    /// Agent apartment manifests cannot define duplicate tool capabilities.
    #[error("agent apartment manifest includes duplicate tool capability `{tool}`")]
    DuplicateToolCapability {
        /// Duplicate tool identifier.
        tool: String,
    },
    /// Agent apartment manifests cannot define duplicate policy capabilities.
    #[error("agent apartment manifest includes duplicate policy capability `{policy}`")]
    DuplicatePolicyCapability {
        /// Duplicate policy capability identifier.
        policy: Name,
    },
    /// Agent apartment manifests cannot define duplicate spend-limit assets.
    #[error("agent apartment manifest includes duplicate spend-limit asset `{asset}`")]
    DuplicateSpendLimitAsset {
        /// Duplicate spend-limit asset identifier.
        asset: String,
    },
}
fn validate_schema_version(
    manifest: &'static str,
    found: u16,
    expected: u16,
) -> Result<(), SoracloudManifestError> {
    if found != expected {
        return Err(SoracloudManifestError::UnsupportedVersion {
            manifest,
            expected,
            found,
        });
    }
    Ok(())
}
fn validate_nonblank_field(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    if value.trim().is_empty() {
        return Err(SoracloudManifestError::EmptyField { manifest, field });
    }
    Ok(())
}
fn validate_peer_id_field(
    manifest: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonblank_field(manifest, "peer_id", value)?;
    let peer_id = value.parse::<PeerId>().map_err(|error| {
        invalid_field(
            manifest,
            "peer_id",
            format!("must be a canonical peer public key: {error}"),
        )
    })?;
    if peer_id.to_string() != value {
        return Err(invalid_field(
            manifest,
            "peer_id",
            "must use the exact canonical peer public-key spelling",
        ));
    }
    Ok(())
}
fn validate_validator_account_peer_id(
    manifest: &'static str,
    validator_account_id: &AccountId,
    peer_id: &str,
) -> Result<(), SoracloudManifestError> {
    validate_peer_id_field(manifest, peer_id)?;
    let signatory = validator_account_id.try_signatory().ok_or_else(|| {
        invalid_field(
            manifest,
            "validator_account_id",
            "account-derived peer identity requires a single-signatory validator account",
        )
    })?;
    if PeerId::from(signatory.clone()).to_string() != peer_id {
        return Err(invalid_field(
            manifest,
            "peer_id",
            "peer must be derived from the validator account's single signatory",
        ));
    }
    Ok(())
}
fn validate_optional_nonempty(
    manifest: &'static str,
    field: &'static str,
    value: Option<&str>,
) -> Result<(), SoracloudManifestError> {
    if let Some(value) = value {
        validate_nonblank_field(manifest, field, value)?;
    }
    Ok(())
}
fn invalid_field(
    manifest: &'static str,
    field: &'static str,
    reason: impl Into<String>,
) -> SoracloudManifestError {
    SoracloudManifestError::InvalidField {
        manifest,
        field,
        reason: reason.into(),
    }
}
/// Runtime expected by the container manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "runtime", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraContainerRuntimeV1 {
    /// Execute IVM bytecode entrypoints.
    #[default]
    Ivm,
    /// Execute a Soracloud Inrou workload for hosted HTTP services.
    Inrou,
}
impl SoraContainerRuntimeV1 {
    /// Returns `true` when the runtime is the deterministic IVM plane.
    #[must_use]
    pub const fn is_deterministic(self) -> bool {
        matches!(self, Self::Ivm)
    }
    /// Returns `true` when the runtime targets the hosted HTTP service plane.
    #[must_use]
    pub const fn is_http_service_runtime(self) -> bool {
        matches!(self, Self::Inrou)
    }
}
/// Guest userspace profile expected by an Inrou VM image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "guest_os", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraInrouGuestOsV1 {
    /// Debian slim guest userspace with `apt` package management.
    #[default]
    DebianSlim,
}
/// Guest ISA profile admitted for an Inrou VM image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "guest_isa", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraInrouGuestIsaV1 {
    /// Native 64-bit x86 guest image.
    #[cfg_attr(feature = "json", norito(rename = "x86_64"))]
    X8664,
    /// Native 64-bit Arm guest image.
    #[cfg_attr(feature = "json", norito(rename = "aarch64"))]
    Aarch64,
}
impl SoraInrouGuestIsaV1 {
    /// Canonical JSON/object-key label for the guest ISA profile.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X8664 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }
    /// Parse a canonical guest ISA label from JSON/object-key text.
    #[must_use]
    pub fn parse_key(value: &str) -> Option<Self> {
        match value {
            "x86_64" => Some(Self::X8664),
            "aarch64" => Some(Self::Aarch64),
            _ => None,
        }
    }
}
/// CDN-like distribution target for Soracloud-published artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "target", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraArtifactDistributionTargetV1 {
    /// Replicate and hydrate from the globally best eligible hosts.
    #[default]
    Global,
    /// Prefer hosts tagged with one of the requested geographies.
    Geographies(BTreeSet<String>),
}
impl SoraArtifactDistributionTargetV1 {
    /// Validate the distribution target labels.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when geography labels are malformed.
    pub fn validate(&self, field: &'static str) -> Result<(), SoracloudManifestError> {
        match self {
            Self::Global => Ok(()),
            Self::Geographies(tags) => {
                if tags.is_empty() {
                    return Err(invalid_field(
                        "sora artifact distribution policy",
                        field,
                        "geography target must include at least one tag",
                    ));
                }
                for tag in tags {
                    validate_distribution_geography_tag(
                        "sora artifact distribution policy",
                        field,
                        tag,
                    )?;
                }
                Ok(())
            }
        }
    }
}
/// Reusable policy for publishing artifacts to Soracloud host storage.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraArtifactDistributionPolicyV1 {
    /// Operator target: global or a set of geography tags.
    pub target: SoraArtifactDistributionTargetV1,
    /// Prefer lower observed latency among otherwise eligible hosts.
    pub prefer_low_latency: bool,
    /// Fall back to latency when host geography is missing or unverifiable.
    pub fallback_to_low_latency_when_geography_unknown: bool,
}
impl Default for SoraArtifactDistributionPolicyV1 {
    fn default() -> Self {
        Self {
            target: SoraArtifactDistributionTargetV1::Global,
            prefer_low_latency: true,
            fallback_to_low_latency_when_geography_unknown: true,
        }
    }
}
impl SoraArtifactDistributionPolicyV1 {
    /// Validate the distribution policy.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when target metadata is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.target.validate("target")
    }
}
/// Immutable `SoraFS` artifact reference used to hydrate Inrou guest images.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPublishedInrouGuestImageArtifactV1 {
    /// `SoraFS` manifest digest hex for the uploaded guest-image artifact bundle.
    pub manifest_digest_hex: String,
    /// CID rendered for the uploaded guest-image artifact bundle.
    pub content_cid: String,
    /// Optional storage manifest identifier returned by the storage pin endpoint.
    ///
    /// The JSON key is mandatory in V1; an unavailable identifier is encoded
    /// explicitly as `null` rather than by omitting the field.
    #[norito(required)]
    pub manifest_id_hex: Option<String>,
    /// Exact copy of the parent guest image's authoritative distribution policy.
    pub distribution: SoraArtifactDistributionPolicyV1,
}
impl SoraPublishedInrouGuestImageArtifactV1 {
    /// Validate the immutable artifact reference.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the artifact reference is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        let manifest_digest = validate_canonical_lower_hex_32(
            "sora published inrou guest image artifact",
            "manifest_digest_hex",
            &self.manifest_digest_hex,
        )?;
        validate_canonical_sorafs_content_cid(
            "sora published inrou guest image artifact",
            "content_cid",
            &self.content_cid,
        )?;
        if let Some(manifest_id_hex) = self.manifest_id_hex.as_ref() {
            let manifest_id = validate_canonical_lower_hex_32(
                "sora published inrou guest image artifact",
                "manifest_id_hex",
                manifest_id_hex,
            )?;
            if manifest_id != manifest_digest {
                return Err(invalid_field(
                    "sora published inrou guest image artifact",
                    "manifest_id_hex",
                    "must exactly equal `manifest_digest_hex`",
                ));
            }
        }
        self.distribution.validate()
    }
}
/// Guest-image member paths for one admitted Inrou ISA profile.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize))]
pub struct SoraInrouGuestImageV1 {
    /// Kernel image member path inside the signed Soracloud VM artifact bundle.
    pub kernel_image_path: String,
    /// Root filesystem image member path inside the signed Soracloud VM artifact bundle.
    pub rootfs_image_path: String,
    /// Optional initrd image member path inside the signed Soracloud VM artifact bundle.
    pub initrd_image_path: Option<String>,
    /// Authoritative distribution policy for the published guest-image artifact.
    pub distribution: SoraArtifactDistributionPolicyV1,
    /// Immutable `SoraFS` artifact that carries the guest image members after release.
    pub published_artifact: Option<SoraPublishedInrouGuestImageArtifactV1>,
}
#[cfg(feature = "json")]
impl JsonDeserialize for SoraInrouGuestImageV1 {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        Self::json_from_value(&value)
    }
    fn json_from_value(value: &Value) -> Result<Self, json::Error> {
        fn take_required<T: JsonDeserialize>(
            object: &mut BTreeMap<String, Value>,
            field: &str,
        ) -> Result<T, json::Error> {
            let value = object
                .remove(field)
                .ok_or_else(|| json::Error::MissingField {
                    field: field.to_owned(),
                })?;
            json::from_value(value)
        }
        let mut object = match value.clone() {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::InvalidField {
                    field: "SoraInrouGuestImageV1".to_owned(),
                    message: format!("expected object, got {other:?}"),
                });
            }
        };
        let kernel_image_path = take_required(&mut object, "kernel_image_path")?;
        let rootfs_image_path = take_required(&mut object, "rootfs_image_path")?;
        let initrd_image_path = take_required(&mut object, "initrd_image_path")?;
        let distribution = take_required(&mut object, "distribution")?;
        let published_artifact = take_required(&mut object, "published_artifact")?;
        if let Some(extra) = object.keys().next().cloned() {
            return Err(json::Error::UnknownField { field: extra });
        }
        Ok(Self {
            kernel_image_path,
            rootfs_image_path,
            initrd_image_path,
            distribution,
            published_artifact,
        })
    }
}
impl SoraInrouGuestImageV1 {
    /// Validate Inrou guest-image bundle member paths.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when one or more image paths are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_inrou_image_member_path("kernel_image_path", &self.kernel_image_path)?;
        validate_inrou_image_member_path("rootfs_image_path", &self.rootfs_image_path)?;
        if let Some(initrd_image_path) = self.initrd_image_path.as_ref() {
            validate_inrou_image_member_path("initrd_image_path", initrd_image_path)?;
        }
        let mut case_folded_member_paths = BTreeSet::new();
        for (field, path) in [
            ("kernel_image_path", Some(self.kernel_image_path.as_str())),
            ("rootfs_image_path", Some(self.rootfs_image_path.as_str())),
            ("initrd_image_path", self.initrd_image_path.as_deref()),
        ] {
            let Some(path) = path else {
                continue;
            };
            if !case_folded_member_paths.insert(path.to_ascii_lowercase()) {
                return Err(invalid_field(
                    "sora inrou guest image",
                    field,
                    "must not collide case-insensitively with another guest-image member path",
                ));
            }
        }
        self.distribution.validate()?;
        if let Some(published_artifact) = self.published_artifact.as_ref() {
            published_artifact.validate()?;
            if published_artifact.distribution != self.distribution {
                return Err(invalid_field(
                    "sora inrou guest image",
                    "published_artifact.distribution",
                    "must exactly match the image distribution policy",
                ));
            }
        }
        Ok(())
    }
}
/// Explicit Inrou VM metadata carried by hosted HTTP container manifests.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct SoraInrouManifestV1 {
    /// Schema version; must equal [`SORA_INROU_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Guest userspace contract expected by the runtime.
    pub guest_os: SoraInrouGuestOsV1,
    /// Admitted guest image assets keyed by guest ISA; at least one native profile is required.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::sora_inrou_guest_images_map")
    )]
    pub guest_images: BTreeMap<SoraInrouGuestIsaV1, SoraInrouGuestImageV1>,
    /// Optional user-data overlay path copied into the bootstrap seed drive.
    pub bootstrap_user_data_path: Option<String>,
    /// SSH public keys injected for tenant administration.
    pub ssh_authorized_keys: Vec<String>,
}
#[cfg(feature = "json")]
impl norito::json::JsonSerialize for SoraInrouManifestV1 {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        json::write_json_string("schema_version", out);
        out.push(':');
        self.schema_version.json_serialize(out);
        out.push(',');
        json::write_json_string("guest_os", out);
        out.push(':');
        self.guest_os.json_serialize(out);
        out.push(',');
        json::write_json_string("guest_images", out);
        out.push(':');
        out.push('{');
        let mut guest_images = self.guest_images.iter();
        if let Some((guest_isa, image)) = guest_images.next() {
            json::write_json_string(guest_isa.as_str(), out);
            out.push(':');
            image.json_serialize(out);
            for (guest_isa, image) in guest_images {
                out.push(',');
                json::write_json_string(guest_isa.as_str(), out);
                out.push(':');
                image.json_serialize(out);
            }
        }
        out.push('}');
        out.push(',');
        json::write_json_string("bootstrap_user_data_path", out);
        out.push(':');
        self.bootstrap_user_data_path.json_serialize(out);
        out.push(',');
        json::write_json_string("ssh_authorized_keys", out);
        out.push(':');
        self.ssh_authorized_keys.json_serialize(out);
        out.push('}');
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        out.begin_container()?;
        out.push_str("{\"schema_version\":")?;
        self.schema_version.json_serialize_to(out)?;
        out.push_str(",\"guest_os\":")?;
        self.guest_os.json_serialize_to(out)?;
        out.push_str(",\"guest_images\":")?;
        out.begin_container()?;
        out.push('{')?;
        let mut guest_images = self.guest_images.iter();
        if let Some((guest_isa, image)) = guest_images.next() {
            json::write_json_string_to(guest_isa.as_str(), out)?;
            out.push(':')?;
            image.json_serialize_to(out)?;
            for (guest_isa, image) in guest_images {
                out.push(',')?;
                json::write_json_string_to(guest_isa.as_str(), out)?;
                out.push(':')?;
                image.json_serialize_to(out)?;
            }
        }
        out.push('}')?;
        out.end_container();
        out.push_str(",\"bootstrap_user_data_path\":")?;
        self.bootstrap_user_data_path.json_serialize_to(out)?;
        out.push_str(",\"ssh_authorized_keys\":")?;
        self.ssh_authorized_keys.json_serialize_to(out)?;
        out.push('}')?;
        out.end_container();
        Ok(())
    }
}
#[cfg(feature = "json")]
impl JsonDeserialize for SoraInrouManifestV1 {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        Self::json_from_value(&value)
    }
    #[allow(clippy::too_many_lines, clippy::single_match_else)]
    fn json_from_value(value: &Value) -> Result<Self, json::Error> {
        fn take_required<T: JsonDeserialize>(
            object: &mut BTreeMap<String, Value>,
            field: &str,
        ) -> Result<T, json::Error> {
            let value = object
                .remove(field)
                .ok_or_else(|| json::Error::MissingField {
                    field: field.to_owned(),
                })?;
            json::from_value(value)
        }
        let mut object = match value.clone() {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::InvalidField {
                    field: "SoraInrouManifestV1".to_owned(),
                    message: format!("expected object, got {other:?}"),
                });
            }
        };
        let schema_version = take_required(&mut object, "schema_version")?;
        let guest_os = take_required(&mut object, "guest_os")?;
        let bootstrap_user_data_path = take_required(&mut object, "bootstrap_user_data_path")?;
        let ssh_authorized_keys = take_required(&mut object, "ssh_authorized_keys")?;
        let guest_images_value =
            object
                .remove("guest_images")
                .ok_or_else(|| json::Error::MissingField {
                    field: "guest_images".to_owned(),
                })?;
        let guest_image_object = match guest_images_value {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::InvalidField {
                    field: "guest_images".to_owned(),
                    message: format!("expected object, got {other:?}"),
                });
            }
        };
        let guest_images = guest_image_object
            .into_iter()
            .map(|(key, value)| -> Result<_, json::Error> {
                let guest_isa = SoraInrouGuestIsaV1::parse_key(&key)
                    .ok_or_else(|| json::Error::UnknownField { field: key.clone() })?;
                let image: SoraInrouGuestImageV1 = json::from_value(value)?;
                Ok((guest_isa, image))
            })
            .collect::<Result<BTreeMap<SoraInrouGuestIsaV1, SoraInrouGuestImageV1>, json::Error>>(
            )?;
        if let Some(extra) = object.keys().next().cloned() {
            return Err(json::Error::UnknownField { field: extra });
        }
        Ok(Self {
            schema_version,
            guest_os,
            guest_images,
            bootstrap_user_data_path,
            ssh_authorized_keys,
        })
    }
}
#[cfg(all(test, feature = "json"))]
mod inrou_manifest_checked_json_tests {
    use super::*;
    #[test]
    fn manifest_map_writer_matches_canonical_bytes_and_exact_bound() {
        let manifest = SoraInrouManifestV1 {
            schema_version: SORA_INROU_MANIFEST_VERSION_V1,
            guest_os: SoraInrouGuestOsV1::DebianSlim,
            guest_images: BTreeMap::from([(
                SoraInrouGuestIsaV1::X8664,
                SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                    rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                    initrd_image_path: None,
                    distribution: SoraArtifactDistributionPolicyV1::default(),
                    published_artifact: None,
                },
            )]),
            bootstrap_user_data_path: Some("/bootstrap/user-data".to_owned()),
            ssh_authorized_keys: vec!["ssh-ed25519 fixture".to_owned()],
        };
        let canonical = json::to_json(&manifest).expect("serialize manifest");
        assert_eq!(
            json::to_json_bounded(&manifest, canonical.len()).expect("serialize at exact bound"),
            canonical
        );
        assert_eq!(
            json::to_json_bounded(&manifest, canonical.len() - 1),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
    }
}
impl SoraInrouManifestV1 {
    /// Validate Inrou image paths and guest bootstrap metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, image
    /// paths are invalid, or SSH authorized keys are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora inrou manifest",
            self.schema_version,
            SORA_INROU_MANIFEST_VERSION_V1,
        )?;
        if self.guest_images.is_empty() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora inrou manifest",
                field: "guest_images",
                reason: "must include at least one native guest image profile".to_owned(),
            });
        }
        for guest_image in self.guest_images.values() {
            guest_image.validate()?;
        }
        if let Some(bootstrap_user_data_path) = self.bootstrap_user_data_path.as_ref() {
            validate_bundle_absolute_path(
                "sora inrou manifest",
                "bootstrap_user_data_path",
                bootstrap_user_data_path,
            )?;
        }
        let mut seen_ssh_authorized_keys = BTreeSet::new();
        for key in &self.ssh_authorized_keys {
            if key.trim().is_empty() {
                return Err(invalid_field(
                    "sora inrou manifest",
                    "ssh_authorized_keys",
                    "must not contain empty SSH public keys",
                ));
            }
            if key.chars().any(char::is_control) {
                return Err(invalid_field(
                    "sora inrou manifest",
                    "ssh_authorized_keys",
                    "must not contain control characters",
                ));
            }
            if !seen_ssh_authorized_keys.insert(key.clone()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora inrou manifest",
                    field: "ssh_authorized_keys",
                    reason: format!("duplicate SSH authorized key `{key}`"),
                });
            }
        }
        Ok(())
    }
}
/// Network egress policy for a service container.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "mode", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraNetworkPolicyV1 {
    /// Open egress is allowed and must be metered by the runtime.
    Open,
    /// No network egress is allowed.
    Isolated,
    /// Egress is allowed only to the listed hostnames and ports.
    Allowlist(Vec<SoraNetworkAllowlistEntryV1>),
}
/// A single allowlist rule for outbound network access.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraNetworkAllowlistEntryV1 {
    /// Allowed outbound hostname.
    pub host: String,
    /// Allowed outbound TCP ports for the hostname.
    pub ports: Vec<u16>,
}
impl SoraNetworkAllowlistEntryV1 {
    /// Construct a hostname + ports allowlist entry.
    #[must_use]
    pub fn new(host: impl Into<String>, ports: impl IntoIterator<Item = u16>) -> Self {
        Self {
            host: host.into(),
            ports: ports.into_iter().collect(),
        }
    }
    /// Return `true` when the rule matches the supplied hostname.
    #[must_use]
    pub fn matches_host(&self, host: &str) -> bool {
        self.host.eq_ignore_ascii_case(host)
    }
    /// Return `true` when the rule admits the supplied TCP port.
    #[must_use]
    pub fn allows_port(&self, port: u16) -> bool {
        self.ports.contains(&port)
    }
}
impl SoraNetworkPolicyV1 {
    /// Return `true` when the policy admits the supplied hostname.
    #[must_use]
    pub fn allows_host(&self, host: &str) -> bool {
        match self {
            Self::Open => true,
            Self::Isolated => false,
            Self::Allowlist(entries) => entries.iter().any(|entry| entry.matches_host(host)),
        }
    }
    /// Return `true` when the policy admits the supplied hostname and port.
    #[must_use]
    pub fn allows_host_port(&self, host: &str, port: u16) -> bool {
        match self {
            Self::Open => true,
            Self::Isolated => false,
            Self::Allowlist(entries) => entries
                .iter()
                .any(|entry| entry.matches_host(host) && entry.allows_port(port)),
        }
    }

    fn validate_for_inrou(&self) -> Result<(), SoracloudManifestError> {
        let Self::Allowlist(entries) = self else {
            return if matches!(self, Self::Isolated) {
                Ok(())
            } else {
                Err(invalid_field(
                    "sora container manifest",
                    "capabilities.network",
                    "Inrou V1 forbids unrestricted network egress",
                ))
            };
        };
        if entries.is_empty() {
            return Err(invalid_field(
                "sora container manifest",
                "capabilities.network",
                "Inrou allowlist egress requires at least one host",
            ));
        }
        let mut seen_hosts = BTreeSet::new();
        for entry in entries {
            let host = entry.host.as_str();
            if host.is_empty()
                || host.trim() != host
                || host.chars().any(char::is_control)
                || host.chars().any(char::is_whitespace)
            {
                return Err(invalid_field(
                    "sora container manifest",
                    "capabilities.network",
                    "Inrou allowlist hosts must be nonempty canonical hostnames or IP literals",
                ));
            }
            if !seen_hosts.insert(host.to_ascii_lowercase()) {
                return Err(invalid_field(
                    "sora container manifest",
                    "capabilities.network",
                    format!("duplicate Inrou allowlist host `{host}`"),
                ));
            }
            if entry.ports.is_empty() {
                return Err(invalid_field(
                    "sora container manifest",
                    "capabilities.network",
                    format!("Inrou allowlist host `{host}` must include at least one TCP port"),
                ));
            }
            let mut seen_ports = BTreeSet::new();
            for port in &entry.ports {
                if *port == 0 || !seen_ports.insert(*port) {
                    return Err(invalid_field(
                        "sora container manifest",
                        "capabilities.network",
                        format!(
                            "Inrou allowlist host `{host}` contains an invalid or duplicate TCP port `{port}`"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}
/// Capability policy enforced by the Sora Container Runtime.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[allow(clippy::struct_excessive_bools)]
#[norito(deny_unknown_fields)]
pub struct SoraCapabilityPolicyV1 {
    /// Egress policy for outbound network access.
    pub network: SoraNetworkPolicyV1,
    /// Whether wallet/key signing capabilities are exposed to the service.
    pub allow_wallet_signing: bool,
    /// Whether deterministic key-value writes are allowed through bindings.
    pub allow_state_writes: bool,
    /// Whether read-only model inference adapters are exposed to the service.
    pub allow_model_inference: bool,
    /// Whether model-training ops are allowed for this workload.
    pub allow_model_training: bool,
}
/// Resource limits for SCR process admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraResourceLimitsV1 {
    /// CPU budget in millicores.
    pub cpu_millis: NonZeroU32,
    /// Maximum resident memory in bytes.
    pub memory_bytes: NonZeroU64,
    /// Maximum ephemeral storage in bytes.
    pub ephemeral_storage_bytes: NonZeroU64,
    /// Maximum number of open file descriptors.
    pub max_open_files: NonZeroU32,
    /// Maximum number of cooperative tasks/threads.
    pub max_tasks: NonZeroU16,
}
/// Lifecycle hooks and probe settings used by SCR.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraLifecycleHooksV1 {
    /// Grace period allowed for service startup.
    pub start_grace_secs: NonZeroU32,
    /// Grace period allowed for service shutdown.
    pub stop_grace_secs: NonZeroU32,
    /// Optional HTTP health endpoint path.
    #[norito(required)]
    pub healthcheck_path: Option<String>,
}
/// Explicit config export injected into the runtime environment or mounted tree.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraConfigExportV1 {
    /// Required config entry being exported.
    pub config_name: String,
    /// Concrete export target.
    pub target: SoraConfigExportTargetV1,
}
impl SoraConfigExportV1 {
    /// Return the required config name referenced by this export.
    #[must_use]
    pub fn config_name(&self) -> &str {
        &self.config_name
    }
    /// Return the unique target identifier used for duplicate detection.
    #[must_use]
    pub fn target_identifier(&self) -> &str {
        match &self.target {
            SoraConfigExportTargetV1::Env(var_name) => var_name,
            SoraConfigExportTargetV1::File(relative_path) => relative_path,
        }
    }
}
/// Target kind for one explicit config export.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "target", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraConfigExportTargetV1 {
    /// Export the canonical JSON payload into an environment variable.
    Env(String),
    /// Export the canonical JSON payload into a mounted relative file path.
    File(String),
}
/// One verified signature entry in a multisig canonical request witness.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CanonicalRequestSignatureWitnessV1 {
    /// Public key that produced this signature.
    pub signer: PublicKey,
    /// Signature over the canonical request witness payload.
    pub signature: Signature,
}
/// Multisignature witness for app-auth canonical HTTP requests.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CanonicalRequestWitnessV1 {
    /// Schema version; must equal [`CANONICAL_REQUEST_WITNESS_VERSION_V1`].
    pub schema_version: u16,
    /// Multisig account authorising the canonical request.
    pub subject_account: AccountId,
    /// Unix timestamp in milliseconds used for freshness checks.
    pub timestamp_ms: u64,
    /// Replay nonce bound to the witness payload.
    pub nonce: String,
    /// Hash of the canonical request bytes reconstructed by the verifier.
    pub canonical_request_hash: Hash,
    /// Verified signature witnesses supplied by multisig participants.
    pub signatures: Vec<CanonicalRequestSignatureWitnessV1>,
}
/// Canonical executable bundle manifest for `Soracloud` workloads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize))]
pub struct SoraContainerManifestV1 {
    /// Schema version; must equal [`SORA_CONTAINER_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Runtime target for the bundle.
    pub runtime: SoraContainerRuntimeV1,
    /// Digest of the code bundle stored in `SoraFS`.
    pub bundle_hash: Hash,
    /// Path inside the `SoraFS` bundle with executable payload.
    pub bundle_path: String,
    /// Entrypoint symbol or executable path.
    pub entrypoint: String,
    /// Static arguments passed at process startup.
    pub args: Vec<String>,
    /// Environment variables supplied at launch.
    ///
    /// Keys must use canonical POSIX environment-variable name syntax: `[A-Za-z_][A-Za-z0-9_]*`.
    pub env: std::collections::BTreeMap<String, String>,
    /// Optional Inrou microVM metadata required for hosted HTTP VM workloads.
    pub inrou: Option<SoraInrouManifestV1>,
    /// Service-scoped config entries that must exist before this revision may start.
    pub required_config_names: Vec<String>,
    /// Service-scoped secret entries that must exist before this revision may start.
    pub required_secret_names: Vec<String>,
    /// Explicit config exports projected into the runtime environment or mounted tree.
    pub config_exports: Vec<SoraConfigExportV1>,
    /// Capability policy enforced by SCR.
    pub capabilities: SoraCapabilityPolicyV1,
    /// Resource limits used at admission/runtime.
    pub resources: SoraResourceLimitsV1,
    /// Lifecycle and health probe settings.
    pub lifecycle: SoraLifecycleHooksV1,
}
#[cfg(feature = "json")]
impl JsonDeserialize for SoraContainerManifestV1 {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        Self::json_from_value(&value)
    }
    fn json_from_value(value: &Value) -> Result<Self, json::Error> {
        fn take_required<T: JsonDeserialize>(
            object: &mut BTreeMap<String, Value>,
            field: &str,
        ) -> Result<T, json::Error> {
            let value = object
                .remove(field)
                .ok_or_else(|| json::Error::MissingField {
                    field: field.to_owned(),
                })?;
            json::from_value(value)
        }
        let mut object = match value.clone() {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::InvalidField {
                    field: "SoraContainerManifestV1".to_owned(),
                    message: format!("expected object, got {other:?}"),
                });
            }
        };
        let schema_version = take_required(&mut object, "schema_version")?;
        let runtime = take_required(&mut object, "runtime")?;
        let bundle_hash = take_required(&mut object, "bundle_hash")?;
        let bundle_path = take_required(&mut object, "bundle_path")?;
        let entrypoint = take_required(&mut object, "entrypoint")?;
        let args = take_required(&mut object, "args")?;
        let env = take_required(&mut object, "env")?;
        let inrou = take_required(&mut object, "inrou")?;
        let required_config_names = take_required(&mut object, "required_config_names")?;
        let required_secret_names = take_required(&mut object, "required_secret_names")?;
        let config_exports = take_required(&mut object, "config_exports")?;
        let capabilities = take_required(&mut object, "capabilities")?;
        let resources = take_required(&mut object, "resources")?;
        let lifecycle = take_required(&mut object, "lifecycle")?;
        if let Some(extra) = object.keys().next().cloned() {
            return Err(json::Error::UnknownField { field: extra });
        }
        Ok(Self {
            schema_version,
            runtime,
            bundle_hash,
            bundle_path,
            entrypoint,
            args,
            env,
            inrou,
            required_config_names,
            required_secret_names,
            config_exports,
            capabilities,
            resources,
            lifecycle,
        })
    }
}
impl SoraContainerManifestV1 {
    /// Validate schema version and deterministic constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required fields are empty.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora container manifest",
            self.schema_version,
            SORA_CONTAINER_MANIFEST_VERSION_V1,
        )?;
        validate_soracloud_digest_hash("sora container manifest", "bundle_hash", self.bundle_hash)?;
        validate_nonblank_field("sora container manifest", "bundle_path", &self.bundle_path)?;
        validate_nonblank_field("sora container manifest", "entrypoint", &self.entrypoint)?;
        for name in self.env.keys() {
            validate_environment_variable_name("env", name)?;
        }
        if self.runtime == SoraContainerRuntimeV1::Inrou {
            validate_bundle_absolute_path(
                "sora container manifest",
                "entrypoint",
                &self.entrypoint,
            )?;
            let Some(inrou) = self.inrou.as_ref() else {
                return Err(invalid_field(
                    "sora container manifest",
                    "inrou",
                    "Inrou runtimes require explicit microVM metadata",
                ));
            };
            inrou.validate()?;
            self.capabilities.network.validate_for_inrou()?;
        } else if self.inrou.is_some() {
            return Err(invalid_field(
                "sora container manifest",
                "inrou",
                "only Inrou runtimes may declare microVM metadata",
            ));
        }
        let mut required_configs = BTreeSet::new();
        for config_name in &self.required_config_names {
            validate_service_material_name(
                "sora container manifest",
                "required_config_names",
                config_name,
            )?;
            if !required_configs.insert(config_name.clone()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora container manifest",
                    field: "required_config_names",
                    reason: format!("duplicate required config `{config_name}`"),
                });
            }
        }
        let mut required_secrets = BTreeSet::new();
        for secret_name in &self.required_secret_names {
            validate_service_material_name(
                "sora container manifest",
                "required_secret_names",
                secret_name,
            )?;
            if !required_secrets.insert(secret_name.clone()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora container manifest",
                    field: "required_secret_names",
                    reason: format!("duplicate required secret `{secret_name}`"),
                });
            }
        }
        let mut config_export_env_targets = BTreeSet::new();
        let mut config_export_file_targets = BTreeSet::new();
        for export in &self.config_exports {
            validate_service_material_name(
                "sora container manifest",
                "config_exports.config_name",
                export.config_name(),
            )?;
            if !required_configs.contains(export.config_name()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora container manifest",
                    field: "config_exports",
                    reason: format!(
                        "config export `{}` must reference a declared required config",
                        export.config_name()
                    ),
                });
            }
            match &export.target {
                SoraConfigExportTargetV1::Env(var_name) => {
                    validate_environment_variable_name("config_exports", var_name)?;
                    if !config_export_env_targets.insert(var_name.clone()) {
                        return Err(SoracloudManifestError::InvalidField {
                            manifest: "sora container manifest",
                            field: "config_exports",
                            reason: format!("duplicate config export env target `{var_name}`"),
                        });
                    }
                }
                SoraConfigExportTargetV1::File(relative_path) => {
                    validate_config_export_relative_path(relative_path)?;
                    if !config_export_file_targets.insert(relative_path.clone()) {
                        return Err(SoracloudManifestError::InvalidField {
                            manifest: "sora container manifest",
                            field: "config_exports",
                            reason: format!(
                                "duplicate config export file target `{relative_path}`"
                            ),
                        });
                    }
                }
            }
        }
        if let Some(path) = self.lifecycle.healthcheck_path.as_ref()
            && !path.starts_with('/')
        {
            return Err(invalid_field(
                "sora container manifest",
                "lifecycle.healthcheck_path",
                "must start with '/'",
            ));
        }
        Ok(())
    }
}
/// Public exposure mode for a service route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "visibility", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraRouteVisibilityV1 {
    /// Route is externally reachable.
    #[default]
    Public,
    /// Route is cluster-internal only.
    Internal,
}
/// TLS requirements for service ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "tls", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraTlsModeV1 {
    /// TLS is mandatory.
    #[default]
    Required,
    /// TLS is optional and may be terminated upstream.
    Optional,
    /// TLS is disabled.
    Disabled,
}
/// Route definition for a deployed service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraRouteTargetV1 {
    /// Hostname assigned by `SoraDNS`.
    pub host: String,
    /// Path prefix exposed by the service.
    pub path_prefix: String,
    /// Internal service port.
    pub service_port: NonZeroU16,
    /// Exposure scope for the route.
    pub visibility: SoraRouteVisibilityV1,
    /// TLS requirements for ingress.
    pub tls_mode: SoraTlsModeV1,
}
/// Rollout/upgrade behavior for the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraRolloutPolicyV1 {
    /// Canary percentage applied before full rollout.
    pub canary_percent: u8,
    /// Maximum replicas unavailable during rollout.
    pub max_unavailable_replicas: u16,
    /// Rolling health window duration in seconds.
    pub health_window_secs: NonZeroU32,
    /// Consecutive health failures before auto rollback.
    pub automatic_rollback_failures: NonZeroU32,
}
/// Reference to a previously admitted container manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraContainerManifestRefV1 {
    /// Hash of the referenced container manifest bytes.
    pub manifest_hash: Hash,
    /// Expected schema version for the referenced manifest.
    pub expected_schema_version: u16,
}
/// Execution plane selected by a service manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "execution_plane", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceExecutionPlaneV1 {
    /// Deterministic certified reads and ordered mailbox execution on IVM.
    #[default]
    DeterministicService,
    /// Hosted HTTP service proxied through Torii/SoraDNS.
    HttpService,
}
/// Lease-backed mutable storage kind attached to an HTTP service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "lease_volume", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraLeaseVolumeKindV1 {
    /// Rebuildable public-service state such as indexes and checkpoints,
    /// shared across replicas of the same hosted-service revision.
    ServiceLeaseVolume,
    /// Confidential service state backed by encrypted or FHE payloads,
    /// shared across replicas of the same hosted-service revision.
    ConfidentialLeaseVolume,
    /// Persistent mutable root disk for an Inrou VM guest, materialized
    /// separately for each replica.
    PersistentRootLeaseVolume,
}
impl SoraLeaseVolumeKindV1 {
    /// Returns `true` when the volume is attached separately to each hosted-HTTP replica.
    #[must_use]
    pub const fn is_per_replica(self) -> bool {
        matches!(self, Self::PersistentRootLeaseVolume)
    }
    /// Returns `true` when the volume is shared across replicas of the same revision.
    #[must_use]
    pub const fn is_shared_across_replicas(self) -> bool {
        !self.is_per_replica()
    }
}
/// Lease-backed mutable storage binding for one hosted HTTP service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraLeaseVolumeBindingV1 {
    /// Human-readable volume identifier.
    pub volume_name: Name,
    /// Lease-backed volume kind.
    pub kind: SoraLeaseVolumeKindV1,
    /// Storage class requested from the underlying Sorafs/Soranet stack.
    pub storage_class: StorageClass,
    /// Declared in-runtime mount path for this volume.
    pub mount_path: String,
    /// Maximum logical bytes retained for this volume.
    pub max_total_bytes: NonZeroU64,
}
impl SoraLeaseVolumeBindingV1 {
    /// Returns `true` when this binding is attached separately to each hosted-HTTP replica.
    #[must_use]
    pub const fn attaches_per_replica(&self) -> bool {
        self.kind.is_per_replica()
    }
    /// Returns `true` when this binding is shared across replicas of the same hosted-HTTP revision.
    #[must_use]
    pub const fn attaches_shared_across_replicas(&self) -> bool {
        self.kind.is_shared_across_replicas()
    }
    /// Validate lease-volume binding invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the mount path is invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field("sora lease volume binding", "mount_path", &self.mount_path)?;
        if !self.mount_path.starts_with('/') {
            return Err(invalid_field(
                "sora lease volume binding",
                "mount_path",
                "must start with '/'",
            ));
        }
        if self.mount_path.chars().any(char::is_control) {
            return Err(invalid_field(
                "sora lease volume binding",
                "mount_path",
                "must not contain control characters",
            ));
        }
        if self.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume && self.mount_path != "/" {
            return Err(invalid_field(
                "sora lease volume binding",
                "mount_path",
                "persistent Inrou root volumes must mount at `/`",
            ));
        }
        Ok(())
    }
}
/// Economic policy required for hosted HTTP services.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHttpServiceEconomicsV1 {
    /// Schema version; must equal [`SORA_HTTP_SERVICE_ECONOMICS_VERSION_V1`].
    pub schema_version: u16,
    /// Scheduler/quota class resolved by the hosting control plane.
    pub quota_class: String,
    /// Anti-spam deployment deposit required to admit the service.
    pub deployment_deposit: Quantity,
    /// Prepaid runtime balance used for fail-closed admission and routing.
    pub prepaid_runtime_balance: Quantity,
    /// Lease duration, measured in consensus blocks.
    pub lease_duration_blocks: NonZeroU64,
    /// Runtime charge applied per active block.
    pub runtime_price_per_block: Quantity,
    /// Storage charge applied per GiB and active block.
    pub storage_price_per_gib_block: Quantity,
    /// Egress charge applied per MiB when runtime accounting reports traffic.
    pub egress_price_per_mib: Quantity,
}
impl Default for SoraHttpServiceEconomicsV1 {
    fn default() -> Self {
        Self {
            schema_version: SORA_HTTP_SERVICE_ECONOMICS_VERSION_V1,
            quota_class: "taira-open".to_owned(),
            deployment_deposit: xor_quantity_from_nanos(1_000_000_000),
            prepaid_runtime_balance: xor_quantity_from_nanos(50_000_000_000),
            lease_duration_blocks: NonZeroU64::new(86_400).expect("non-zero lease duration"),
            runtime_price_per_block: xor_quantity_from_nanos(250_000),
            storage_price_per_gib_block: xor_quantity_from_nanos(25_000),
            egress_price_per_mib: xor_quantity_from_nanos(5_000),
        }
    }
}
impl SoraHttpServiceEconomicsV1 {
    /// Validate hosted-service economic policy.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema version or required string
    /// fields are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora http service economics",
            self.schema_version,
            SORA_HTTP_SERVICE_ECONOMICS_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora http service economics",
            "quota_class",
            &self.quota_class,
        )?;
        for (field, value) in [
            ("deployment_deposit", &self.deployment_deposit),
            ("prepaid_runtime_balance", &self.prepaid_runtime_balance),
            (
                "runtime_price_per_block",
                &self.runtime_price_per_block,
            ),
            (
                "storage_price_per_gib_block",
                &self.storage_price_per_gib_block,
            ),
            ("egress_price_per_mib", &self.egress_price_per_mib),
        ] {
            if value.is_zero() {
                return Err(invalid_field(
                    "sora http service economics",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        Ok(())
    }
}
/// Authoritative hosted-service lease status projected by the control plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceLeaseStatusV1 {
    /// Lease is active and the service may be routed/materialized.
    #[default]
    Active,
    /// Lease expired at or before the observed consensus height.
    Expired,
    /// Prepaid runtime balance is exhausted.
    Exhausted,
    /// Lease was suspended by policy and must fail closed.
    Suspended,
}
/// Maximum authenticated reporter identities retained in one reporting epoch.
///
/// This consensus constant bounds world-state and Norito growth under repeated
/// revision rollout or validator churn. Once the bound is reached, the exact
/// newly assigned reporter may advance the reporting epoch only after every
/// prior checkpoint is terminal and no prior reporter remains actively placed.
pub const SORA_SERVICE_LEASE_MAX_EGRESS_REPORTER_CHECKPOINTS_V1: usize = 4_096;
/// Immutable placement evidence bound to one admitted egress reporter.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceLeaseReporterAssignmentV1 {
    /// Schema version; must equal
    /// [`SORA_SERVICE_LEASE_REPORTER_ASSIGNMENT_VERSION_V1`].
    pub schema_version: u16,
    /// Exact service revision served by the assigned replica.
    pub service_version: String,
    /// Complete authoritative Inrou placement admitted for this reporter.
    pub placement: SoraInrouReplicaPlacementV1,
    /// Consensus timestamp of the placement reconciliation that produced this assignment.
    pub placement_reconciled_at_ms: u64,
}
impl SoraServiceLeaseReporterAssignmentV1 {
    /// Validate immutable reporter-assignment evidence.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service lease reporter assignment",
            self.schema_version,
            SORA_SERVICE_LEASE_REPORTER_ASSIGNMENT_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora service lease reporter assignment",
            "service_version",
            &self.service_version,
        )?;
        self.placement.validate()?;
        if self.placement_reconciled_at_ms == 0 {
            return Err(invalid_field(
                "sora service lease reporter assignment",
                "placement_reconciled_at_ms",
                "must be greater than zero",
            ));
        }
        Ok(())
    }
}
/// One reporting-epoch-bound replica reporter's monotonic egress checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceLeaseEgressCheckpointV1 {
    /// Reporting epoch in which this checkpoint was admitted.
    pub reporting_epoch: u64,
    /// Immutable placement evidence authenticated when the checkpoint was admitted.
    pub assignment: SoraServiceLeaseReporterAssignmentV1,
    /// Monotonic egress bytes emitted by this exact reporter identity.
    pub accounted_egress_bytes: u64,
    /// Consensus height of the most recent accepted update for this reporter.
    pub last_updated_height: u64,
    /// Whether this reporter identity has submitted its terminal checkpoint.
    ///
    /// An identical active placement may reopen the checkpoint before serving
    /// again. A former reporter may only replay the exact terminal value.
    pub finalize_reporter: bool,
    /// Whether consensus force-finalized an idle reporter after the protocol grace.
    pub forced_finalization: bool,
}
/// Exact input accepted for one hosted-service egress checkpoint transition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceLeaseUsageAuditV1 {
    /// Schema version; must equal [`SORA_SERVICE_LEASE_USAGE_AUDIT_VERSION_V1`].
    pub schema_version: u16,
    /// Reporting epoch targeted by the accepted transition.
    pub reporting_epoch: u64,
    /// Immutable placement evidence authenticated for this transition.
    pub assignment: SoraServiceLeaseReporterAssignmentV1,
    /// Exact monotonic bytes supplied by this reporter identity.
    pub replica_accounted_egress_bytes: u64,
    /// Whether the reporter closed its current-epoch checkpoint.
    pub finalize_reporter: bool,
}
impl SoraServiceLeaseUsageAuditV1 {
    /// Validate structural lease-usage audit material.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service lease usage audit",
            self.schema_version,
            SORA_SERVICE_LEASE_USAGE_AUDIT_VERSION_V1,
        )?;
        if self.reporting_epoch == 0 {
            return Err(invalid_field(
                "sora service lease usage audit",
                "reporting_epoch",
                "must be greater than zero",
            ));
        }
        self.assignment.validate()?;
        Ok(())
    }
}
/// Typed audit payload for one hosted-service reporting-epoch rollover.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceLeaseReportingEpochRolloverV1 {
    /// Schema version; must equal
    /// [`SORA_SERVICE_LEASE_REPORTING_EPOCH_ROLLOVER_VERSION_V1`].
    pub schema_version: u16,
    /// Immutable economic lease incarnation to which the rollover belongs.
    pub lease_started_height: u64,
    /// Epoch whose terminal counters were settled.
    pub previous_reporting_epoch: u64,
    /// Exact successor epoch opened by the rollover.
    pub new_reporting_epoch: u64,
    /// Active validator that opened the successor epoch.
    pub reporter_account_id: AccountId,
    /// Active service revision assigned to the successor reporter.
    pub active_service_version: String,
    /// One-based replica slot assigned to the successor reporter.
    pub replica_slot: u16,
    /// Number of terminal prior-epoch checkpoints folded into settlement.
    pub finalized_checkpoint_count: u32,
    /// Number of idle prior-epoch checkpoints force-finalized during rollover.
    pub forced_finalized_checkpoint_count: u32,
    /// Exact prior-epoch bytes added to the settlement baseline.
    pub settled_egress_bytes_delta: u128,
    /// Exact cumulative settled bytes after the rollover.
    pub settled_egress_bytes: u128,
}
impl SoraServiceLeaseReportingEpochRolloverV1 {
    /// Validate the bound rollover transition.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the epoch transition, trigger,
    /// or settlement fields do not describe one exact rollover.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service lease reporting epoch rollover",
            self.schema_version,
            SORA_SERVICE_LEASE_REPORTING_EPOCH_ROLLOVER_VERSION_V1,
        )?;
        if self.lease_started_height == 0 {
            return Err(invalid_field(
                "sora service lease reporting epoch rollover",
                "lease_started_height",
                "must be greater than zero",
            ));
        }
        if self.previous_reporting_epoch == 0
            || self.previous_reporting_epoch.checked_add(1) != Some(self.new_reporting_epoch)
        {
            return Err(invalid_field(
                "sora service lease reporting epoch rollover",
                "new_reporting_epoch",
                "must be the checked successor of a non-zero previous_reporting_epoch",
            ));
        }
        validate_nonblank_field(
            "sora service lease reporting epoch rollover",
            "active_service_version",
            &self.active_service_version,
        )?;
        if self.replica_slot == 0 {
            return Err(invalid_field(
                "sora service lease reporting epoch rollover",
                "replica_slot",
                "must be greater than zero",
            ));
        }
        if usize::try_from(self.finalized_checkpoint_count).ok()
            != Some(SORA_SERVICE_LEASE_MAX_EGRESS_REPORTER_CHECKPOINTS_V1)
        {
            return Err(invalid_field(
                "sora service lease reporting epoch rollover",
                "finalized_checkpoint_count",
                "must equal the reporting-epoch checkpoint limit",
            ));
        }
        if self.forced_finalized_checkpoint_count > self.finalized_checkpoint_count {
            return Err(invalid_field(
                "sora service lease reporting epoch rollover",
                "forced_finalized_checkpoint_count",
                "must not exceed finalized_checkpoint_count",
            ));
        }
        if self
            .settled_egress_bytes
            .checked_sub(self.settled_egress_bytes_delta)
            .is_none()
        {
            return Err(invalid_field(
                "sora service lease reporting epoch rollover",
                "settled_egress_bytes",
                "must be greater than or equal to settled_egress_bytes_delta",
            ));
        }
        Ok(())
    }
}
/// Authoritative lease and accounting state for a hosted HTTP service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceLeaseStateV1 {
    /// Schema version; must equal [`SORA_SERVICE_LEASE_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Current fail-closed status.
    pub status: SoraServiceLeaseStatusV1,
    /// Scheduler/quota class assigned to the service.
    pub quota_class: String,
    /// Anti-spam deployment deposit locked for the service.
    pub deployment_deposit: Quantity,
    /// Prepaid runtime balance available to the service.
    pub prepaid_runtime_balance: Quantity,
    /// Runtime charge applied per active block.
    pub runtime_price_per_block: Quantity,
    /// Storage charge applied per GiB and active block.
    pub storage_price_per_gib_block: Quantity,
    /// Egress charge applied per MiB when usage is reported.
    pub egress_price_per_mib: Quantity,
    /// Consensus block height when the lease became active.
    pub lease_started_height: u64,
    /// Consensus block height at which the lease must fail closed.
    pub lease_expires_height: u64,
    /// Monotonic reporting epoch, independent of the economic lease clock.
    pub reporting_epoch: u64,
    /// Exact bytes settled from all finalized prior reporting epochs.
    pub settled_egress_bytes: u128,
    /// Canonically sorted reporter checkpoints keyed by reporting epoch,
    /// revision, slot, and validator. Every retained checkpoint belongs to the
    /// current reporting epoch.
    pub egress_reporter_checkpoints: Vec<SoraServiceLeaseEgressCheckpointV1>,
    /// Cached exact sum of settled bytes and all current-epoch checkpoints.
    pub accounted_egress_bytes: u128,
    /// Human-readable reason when the lease is not active.
    #[norito(required)]
    pub last_status_reason: Option<String>,
}
impl SoraServiceLeaseStateV1 {
    /// Recompute the deterministic exact aggregate egress total.
    #[must_use]
    pub fn recomputed_accounted_egress_bytes(&self) -> Option<u128> {
        self.egress_reporter_checkpoints
            .iter()
            .try_fold(self.settled_egress_bytes, |total, checkpoint| {
                total.checked_add(u128::from(checkpoint.accounted_egress_bytes))
            })
    }

    /// Refresh the cached aggregate egress total after a checkpoint mutation.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] if the exact `u128` aggregate overflows.
    pub fn refresh_accounted_egress_bytes(&mut self) -> Result<(), SoracloudManifestError> {
        self.accounted_egress_bytes =
            self.recomputed_accounted_egress_bytes().ok_or_else(|| {
                invalid_field(
                    "sora service lease state",
                    "accounted_egress_bytes",
                    "settled and current-epoch egress total overflows u128",
                )
            })?;
        Ok(())
    }

    /// Validate lease-accounting invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when required lifecycle or pricing fields are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service lease state",
            self.schema_version,
            SORA_SERVICE_LEASE_STATE_VERSION_V1,
        )?;
        validate_nonblank_field("sora service lease state", "quota_class", &self.quota_class)?;
        for (field, value) in [
            ("deployment_deposit", &self.deployment_deposit),
            (
                "runtime_price_per_block",
                &self.runtime_price_per_block,
            ),
            (
                "storage_price_per_gib_block",
                &self.storage_price_per_gib_block,
            ),
            ("egress_price_per_mib", &self.egress_price_per_mib),
        ] {
            if value.is_zero() {
                return Err(invalid_field(
                    "sora service lease state",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        for (field, value) in [
            ("lease_started_height", self.lease_started_height),
            ("lease_expires_height", self.lease_expires_height),
            ("reporting_epoch", self.reporting_epoch),
        ] {
            if value == 0 {
                return Err(invalid_field(
                    "sora service lease state",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        if self.lease_expires_height <= self.lease_started_height {
            return Err(invalid_field(
                "sora service lease state",
                "lease_expires_height",
                "must be greater than lease_started_height",
            ));
        }
        if self
            .last_status_reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(invalid_field(
                "sora service lease state",
                "last_status_reason",
                "must not be empty when provided",
            ));
        }
        for checkpoint in &self.egress_reporter_checkpoints {
            if checkpoint.reporting_epoch != self.reporting_epoch {
                return Err(invalid_field(
                    "sora service lease state",
                    "egress_reporter_checkpoints",
                    "checkpoint reporting_epoch must match the active reporting_epoch",
                ));
            }
            checkpoint.assignment.validate()?;
            if checkpoint.last_updated_height == 0 {
                return Err(invalid_field(
                    "sora service lease state",
                    "egress_reporter_checkpoints",
                    "last_updated_height must be greater than zero",
                ));
            }
            if checkpoint.forced_finalization && !checkpoint.finalize_reporter {
                return Err(invalid_field(
                    "sora service lease state",
                    "egress_reporter_checkpoints",
                    "forced_finalization requires a terminal checkpoint",
                ));
            }
        }
        if self.egress_reporter_checkpoints.len()
            > SORA_SERVICE_LEASE_MAX_EGRESS_REPORTER_CHECKPOINTS_V1
        {
            return Err(invalid_field(
                "sora service lease state",
                "egress_reporter_checkpoints",
                "exceeds the protocol reporter checkpoint limit",
            ));
        }
        if self
            .egress_reporter_checkpoints
            .windows(2)
            .any(|checkpoints| {
                let left = (
                    checkpoints[0].reporting_epoch,
                    checkpoints[0].assignment.service_version.as_str(),
                    checkpoints[0].assignment.placement.replica_slot,
                    &checkpoints[0].assignment.placement.validator_account_id,
                );
                let right = (
                    checkpoints[1].reporting_epoch,
                    checkpoints[1].assignment.service_version.as_str(),
                    checkpoints[1].assignment.placement.replica_slot,
                    &checkpoints[1].assignment.placement.validator_account_id,
                );
                left >= right
            })
        {
            return Err(invalid_field(
                "sora service lease state",
                "egress_reporter_checkpoints",
                "must be strictly sorted by reporting epoch, revision, slot, and validator",
            ));
        }
        let recomputed_accounted_egress_bytes =
            self.recomputed_accounted_egress_bytes().ok_or_else(|| {
                invalid_field(
                    "sora service lease state",
                    "accounted_egress_bytes",
                    "settled and current-epoch egress total overflows u128",
                )
            })?;
        if self.accounted_egress_bytes != recomputed_accounted_egress_bytes {
            return Err(invalid_field(
                "sora service lease state",
                "accounted_egress_bytes",
                "must equal the exact settled and current-epoch checkpoint sum",
            ));
        }
        Ok(())
    }
    /// Estimated accounting blocks elapsed under the current lease.
    #[must_use]
    pub fn billed_blocks_at(&self, current_height: u64) -> u64 {
        current_height
            .min(self.lease_expires_height)
            .saturating_sub(self.lease_started_height)
    }
    /// Estimated remaining nominal prepaid balance at the observed height.
    ///
    /// # Errors
    /// Returns a bounded-domain error if an exact accounting intermediate is unrepresentable.
    pub fn remaining_balance(
        &self,
        current_height: u64,
        accounted_storage_bytes: u64,
    ) -> Result<Quantity, NumericOperationError> {
        let billed_blocks = self.billed_blocks_at(current_height);
        let runtime_cost = self
            .runtime_price_per_block
            .try_mul_decimal(&Numeric::from(billed_blocks))?;
        let storage_gib =
            u128::from(accounted_storage_bytes).div_ceil(u128::from(SORA_STORAGE_BYTES_PER_GIB));
        let storage_units = u128::from(billed_blocks) * storage_gib;
        let storage_cost = self
            .storage_price_per_gib_block
            .try_mul_decimal(&Numeric::new(storage_units, 0))?;
        let egress_mib = u128::from(self.accounted_egress_bytes)
            .div_ceil(u128::from(SORA_NETWORK_BYTES_PER_MIB));
        let egress_cost = self
            .egress_price_per_mib
            .try_mul_decimal(&Numeric::new(egress_mib, 0))?;
        let total_cost = runtime_cost
            .checked_add(&storage_cost)?
            .checked_add(&egress_cost)?;
        if total_cost >= self.prepaid_runtime_balance {
            Ok(Quantity::zero())
        } else {
            self.prepaid_runtime_balance.checked_sub(&total_cost)
        }
    }
    /// Effective lease status at the observed height.
    ///
    /// # Errors
    /// Returns a bounded-domain accounting error.
    pub fn status_at(
        &self,
        current_height: u64,
        accounted_storage_bytes: u64,
    ) -> Result<SoraServiceLeaseStatusV1, NumericOperationError> {
        if self.status == SoraServiceLeaseStatusV1::Suspended {
            return Ok(SoraServiceLeaseStatusV1::Suspended);
        }
        if current_height >= self.lease_expires_height {
            return Ok(SoraServiceLeaseStatusV1::Expired);
        }
        if self
            .remaining_balance(current_height, accounted_storage_bytes)?
            .is_zero()
        {
            return Ok(SoraServiceLeaseStatusV1::Exhausted);
        }
        Ok(self.status)
    }
    /// Returns `true` when the lease is still active at the observed height.
    ///
    /// # Errors
    /// Returns a bounded-domain accounting error.
    pub fn is_active_at(
        &self,
        current_height: u64,
        accounted_storage_bytes: u64,
    ) -> Result<bool, NumericOperationError> {
        Ok(self.status_at(current_height, accounted_storage_bytes)?
            == SoraServiceLeaseStatusV1::Active)
    }
}
/// Derive the domain-separated commitment to a complete hosted-service lease state.
#[must_use]
pub fn derive_soracloud_service_lease_commitment_v1(
    lease: &SoraServiceLeaseStateV1,
) -> Hash {
    let mut transcript = "soracloud:service-lease-state:v1".encode();
    transcript.extend(lease.encode());
    Hash::new(transcript)
}
/// Authoritative leased-volume state recorded by the hosting control plane.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceLeaseVolumeStateV1 {
    /// Schema version; must equal [`SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Logical volume identifier.
    pub volume_name: Name,
    /// Lease-backed volume kind.
    pub kind: SoraLeaseVolumeKindV1,
    /// Storage class requested by the service manifest.
    pub storage_class: StorageClass,
    /// Declared in-runtime mount path.
    pub mount_path: String,
    /// Maximum logical bytes retained for this volume.
    pub max_total_bytes: u64,
    /// Consensus height when the binding lease became active.
    pub lease_started_height: u64,
    /// Consensus height when the binding lease expires.
    pub lease_expires_height: u64,
    /// Monotonic platform-side generation for the authoritative binding.
    pub authoritative_generation: u64,
    /// Latest sequence that materialized this binding on a host, when known.
    #[norito(required)]
    pub last_materialized_sequence: Option<u64>,
}
impl SoraServiceLeaseVolumeStateV1 {
    /// Validate authoritative leased-volume metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when lifecycle or mount invariants are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service lease volume state",
            self.schema_version,
            SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora service lease volume state",
            "mount_path",
            &self.mount_path,
        )?;
        if !self.mount_path.starts_with('/') {
            return Err(invalid_field(
                "sora service lease volume state",
                "mount_path",
                "must start with '/'",
            ));
        }
        for (field, value) in [
            ("max_total_bytes", self.max_total_bytes),
            ("lease_started_height", self.lease_started_height),
            ("lease_expires_height", self.lease_expires_height),
            ("authoritative_generation", self.authoritative_generation),
        ] {
            if value == 0 {
                return Err(invalid_field(
                    "sora service lease volume state",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        if self.lease_expires_height <= self.lease_started_height {
            return Err(invalid_field(
                "sora service lease volume state",
                "lease_expires_height",
                "must be greater than lease_started_height",
            ));
        }
        if self
            .last_materialized_sequence
            .is_some_and(|sequence| sequence == 0)
        {
            return Err(invalid_field(
                "sora service lease volume state",
                "last_materialized_sequence",
                "must be greater than zero when provided",
            ));
        }
        Ok(())
    }
    /// Returns `true` when the volume lease is still active.
    #[must_use]
    pub fn is_active_at(&self, current_height: u64) -> bool {
        current_height < self.lease_expires_height
    }
}
/// State namespace addressed by a service binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "scope", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraStateScopeV1 {
    /// Account metadata namespace.
    AccountMetadata,
    /// Domain metadata namespace.
    DomainMetadata,
    /// Trigger-scoped state namespace.
    TriggerState,
    /// Service-local runtime namespace.
    #[default]
    ServiceState,
    /// Confidential namespace for sensitive records.
    ConfidentialState,
}
/// Mutation mode allowed by the binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "mutability", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraStateMutabilityV1 {
    /// Binding is read-only.
    #[default]
    ReadOnly,
    /// Binding supports append-only writes.
    AppendOnly,
    /// Binding supports full read/write updates.
    ReadWrite,
}
/// Encryption policy expected for values in the binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "encryption", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraStateEncryptionV1 {
    /// Values are stored in plaintext.
    #[default]
    Plaintext,
    /// Values are client-encrypted before submission.
    ClientCiphertext,
    /// Values are FHE ciphertexts.
    FheCiphertext,
}
/// Deterministic state binding contract for an SCR service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraStateBindingV1 {
    /// Schema version; must equal [`SORA_STATE_BINDING_VERSION_V1`].
    pub schema_version: u16,
    /// Human-readable binding identifier.
    pub binding_name: Name,
    /// Canonical namespace targeted by this binding.
    pub scope: SoraStateScopeV1,
    /// Mutation class allowed by the binding.
    pub mutability: SoraStateMutabilityV1,
    /// Encryption policy for values under this binding.
    pub encryption: SoraStateEncryptionV1,
    /// Prefix that scopes all allowed keys.
    pub key_prefix: String,
    /// Maximum bytes per item written through this binding.
    pub max_item_bytes: NonZeroU64,
    /// Maximum cumulative bytes for the binding namespace.
    pub max_total_bytes: NonZeroU64,
}
impl SoraStateBindingV1 {
    /// Validate schema version and namespace limits.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// binding fields violate deterministic constraints.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora state binding",
            self.schema_version,
            SORA_STATE_BINDING_VERSION_V1,
        )?;
        validate_nonblank_field("sora state binding", "key_prefix", &self.key_prefix)?;
        if !self.key_prefix.starts_with('/') {
            return Err(invalid_field(
                "sora state binding",
                "key_prefix",
                "must start with '/'",
            ));
        }
        if self.max_item_bytes > self.max_total_bytes {
            return Err(invalid_field(
                "sora state binding",
                "max_item_bytes",
                "cannot exceed max_total_bytes",
            ));
        }
        if self.scope == SoraStateScopeV1::ConfidentialState
            && self.encryption == SoraStateEncryptionV1::Plaintext
        {
            return Err(invalid_field(
                "sora state binding",
                "encryption",
                "confidential state requires ciphertext encryption",
            ));
        }
        Ok(())
    }
}
/// Handler class for Soracloud runtime entrypoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "class", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceHandlerClassV1 {
    /// Certified static-asset serving.
    Asset,
    /// Certified local query execution.
    #[default]
    Query,
    /// Ordered replicated state mutation.
    Update,
    /// Ordered replicated private/ciphertext/secret mutation.
    PrivateUpdate,
}
/// Certification mode attached to local fast-path responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "certified_response", content = "value")
)]
#[norito(deny_unknown_fields)]
pub enum SoraCertifiedResponsePolicyV1 {
    /// No certification is attached.
    #[default]
    None,
    /// Response is bound to a committed state/root snapshot.
    StateCommitment,
    /// Response is bound to an execution/audit receipt.
    AuditReceipt,
}
/// Artifact category referenced by a service revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "artifact_kind", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraArtifactKindV1 {
    /// Executable service bundle.
    Bundle,
    /// Static asset served by an asset handler.
    #[default]
    StaticAsset,
    /// Durable execution journal.
    Journal,
    /// Durable checkpoint/snapshot.
    Checkpoint,
    /// Model artifact metadata bundle.
    ModelArtifact,
    /// Model-weight binary bundle.
    ModelWeights,
}
/// Content-addressed artifact reference attached to a service revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraArtifactRefV1 {
    /// Artifact class referenced by the service revision.
    pub kind: SoraArtifactKindV1,
    /// Content-addressed artifact digest.
    pub artifact_hash: Hash,
    /// Canonical bundle-relative or service-relative path for the artifact.
    pub artifact_path: String,
    /// Optional handler that consumes or serves the artifact.
    #[norito(required)]
    pub handler_name: Option<Name>,
}
impl SoraArtifactRefV1 {
    /// Validate deterministic artifact-reference constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when path fields are empty or contain control characters.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash("sora artifact ref", "artifact_hash", self.artifact_hash)?;
        validate_nonblank_field("sora artifact ref", "artifact_path", &self.artifact_path)?;
        if self.artifact_path.chars().any(char::is_control) {
            return Err(invalid_field(
                "sora artifact ref",
                "artifact_path",
                "must not contain control characters",
            ));
        }
        Ok(())
    }
}
/// Ordered mailbox contract attached to replicated service handlers.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraMailboxContractV1 {
    /// Stable logical queue name.
    pub queue_name: Name,
    /// Maximum pending messages retained for the queue.
    pub max_pending_messages: NonZeroU32,
    /// Maximum payload size per message.
    pub max_message_bytes: NonZeroU64,
    /// Retention bound in consensus blocks.
    pub retention_blocks: NonZeroU32,
}
impl SoraMailboxContractV1 {
    /// Validate deterministic mailbox-contract constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the mailbox limits are internally inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.max_message_bytes.get() < 16 {
            return Err(invalid_field(
                "sora mailbox contract",
                "max_message_bytes",
                "must be at least 16 bytes",
            ));
        }
        Ok(())
    }
}
/// Runtime handler definition exposed by a service revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceHandlerV1 {
    /// Stable logical handler identifier.
    pub handler_name: Name,
    /// Execution class for the handler.
    pub class: SoraServiceHandlerClassV1,
    /// Entrypoint symbol/function for this handler.
    pub entrypoint: String,
    /// Optional path suffix relative to the service route prefix.
    #[norito(required)]
    pub route_path: Option<String>,
    /// Certification mode for responses emitted by this handler.
    pub certified_response: SoraCertifiedResponsePolicyV1,
    /// Ordered mailbox contract for replicated handlers.
    #[norito(required)]
    pub mailbox: Option<SoraMailboxContractV1>,
}
impl SoraServiceHandlerV1 {
    /// Validate handler classification and deterministic routing rules.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when entrypoint/routing fields are
    /// invalid or handler-class invariants are violated.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field("sora service handler", "entrypoint", &self.entrypoint)?;
        if let Some(route_path) = self.route_path.as_ref() {
            if route_path.trim().is_empty() {
                return Err(invalid_field(
                    "sora service handler",
                    "route_path",
                    "must not be empty when provided",
                ));
            }
            if !route_path.starts_with('/') {
                return Err(invalid_field(
                    "sora service handler",
                    "route_path",
                    "must start with '/'",
                ));
            }
        }
        match self.class {
            SoraServiceHandlerClassV1::Asset | SoraServiceHandlerClassV1::Query => {
                if self.certified_response == SoraCertifiedResponsePolicyV1::None {
                    return Err(invalid_field(
                        "sora service handler",
                        "certified_response",
                        "asset/query handlers must be certified",
                    ));
                }
                if self.mailbox.is_some() {
                    return Err(invalid_field(
                        "sora service handler",
                        "mailbox",
                        "asset/query handlers must not declare a mailbox",
                    ));
                }
            }
            SoraServiceHandlerClassV1::Update | SoraServiceHandlerClassV1::PrivateUpdate => {
                if self.certified_response != SoraCertifiedResponsePolicyV1::None {
                    return Err(invalid_field(
                        "sora service handler",
                        "certified_response",
                        "update/private_update handlers must execute through the mailbox path",
                    ));
                }
                let Some(mailbox) = self.mailbox.as_ref() else {
                    return Err(invalid_field(
                        "sora service handler",
                        "mailbox",
                        "update/private_update handlers require a mailbox contract",
                    ));
                };
                mailbox.validate()?;
            }
        }
        Ok(())
    }
}
/// Canonical deployment manifest for a routable `Soracloud` service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceManifestV1 {
    /// Schema version; must equal [`SORA_SERVICE_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Logical service identifier.
    pub service_name: Name,
    /// Human-readable service version label.
    pub service_version: String,
    /// Execution plane selected by this service revision.
    pub execution_plane: SoraServiceExecutionPlaneV1,
    /// Reference to the executable container manifest.
    pub container: SoraContainerManifestRefV1,
    /// Desired replica count.
    pub replicas: NonZeroU16,
    /// Optional route exposure metadata.
    #[norito(required)]
    pub route: Option<SoraRouteTargetV1>,
    /// Rollout and rollback policy.
    pub rollout: SoraRolloutPolicyV1,
    /// Hosted-service economics used for prepaid open deployment.
    pub economics: SoraHttpServiceEconomicsV1,
    /// State bindings exposed to the service.
    pub state_bindings: Vec<SoraStateBindingV1>,
    /// Lease-backed mutable storage volumes exposed to hosted HTTP services.
    pub lease_volumes: Vec<SoraLeaseVolumeBindingV1>,
    /// Runtime handler contracts exposed by the revision.
    pub handlers: Vec<SoraServiceHandlerV1>,
    /// Content-addressed artifacts referenced by the revision.
    pub artifacts: Vec<SoraArtifactRefV1>,
}
impl SoraServiceManifestV1 {
    /// Validate schema version, routing constraints, and binding invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, route
    /// fields are invalid, or binding constraints fail.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service manifest",
            self.schema_version,
            SORA_SERVICE_MANIFEST_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora service manifest",
            "service_version",
            &self.service_version,
        )?;
        validate_soracloud_digest_hash(
            "sora service manifest",
            "container.manifest_hash",
            self.container.manifest_hash,
        )?;
        if self.container.expected_schema_version != SORA_CONTAINER_MANIFEST_VERSION_V1 {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora service manifest",
                field: "container.expected_schema_version",
                reason: format!(
                    "must equal {SORA_CONTAINER_MANIFEST_VERSION_V1}, found {}",
                    self.container.expected_schema_version
                ),
            });
        }
        if self.rollout.canary_percent > 100 {
            return Err(invalid_field(
                "sora service manifest",
                "rollout.canary_percent",
                "must be within 0..=100",
            ));
        }
        self.validate_route()?;
        self.economics.validate()?;
        let mut seen = BTreeSet::new();
        for binding in &self.state_bindings {
            binding.validate()?;
            if !seen.insert(binding.binding_name.clone()) {
                return Err(SoracloudManifestError::DuplicateStateBinding {
                    binding: binding.binding_name.clone(),
                });
            }
        }
        self.validate_lease_volumes()?;
        if self.execution_plane == SoraServiceExecutionPlaneV1::DeterministicService
            && self.handlers.is_empty()
        {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "sora service manifest",
                field: "handlers",
            });
        }
        let handler_names = self.validate_handlers()?;
        self.validate_artifacts(&handler_names)?;
        self.validate_execution_plane_requirements()?;
        Ok(())
    }
    fn validate_route(&self) -> Result<(), SoracloudManifestError> {
        if let Some(route) = self.route.as_ref() {
            validate_nonblank_field("sora service manifest", "route.host", &route.host)?;
            if !route.path_prefix.starts_with('/') {
                return Err(invalid_field(
                    "sora service manifest",
                    "route.path_prefix",
                    "must start with '/'",
                ));
            }
        }
        Ok(())
    }
    fn validate_lease_volumes(&self) -> Result<(), SoracloudManifestError> {
        let mut seen_lease_volumes = BTreeSet::new();
        for volume in &self.lease_volumes {
            volume.validate()?;
            if !seen_lease_volumes.insert(volume.volume_name.clone()) {
                return Err(SoracloudManifestError::DuplicateLeaseVolume {
                    volume: volume.volume_name.clone(),
                });
            }
        }
        Ok(())
    }
    fn validate_handlers(&self) -> Result<BTreeSet<Name>, SoracloudManifestError> {
        let mut handler_names = BTreeSet::new();
        for handler in &self.handlers {
            handler.validate()?;
            if !handler_names.insert(handler.handler_name.clone()) {
                return Err(SoracloudManifestError::DuplicateHandler {
                    handler: handler.handler_name.clone(),
                });
            }
        }
        Ok(handler_names)
    }
    fn validate_artifacts(
        &self,
        handler_names: &BTreeSet<Name>,
    ) -> Result<(), SoracloudManifestError> {
        for artifact in &self.artifacts {
            artifact.validate()?;
            if let Some(handler_name) = artifact.handler_name.as_ref()
                && !handler_names.contains(handler_name)
            {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora service manifest",
                    field: "artifacts.handler_name",
                    reason: format!("artifact references unknown handler `{handler_name}`"),
                });
            }
        }
        Ok(())
    }
    fn validate_execution_plane_requirements(&self) -> Result<(), SoracloudManifestError> {
        match self.execution_plane {
            SoraServiceExecutionPlaneV1::DeterministicService => {
                if !self.lease_volumes.is_empty() {
                    return Err(invalid_field(
                        "sora service manifest",
                        "lease_volumes",
                        "deterministic services must not declare lease-backed HTTP service volumes",
                    ));
                }
            }
            SoraServiceExecutionPlaneV1::HttpService => {
                if self.route.is_none() {
                    return Err(invalid_field(
                        "sora service manifest",
                        "route",
                        "http services must declare a public or internal route",
                    ));
                }
                if !self.state_bindings.is_empty() {
                    return Err(invalid_field(
                        "sora service manifest",
                        "state_bindings",
                        "http services must use lease-backed storage instead of deterministic state bindings",
                    ));
                }
                if !self.handlers.is_empty() {
                    return Err(invalid_field(
                        "sora service manifest",
                        "handlers",
                        "http services must not declare deterministic handler contracts",
                    ));
                }
                let minimum_prepaid = self.minimum_hosted_runtime_prepaid().map_err(|error| {
                    SoracloudManifestError::InvalidField {
                        manifest: "sora service manifest",
                        field: "economics.prepaid_runtime_balance",
                        reason: format!("minimum prepaid calculation failed: {error}"),
                    }
                })?;
                if self.economics.prepaid_runtime_balance < minimum_prepaid {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "sora service manifest",
                        field: "economics.prepaid_runtime_balance",
                        reason: format!(
                            "must cover at least one billed runtime+storage sequence ({minimum_prepaid})"
                        ),
                    });
                }
            }
        }
        Ok(())
    }
    fn minimum_hosted_runtime_prepaid(&self) -> Result<Quantity, NumericOperationError> {
        let storage_bytes = self
            .lease_volumes
            .iter()
            .map(|volume| u128::from(volume.max_total_bytes.get()))
            .sum::<u128>();
        let storage_gib = if storage_bytes == 0 {
            0
        } else {
            storage_bytes.div_ceil(u128::from(SORA_STORAGE_BYTES_PER_GIB))
        };
        let storage_cost = self
            .economics
            .storage_price_per_gib_block
            .try_mul_decimal(&Numeric::new(storage_gib, 0))?;
        self.economics
            .runtime_price_per_block
            .checked_add(&storage_cost)
    }
}
/// Upgrade mode for long-lived AI agent apartments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "upgrade_policy", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum AgentUpgradePolicyV1 {
    /// Apartments can only be upgraded through explicit governance actions.
    #[default]
    Governed,
    /// Apartments can be upgraded automatically once checks pass.
    Automatic,
    /// Apartment revision is pinned and cannot be upgraded.
    Pinned,
}
/// Tool-level execution cap for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AgentToolCapabilityV1 {
    /// Stable tool identifier.
    pub tool: String,
    /// Maximum invocations allowed per accounting epoch.
    pub max_invocations_per_epoch: NonZeroU32,
    /// Whether the tool may perform network egress.
    pub allow_network: bool,
    /// Whether the tool may write to local persistent files.
    pub allow_filesystem_write: bool,
}
/// Spend guardrail for a specific asset under apartment policy.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AgentSpendLimitV1 {
    /// Asset definition identifier (for example `61CtjvNd9T3THAR65GsMVHr82Bjc`).
    pub asset_definition: String,
    /// Maximum nominal amount spendable per transaction.
    pub max_per_tx: Quantity,
    /// Maximum nominal amount spendable per day.
    pub max_per_day: Quantity,
}
/// Deterministic policy manifest for a persistent AI agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AgentApartmentManifestV1 {
    /// Schema version; must equal [`AGENT_APARTMENT_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Logical apartment identifier.
    pub apartment_name: Name,
    /// Reference to the executable container manifest.
    pub container: SoraContainerManifestRefV1,
    /// Tool-level capability policy.
    pub tool_capabilities: Vec<AgentToolCapabilityV1>,
    /// Additional high-level policy capability identifiers.
    pub policy_capabilities: Vec<Name>,
    /// Wallet spend limits across allowed assets.
    pub spend_limits: Vec<AgentSpendLimitV1>,
    /// Total state quota reserved for apartment memory.
    pub state_quota_bytes: NonZeroU64,
    /// Apartment-wide network egress policy.
    pub network_egress: SoraNetworkPolicyV1,
    /// Upgrade policy for apartment revisions.
    pub upgrade_policy: AgentUpgradePolicyV1,
}
impl AgentApartmentManifestV1 {
    /// Compute the canonical hash of the apartment manifest.
    #[must_use]
    pub fn manifest_hash(&self) -> Hash {
        Hash::new(Encode::encode(self))
    }
    /// Validate schema version and deterministic policy constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// policy fields violate deterministic constraints.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "agent apartment manifest",
            self.schema_version,
            AGENT_APARTMENT_MANIFEST_VERSION_V1,
        )?;
        validate_soracloud_digest_hash(
            "agent apartment manifest",
            "container.manifest_hash",
            self.container.manifest_hash,
        )?;
        if self.container.expected_schema_version != SORA_CONTAINER_MANIFEST_VERSION_V1 {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "agent apartment manifest",
                field: "container.expected_schema_version",
                reason: format!(
                    "must equal {SORA_CONTAINER_MANIFEST_VERSION_V1}, found {}",
                    self.container.expected_schema_version
                ),
            });
        }
        let mut seen_tools = BTreeSet::new();
        for tool_capability in &self.tool_capabilities {
            let tool = tool_capability.tool.trim();
            validate_nonblank_field("agent apartment manifest", "tool_capabilities.tool", tool)?;
            if !seen_tools.insert(tool.to_owned()) {
                return Err(SoracloudManifestError::DuplicateToolCapability {
                    tool: tool.to_owned(),
                });
            }
        }
        let mut seen_policies = BTreeSet::new();
        for policy in &self.policy_capabilities {
            if !seen_policies.insert(policy.clone()) {
                return Err(SoracloudManifestError::DuplicatePolicyCapability {
                    policy: policy.clone(),
                });
            }
        }
        let mut seen_spend_assets = BTreeSet::new();
        for limit in &self.spend_limits {
            let asset = limit.asset_definition.trim();
            validate_nonblank_field(
                "agent apartment manifest",
                "spend_limits.asset_definition",
                asset,
            )?;
            if limit.max_per_tx.is_zero() || limit.max_per_day.is_zero() {
                return Err(invalid_field(
                    "agent apartment manifest",
                    "spend_limits",
                    "spend limits must be greater than zero",
                ));
            }
            if limit.max_per_tx > limit.max_per_day {
                return Err(invalid_field(
                    "agent apartment manifest",
                    "spend_limits.max_per_tx",
                    "cannot exceed max_per_day",
                ));
            }
            if !seen_spend_assets.insert(asset.to_owned()) {
                return Err(SoracloudManifestError::DuplicateSpendLimitAsset {
                    asset: asset.to_owned(),
                });
            }
        }
        if let SoraNetworkPolicyV1::Allowlist(entries) = &self.network_egress {
            if entries.is_empty() {
                return Err(invalid_field(
                    "agent apartment manifest",
                    "network_egress",
                    "allowlist must include at least one host",
                ));
            }
            let mut seen_hosts = BTreeSet::new();
            for entry in entries {
                let normalized = entry.host.trim();
                if normalized.is_empty() {
                    return Err(invalid_field(
                        "agent apartment manifest",
                        "network_egress",
                        "allowlist host entries must be non-empty",
                    ));
                }
                if normalized.chars().any(char::is_control)
                    || normalized.chars().any(char::is_whitespace)
                {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "agent apartment manifest",
                        field: "network_egress",
                        reason: format!(
                            "allowlist host `{normalized}` contains invalid characters"
                        ),
                    });
                }
                if !seen_hosts.insert(normalized.to_ascii_lowercase()) {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "agent apartment manifest",
                        field: "network_egress",
                        reason: format!("duplicate allowlist host `{normalized}`"),
                    });
                }
                if entry.ports.is_empty() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "agent apartment manifest",
                        field: "network_egress",
                        reason: format!(
                            "allowlist host `{normalized}` must include at least one port"
                        ),
                    });
                }
                let mut seen_ports = BTreeSet::new();
                for port in &entry.ports {
                    if *port == 0 {
                        return Err(SoracloudManifestError::InvalidField {
                            manifest: "agent apartment manifest",
                            field: "network_egress",
                            reason: format!(
                                "allowlist host `{normalized}` contains invalid port `0`"
                            ),
                        });
                    }
                    if !seen_ports.insert(*port) {
                        return Err(SoracloudManifestError::InvalidField {
                            manifest: "agent apartment manifest",
                            field: "network_egress",
                            reason: format!(
                                "allowlist host `{normalized}` contains duplicate port `{port}`"
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
