//! Public-only canonical first-device hardware release preparation.
//!
//! All originals arrive on inherited regular-file descriptors. This command neither signs nor
//! installs anything. A successful release output proves only its actual threshold signatures;
//! it does not grant financial authority, device readiness, or an installed bootstrap root.
#[cfg(unix)]
fn main() {
    preparation::main();
}
#[cfg(not(unix))]
fn main() {
    eprintln!(
        "{{\"schema\":\"bpng.hardware-evidence-native-error.v1\",\"code\":\"descriptor_platform_rejected\"}}"
    );
    std::process::exit(2);
}
#[cfg(unix)]
mod preparation {
    use iroha_crypto::{Algorithm, PublicKey, Signature};
    use iroha_data_model::kagemusha::{
        KagemushaAppKeySecurityLevelV1, KagemushaHardwareBootstrapReleaseApprovalV1,
        KagemushaHardwareEvidenceBootstrapManifestV1,
        KagemushaHardwareEvidenceCompiledBindingOriginalV1, KagemushaHardwareEvidenceJniArtifactV1,
        KagemushaPlayIntegrityPolicyV1, KagemushaReleaseAuthorityPolicyV1,
        KagemushaSignedHardwareBootstrapReleaseV1, hardware_bootstrap_decode_v1,
        hardware_bootstrap_encode_v1,
    };
    use norito::json::{Map, Value};
    use sha2::{Digest as _, Sha256};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{self, File, OpenOptions},
        io::Write as _,
        os::unix::fs::{DirBuilderExt as _, FileExt as _, MetadataExt as _, OpenOptionsExt as _},
        path::Path,
    };

    type Result<T> = std::result::Result<T, &'static str>;
    const MAX_ORIGINAL: usize = 192 * 1024;
    const MAX_REQUEST: usize = 64 * 1024;

    pub(super) fn main() {
        match run(&std::env::args_os().collect::<Vec<_>>()) {
            Ok(receipt) => print!("{}", receipt),
            Err(code) => {
                // Closed error codes contain no caller values, paths, signatures, or JSON payloads.
                eprintln!(
                    "{{\"schema\":\"bpng.hardware-evidence-native-error.v1\",\"code\":\"{code}\"}}"
                );
                std::process::exit(2);
            }
        }
    }

    fn run(args: &[std::ffi::OsString]) -> Result<String> {
        if args.len() != 5
            || args[1] != "--input-fd"
            || args[2] != "3"
            || args[3] != "--output-directory"
        {
            return Err("arguments_rejected");
        }
        let output = Path::new(&args[4]);
        require_new_directory(output)?;
        let request_bytes = read_fd(3, None, MAX_REQUEST)?;
        let request =
            norito::json::from_slice_value(&request_bytes).map_err(|_| "request_json_rejected")?;
        let input = parse_request(&request)?;
        // Complete descriptor intake and all typed authentication precede output creation.
        let mut originals = BTreeMap::new();
        for (role, pin) in &input.originals {
            originals.insert(role.clone(), read_fd(pin.fd, Some(pin), MAX_ORIGINAL)?);
        }
        let preparation = prepare(&input.duty, &originals)?;
        emit(output, &input.duty, preparation)
    }

    #[derive(Clone, Copy)]
    struct Pin {
        fd: i32,
        size: u64,
        sha: [u8; 32],
    }
    struct Input {
        duty: String,
        originals: BTreeMap<String, Pin>,
    }
    struct Preparation {
        authority_digest: [u8; 32],
        manifest_digest: Option<[u8; 32]>,
        outputs: BTreeMap<String, Vec<u8>>,
    }

    fn parse_request(value: &Value) -> Result<Input> {
        let m = fields(value, &["schema", "duty", "originals"])?;
        if text(m, "schema")? != "bpng.hardware-evidence-native-input.v1" {
            return Err("request_schema_rejected");
        }
        let duty = text(m, "duty")?.to_owned();
        let roles = m["originals"]
            .as_object()
            .ok_or("original_roles_rejected")?;
        validate_roles(&duty, roles.keys().map(String::as_str))?;
        let mut originals = BTreeMap::new();
        let mut descriptors = BTreeSet::new();
        for (role, v) in roles {
            let p = fields(v, &["fd", "sizeBytes", "sha256"])?;
            let fd: i32 = number(p, "fd")?
                .try_into()
                .map_err(|_| "original_fd_rejected")?;
            let size = number(p, "sizeBytes")?;
            if fd < 4 || !descriptors.insert(fd) || size == 0 || size > MAX_ORIGINAL as u64 {
                return Err("original_pin_rejected");
            }
            originals.insert(
                role.clone(),
                Pin {
                    fd,
                    size,
                    sha: digest(p, "sha256")?,
                },
            );
        }
        Ok(Input { duty, originals })
    }

    fn validate_roles<'a>(duty: &str, offered: impl Iterator<Item = &'a str>) -> Result<()> {
        let offered: BTreeSet<_> = offered.collect();
        let fixed: &[&str] = match duty {
            "authority" => &["policy_input"],
            "compiled-binding" => &["authority_policy", "compiled_binding_input"],
            "manifest" => &["authority_policy", "manifest_input"],
            "approval" => &["authority_policy", "manifest", "approval_input"],
            "release" => &["authority_policy", "manifest"],
            _ => return Err("duty_rejected"),
        };
        if duty != "release" {
            if offered != fixed.iter().copied().collect() {
                return Err("original_roles_rejected");
            }
        } else {
            if !(3..=34).contains(&offered.len()) {
                return Err("release_roles_rejected");
            }
            let count = offered.len() - 2;
            let expected: BTreeSet<_> = fixed
                .iter()
                .map(|s| s.to_string())
                .chain((0..count).map(|i| format!("approval_{i:03}")))
                .collect();
            if offered.iter().copied().collect::<BTreeSet<_>>()
                != expected.iter().map(String::as_str).collect()
            {
                return Err("release_roles_rejected");
            }
        }
        Ok(())
    }

    fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
        a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.len() == b.len()
            && a.uid() == b.uid()
            && a.mode() == b.mode()
            && a.nlink() == b.nlink()
            && a.mtime() == b.mtime()
            && a.mtime_nsec() == b.mtime_nsec()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
    }
    fn read_fd(fd: i32, pin: Option<&Pin>, maximum: usize) -> Result<Vec<u8>> {
        // Duplicate only this inherited descriptor through the fixed OS descriptor directory.
        // No original locator is accepted or reopened, and invalid descriptors fail safely.
        // Positional reads preserve the inherited offset; the original remains owned by its
        // process for the full duty. This uses no unsafe raw-descriptor ownership conversion.
        if fd < 3 {
            return Err("descriptor_rejected");
        }
        let file = File::open(format!("/dev/fd/{fd}")).map_err(|_| "descriptor_rejected")?;
        let before = file.metadata().map_err(|_| "descriptor_rejected")?;
        if !before.is_file()
            || before.len() == 0
            || before.len() > maximum as u64
            || pin.is_some_and(|p| p.size != before.len())
        {
            return Err("descriptor_bound_rejected");
        }
        let mut bytes = vec![0u8; before.len() as usize];
        let mut offset = 0;
        while offset < bytes.len() {
            let count = file
                .read_at(&mut bytes[offset..], offset as u64)
                .map_err(|_| "descriptor_read_rejected")?;
            if count == 0 {
                return Err("descriptor_short_read");
            }
            offset += count;
        }
        let mut extra = [0u8; 1];
        if file
            .read_at(&mut extra, bytes.len() as u64)
            .map_err(|_| "descriptor_read_rejected")?
            != 0
            || !same_file(
                &before,
                &file.metadata().map_err(|_| "descriptor_recheck_rejected")?,
            )
            || pin.is_some_and(|p| sha(&bytes) != p.sha)
        {
            return Err("descriptor_original_changed");
        }
        Ok(bytes)
    }

    fn prepare(duty: &str, originals: &BTreeMap<String, Vec<u8>>) -> Result<Preparation> {
        validate_roles(duty, originals.keys().map(String::as_str))?;
        let policy: KagemushaReleaseAuthorityPolicyV1 = if duty == "authority" {
            authority_input(&originals["policy_input"])?
        } else {
            KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(
                &originals["authority_policy"],
            )
            .map_err(|_| "authority_original_rejected")?
        };
        policy.validate().map_err(|_| "authority_policy_rejected")?;
        if policy
            .authorized_signers
            .iter()
            .any(|p| p.algorithm() != Algorithm::Ed25519)
        {
            return Err("authority_algorithm_rejected");
        }
        let authority_digest = policy
            .canonical_digest()
            .map_err(|_| "authority_digest_rejected")?;
        let mut outputs = BTreeMap::new();
        let mut manifest_digest = None;
        match duty {
            "authority" => {
                outputs.insert("authority-policy.norito".into(), policy_original(&policy)?);
            }
            "compiled-binding" => {
                let v = json_original(&originals["compiled_binding_input"])?;
                let m = fields(
                    &v,
                    &[
                        "version",
                        "app_source_sha256",
                        "sdk_source_sha256",
                        "native_abi",
                    ],
                )?;
                let compiled = KagemushaHardwareEvidenceCompiledBindingOriginalV1 {
                    version: number(m, "version")?
                        .try_into()
                        .map_err(|_| "compiled_version_rejected")?,
                    authority_policy_original: originals["authority_policy"].clone(),
                    app_source_sha256: digest(m, "app_source_sha256")?,
                    sdk_source_sha256: digest(m, "sdk_source_sha256")?,
                    native_abi: number(m, "native_abi")?
                        .try_into()
                        .map_err(|_| "compiled_abi_rejected")?,
                };
                compiled
                    .validate()
                    .map_err(|_| "compiled_binding_rejected")?;
                if compiled.native_abi != iroha_data_model::privacy::PRIVACY_BRIDGE_ABI_VERSION_V1 {
                    return Err("compiled_abi_rejected");
                }
                outputs.insert(
                    "hardware-compiled-binding.norito".into(),
                    encode(&compiled)?,
                );
            }
            "manifest" | "approval" | "release" => {
                let manifest = if duty == "manifest" {
                    manifest_input(&originals["manifest_input"])?
                } else {
                    hardware_bootstrap_decode_v1::<KagemushaHardwareEvidenceBootstrapManifestV1>(
                        &originals["manifest"],
                    )
                    .map_err(|_| "manifest_original_rejected")?
                };
                manifest.validate().map_err(|_| "manifest_rejected")?;
                if manifest.native_abi != iroha_data_model::privacy::PRIVACY_BRIDGE_ABI_VERSION_V1 {
                    return Err("manifest_abi_rejected");
                }
                if manifest.authority_policy_digest != authority_digest {
                    return Err("manifest_authority_rejected");
                }
                manifest_digest = Some(manifest.digest().map_err(|_| "manifest_digest_rejected")?);
                match duty {
                    "manifest" => {
                        outputs.insert("manifest.norito".into(), encode(&manifest)?);
                        outputs.insert(
                            "signing-message.bin".into(),
                            manifest
                                .signing_bytes()
                                .map_err(|_| "signing_message_rejected")?,
                        );
                    }
                    "approval" => {
                        let v = json_original(&originals["approval_input"])?;
                        let m = fields(&v, &["public_key", "signature_hex"])?;
                        let public_key = key(m, "public_key")?;
                        let signature =
                            Signature::try_from_bytes(&unhex::<64>(text(m, "signature_hex")?)?)
                                .map_err(|_| "approval_signature_rejected")?;
                        if public_key.algorithm() != Algorithm::Ed25519
                            || policy
                                .authorized_signers
                                .binary_search(&public_key)
                                .is_err()
                        {
                            return Err("approval_signer_rejected");
                        }
                        signature
                            .verify(
                                &public_key,
                                &manifest
                                    .signing_bytes()
                                    .map_err(|_| "signing_message_rejected")?,
                            )
                            .map_err(|_| "approval_verification_rejected")?;
                        outputs.insert(
                            "approval.norito".into(),
                            encode(&KagemushaHardwareBootstrapReleaseApprovalV1 {
                                public_key,
                                signature,
                            })?,
                        );
                    }
                    "release" => {
                        let approvals = (0..originals.len() - 2)
                            .map(|i| {
                                hardware_bootstrap_decode_v1::<
                                    KagemushaHardwareBootstrapReleaseApprovalV1,
                                >(
                                    &originals[&format!("approval_{i:03}")]
                                )
                                .map_err(|_| "approval_original_rejected")
                            })
                            .collect::<Result<Vec<_>>>()?;
                        let release = KagemushaSignedHardwareBootstrapReleaseV1 {
                            manifest,
                            approvals,
                        };
                        release
                            .authenticate(&policy)
                            .map_err(|_| "release_authentication_rejected")?;
                        outputs.insert(
                            "hardware-bootstrap-release.norito".into(),
                            encode(&release)?,
                        );
                    }
                    _ => unreachable!(),
                }
            }
            _ => return Err("duty_rejected"),
        }
        Ok(Preparation {
            authority_digest,
            manifest_digest,
            outputs,
        })
    }

    fn policy_original(policy: &KagemushaReleaseAuthorityPolicyV1) -> Result<Vec<u8>> {
        let bytes = norito::encode_canonical(policy).map_err(|_| "policy_encoding_rejected")?;
        KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&bytes)
            .map_err(|_| "policy_encoding_rejected")?;
        Ok(bytes)
    }
    fn encode<T: norito::NoritoSerialize>(v: &T) -> Result<Vec<u8>> {
        hardware_bootstrap_encode_v1(v).map_err(|_| "original_encoding_rejected")
    }
    fn json_original(bytes: &[u8]) -> Result<Value> {
        if bytes.is_empty() || bytes.len() > MAX_ORIGINAL {
            return Err("json_original_bound_rejected");
        }
        norito::json::from_slice_value(bytes).map_err(|_| "json_original_rejected")
    }
    fn fields<'a>(value: &'a Value, expected: &[&str]) -> Result<&'a Map> {
        let m = value.as_object().ok_or("json_object_rejected")?;
        if m.len() != expected.len() || expected.iter().any(|k| !m.contains_key(*k)) {
            return Err("json_fields_rejected");
        }
        Ok(m)
    }
    fn text<'a>(m: &'a Map, k: &str) -> Result<&'a str> {
        m[k].as_str().ok_or("json_text_rejected")
    }
    fn number(m: &Map, k: &str) -> Result<u64> {
        m[k].as_u64().ok_or("json_integer_rejected")
    }
    fn boolean(m: &Map, k: &str) -> Result<bool> {
        m[k].as_bool().ok_or("json_boolean_rejected")
    }
    fn digest(m: &Map, k: &str) -> Result<[u8; 32]> {
        unhex(text(m, k)?)
    }
    fn key(m: &Map, k: &str) -> Result<PublicKey> {
        text(m, k)?.parse().map_err(|_| "public_key_rejected")
    }
    fn unhex<const N: usize>(s: &str) -> Result<[u8; N]> {
        if s.len() != 2 * N
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("lowercase_hex_rejected");
        }
        let mut out = [0u8; N];
        for (i, c) in s.as_bytes().chunks_exact(2).enumerate() {
            let nibble = |b| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            out[i] = nibble(c[0]) * 16 + nibble(c[1]);
        }
        Ok(out)
    }
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
    fn sha(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn authority_input(bytes: &[u8]) -> Result<KagemushaReleaseAuthorityPolicyV1> {
        let value = json_original(bytes)?;
        let m = fields(
            &value,
            &[
                "version",
                "authority_set_id",
                "threshold",
                "authorized_signers",
            ],
        )?;
        let offered = m["authorized_signers"]
            .as_array()
            .ok_or("authority_signers_rejected")?;
        if offered.is_empty() || offered.len() > 64 {
            return Err("authority_signers_rejected");
        }
        let signers = offered
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or("authority_signer_rejected")?
                    .parse::<PublicKey>()
                    .map_err(|_| "authority_signer_rejected")
            })
            .collect::<Result<Vec<_>>>()?;
        let policy = KagemushaReleaseAuthorityPolicyV1 {
            version: number(m, "version")?
                .try_into()
                .map_err(|_| "authority_version_rejected")?,
            authority_set_id: digest(m, "authority_set_id")?,
            threshold: number(m, "threshold")?
                .try_into()
                .map_err(|_| "authority_threshold_rejected")?,
            authorized_signers: signers,
        };
        policy.validate().map_err(|_| "authority_policy_rejected")?;
        if policy
            .authorized_signers
            .iter()
            .any(|key| key.algorithm() != Algorithm::Ed25519)
        {
            return Err("authority_algorithm_rejected");
        }
        Ok(policy)
    }

    fn manifest_input(bytes: &[u8]) -> Result<KagemushaHardwareEvidenceBootstrapManifestV1> {
        let value = json_original(bytes)?;
        let m = fields(
            &value,
            &[
                "version",
                "purpose",
                "authority_policy_digest",
                "network_id",
                "app_package",
                "app_version_code",
                "core_origin",
                "app_signing_identity_digest",
                "app_distribution_digest",
                "app_source_sha256",
                "app_code_sha256",
                "sdk_source_sha256",
                "jni_artifacts",
                "native_abi",
                "evidence_issuer",
                "raw_verifier_policy_digest",
                "google_oauth_issuer",
                "google_oauth_client_id",
                "google_cloud_project_number",
                "play_integrity_policy",
                "allowed_android_security_levels",
                "native_clock_selection_digest",
                "native_clock_base_urls",
                "policy_epoch",
                "not_before_ms",
                "expires_at_ms",
                "maximum_attempt_lifetime_ms",
            ],
        )?;
        let p = fields(
            &m["play_integrity_policy"],
            &[
                "policy_digest",
                "maximum_evidence_age_ms",
                "maximum_refresh_interval_ms",
                "require_play_recognized",
                "require_licensed",
                "minimum_device_integrity",
            ],
        )?;
        let levels = m["allowed_android_security_levels"]
            .as_array()
            .ok_or("hardware_levels_rejected")?
            .iter()
            .map(|v| match v.as_u64() {
                Some(1) => Ok(KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment),
                Some(2) => Ok(KagemushaAppKeySecurityLevelV1::StrongBox),
                _ => Err("hardware_levels_rejected"),
            })
            .collect::<Result<Vec<_>>>()?;
        let urls = m["native_clock_base_urls"]
            .as_array()
            .ok_or("clock_urls_rejected")?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or("clock_urls_rejected"))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| "clock_urls_rejected")?;
        let artifacts = m["jni_artifacts"]
            .as_array()
            .ok_or("jni_artifacts_rejected")?;
        if !(1..=4).contains(&artifacts.len()) {
            return Err("jni_artifacts_rejected");
        }
        let artifacts = artifacts
            .iter()
            .map(|v| {
                let a = fields(v, &["android_abi", "sha256"])?;
                Ok(KagemushaHardwareEvidenceJniArtifactV1 {
                    android_abi: text(a, "android_abi")?.to_owned(),
                    sha256: digest(a, "sha256")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(KagemushaHardwareEvidenceBootstrapManifestV1 {
            version: number(m, "version")?
                .try_into()
                .map_err(|_| "manifest_version_rejected")?,
            purpose: number(m, "purpose")?
                .try_into()
                .map_err(|_| "manifest_purpose_rejected")?,
            authority_policy_digest: digest(m, "authority_policy_digest")?,
            network_id: digest(m, "network_id")?,
            app_package: text(m, "app_package")?.to_owned(),
            app_version_code: number(m, "app_version_code")?,
            core_origin: text(m, "core_origin")?.to_owned(),
            app_signing_identity_digest: digest(m, "app_signing_identity_digest")?,
            app_distribution_digest: digest(m, "app_distribution_digest")?,
            app_source_sha256: digest(m, "app_source_sha256")?,
            app_code_sha256: digest(m, "app_code_sha256")?,
            sdk_source_sha256: digest(m, "sdk_source_sha256")?,
            jni_artifacts: artifacts,
            native_abi: number(m, "native_abi")?
                .try_into()
                .map_err(|_| "manifest_abi_rejected")?,
            evidence_issuer: key(m, "evidence_issuer")?,
            raw_verifier_policy_digest: digest(m, "raw_verifier_policy_digest")?,
            google_oauth_issuer: text(m, "google_oauth_issuer")?.to_owned(),
            google_oauth_client_id: text(m, "google_oauth_client_id")?.to_owned(),
            google_cloud_project_number: number(m, "google_cloud_project_number")?,
            play_integrity_policy: KagemushaPlayIntegrityPolicyV1 {
                policy_digest: digest(p, "policy_digest")?,
                maximum_evidence_age_ms: number(p, "maximum_evidence_age_ms")?,
                maximum_refresh_interval_ms: number(p, "maximum_refresh_interval_ms")?,
                require_play_recognized: boolean(p, "require_play_recognized")?,
                require_licensed: boolean(p, "require_licensed")?,
                minimum_device_integrity: number(p, "minimum_device_integrity")?
                    .try_into()
                    .map_err(|_| "pi_level_rejected")?,
            },
            allowed_android_security_levels: levels,
            native_clock_selection_digest: digest(m, "native_clock_selection_digest")?,
            native_clock_base_urls: urls,
            policy_epoch: number(m, "policy_epoch")?,
            not_before_ms: number(m, "not_before_ms")?,
            expires_at_ms: number(m, "expires_at_ms")?,
            maximum_attempt_lifetime_ms: number(m, "maximum_attempt_lifetime_ms")?,
        })
    }

    fn require_new_directory(output: &Path) -> Result<()> {
        if !output.is_absolute()
            || output.components().any(|p| {
                matches!(
                    p,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err("output_path_rejected");
        }
        if output.symlink_metadata().is_ok() {
            return Err("output_already_exists");
        }
        let parent = output.parent().ok_or("output_parent_rejected")?;
        if parent
            .canonicalize()
            .map_err(|_| "output_parent_rejected")?
            != parent
        {
            return Err("output_parent_rejected");
        }
        Ok(())
    }
    fn emit(output: &Path, duty: &str, prepared: Preparation) -> Result<String> {
        require_new_directory(output)?;
        let parent = output.parent().ok_or("output_parent_rejected")?;
        let held_parent = File::open(parent).map_err(|_| "output_parent_rejected")?;
        let parent_before = held_parent
            .metadata()
            .map_err(|_| "output_parent_rejected")?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(output)
            .map_err(|_| "output_create_rejected")?;
        let held_directory = File::open(output).map_err(|_| "output_directory_rejected")?;
        let mut entries = Map::new();
        for (name, bytes) in &prepared.outputs {
            write_original(output, name, bytes)?;
            entries.insert(
                name.clone(),
                Value::Object(Map::from([
                    ("sha256".into(), Value::String(hex(&sha(bytes)))),
                    (
                        "sizeBytes".into(),
                        Value::Number((bytes.len() as u64).into()),
                    ),
                ])),
            );
        }
        let receipt = Value::Object(Map::from([
            (
                "schema".into(),
                Value::String("bpng.hardware-evidence-native-preparation.v1".into()),
            ),
            ("duty".into(), Value::String(duty.into())),
            (
                "authorityPolicyDigest".into(),
                Value::String(hex(&prepared.authority_digest)),
            ),
            (
                "manifestDigest".into(),
                prepared
                    .manifest_digest
                    .map(|d| Value::String(hex(&d)))
                    .unwrap_or(Value::Null),
            ),
            ("outputs".into(), Value::Object(entries)),
        ]));
        let receipt = norito::json::to_string(&receipt).map_err(|_| "receipt_encoding_rejected")?;
        // The durable receipt and stdout are the same sole canonical JSON line.
        let receipt = format!("{receipt}\n");
        write_original(output, "receipt.json", receipt.as_bytes())?;
        held_directory
            .sync_all()
            .map_err(|_| "directory_sync_rejected")?;
        let parent_now = fs::metadata(parent).map_err(|_| "output_parent_rejected")?;
        if parent_before.dev() != parent_now.dev()
            || parent_before.ino() != parent_now.ino()
            || held_directory
                .metadata()
                .map_err(|_| "output_directory_rejected")?
                .ino()
                != fs::metadata(output)
                    .map_err(|_| "output_directory_rejected")?
                    .ino()
        {
            return Err("output_directory_changed");
        }
        held_parent.sync_all().map_err(|_| "parent_sync_rejected")?;
        Ok(receipt)
    }
    fn write_original(output: &Path, name: &str, bytes: &[u8]) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(output.join(name))
            .map_err(|_| "output_original_create_rejected")?;
        file.write_all(bytes)
            .map_err(|_| "output_original_write_rejected")?;
        file.sync_all().map_err(|_| "output_original_sync_rejected")
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn synthetic_public_inputs() -> (
            KagemushaReleaseAuthorityPolicyV1,
            Vec<iroha_crypto::KeyPair>,
            Vec<u8>,
        ) {
            // Known-public deterministic test signers, never an installed or shipping authority.
            let mut signers = vec![
                iroha_crypto::KeyPair::from_seed(vec![31; 32], Algorithm::Ed25519),
                iroha_crypto::KeyPair::from_seed(vec![32; 32], Algorithm::Ed25519),
            ];
            signers.sort_by(|a, b| a.public_key().cmp(b.public_key()));
            let policy = KagemushaReleaseAuthorityPolicyV1 {
                version: 1,
                authority_set_id: [2; 32],
                threshold: 2,
                authorized_signers: signers.iter().map(|s| s.public_key().clone()).collect(),
            };
            let input = format!(r#"{{"version":1,"purpose":1,"authority_policy_digest":"{}","network_id":"{}",
          "app_package":"test.known.public","app_version_code":1,"core_origin":"https://known.public.example",
          "app_signing_identity_digest":"{}","app_distribution_digest":"{}","app_source_sha256":"{}",
          "app_code_sha256":"{}","sdk_source_sha256":"{}","jni_artifacts":[{{"android_abi":"arm64-v8a","sha256":"{}"}}],"native_abi":25,
          "evidence_issuer":"{}","raw_verifier_policy_digest":"{}","google_oauth_issuer":"https://accounts.google.com",
          "google_oauth_client_id":"known-public-test-audience","google_cloud_project_number":1,
          "play_integrity_policy":{{"policy_digest":"{}","maximum_evidence_age_ms":120000,"maximum_refresh_interval_ms":120000,
             "require_play_recognized":true,"require_licensed":true,"minimum_device_integrity":1}},
          "allowed_android_security_levels":[1,2],"native_clock_selection_digest":"{}",
          "native_clock_base_urls":["https://known.public.example/validators/1/","https://known.public.example/validators/2/",
             "https://known.public.example/validators/3/","https://known.public.example/validators/4/"],
          "policy_epoch":1,"not_before_ms":1,"expires_at_ms":1000000,"maximum_attempt_lifetime_ms":120000}}"#,
          hex(&policy.canonical_digest().unwrap()),hex(&[3;32]),hex(&[4;32]),hex(&[5;32]),hex(&[6;32]),hex(&[12;32]),
          hex(&[7;32]),hex(&[8;32]),signers[0].public_key(),hex(&[9;32]),hex(&[10;32]),hex(&[11;32])).into_bytes();
            (policy, signers, input)
        }
        fn authority_json(policy: &KagemushaReleaseAuthorityPolicyV1) -> Vec<u8> {
            let signers = norito::json::to_vec(&policy.authorized_signers).unwrap();
            format!(r#"{{"version":{},"authority_set_id":"{}","threshold":{},"authorized_signers":{}}}"#,
                policy.version, hex(&policy.authority_set_id), policy.threshold,
                std::str::from_utf8(&signers).unwrap()).into_bytes()
        }
        #[test]
        fn authority_public_proposal_uses_sole_hex_original_and_exact_signer_inventory() {
            let (policy, _, _) = synthetic_public_inputs();
            let bytes = authority_json(&policy);
            assert_eq!(authority_input(&bytes).unwrap(), policy);
            // A release policy is public DATA. Parsing never manufactures a private signer.
            let mut value = json_original(&bytes).unwrap();
            value.as_object_mut().unwrap().insert(
                "authority_set_id".into(),
                Value::Array(
                    policy
                        .authority_set_id
                        .iter()
                        .map(|b| Value::Number(u64::from(*b).into()))
                        .collect(),
                ),
            );
            assert!(authority_input(&norito::json::to_vec(&value).unwrap()).is_err());
            for (field, replacement) in [
                ("threshold", Value::Number(0_u64.into())),
                ("version", Value::Number(2_u64.into())),
                ("authority_set_id", Value::String("A".repeat(64))),
                ("authorized_signers", Value::Array(vec![])),
            ] {
                let mut value = json_original(&bytes).unwrap();
                value
                    .as_object_mut()
                    .unwrap()
                    .insert(field.into(), replacement);
                assert!(authority_input(&norito::json::to_vec(&value).unwrap()).is_err());
            }
            let mut value = json_original(&bytes).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert("private_key".into(), Value::String("refused".into()));
            assert!(authority_input(&norito::json::to_vec(&value).unwrap()).is_err());
        }
        #[test]
        fn all_five_duties_use_exact_model_codec_and_real_threshold_signatures() {
            let (policy, signers, manifest_json) = synthetic_public_inputs();
            let p = prepare(
                "authority",
                &BTreeMap::from([("policy_input".into(), authority_json(&policy))]),
            )
            .unwrap();
            let policy_bytes = p.outputs["authority-policy.norito"].clone();
            assert_eq!(
                KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes).unwrap(),
                policy
            );
            let compiled_json = format!(
            r#"{{"version":1,"app_source_sha256":"{}","sdk_source_sha256":"{}","native_abi":25}}"#,
            hex(&[6; 32]),
            hex(&[7; 32])
        )
        .into_bytes();
            let compiled = prepare(
                "compiled-binding",
                &BTreeMap::from([
                    ("authority_policy".into(), policy_bytes.clone()),
                    ("compiled_binding_input".into(), compiled_json),
                ]),
            )
            .unwrap();
            let compiled: KagemushaHardwareEvidenceCompiledBindingOriginalV1 =
                hardware_bootstrap_decode_v1(&compiled.outputs["hardware-compiled-binding.norito"])
                    .unwrap();
            assert_eq!(compiled.authority_policy_original, policy_bytes);
            assert_eq!(compiled.validate().unwrap(), policy);
            let m = prepare(
                "manifest",
                &BTreeMap::from([
                    ("authority_policy".into(), policy_bytes.clone()),
                    ("manifest_input".into(), manifest_json),
                ]),
            )
            .unwrap();
            let manifest_bytes = m.outputs["manifest.norito"].clone();
            let manifest: KagemushaHardwareEvidenceBootstrapManifestV1 =
                hardware_bootstrap_decode_v1(&manifest_bytes).unwrap();
            assert_eq!(
                manifest.signing_bytes().unwrap(),
                m.outputs["signing-message.bin"]
            );
            let mut release_inputs = BTreeMap::from([
                ("authority_policy".into(), policy_bytes.clone()),
                ("manifest".into(), manifest_bytes.clone()),
            ]);
            for (i, signer) in signers.iter().enumerate() {
                let signature =
                    Signature::new(signer.private_key(), &m.outputs["signing-message.bin"]);
                let approval_input = format!(
                    r#"{{"public_key":"{}","signature_hex":"{}"}}"#,
                    signer.public_key(),
                    hex(signature.payload())
                )
                .into_bytes();
                let a = prepare(
                    "approval",
                    &BTreeMap::from([
                        ("authority_policy".into(), policy_bytes.clone()),
                        ("manifest".into(), manifest_bytes.clone()),
                        ("approval_input".into(), approval_input),
                    ]),
                )
                .unwrap();
                assert_eq!(a.manifest_digest, m.manifest_digest);
                release_inputs.insert(
                    format!("approval_{i:03}"),
                    a.outputs["approval.norito"].clone(),
                );
            }
            let r = prepare("release", &release_inputs).unwrap();
            let decoded: KagemushaSignedHardwareBootstrapReleaseV1 =
                hardware_bootstrap_decode_v1(&r.outputs["hardware-bootstrap-release.norito"])
                    .unwrap();
            decoded.authenticate(&policy).unwrap();
            assert_eq!(decoded.manifest, manifest);
            let mut missing = release_inputs.clone();
            missing.remove("approval_001");
            assert!(prepare("release", &missing).is_err());
            let mut duplicate = release_inputs.clone();
            duplicate.insert("approval_001".into(), duplicate["approval_000"].clone());
            assert!(prepare("release", &duplicate).is_err());
            let mut reversed = release_inputs;
            let a = reversed["approval_000"].clone();
            let b = reversed["approval_001"].clone();
            reversed.insert("approval_000".into(), b);
            reversed.insert("approval_001".into(), a);
            assert!(prepare("release", &reversed).is_err());
        }
        #[test]
        fn manifest_fields_and_approval_scope_cannot_be_substituted() {
            let (policy, signers, input) = synthetic_public_inputs();
            let policy_bytes = policy_original(&policy).unwrap();
            let mut v = json_original(&input).unwrap();
            v.as_object_mut()
                .unwrap()
                .insert("private_key".into(), Value::String("forbidden".into()));
            assert!(manifest_input(&norito::json::to_vec(&v).unwrap()).is_err());
            let manifest = manifest_input(&input).unwrap();
            let sig = Signature::new(signers[0].private_key(), &manifest.signing_bytes().unwrap());
            let offered = format!(
                r#"{{"public_key":"{}","signature_hex":"{}"}}"#,
                signers[0].public_key(),
                hex(sig.payload())
            )
            .into_bytes();
            let mut changed = manifest.clone();
            changed.jni_artifacts[0].sha256 = [77; 32];
            assert!(
                prepare(
                    "approval",
                    &BTreeMap::from([
                        ("authority_policy".into(), policy_bytes.clone()),
                        ("manifest".into(), encode(&changed).unwrap()),
                        ("approval_input".into(), offered)
                    ])
                )
                .is_err()
            );
            let mut originals = BTreeMap::from([
                ("authority_policy".into(), policy_bytes),
                ("manifest_input".into(), input),
            ]);
            let mut v = json_original(&originals["manifest_input"]).unwrap();
            v.as_object_mut().unwrap().insert(
                "authority_policy_digest".into(),
                Value::String(hex(&[88; 32])),
            );
            originals.insert("manifest_input".into(), norito::json::to_vec(&v).unwrap());
            assert!(prepare("manifest", &originals).is_err());
        }
        #[test]
        fn durable_outputs_are_owner_only_and_receipt_matches_actual_files() {
            let (policy, _, _) = synthetic_public_inputs();
            let prepared = prepare(
                "authority",
                &BTreeMap::from([("policy_input".into(), authority_json(&policy))]),
            )
            .unwrap();
            let parent = std::env::temp_dir().canonicalize().unwrap();
            let output = parent.join(format!("hardware-codec-output-{}", std::process::id()));
            let receipt = emit(&output, "authority", prepared).unwrap();
            assert!(receipt.ends_with('\n'));
            assert!(!receipt.ends_with("\n\n"));
            assert_eq!(fs::metadata(&output).unwrap().mode() & 0o777, 0o700);
            for name in ["authority-policy.norito", "receipt.json"] {
                assert_eq!(
                    fs::metadata(output.join(name)).unwrap().mode() & 0o777,
                    0o600
                );
            }
            assert_eq!(
                fs::read(output.join("receipt.json")).unwrap(),
                receipt.as_bytes()
            );
            let parsed = json_original(receipt.as_bytes()).unwrap();
            let bytes = fs::read(output.join("authority-policy.norito")).unwrap();
            assert_eq!(
                parsed["outputs"]["authority-policy.norito"]["sha256"]
                    .as_str()
                    .unwrap(),
                hex(&sha(&bytes))
            );
            assert!(require_new_directory(&output).is_err());
            fs::remove_dir_all(output).unwrap();
        }
        #[test]
        fn roles_are_closed_and_release_approvals_contiguous() {
            for (duty, roles) in [
                ("authority", vec!["policy_input"]),
                (
                    "compiled-binding",
                    vec!["authority_policy", "compiled_binding_input"],
                ),
                ("manifest", vec!["authority_policy", "manifest_input"]),
                (
                    "approval",
                    vec!["authority_policy", "manifest", "approval_input"],
                ),
                (
                    "release",
                    vec![
                        "authority_policy",
                        "manifest",
                        "approval_000",
                        "approval_001",
                    ],
                ),
            ] {
                validate_roles(duty, roles.iter().copied()).unwrap();
                let mut extra = roles;
                extra.push("private_key");
                assert!(validate_roles(duty, extra.into_iter()).is_err());
            }
            assert!(
                validate_roles(
                    "release",
                    ["authority_policy", "manifest", "approval_001"].into_iter()
                )
                .is_err()
            );
            assert!(
                validate_roles("release", ["authority_policy", "manifest"].into_iter()).is_err()
            );
        }
        #[test]
        fn duplicate_alias_fd_unknown_fields_and_noncanonical_pins_reject() {
            let sha = hex(&[1; 32]);
            for text in [
                format!(
                    r#"{{"schema":"bpng.hardware-evidence-native-input.v1","duty":"authority","originals":{{"policy_input":{{"fd":3,"sizeBytes":1,"sha256":"{sha}"}}}}}}"#
                ),
                format!(
                    r#"{{"schema":"bpng.hardware-evidence-native-input.v1","duty":"compiled-binding","originals":{{"authority_policy":{{"fd":4,"sizeBytes":1,"sha256":"{sha}"}},"compiled_binding_input":{{"fd":4,"sizeBytes":1,"sha256":"{sha}"}}}}}}"#
                ),
            ] {
                assert!(parse_request(&norito::json::from_str::<Value>(&text).unwrap()).is_err());
            }
            assert!(unhex::<32>(&"A".repeat(64)).is_err());
            assert!(unhex::<32>(&"0".repeat(63)).is_err());
            assert!(
                norito::json::from_str::<Value>(r#"{"duty":"authority","duty":"release"}"#)
                    .is_err()
            );
        }
        #[test]
        fn descriptor_reads_zero_offset_and_rechecks_original_length_and_digest() {
            use std::io::{Seek as _, SeekFrom};
            use std::os::fd::AsRawFd as _;
            let root =
                std::env::temp_dir().join(format!("hardware-codec-fd-{}", std::process::id()));
            fs::create_dir(&root).unwrap();
            let path = root.join("public-original");
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            file.write_all(b"public").unwrap();
            file.seek(SeekFrom::End(0)).unwrap();
            let pin = Pin {
                fd: file.as_raw_fd(),
                size: 6,
                sha: sha(b"public"),
            };
            assert_eq!(read_fd(pin.fd, Some(&pin), 10).unwrap(), b"public");
            assert!(
                read_fd(
                    pin.fd,
                    Some(&Pin {
                        sha: [0; 32],
                        ..pin
                    }),
                    10
                )
                .is_err()
            );
            assert!(read_fd(pin.fd, Some(&Pin { size: 5, ..pin }), 10).is_err());
            assert!(read_fd(pin.fd, Some(&pin), 5).is_err());
            assert_eq!(file.stream_position().unwrap(), 6);
            drop(file);
            fs::remove_dir_all(root).unwrap();
        }
        #[test]
        fn output_paths_are_new_absolute_and_canonical() {
            assert!(require_new_directory(Path::new("relative-output")).is_err());
            assert!(require_new_directory(Path::new("/tmp/../tmp/unsafe-output")).is_err());
            assert!(require_new_directory(Path::new("/")).is_err());
        }
    }
}
