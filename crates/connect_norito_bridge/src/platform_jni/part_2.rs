pub(super) fn java_privacy_compiled_profile_catalog_archive() -> Result<Vec<u8>, String> {
    privacy_compiled_profile_catalog_archive_v1()
        .map(<[u8]>::to_vec)
        .map_err(|_| "failed to derive local compiled-profile catalog archive".to_owned())
}
pub(super) fn java_privacy_exact12_fixture_bundle_archive() -> Result<Vec<u8>, String> {
    let mut archive = privacy_exact12_fixture_bundle_bytes_v1()
        .map_err(|err| format!("failed to encode exact-12 privacy fixture bundle: {err}"))?;
    if archive.len() > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1 {
        archive.fill(0);
        return Err("exact-12 privacy fixture bundle exceeds maximum length".to_owned());
    }
    let status = validate_privacy_exact12_fixture_bundle_v1(&archive);
    if !status.is_valid() {
        archive.fill(0);
        return Err(format!(
            "exact-12 privacy fixture bundle validation failed with status {}",
            status.code()
        ));
    }
    Ok(archive)
}
pub(super) fn java_privacy_validate_exact12_fixture_bundle_bytes(
    archive: Option<&[u8]>,
) -> jni::sys::jint {
    let Some(archive) = archive else {
        return PrivacyExact12FixtureBundleValidationStatusV1::NullPointer.code();
    };
    if archive.len() > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1 {
        return PrivacyExact12FixtureBundleValidationStatusV1::ArchiveTooLarge.code();
    }
    validate_privacy_exact12_fixture_bundle_v1(archive).code()
}
pub(super) fn java_native_privacy_validate_compiled_profile_catalog(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    if archive.is_null() {
        return PrivacyCompiledProfileCatalogArchiveValidationStatusV1::NullPointer.code();
    }
    let archive_len = match env.get_array_length(&archive) {
        Ok(value) => match usize::try_from(value) {
            Ok(value) => value,
            Err(_) => {
                return PrivacyCompiledProfileCatalogArchiveValidationStatusV1::ArchiveTooLarge
                    .code();
            }
        },
        Err(_) => {
            return PrivacyCompiledProfileCatalogArchiveValidationStatusV1::MalformedArchive.code();
        }
    };
    if archive_len > PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES_V1 {
        return PrivacyCompiledProfileCatalogArchiveValidationStatusV1::ArchiveTooLarge.code();
    }
    match env.convert_byte_array(&archive) {
        Ok(bytes) => validate_local_privacy_compiled_profile_catalog_archive_v1(&bytes).code(),
        Err(_) => PrivacyCompiledProfileCatalogArchiveValidationStatusV1::MalformedArchive.code(),
    }
}
pub(super) fn java_native_privacy_validate_exact12_capability_manifest(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    if archive.is_null() {
        return PrivacyCapabilityArchiveValidationStatusV1::NullPointer.code();
    }
    let archive_len = match env.get_array_length(&archive) {
        Ok(value) => match usize::try_from(value) {
            Ok(value) => value,
            Err(_) => {
                return PrivacyCapabilityArchiveValidationStatusV1::ArchiveTooLarge.code();
            }
        },
        Err(_) => return PrivacyCapabilityArchiveValidationStatusV1::MalformedArchive.code(),
    };
    if archive_len > PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1 {
        return PrivacyCapabilityArchiveValidationStatusV1::ArchiveTooLarge.code();
    }
    match env.convert_byte_array(&archive) {
        Ok(bytes) => validate_privacy_capability_archive_v1(&bytes).code(),
        Err(_) => PrivacyCapabilityArchiveValidationStatusV1::MalformedArchive.code(),
    }
}
pub(super) fn java_privacy_exact12_capability_tuple_admitted(
    archive: &[u8],
    protocol_index: jni::sys::jint,
) -> bool {
    if !validate_privacy_capability_archive_v1(archive).is_valid() {
        return false;
    }
    let Ok(index) = usize::try_from(protocol_index) else {
        return false;
    };
    let Some(expected_protocol) = PrivacyProtocolIdV1::ALL.get(index).copied() else {
        return false;
    };
    let Ok(manifest) = norito::decode_from_bytes::<PrivacyExact12CapabilityManifestV1>(archive)
    else {
        return false;
    };
    let Ok(catalog) = compiled_privacy_profile_catalog_v1() else {
        return false;
    };
    let (Some(committed), Some(local)) =
        (manifest.protocols.get(index), catalog.protocols.get(index))
    else {
        return false;
    };
    committed.protocol_id == expected_protocol
        && local.protocol_id == expected_protocol
        && committed.is_network_available()
        && committed.compiled_profile == local.compiled_profile
}
pub(super) fn java_native_privacy_require_exact12_capability_tuple(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
    protocol_index: jni::sys::jint,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    if archive.is_null() {
        return JNI_FALSE;
    }
    let archive_len = match env.get_array_length(&archive) {
        Ok(value) => match usize::try_from(value) {
            Ok(value) => value,
            Err(_) => return JNI_FALSE,
        },
        Err(_) => return JNI_FALSE,
    };
    if archive_len == 0 || archive_len > PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1 {
        return JNI_FALSE;
    }
    let Ok(mut archive_bytes) = env.convert_byte_array(&archive) else {
        return JNI_FALSE;
    };
    let admitted = java_privacy_exact12_capability_tuple_admitted(&archive_bytes, protocol_index);
    archive_bytes.fill(0);
    if admitted { JNI_TRUE } else { JNI_FALSE }
}
pub(super) fn java_privacy_exact12_submit_proof_admitted(
    manifest_archive: &[u8],
    protocol_index: jni::sys::jint,
    instruction_archive: &[u8],
) -> bool {
    if !java_privacy_exact12_capability_tuple_admitted(manifest_archive, protocol_index)
        || instruction_archive.is_empty()
        || instruction_archive.len()
            > usize::try_from(TAIRA_PRIVACY_MAX_ACTION_BYTES_V1)
                .expect("u32 privacy action limit fits usize")
    {
        return false;
    }
    let Ok(index) = usize::try_from(protocol_index) else {
        return false;
    };
    let Some(expected_protocol) = PrivacyProtocolIdV1::ALL.get(index).copied() else {
        return false;
    };
    let Ok(manifest) =
        norito::decode_from_bytes::<PrivacyExact12CapabilityManifestV1>(manifest_archive)
    else {
        return false;
    };
    let Some(committed) = manifest.protocols.get(index) else {
        return false;
    };
    let Some(activation) = committed.activation.as_ref() else {
        return false;
    };
    let Ok(instruction) = norito::decode_canonical::<SubmitPrivacyProofV1>(instruction_archive)
    else {
        return false;
    };
    instruction.envelope.protocol_id == expected_protocol
        && instruction
            .envelope
            .validate_against_activation(
                activation,
                &manifest.consensus_policy.current_limits,
                manifest.committed_height,
            )
            .is_ok()
}
pub(super) fn java_native_privacy_validate_exact12_submit_proof_construction(
    env: &mut jni::JNIEnv<'_>,
    manifest_archive: jni::objects::JByteArray<'_>,
    protocol_index: jni::sys::jint,
    instruction_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    if manifest_archive.is_null() || instruction_archive.is_null() {
        return JNI_FALSE;
    }
    let manifest_len = match env.get_array_length(&manifest_archive) {
        Ok(value) => match usize::try_from(value) {
            Ok(value) => value,
            Err(_) => return JNI_FALSE,
        },
        Err(_) => return JNI_FALSE,
    };
    let instruction_len = match env.get_array_length(&instruction_archive) {
        Ok(value) => match usize::try_from(value) {
            Ok(value) => value,
            Err(_) => return JNI_FALSE,
        },
        Err(_) => return JNI_FALSE,
    };
    if manifest_len == 0
        || manifest_len > PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1
        || instruction_len == 0
        || instruction_len
            > usize::try_from(TAIRA_PRIVACY_MAX_ACTION_BYTES_V1)
                .expect("u32 privacy action limit fits usize")
    {
        return JNI_FALSE;
    }
    let Ok(mut manifest_bytes) = env.convert_byte_array(&manifest_archive) else {
        return JNI_FALSE;
    };
    let instruction_result = env.convert_byte_array(&instruction_archive);
    let Ok(mut instruction_bytes) = instruction_result else {
        manifest_bytes.fill(0);
        return JNI_FALSE;
    };
    let admitted = java_privacy_exact12_submit_proof_admitted(
        &manifest_bytes,
        protocol_index,
        &instruction_bytes,
    );
    manifest_bytes.fill(0);
    instruction_bytes.fill(0);
    if admitted { JNI_TRUE } else { JNI_FALSE }
}
pub(super) fn java_privacy_exact12_capability_manifest_inspection(
    archive: &[u8],
) -> Result<Vec<u8>, String> {
    let status = validate_privacy_capability_archive_v1(archive);
    if !status.is_valid() {
        return Err(format!(
            "exact-12 capability manifest validation failed with status {}",
            status.code()
        ));
    }
    let manifest = norito::decode_from_bytes::<PrivacyExact12CapabilityManifestV1>(archive)
        .map_err(|error| {
            format!("failed to decode validated exact-12 capability manifest: {error}")
        })?;
    let catalog = compiled_privacy_profile_catalog_v1()
        .map_err(|error| format!("failed to derive local compiled-profile catalog: {error}"))?;
    if manifest.protocols.len() != catalog.protocols.len() {
        return Err("exact-12 capability manifest and local catalog row counts differ".to_owned());
    }
    let local_compiled_tuple_matches = manifest
        .protocols
        .iter()
        .zip(&catalog.protocols)
        .map(|(committed, local)| {
            committed.protocol_id == local.protocol_id
                && committed.compiled_profile == local.compiled_profile
        })
        .collect::<Vec<_>>();
    let mut projection = JsonMap::new();
    projection.insert(
        "manifest".to_owned(),
        norito::json::to_value(&manifest)
            .map_err(|error| format!("failed to inspect exact-12 capability manifest: {error}"))?,
    );
    projection.insert(
        "local_compiled_tuple_matches".to_owned(),
        norito::json::to_value(&local_compiled_tuple_matches)
            .map_err(|error| format!("failed to inspect local profile tuple matches: {error}"))?,
    );
    norito::json::to_vec(&JsonValue::Object(projection))
        .map_err(|error| format!("failed to encode exact-12 capability inspection: {error}"))
}
pub(super) fn java_native_privacy_inspect_exact12_capability_manifest(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    if archive.is_null() {
        return std::ptr::null_mut();
    }
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let archive_len = env
            .get_array_length(&archive)
            .map_err(|error| error.to_string())?;
        let archive_len = usize::try_from(archive_len)
            .map_err(|_| "exact-12 capability manifest length is invalid".to_owned())?;
        if archive_len == 0 || archive_len > PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1 {
            return Err("exact-12 capability manifest length is outside its bound".to_owned());
        }
        let mut archive_bytes = env
            .convert_byte_array(&archive)
            .map_err(|error| error.to_string())?;
        let inspection_result = java_privacy_exact12_capability_manifest_inspection(&archive_bytes);
        archive_bytes.fill(0);
        let mut inspection = inspection_result?;
        let array_result = env
            .byte_array_from_slice(&inspection)
            .map_err(|error| error.to_string());
        inspection.fill(0);
        Ok(array_result?.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_state(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_privacy_compiled_profile_catalog(
    env: &mut jni::JNIEnv<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let mut archive = java_privacy_compiled_profile_catalog_archive()?;
        let array_result = env
            .byte_array_from_slice(&archive)
            .map_err(|err| err.to_string());
        archive.fill(0);
        let array = array_result?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_state(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_privacy_validate_exact12_fixture_bundle(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    if archive.is_null() {
        return java_privacy_validate_exact12_fixture_bundle_bytes(None);
    }
    let archive_len = match env.get_array_length(&archive) {
        Ok(value) => match usize::try_from(value) {
            Ok(value) => value,
            Err(_) => {
                return PrivacyExact12FixtureBundleValidationStatusV1::ArchiveTooLarge.code();
            }
        },
        Err(_) => return PrivacyExact12FixtureBundleValidationStatusV1::MalformedArchive.code(),
    };
    if archive_len > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1 {
        return PrivacyExact12FixtureBundleValidationStatusV1::ArchiveTooLarge.code();
    }
    match env.convert_byte_array(&archive) {
        Ok(bytes) => java_privacy_validate_exact12_fixture_bundle_bytes(Some(&bytes)),
        Err(_) => PrivacyExact12FixtureBundleValidationStatusV1::MalformedArchive.code(),
    }
}
pub(super) fn java_native_privacy_exact12_fixture_bundle(
    env: &mut jni::JNIEnv<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let mut archive = java_privacy_exact12_fixture_bundle_archive()?;
        let array_result = env
            .byte_array_from_slice(&archive)
            .map_err(|err| err.to_string());
        archive.fill(0);
        let array = array_result?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_state(env, message);
            std::ptr::null_mut()
        }
    }
}
