pub(super) fn java_native_kagemusha_create_recipient_receive_offer_v2(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    lineage: jni::objects::JByteArray<'_>,
    publisher_checkpoint_envelope: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "recipient receive-offer creation", |env| {
        let request = read_java_byte_array_bounded(
            env,
            &request,
            "request",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
        )
        .ok_or_else(|| "request must be a bounded nonempty archive".to_owned())?;
        let lineage = read_java_byte_array_bounded(
            env,
            &lineage,
            "lineage",
            iroha_torii_shared::offline_api::OFFLINE_RECIPIENT_LINEAGE_MAX_RESPONSE_BYTES,
        )
        .ok_or_else(|| "lineage must be a bounded nonempty archive".to_owned())?;
        let publisher_checkpoint_envelope = read_java_byte_array_bounded(
            env,
            &publisher_checkpoint_envelope,
            "publisherCheckpointEnvelope",
            iroha_torii_shared::offline_api::OFFLINE_RECIPIENT_OFFER_MAX_PUBLISHER_ENVELOPE_BYTES,
        )
        .ok_or_else(|| {
            "publisherCheckpointEnvelope must be a bounded nonempty envelope".to_owned()
        })?;
        let offer = kagemusha_recipient_receive_offer_create_v2(
            &request,
            &lineage,
            &publisher_checkpoint_envelope,
        )
        .map_err(|_| {
            "request, lineage, publisher envelope, or direct-peer size was rejected".to_owned()
        })?;
        env.byte_array_from_slice(&offer)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_project_recipient_receive_offer_v2(
    env: &mut jni::JNIEnv<'_>,
    offer: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, String> {
        let offer = read_java_byte_array_bounded(
            env,
            &offer,
            "offer",
            iroha_torii_shared::offline_api::OFFLINE_RECIPIENT_OFFER_MAX_PEER_BYTES,
        )
        .ok_or_else(|| "offer must be a bounded nonempty archive".to_owned())?;
        let projected = kagemusha_recipient_receive_offer_project_v2(&offer)
            .map_err(|_| "offer is non-canonical, mismatched, or lacks an envelope".to_owned())?;
        java_kagemusha_byte_arrays(
            env,
            &[
                projected.request_archive,
                projected.lineage_archive,
                projected.publisher_checkpoint_envelope,
            ],
        )
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_kagemusha_verify_recipient_receive_offer_v2(
    env: &mut jni::JNIEnv<'_>,
    offer: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, String> {
        let offer = read_java_byte_array_bounded(
            env,
            &offer,
            "offer",
            iroha_torii_shared::offline_api::OFFLINE_RECIPIENT_OFFER_MAX_PEER_BYTES,
        )
        .ok_or_else(|| "offer must be a bounded nonempty archive".to_owned())?;
        let verified_at_ms = u64::try_from(verified_at_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "verifiedAtMilliseconds must be positive".to_owned())?;
        let trusted_checkpoint_height = u64::try_from(trusted_checkpoint_height)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "trustedCheckpointHeight must be positive".to_owned())?;
        let trusted_checkpoint_context_id: [u8; 32] = read_java_byte_array_bounded(
            env,
            &trusted_checkpoint_context_id,
            "trustedCheckpointContextId",
            32,
        )
        .ok_or_else(|| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?
        .try_into()
        .map_err(|_| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?;
        if trusted_checkpoint_context_id.iter().all(|byte| *byte == 0) {
            return Err("trustedCheckpointContextId must be a non-zero Iroha hash".to_owned());
        }
        let (projected, verified) = kagemusha_recipient_receive_offer_verify_v2(
            &offer,
            verified_at_ms,
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
        )
        .map_err(|_| {
            "offer request, active-state proof, or finality suffix was rejected".to_owned()
        })?;
        java_kagemusha_byte_arrays(
            env,
            &[
                projected.request_archive,
                verified.lineage_archive,
                projected.publisher_checkpoint_envelope,
                verified.promoted_checkpoint.to_vec(),
            ],
        )
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_kagemusha_project_recipient_request_v2(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "recipient request projection", |env| {
        let request = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
        >(
            env,
            &request,
            "recipientRequest",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2,
        )?;
        request
            .validate_public_binding()
            .map_err(|_| "recipient request signature or binding is invalid".to_owned())?;
        let receiver_public_key = request.receiver_public_key.as_sec1_bytes();
        let digest = request
            .digest()
            .map_err(|_| "recipient request digest is invalid".to_owned())?;
        java_kagemusha_byte_arrays(
            env,
            &[
                request.network_id.as_bytes().to_vec(),
                request.asset.to_string().into_bytes(),
                request.amount.atomic_units.to_string().into_bytes(),
                request.amount.scale.to_string().into_bytes(),
                request.recipient.to_string().into_bytes(),
                request.receiver_device_id.into_bytes(),
                request.request_id.to_vec(),
                request.issued_at_ms.to_string().into_bytes(),
                request.expires_at_ms.to_string().into_bytes(),
                request.recipient_output.note_commitment.to_vec(),
                request.recipient_output.spend_nullifier.to_vec(),
                request.recipient_key_reference.to_vec(),
                receiver_public_key.to_vec(),
                digest.to_vec(),
            ],
        )
    })
}
pub(super) fn java_kagemusha_decode_archive<T>(
    env: &mut jni::JNIEnv<'_>,
    archive: &jni::objects::JByteArray<'_>,
    field: &str,
) -> Result<T, String>
where
    T: KagemushaCanonicalDecodeSchema + NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let bytes = read_java_byte_array_bounded(
        env,
        archive,
        field,
        KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4,
    )
    .ok_or_else(|| {
        format!(
            "{field} must contain 1..{KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4} bytes"
        )
    })?;
    decode_canonical_kagemusha_archive(&bytes)
        .map_err(|_| format!("{field} is not a canonical typed archive"))
}
pub(super) fn java_kagemusha_decode_sensitive_archive<T>(
    env: &mut jni::JNIEnv<'_>,
    archive: &jni::objects::JByteArray<'_>,
    field: &str,
) -> Result<T, String>
where
    T: KagemushaCanonicalDecodeSchema + KagemushaSensitiveArchive + NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let bytes = Zeroizing::new(
        read_java_byte_array_bounded(
            env,
            archive,
            field,
            KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4,
        )
        .ok_or_else(|| {
            format!(
                "{field} must contain 1..{KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4} bytes"
            )
        })?,
    );
    decode_canonical_kagemusha_sensitive_archive(bytes.as_slice())
        .map_err(|_| format!("{field} is not a canonical typed archive"))
}
pub(super) fn java_kagemusha_decode_archive_bounded<T>(
    env: &mut jni::JNIEnv<'_>,
    archive: &jni::objects::JByteArray<'_>,
    field: &str,
    maximum: usize,
) -> Result<T, String>
where
    T: KagemushaCanonicalDecodeSchema + NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let bytes = read_java_byte_array_bounded(env, archive, field, maximum)
        .ok_or_else(|| format!("{field} must contain 1..{maximum} bytes"))?;
    decode_canonical_kagemusha_archive(&bytes)
        .map_err(|_| format!("{field} is not a canonical typed archive"))
}
pub(super) struct JavaKagemushaSensitiveOpeningV2 {
    value: KagemushaNoteOpeningV2,
}
impl Drop for JavaKagemushaSensitiveOpeningV2 {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}
pub(super) struct JavaKagemushaSensitiveMembershipWitnessV2 {
    value: KagemushaNoteMembershipWitnessV2,
}
impl Drop for JavaKagemushaSensitiveMembershipWitnessV2 {
    fn drop(&mut self) {
        zeroize_kagemusha_note_membership_witness_v2(&mut self.value);
    }
}
pub(super) fn java_kagemusha_optional_opening(
    env: &mut jni::JNIEnv<'_>,
    archive: &jni::objects::JByteArray<'_>,
    field: &str,
) -> Result<Option<JavaKagemushaSensitiveOpeningV2>, String> {
    let bytes = Zeroizing::new(
        read_java_byte_array(env, archive, field)
            .ok_or_else(|| format!("{field} must be bytes"))?,
    );
    if bytes.is_empty() {
        return Ok(None);
    }
    let opening = decode_canonical_kagemusha_sensitive_archive::<KagemushaNoteOpeningV2>(&bytes)
        .map_err(|_| format!("{field} is not a canonical note opening"))?;
    opening
        .validate()
        .map_err(|_| format!("{field} is invalid"))?;
    Ok(Some(JavaKagemushaSensitiveOpeningV2 { value: opening }))
}
pub(super) const KAGEMUSHA_JVM_EXACT_STATE_PROJECTION_VERSION_V1: u32 = 1;
pub(super) fn java_kagemusha_projection_version_v1() -> Vec<u8> {
    KAGEMUSHA_JVM_EXACT_STATE_PROJECTION_VERSION_V1
        .to_be_bytes()
        .to_vec()
}
pub(super) fn java_kagemusha_count_v1(value: usize, field: &str) -> Result<Vec<u8>, String> {
    if !(1..=iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2)
        .contains(&value)
    {
        return Err(format!("{field} count is outside the exact-state limit"));
    }
    u32::try_from(value)
        .map(u32::to_be_bytes)
        .map(|bytes| bytes.to_vec())
        .map_err(|_| format!("{field} count exceeds the projection range"))
}
pub(super) fn java_kagemusha_validate_claims_v1(
    claims: &[iroha_data_model::offline::KagemushaRecursiveSpendBranchClaimV2],
) -> Result<(), String> {
    java_kagemusha_count_v1(claims.len(), "branchClaims")?;
    for claim in claims {
        claim
            .validate()
            .map_err(|_| "branch claim is invalid".to_owned())?;
    }
    if claims.windows(2).any(|pair| pair[0].path >= pair[1].path)
        || claims.iter().enumerate().any(|(index, left)| {
            claims[index + 1..]
                .iter()
                .any(|right| left.path.conflicts_with(right.path))
        })
    {
        return Err("branch claims are not canonical independent exact-state paths".to_owned());
    }
    Ok(())
}
/// Append the canonical exact-state projection of one independently spendable branch.
///
/// The tuple is deliberately self-authenticating: the bundle is validated first and is the
/// sole source of every projected public field. Claims are emitted individually in their
/// already-canonical statement order, preceded by a fixed-width count. No retired parent-claim
/// or claim-digest convenience value is synthesized.
pub(super) fn java_kagemusha_append_branch_projection_v1(
    fields: &mut Vec<Vec<u8>>,
    bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
    witness: &KagemushaNoteMembershipWitnessV2,
) -> Result<(), String> {
    bundle
        .validate_public_binding()
        .map_err(|_| "branch bundle binding is invalid".to_owned())?;
    java_kagemusha_validate_claims_v1(&bundle.statement.branch_claims)?;
    validate_kagemusha_note_membership_witness_v2(witness)
        .map_err(|_| "branch membership witness is invalid".to_owned())?;
    if witness.input_path.root != bundle.statement.final_root
        || witness.dummy_input_path.root != bundle.statement.final_root
    {
        return Err("branch membership witness does not bind the bundle root".to_owned());
    }
    let note = &bundle.statement.current_note;
    let bundle_archive = norito::to_bytes(bundle)
        .map_err(|error| format!("failed to encode branch bundle: {error}"))?;
    let witness_archive = norito::to_bytes(witness)
        .map_err(|error| format!("failed to encode branch witness: {error}"))?;
    let artifact_binding = norito::to_bytes(&bundle.statement.artifact_binding)
        .map_err(|error| format!("failed to encode branch artifact binding: {error}"))?;
    let bundle_digest = bundle
        .digest()
        .map_err(|_| "branch bundle digest is invalid".to_owned())?;
    fields.extend([
        bundle_archive,
        witness_archive,
        note.note_commitment.to_vec(),
        note.spend_nullifier.to_vec(),
        note.amount.atomic_units.to_string().into_bytes(),
        note.amount.scale.to_string().into_bytes(),
        bundle.statement.peer_hop_count.to_string().into_bytes(),
        bundle.statement.proof_step_count.to_string().into_bytes(),
        bundle_digest.to_vec(),
        artifact_binding,
        java_kagemusha_count_v1(bundle.statement.branch_claims.len(), "branchClaims")?,
    ]);
    for claim in &bundle.statement.branch_claims {
        fields.push(
            norito::to_bytes(claim)
                .map_err(|error| format!("failed to encode branch claim: {error}"))?,
        );
    }
    Ok(())
}
pub(super) fn java_kagemusha_byte_array_vector(
    env: &mut jni::JNIEnv<'_>,
    values: &jni::objects::JObjectArray<'_>,
    field: &str,
) -> Result<Vec<Zeroizing<Vec<u8>>>, String> {
    let count = env
        .get_array_length(values)
        .map_err(|error| format!("failed to read {field} count: {error}"))?;
    let mut result = Vec::with_capacity(usize::try_from(count).unwrap_or_default());
    for index in 0..count {
        let object = env
            .get_object_array_element(values, index)
            .map_err(|error| format!("failed to read {field}[{index}]: {error}"))?;
        if object.is_null() {
            return Err(format!("{field}[{index}] must be a byte array"));
        }
        let array = jni::objects::JByteArray::from(object);
        let bytes = read_java_byte_array(env, &array, &format!("{field}[{index}]"))
            .ok_or_else(|| format!("{field}[{index}] must be a byte array"))?;
        result.push(Zeroizing::new(bytes));
    }
    Ok(result)
}
pub(super) fn java_kagemusha_byte_array_vector_bounded(
    env: &mut jni::JNIEnv<'_>,
    values: &jni::objects::JObjectArray<'_>,
    field: &str,
    maximum_count: usize,
    maximum_bytes: usize,
) -> Result<Vec<Zeroizing<Vec<u8>>>, String> {
    let count = env
        .get_array_length(values)
        .map_err(|error| format!("failed to read {field} count: {error}"))?;
    let count = usize::try_from(count).map_err(|_| format!("{field} count is invalid"))?;
    if count > maximum_count {
        return Err(format!(
            "{field} must contain at most {maximum_count} entries"
        ));
    }
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let object = env
            .get_object_array_element(
                values,
                i32::try_from(index).map_err(|_| format!("{field} count is invalid"))?,
            )
            .map_err(|error| format!("failed to read {field}[{index}]: {error}"))?;
        if object.is_null() {
            return Err(format!("{field}[{index}] must be a byte array"));
        }
        let array = jni::objects::JByteArray::from(object);
        let bytes =
            read_java_byte_array_bounded(env, &array, &format!("{field}[{index}]"), maximum_bytes)
                .ok_or_else(|| format!("{field}[{index}] must contain 1..{maximum_bytes} bytes"))?;
        result.push(Zeroizing::new(bytes));
    }
    Ok(result)
}
pub(super) fn java_kagemusha_decimal_u32(bytes: &[u8], field: &str) -> Result<u32, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{field} must be UTF-8"))?;
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(format!("{field} must be a canonical unsigned decimal"));
    }
    text.parse::<u32>()
        .map_err(|_| format!("{field} must fit in u32"))
}
pub(super) fn java_kagemusha_output_path_v4(
    leaf_index: u32,
    flattened_siblings: &[u8],
    directions: &[u8],
    root_bytes: &[u8],
    field: &str,
) -> Result<KagemushaConfidentialMerklePathV2, String> {
    let tree_depth = iroha_data_model::offline::KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2;
    if flattened_siblings.len() != tree_depth * 32 {
        return Err(format!(
            "{field}.siblings must contain exactly {tree_depth} 32-byte nodes"
        ));
    }
    let root: [u8; 32] = root_bytes
        .try_into()
        .map_err(|_| format!("{field}.root must contain exactly 32 bytes"))?;
    if root == [0; 32] {
        return Err(format!("{field}.root must be non-zero"));
    }
    let path = KagemushaConfidentialMerklePathV2 {
        siblings: flattened_siblings
            .chunks_exact(32)
            .map(|chunk| <[u8; 32]>::try_from(chunk).expect("32-byte chunk"))
            .collect(),
        directions: directions.to_vec(),
        root,
    };
    path.validate_for_leaf_index(leaf_index)
        .map_err(|_| format!("{field} shape, directions, or leaf index is invalid"))?;
    Ok(path)
}
pub(super) fn java_kagemusha_output_leaf_fields_v4(
    fields: &[Zeroizing<Vec<u8>>],
    field: &str,
) -> Result<Option<KagemushaOutputMembershipLeafPathsV4>, String> {
    if fields.is_empty() {
        return Ok(None);
    }
    if fields.len() != 7 {
        return Err(format!("{field} must contain exactly seven fields"));
    }
    let leaf_index = java_kagemusha_decimal_u32(&fields[0], &format!("{field}.leafIndex"))?;
    Ok(Some(KagemushaOutputMembershipLeafPathsV4 {
        leaf_index,
        update_path: java_kagemusha_output_path_v4(
            leaf_index,
            &fields[1],
            &fields[2],
            &fields[3],
            &format!("{field}.updatePath"),
        )?,
        membership_path: java_kagemusha_output_path_v4(
            leaf_index,
            &fields[4],
            &fields[5],
            &fields[6],
            &format!("{field}.membershipPath"),
        )?,
    }))
}
pub(super) fn java_kagemusha_copy_c_archive_v4(
    status: c_int,
    output: *mut c_uchar,
    output_len: c_ulong,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    if status != 0 {
        if !output.is_null() {
            connect_norito_free(output);
        }
        return Err(format!("{label} was rejected with native status {status}"));
    }
    let length = match usize::try_from(output_len) {
        Ok(length) => length,
        Err(_) => {
            if !output.is_null() {
                connect_norito_free(output);
            }
            return Err(format!("{label} result length exceeds the JVM range"));
        }
    };
    if output.is_null() || kagemusha_archive_out_of_bounds_for(length, maximum) {
        if !output.is_null() {
            connect_norito_free(output);
        }
        return Err(format!("{label} returned an invalid result archive"));
    }
    let archive = unsafe { std::slice::from_raw_parts(output, length) }.to_vec();
    connect_norito_free(output);
    Ok(archive)
}
pub(super) fn java_native_kagemusha_build_output_membership_frontier_v4(
    env: &mut jni::JNIEnv<'_>,
    leaf_index: jni::sys::jint,
    flattened_siblings: jni::objects::JByteArray<'_>,
    directions: jni::objects::JByteArray<'_>,
    root: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 output frontier construction", |env| {
        let leaf_index =
            u32::try_from(leaf_index).map_err(|_| "leafIndex must be non-negative".to_owned())?;
        let flattened_siblings = Zeroizing::new(
            read_java_byte_array(env, &flattened_siblings, "flattenedSiblings")
                .ok_or_else(|| "flattenedSiblings must be bytes".to_owned())?,
        );
        if flattened_siblings.len()
            != iroha_core::zk::confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 * 32
        {
            return Err("flattenedSiblings has the wrong length".to_owned());
        }
        let directions = Zeroizing::new(
            read_java_byte_array(env, &directions, "directions")
                .ok_or_else(|| "directions must be bytes".to_owned())?,
        );
        if directions.len() != iroha_core::zk::confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 {
            return Err("directions has the wrong length".to_owned());
        }
        let root = java_kagemusha_fixed32(env, &root, "root")?;
        let mut output = std::ptr::null_mut();
        let mut output_len = 0;
        let status = unsafe {
            connect_norito_kagemusha_output_membership_frontier_build_v4(
                leaf_index,
                flattened_siblings.as_ptr(),
                c_ulong::try_from(flattened_siblings.len())
                    .map_err(|_| "flattenedSiblings length exceeds native range")?,
                directions.as_ptr(),
                c_ulong::try_from(directions.len())
                    .map_err(|_| "directions length exceeds native range")?,
                root.as_ptr(),
                32,
                &mut output,
                &mut output_len,
            )
        };
        let archive = Zeroizing::new(java_kagemusha_copy_c_archive_v4(
            status,
            output,
            output_len,
            KAGEMUSHA_OUTPUT_MEMBERSHIP_FRONTIER_MAX_BYTES_V4,
            "V4 output frontier construction",
        )?);
        env.byte_array_from_slice(archive.as_slice())
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_kagemusha_output_path_projection_fields_v4(
    leaf_index: u32,
    path: &KagemushaConfidentialMerklePathV2,
) -> Vec<Vec<u8>> {
    vec![
        leaf_index.to_string().into_bytes(),
        path.siblings.iter().flatten().copied().collect(),
        path.directions.clone(),
        path.root.to_vec(),
    ]
}
pub(super) fn java_kagemusha_output_leaf_projection_fields_v4(
    leaf: Option<&KagemushaOutputMembershipLeafPathsV4>,
) -> Vec<Vec<u8>> {
    let Some(leaf) = leaf else {
        return vec![Vec::new(); 7];
    };
    let mut fields =
        java_kagemusha_output_path_projection_fields_v4(leaf.leaf_index, &leaf.update_path);
    let membership =
        java_kagemusha_output_path_projection_fields_v4(leaf.leaf_index, &leaf.membership_path);
    fields.extend(membership.into_iter().skip(1));
    fields
}
pub(super) fn java_native_kagemusha_derive_output_membership_paths_v4(
    env: &mut jni::JNIEnv<'_>,
    frontier: jni::objects::JByteArray<'_>,
    recipient_commitment: jni::objects::JByteArray<'_>,
    change_commitment: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "V4 output membership derivation", |env| {
        let frontier = Zeroizing::new(
            read_java_byte_array_bounded(
                env,
                &frontier,
                "frontier",
                KAGEMUSHA_OUTPUT_MEMBERSHIP_FRONTIER_MAX_BYTES_V4,
            )
            .ok_or_else(|| "frontier must be canonical bytes".to_owned())?,
        );
        let recipient = Zeroizing::new(
            read_java_byte_array(env, &recipient_commitment, "recipientCommitment")
                .ok_or_else(|| "recipientCommitment must be bytes".to_owned())?,
        );
        let change = Zeroizing::new(
            read_java_byte_array(env, &change_commitment, "changeCommitment")
                .ok_or_else(|| "changeCommitment must be bytes".to_owned())?,
        );
        if (recipient.is_empty() && change.is_empty())
            || (!recipient.is_empty() && recipient.len() != 32)
            || (!change.is_empty() && change.len() != 32)
        {
            return Err(
                "recipientCommitment or changeCommitment must contain a non-zero digest".to_owned(),
            );
        }
        if recipient.iter().all(|byte| *byte == 0) && !recipient.is_empty()
            || change.iter().all(|byte| *byte == 0) && !change.is_empty()
        {
            return Err("output commitments must be non-zero".to_owned());
        }
        let recipient_pointer = if recipient.is_empty() {
            std::ptr::null()
        } else {
            recipient.as_ptr()
        };
        let change_pointer = if change.is_empty() {
            std::ptr::null()
        } else {
            change.as_ptr()
        };
        let mut output = std::ptr::null_mut();
        let mut output_len = 0;
        let status = unsafe {
            connect_norito_kagemusha_output_membership_paths_derive_v4(
                frontier.as_ptr(),
                c_ulong::try_from(frontier.len())
                    .map_err(|_| "frontier length exceeds native range")?,
                recipient_pointer,
                c_ulong::try_from(recipient.len())
                    .map_err(|_| "recipientCommitment length exceeds native range")?,
                change_pointer,
                c_ulong::try_from(change.len())
                    .map_err(|_| "changeCommitment length exceeds native range")?,
                &mut output,
                &mut output_len,
            )
        };
        let archive = Zeroizing::new(java_kagemusha_copy_c_archive_v4(
            status,
            output,
            output_len,
            KAGEMUSHA_OUTPUT_MEMBERSHIP_PATHS_MAX_BYTES_V4,
            "V4 output membership derivation",
        )?);
        let mut paths = decode_canonical_kagemusha_sensitive_archive::<
            KagemushaOutputMembershipPathsV4,
        >(archive.as_slice())
        .map_err(|_| "native output membership archive is not canonical".to_owned())?;
        let mut fields = Vec::with_capacity(21);
        fields.push(archive.to_vec());
        fields.push(paths.initial_root.to_vec());
        fields.push(paths.final_root.to_vec());
        fields.extend(java_kagemusha_output_leaf_projection_fields_v4(
            paths.recipient.as_ref(),
        ));
        fields.extend(java_kagemusha_output_leaf_projection_fields_v4(
            paths.change.as_ref(),
        ));
        fields.extend(java_kagemusha_output_path_projection_fields_v4(
            paths.dummy_leaf_index,
            &paths.dummy_path,
        ));
        paths.zeroize();
        java_kagemusha_secret_byte_arrays(env, &mut fields)
    })
}
pub(super) fn java_native_kagemusha_validate_spendable_branch_v4(
    env: &mut jni::JNIEnv<'_>,
    bundle: jni::objects::JByteArray<'_>,
    provenance: jni::objects::JByteArray<'_>,
    membership_witness: jni::objects::JByteArray<'_>,
    opening: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let _permit = match try_preacquire_kagemusha_heavy_proof_permit_v4() {
        Ok(permit) => permit,
        Err(_) => {
            throw_java_illegal_state(
                env,
                "Kagemusha V4 spendable branch validation is busy; retry after the active proof completes"
                    .to_owned(),
            );
            return std::ptr::null_mut();
        }
    };
    java_kagemusha_archive_array_result(env, "V4 spendable branch validation", |env| {
        let block_height = u64::try_from(block_height)
            .ok()
            .filter(|height| *height != 0)
            .ok_or_else(|| "blockHeight must be positive".to_owned())?;
        let bundle = Zeroizing::new(
            read_java_byte_array_bounded(
                env,
                &bundle,
                "bundle",
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
            )
            .ok_or_else(|| "bundle must be canonical bytes".to_owned())?,
        );
        let provenance = Zeroizing::new(
            read_java_byte_array_bounded(
                env,
                &provenance,
                "provenance",
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4,
            )
            .ok_or_else(|| "provenance must be canonical bytes".to_owned())?,
        );
        let membership_witness = Zeroizing::new(
            read_java_byte_array_bounded(
                env,
                &membership_witness,
                "membershipWitness",
                KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
            )
            .ok_or_else(|| "membershipWitness must be canonical bytes".to_owned())?,
        );
        let opening = Zeroizing::new(
            read_java_byte_array_bounded(
                env,
                &opening,
                "opening",
                KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
            )
            .ok_or_else(|| "opening must be canonical bytes".to_owned())?,
        );
        let mut output = std::ptr::null_mut();
        let mut output_len = 0;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_branch_validate_v4(
                bundle.as_ptr(),
                c_ulong::try_from(bundle.len())
                    .map_err(|_| "bundle length exceeds native range")?,
                provenance.as_ptr(),
                c_ulong::try_from(provenance.len())
                    .map_err(|_| "provenance length exceeds native range")?,
                membership_witness.as_ptr(),
                c_ulong::try_from(membership_witness.len())
                    .map_err(|_| "membershipWitness length exceeds native range")?,
                opening.as_ptr(),
                c_ulong::try_from(opening.len())
                    .map_err(|_| "opening length exceeds native range")?,
                block_height,
                &mut output,
                &mut output_len,
            )
        };
        let archive = Zeroizing::new(java_kagemusha_copy_c_archive_v4(
            status,
            output,
            output_len,
            KAGEMUSHA_OUTPUT_MEMBERSHIP_FRONTIER_MAX_BYTES_V4,
            "V4 spendable branch validation",
        )?);
        env.byte_array_from_slice(archive.as_slice())
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_validate_branch_v4(
    env: &mut jni::JNIEnv<'_>,
    bundle: jni::objects::JByteArray<'_>,
    provenance: jni::objects::JByteArray<'_>,
    membership_witness: jni::objects::JByteArray<'_>,
    opening: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let _permit = match try_preacquire_kagemusha_heavy_proof_permit_v4() {
        Ok(permit) => permit,
        Err(_) => {
            throw_java_illegal_state(
                env,
                "Candidate-lab Kagemusha V4 branch validation is busy; retry after the active proof completes"
                    .to_owned(),
            );
            return std::ptr::null_mut();
        }
    };
    java_kagemusha_archive_array_result(env, "candidate-lab V4 branch validation", |env| {
        let bundle = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
        >(
            env,
            &bundle,
            "bundle",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        )?;
        let provenance = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
        >(
            env,
            &provenance,
            "topUpProvenance",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4,
        )?;
        let witness = JavaKagemushaSensitiveMembershipWitnessV2 {
            value: java_kagemusha_decode_sensitive_archive::<KagemushaNoteMembershipWitnessV2>(
                env,
                &membership_witness,
                "membershipWitness",
            )?,
        };
        let opening = JavaKagemushaSensitiveOpeningV2 {
            value: java_kagemusha_decode_sensitive_archive::<KagemushaNoteOpeningV2>(
                env, &opening, "opening",
            )?,
        };
        let block_height = u64::try_from(block_height)
            .ok()
            .filter(|height| *height != 0)
            .ok_or_else(|| "blockHeight must be positive".to_owned())?;
        let outcome = (|| {
            let installed = require_kagemusha_candidate_evidence_lab_artifact_binding_v4(
                &bundle.statement.artifact_binding,
            )
            .map_err(|_| "candidate-lab artifact binding is unavailable".to_owned())?;
            let frontier = validate_kagemusha_recursive_spend_branch_against_installed_v4(
                &bundle,
                &provenance,
                &witness.value,
                &opening.value,
                block_height,
                &installed,
            )
            .map_err(|_| "candidate-lab branch proof or opening is invalid".to_owned())?;
            norito::to_bytes(&frontier)
                .map_err(|error| format!("failed to encode candidate-lab frontier: {error}"))
        })();
        let archive = outcome?;
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_build_output_membership_paths_v4(
    env: &mut jni::JNIEnv<'_>,
    initial_root: jni::objects::JByteArray<'_>,
    final_root: jni::objects::JByteArray<'_>,
    recipient_fields: jni::objects::JObjectArray<'_>,
    change_fields: jni::objects::JObjectArray<'_>,
    dummy_fields: jni::objects::JObjectArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 output membership construction", |env| {
        let initial_root = java_kagemusha_fixed32(env, &initial_root, "initialRoot")?;
        let final_root = java_kagemusha_fixed32(env, &final_root, "finalRoot")?;
        let recipient_fields =
            java_kagemusha_byte_array_vector(env, &recipient_fields, "recipientFields")?;
        let change_fields = java_kagemusha_byte_array_vector(env, &change_fields, "changeFields")?;
        let dummy_fields = java_kagemusha_byte_array_vector(env, &dummy_fields, "dummyFields")?;
        if dummy_fields.len() != 4 {
            return Err("dummyFields must contain exactly four fields".to_owned());
        }
        let recipient = java_kagemusha_output_leaf_fields_v4(&recipient_fields, "recipientFields")?;
        let change = java_kagemusha_output_leaf_fields_v4(&change_fields, "changeFields")?;
        let dummy_leaf_index =
            java_kagemusha_decimal_u32(&dummy_fields[0], "dummyFields.leafIndex")?;
        let dummy_path = java_kagemusha_output_path_v4(
            dummy_leaf_index,
            &dummy_fields[1],
            &dummy_fields[2],
            &dummy_fields[3],
            "dummyFields.path",
        )?;
        let operation = match (recipient.is_some(), change.is_some()) {
            (true, false) => KagemushaOutputMembershipOperationV4::Init,
            (true, true) => KagemushaOutputMembershipOperationV4::Split,
            (false, true) => KagemushaOutputMembershipOperationV4::RedemptionChange,
            (false, false) => {
                return Err("recipientFields or changeFields must be present".to_owned());
            }
        };
        let mut paths = KagemushaOutputMembershipPathsV4 {
            initial_root,
            final_root,
            recipient,
            change,
            dummy_leaf_index,
            dummy_path,
        };
        paths
            .validate_shape(operation)
            .map_err(|_| "V4 output membership fields are invalid".to_owned())?;
        let archive_result = norito::to_bytes(&paths)
            .map_err(|error| format!("failed to encode V4 output membership: {error}"));
        paths.zeroize();
        let archive = Zeroizing::new(archive_result?);
        env.byte_array_from_slice(archive.as_slice())
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn kagemusha_append_inputs_conflict_v4(
    left: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
    right: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
) -> bool {
    let left_note = &left.statement.current_note;
    let right_note = &right.statement.current_note;
    left_note.note_commitment == right_note.note_commitment
        || left_note.note_commitment == right_note.spend_nullifier
        || left_note.spend_nullifier == right_note.note_commitment
        || left_note.spend_nullifier == right_note.spend_nullifier
        || left.statement.branch_claims.iter().any(|left_claim| {
            right
                .statement
                .branch_claims
                .iter()
                .any(|right_claim| left_claim.path.conflicts_with(right_claim.path))
        })
}
pub(super) struct JavaKagemushaAppendInputV4 {
    canonical_sha256: [u8; 32],
    bundle: iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
    topup_provenance: iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
    opening: KagemushaNoteOpeningV2,
    witness: KagemushaNoteMembershipWitnessV2,
}
impl Drop for JavaKagemushaAppendInputV4 {
    fn drop(&mut self) {
        self.opening.zeroize();
        zeroize_kagemusha_note_membership_witness_v2(&mut self.witness);
    }
}
pub(super) fn java_native_kagemusha_project_peer_payment_v4(
    env: &mut jni::JNIEnv<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "peer payment projection", |env| {
        let payment = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendPeerPaymentV4,
        >(
            env,
            &payment,
            "payment",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V4,
        )?;
        payment
            .validate_public_binding()
            .map_err(|_| "peer payment binding is invalid".to_owned())?;
        let operation_id = payment
            .operation_id()
            .map_err(|_| "peer payment operation id is invalid".to_owned())?;
        let recipient_request_digest = payment
            .recipient_request_digest()
            .map_err(|_| "peer payment request digest is invalid".to_owned())?;
        let topup_provenance = norito::to_bytes(&payment.topup_provenance)
            .map_err(|error| format!("failed to encode peer top-up provenance: {error}"))?;
        let mut fields = vec![
            java_kagemusha_projection_version_v1(),
            operation_id.to_vec(),
            recipient_request_digest.to_vec(),
            topup_provenance,
        ];
        java_kagemusha_append_branch_projection_v1(
            &mut fields,
            &payment.recipient_bundle,
            &payment.recipient_membership_witness,
        )?;
        java_kagemusha_byte_arrays(env, &fields)
    })
}
pub(super) fn java_native_kagemusha_project_init_result_v4(
    env: &mut jni::JNIEnv<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "V4 init result projection", |env| {
        let result = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendInitResultV4,
        >(env, &result, "initResult")?;
        result
            .bundle
            .validate_public_binding()
            .map_err(|_| "V4 init result binding is invalid".to_owned())?;
        result
            .membership_witness
            .validate_for_statement_v4(&result.bundle.statement)
            .map_err(|_| "V4 init membership witness is invalid".to_owned())?;
        result
            .topup_provenance
            .validate_for_bundle(&result.bundle)
            .map_err(|_| "V4 init top-up provenance is invalid".to_owned())?;
        let statement_digest = result
            .bundle
            .statement
            .digest()
            .map_err(|_| "V4 init statement digest is invalid".to_owned())?;
        if result.public_statement_digest == [0; 32]
            || result.public_statement_digest != statement_digest
            || result.public_statement_digest
                != result.bundle.recursive_proof.public_statement_digest
        {
            return Err("V4 init public statement digest is invalid".to_owned());
        }
        let provenance = norito::to_bytes(&result.topup_provenance)
            .map_err(|error| format!("failed to encode V4 init top-up provenance: {error}"))?;
        let mut fields = vec![java_kagemusha_projection_version_v1(), provenance];
        java_kagemusha_append_branch_projection_v1(
            &mut fields,
            &result.bundle,
            &result.membership_witness,
        )?;
        fields.push(result.public_statement_digest.to_vec());
        java_kagemusha_byte_arrays(env, &fields)
    })
}
pub(super) fn java_native_kagemusha_project_split_result_v4(
    env: &mut jni::JNIEnv<'_>,
    split: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "split result projection", |env| {
        let split = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendSplitResultV4,
        >(env, &split, "splitResult")?;
        split
            .validate_public_binding()
            .map_err(|_| "split result binding is invalid".to_owned())?;
        let payment =
            iroha_data_model::offline::KagemushaRecursiveSpendPeerPaymentV4::from_split_result(
                &split,
            )
            .map_err(|_| "recipient peer-payment projection failed".to_owned())?;
        let payment_archive = norito::to_bytes(&payment)
            .map_err(|error| format!("failed to encode recipient peer payment: {error}"))?;
        if payment_archive.len() > KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V4 {
            return Err("recipient peer payment exceeds the V4 peer archive limit".to_owned());
        }
        let mut fields = vec![
            java_kagemusha_projection_version_v1(),
            payment_archive,
            split.split.operation_id.to_vec(),
            split.split.recipient_request_digest.to_vec(),
            split.split_binding_digest.to_vec(),
            norito::to_bytes(&split.recipient_topup_provenance).map_err(|error| {
                format!("failed to encode recipient top-up provenance: {error}")
            })?,
        ];
        java_kagemusha_append_branch_projection_v1(
            &mut fields,
            &split.recipient_bundle,
            &split.recipient_membership_witness,
        )?;
        let change_present = validate_kagemusha_optional_branch_presence_v2(
            split.change_bundle.is_some(),
            split.change_membership_witness.is_some(),
        )
        .map_err(|_| "split result has incomplete change material".to_owned())?;
        fields.push(vec![u8::from(change_present)]);
        if let (Some(bundle), Some(witness)) =
            (&split.change_bundle, &split.change_membership_witness)
        {
            fields.push(
                norito::to_bytes(
                    split
                        .change_topup_provenance
                        .as_ref()
                        .ok_or_else(|| "split result is missing change provenance".to_owned())?,
                )
                .map_err(|error| format!("failed to encode change top-up provenance: {error}"))?,
            );
            java_kagemusha_append_branch_projection_v1(&mut fields, bundle, witness)?;
        }
        java_kagemusha_byte_arrays(env, &fields)
    })
}
pub(super) fn java_native_kagemusha_project_verify_result_v4(
    env: &mut jni::JNIEnv<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "verify result projection", |env| {
        let result = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendVerifyResultV4,
        >(env, &result, "verifyResult")?;
        result
            .validate_public_binding()
            .map_err(|_| "verify result binding is invalid".to_owned())?;
        let summary = &result.summary;
        summary
            .amount
            .validate()
            .map_err(|_| "verified amount is invalid".to_owned())?;
        summary
            .artifact_binding
            .validate()
            .map_err(|_| "verified artifact binding is invalid".to_owned())?;
        java_kagemusha_validate_claims_v1(&summary.branch_claims)?;
        if summary.note_commitment == [0; 32]
            || summary.spend_nullifier == [0; 32]
            || summary.note_commitment == summary.spend_nullifier
            || summary.bundle_digest == [0; 32]
            || summary.hop_count
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || summary.proof_step_count == 0
            || summary.proof_step_count
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2
        {
            return Err("verified exact-state summary is invalid".to_owned());
        }
        let artifact_binding = norito::to_bytes(&summary.artifact_binding)
            .map_err(|error| format!("failed to encode verified artifact binding: {error}"))?;
        let mut fields = vec![
            java_kagemusha_projection_version_v1(),
            vec![u8::from(result.valid)],
            vec![u8::from(result.chain_admissible)],
            vec![u8::from(result.lineage_redeemable)],
            vec![u8::from(result.witnessless_redemption_supported)],
            summary.note_commitment.to_vec(),
            summary.spend_nullifier.to_vec(),
            summary.amount.atomic_units.to_string().into_bytes(),
            summary.amount.scale.to_string().into_bytes(),
            summary.hop_count.to_string().into_bytes(),
            summary.proof_step_count.to_string().into_bytes(),
            summary.bundle_digest.to_vec(),
            summary.asset.to_string().into_bytes(),
            artifact_binding,
            result.recipient_request_digest.to_vec(),
            result.request_output_binding_digest.to_vec(),
            result.verifier_key_id.backend.as_bytes().to_vec(),
            result.verifier_key_id.name.as_bytes().to_vec(),
            result.verifier_circuit_id.as_bytes().to_vec(),
            result
                .verifier_activation_height
                .map_or_else(Vec::new, |height| height.to_string().into_bytes()),
            result
                .verifier_withdraw_height
                .map_or_else(Vec::new, |height| height.to_string().into_bytes()),
            result.verified_at_block_height.to_string().into_bytes(),
            result.verified_at_ms.to_string().into_bytes(),
            java_kagemusha_count_v1(summary.branch_claims.len(), "branchClaims")?,
        ];
        for claim in &summary.branch_claims {
            fields.push(
                norito::to_bytes(claim)
                    .map_err(|error| format!("failed to encode verified branch claim: {error}"))?,
            );
        }
        java_kagemusha_byte_arrays(env, &fields)
    })
}
pub(super) fn java_native_kagemusha_project_redeem_build_result_v4(
    env: &mut jni::JNIEnv<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "redeem build projection", |env| {
        let result = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendRedeemBuildResultV4,
        >(
            env,
            &result,
            "redeemBuildResult",
            KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4,
        )?;
        result
            .validate_public_binding()
            .map_err(|_| "redeem build result binding is invalid".to_owned())?;
        let unsigned = norito::to_bytes(&result.unsigned)
            .map_err(|error| format!("failed to encode unsigned redemption: {error}"))?;
        let mut fields = vec![
            java_kagemusha_projection_version_v1(),
            unsigned,
            result.authorization_digest.to_vec(),
            result.operation_id.to_vec(),
        ];
        let change_present = validate_kagemusha_optional_branch_presence_v2(
            result.offline_change_bundle.is_some(),
            result.offline_change_membership_witness.is_some(),
        )
        .map_err(|_| "redeem build result has incomplete change material".to_owned())?;
        fields.push(vec![u8::from(change_present)]);
        if let (Some(bundle), Some(witness)) = (
            &result.offline_change_bundle,
            &result.offline_change_membership_witness,
        ) {
            fields.push(
                norito::to_bytes(result.offline_change_topup_provenance.as_ref().ok_or_else(
                    || "redeem build result is missing change provenance".to_owned(),
                )?)
                .map_err(|error| {
                    format!("failed to encode redemption change top-up provenance: {error}")
                })?,
            );
            java_kagemusha_append_branch_projection_v1(&mut fields, bundle, witness)?;
        }
        java_kagemusha_byte_arrays(env, &fields)
    })
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_project_redeem_result_v4(
    env: &mut jni::JNIEnv<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "candidate-lab redeem projection", |env| {
        let result = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendRedeemBuildResultV4,
        >(
            env,
            &result,
            "redeemBuildResult",
            KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4,
        )?;
        result
            .validate_public_binding()
            .map_err(|_| "candidate-lab redeem result binding is invalid".to_owned())?;
        let unsigned = norito::to_bytes(&result.unsigned)
            .map_err(|error| format!("failed to encode unsigned redemption: {error}"))?;
        let redeemed_atomic_units = result.unsigned.amount.atomic_units.to_string().into_bytes();
        let redeemed_scale = result.unsigned.amount.scale.to_string().into_bytes();
        let mut fields = vec![
            java_kagemusha_projection_version_v1(),
            unsigned,
            result.authorization_digest.to_vec(),
            result.operation_id.to_vec(),
        ];
        let change_present = validate_kagemusha_optional_branch_presence_v2(
            result.offline_change_bundle.is_some(),
            result.offline_change_membership_witness.is_some(),
        )
        .map_err(|_| "candidate-lab redeem result has incomplete change material".to_owned())?;
        fields.push(vec![u8::from(change_present)]);
        if let (Some(bundle), Some(witness)) = (
            &result.offline_change_bundle,
            &result.offline_change_membership_witness,
        ) {
            fields.push(
                norito::to_bytes(result.offline_change_topup_provenance.as_ref().ok_or_else(
                    || "candidate-lab redeem result is missing change provenance".to_owned(),
                )?)
                .map_err(|error| {
                    format!("failed to encode redemption change top-up provenance: {error}")
                })?,
            );
            java_kagemusha_append_branch_projection_v1(&mut fields, bundle, witness)?;
        }
        // Explicitly append the native-validated public redemption amount so
        // the evidence app can prove redeemed + change == verified input.
        fields.push(redeemed_atomic_units);
        fields.push(redeemed_scale);
        java_kagemusha_byte_arrays(env, &fields)
    })
}
pub(super) fn java_native_kagemusha_prepare_acknowledgement_v2(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    accepted_at_ms: jni::sys::jlong,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "acknowledgement preparation", |env| {
        let request = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
        >(
            env,
            &request,
            "recipientRequest",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2,
        )?;
        let payment = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendPeerPaymentV4,
        >(
            env,
            &payment,
            "peerPayment",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V4,
        )?;
        let accepted_at_ms = u64::try_from(accepted_at_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "acceptedAtMilliseconds must be positive".to_owned())?;
        let payload =
            kagemusha_receiver_acknowledgement_payload_v2(&request, &payment, accepted_at_ms)
                .map_err(|_| "request/payment acknowledgement binding failed".to_owned())?;
        let payload_archive = norito::to_bytes(&payload)
            .map_err(|error| format!("failed to encode acknowledgement payload: {error}"))?;
        let signing_bytes = payload
            .signing_bytes()
            .map_err(|_| "failed to derive acknowledgement signing bytes".to_owned())?;
        java_kagemusha_byte_arrays(
            env,
            &[
                payload_archive,
                signing_bytes,
                payload.operation_id.to_vec(),
                payload.recipient_request_digest.to_vec(),
                payload.payment_bundle_digest.to_vec(),
                payload.recipient_commitment.to_vec(),
            ],
        )
    })
}
pub(super) fn java_native_kagemusha_create_acknowledgement_v2(
    env: &mut jni::JNIEnv<'_>,
    payload: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "acknowledgement signing", |env| {
        let payload = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaReceiverAcknowledgementPayloadV2,
        >(env, &payload, "acknowledgementPayload")?;
        let signature = read_java_byte_array(env, &signature, "signature")
            .ok_or_else(|| "signature must be bytes".to_owned())?;
        let request = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
        >(
            env,
            &request,
            "recipientRequest",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2,
        )?;
        let payment = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendPeerPaymentV4,
        >(
            env,
            &payment,
            "peerPayment",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V4,
        )?;
        let acknowledgement = iroha_data_model::offline::KagemushaReceiverAcknowledgementV2 {
            payload,
            signature: iroha_data_model::offline::KagemushaDeviceSignatureV2::from_raw_bytes(
                &signature,
            )
            .map_err(|_| "signature is malformed".to_owned())?,
        };
        let archive = acknowledgement
            .canonical_archive_for_payment_v4(&request, &payment.recipient_bundle)
            .map_err(|_| "acknowledgement signature or payment binding failed".to_owned())?;
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_verify_acknowledgement_v2(
    env: &mut jni::JNIEnv<'_>,
    acknowledgement: jni::objects::JByteArray<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "acknowledgement verification", |env| {
        let acknowledgement = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaReceiverAcknowledgementV2,
        >(env, &acknowledgement, "acknowledgement")?;
        let request = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
        >(
            env,
            &request,
            "recipientRequest",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2,
        )?;
        let payment = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendPeerPaymentV4,
        >(
            env,
            &payment,
            "peerPayment",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V4,
        )?;
        let result = acknowledgement
            .verified_result_v4(&request, &payment.recipient_bundle)
            .map_err(|_| "acknowledgement signature or binding failed".to_owned())?;
        result
            .validate_public_binding()
            .map_err(|_| "acknowledgement result binding failed".to_owned())?;
        java_kagemusha_byte_arrays(
            env,
            &[
                vec![u8::from(result.valid)],
                result.operation_id.to_vec(),
                result.recipient_request_digest.to_vec(),
                result.payment_bundle_digest.to_vec(),
                result.acknowledgement_digest.to_vec(),
            ],
        )
    })
}
pub(super) fn java_native_kagemusha_build_init_request_v4(
    env: &mut jni::JNIEnv<'_>,
    anchor: jni::objects::JByteArray<'_>,
    finality_proof: jni::objects::JByteArray<'_>,
    roster_artifact: jni::objects::JByteArray<'_>,
    opening: jni::objects::JByteArray<'_>,
    output_membership: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 init request construction", |env| {
        let anchor = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV4,
        >(env, &anchor, "topUpAnchor")?;
        let finality_proof = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaTopUpFinalityProofV2,
        >(env, &finality_proof, "topUpFinalityProof")?;
        let roster_artifact = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaTopUpFinalityRosterArtifactV2,
        >(env, &roster_artifact, "topUpFinalityRosterArtifact")?;
        let opening = java_kagemusha_decode_sensitive_archive::<KagemushaNoteOpeningV2>(
            env, &opening, "opening",
        )?;
        let output_membership = java_kagemusha_decode_sensitive_archive::<
            KagemushaOutputMembershipPathsV4,
        >(env, &output_membership, "outputMembership")?;
        let request = iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV4 {
            artifact_binding: anchor.artifact_binding.clone(),
            topup_anchor: anchor,
            topup_finality_proof: finality_proof,
            topup_finality_roster_artifact: roster_artifact,
        };
        let mut local = KagemushaRecursiveSpendInitLocalRequestV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
            request,
            opening,
            output_membership,
        };
        local
            .validate_shape()
            .map_err(|_| "V4 init request fields are invalid".to_owned())?;
        let archive_result = norito::to_bytes(&local)
            .map_err(|error| format!("failed to encode V4 init request: {error}"));
        local.zeroize();
        let archive = Zeroizing::new(archive_result?);
        env.byte_array_from_slice(archive.as_slice())
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_build_topup_provenance_v4(
    env: &mut jni::JNIEnv<'_>,
    bundle: jni::objects::JByteArray<'_>,
    roster: jni::objects::JByteArray<'_>,
    anchors: jni::objects::JObjectArray<'_>,
    finality_proofs: jni::objects::JObjectArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 top-up provenance construction", |env| {
        let bundle = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
        >(
            env,
            &bundle,
            "bundle",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        )?;
        let roster = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaTopUpFinalityRosterArtifactV2,
        >(
            env,
            &roster,
            "topUpFinalityRosterArtifact",
            iroha_data_model::offline::KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2
                as usize,
        )?;
        let anchor_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &anchors,
            "topUpAnchors",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2,
            iroha_data_model::offline::KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2 as usize,
        )?;
        let proof_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &finality_proofs,
            "topUpFinalityProofs",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2,
            iroha_data_model::offline::KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2 as usize,
        )?;
        if anchor_archives.is_empty()
            || anchor_archives.len()
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            || proof_archives.len() != anchor_archives.len()
        {
            return Err(
                "topUpAnchors and topUpFinalityProofs must have the same 1..2 count".to_owned(),
            );
        }
        let mut evidence = Vec::with_capacity(anchor_archives.len());
        for index in 0..anchor_archives.len() {
            if anchor_archives[index].is_empty()
                || anchor_archives[index].len()
                    > iroha_data_model::offline::KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2
                        as usize
            {
                return Err(format!("topUpAnchors[{index}] exceeds the V4 anchor limit"));
            }
            if proof_archives[index].is_empty()
                || proof_archives[index].len()
                    > iroha_data_model::offline::KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2
                        as usize
            {
                return Err(format!(
                    "topUpFinalityProofs[{index}] exceeds the V4 finality-proof limit"
                ));
            }
            let topup_anchor = decode_canonical_kagemusha_archive::<
                iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV4,
            >(&anchor_archives[index])
            .map_err(|_| format!("topUpAnchors[{index}] is not a canonical V4 archive"))?;
            let topup_finality_proof = decode_canonical_kagemusha_archive::<
                iroha_data_model::offline::KagemushaTopUpFinalityProofV2,
            >(&proof_archives[index])
            .map_err(|_| format!("topUpFinalityProofs[{index}] is not a canonical V4 archive"))?;
            evidence.push(
                iroha_data_model::offline::KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
                    topup_anchor,
                    topup_finality_proof,
                },
            );
        }
        let block_height = u64::try_from(block_height)
            .ok()
            .filter(|height| *height > 0)
            .ok_or_else(|| "blockHeight must be positive".to_owned())?;
        let provenance = iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4 {
            topup_finality_roster_artifact: roster,
            topup_finality_evidence: evidence,
        };
        validate_kagemusha_topup_provenance_for_bundle_v4(&bundle, &provenance, block_height)
            .map_err(|_| {
                "V4 top-up provenance does not match the bundle or installed release".to_owned()
            })?;
        let archive = norito::to_bytes(&provenance)
            .map_err(|error| format!("failed to encode V4 top-up provenance: {error}"))?;
        if kagemusha_archive_out_of_bounds_for(
            archive.len(),
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4,
        ) {
            return Err("encoded V4 top-up provenance exceeds its archive limit".to_owned());
        }
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_validate_topup_provenance_v4(
    env: &mut jni::JNIEnv<'_>,
    bundle: jni::objects::JByteArray<'_>,
    provenance: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 top-up provenance validation", |env| {
        let bundle = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
        >(
            env,
            &bundle,
            "bundle",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        )?;
        let provenance = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
        >(
            env,
            &provenance,
            "topUpProvenance",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4,
        )?;
        let block_height = u64::try_from(block_height)
            .ok()
            .filter(|height| *height > 0)
            .ok_or_else(|| "blockHeight must be positive".to_owned())?;
        validate_kagemusha_topup_provenance_for_bundle_v4(&bundle, &provenance, block_height)
            .map_err(|_| {
                "V4 top-up provenance does not match the bundle or installed release".to_owned()
            })?;
        let archive = norito::to_bytes(&provenance)
            .map_err(|error| format!("failed to encode V4 top-up provenance: {error}"))?;
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_build_append_request_with_policy_v4(
    env: &mut jni::JNIEnv<'_>,
    bundles: jni::objects::JObjectArray<'_>,
    topup_provenances: jni::objects::JObjectArray<'_>,
    openings: jni::objects::JObjectArray<'_>,
    membership_witnesses: jni::objects::JObjectArray<'_>,
    change_opening: jni::objects::JByteArray<'_>,
    output_membership: jni::objects::JByteArray<'_>,
    verifier_commitment: jni::objects::JByteArray<'_>,
    operation_id: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
    duplicate_single_input_for_negative_test: bool,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 append request construction", |env| {
        let maximum_inputs = iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2;
        let bundle_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &bundles,
            "bundles",
            maximum_inputs,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        )?;
        let provenance_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &topup_provenances,
            "topUpProvenances",
            maximum_inputs,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4,
        )?;
        let opening_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &openings,
            "openings",
            maximum_inputs,
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
        )?;
        let witness_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &membership_witnesses,
            "membershipWitnesses",
            maximum_inputs,
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
        )?;
        let input_count = bundle_archives.len();
        if input_count == 0
            || input_count > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            || provenance_archives.len() != input_count
            || opening_archives.len() != input_count
            || witness_archives.len() != input_count
            || (duplicate_single_input_for_negative_test && input_count != 1)
        {
            return Err(
                "bundles, topUpProvenances, openings, and membershipWitnesses must have the same 1..2 count"
                    .to_owned(),
            );
        }
        let mut keyed_inputs = Vec::with_capacity(input_count);
        for index in 0..input_count {
            let bundle = decode_canonical_kagemusha_recursive_archive::<
                iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
            >(&bundle_archives[index])
            .map_err(|_| format!("bundles[{index}] is not a canonical V4 archive"))?;
            validate_kagemusha_recursive_spend_bundle_shape_v4(&bundle)
                .map_err(|_| format!("bundles[{index}] V4 binding is invalid"))?;
            if provenance_archives[index].is_empty()
                || provenance_archives[index].len()
                    > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4
            {
                return Err(format!(
                    "topUpProvenances[{index}] exceeds the V4 provenance limit"
                ));
            }
            let topup_provenance = decode_canonical_kagemusha_archive::<
                iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
            >(&provenance_archives[index])
            .map_err(|_| format!("topUpProvenances[{index}] is not a canonical V4 archive"))?;
            topup_provenance
                .validate_for_bundle(&bundle)
                .map_err(|_| format!("topUpProvenances[{index}] binding is invalid"))?;
            let opening = decode_canonical_kagemusha_sensitive_archive::<KagemushaNoteOpeningV2>(
                &opening_archives[index],
            )
            .map_err(|_| format!("openings[{index}] is not a canonical typed archive"))?;
            opening
                .validate()
                .map_err(|_| format!("openings[{index}] is invalid"))?;
            let witness = JavaKagemushaSensitiveMembershipWitnessV2 {
                value: decode_canonical_kagemusha_sensitive_archive::<
                    KagemushaNoteMembershipWitnessV2,
                >(&witness_archives[index])
                .map_err(|_| {
                    format!("membershipWitnesses[{index}] is not a canonical typed archive")
                })?,
            };
            validate_kagemusha_note_membership_witness_v2(&witness.value)
                .map_err(|_| format!("membershipWitnesses[{index}] is invalid"))?;
            keyed_inputs.push(JavaKagemushaAppendInputV4 {
                canonical_sha256: Sha256::digest(&bundle_archives[index]).into(),
                bundle,
                topup_provenance,
                opening,
                witness: witness.value.clone(),
            });
        }
        keyed_inputs.sort_unstable_by_key(|input| input.canonical_sha256);
        if keyed_inputs
            .windows(2)
            .any(|pair| pair[0].canonical_sha256 == pair[1].canonical_sha256)
        {
            return Err("V4 append inputs contain a duplicate bundle".to_owned());
        }
        if keyed_inputs.len() == 2
            && kagemusha_append_inputs_conflict_v4(&keyed_inputs[0].bundle, &keyed_inputs[1].bundle)
        {
            return Err("V4 append inputs contain conflicting exact-state branches".to_owned());
        }
        let first_statement = &keyed_inputs[0].bundle.statement;
        if keyed_inputs.iter().any(|input| {
            input.bundle.statement.network_id != first_statement.network_id
                || input.bundle.statement.asset != first_statement.asset
                || input.bundle.statement.asset_scale != first_statement.asset_scale
                || input.bundle.statement.final_root != first_statement.final_root
                || input.bundle.statement.artifact_binding != first_statement.artifact_binding
                || input.witness.input_path.root != input.bundle.statement.final_root
                || input.witness.dummy_input_path.root != input.bundle.statement.final_root
        }) {
            return Err("V4 append inputs do not share one authenticated state context".to_owned());
        }
        let change_opening =
            java_kagemusha_optional_opening(env, &change_opening, "changeOpening")?;
        let output_membership = java_kagemusha_decode_sensitive_archive::<
            KagemushaOutputMembershipPathsV4,
        >(env, &output_membership, "outputMembership")?;
        let verifier_commitment =
            java_kagemusha_fixed32(env, &verifier_commitment, "verifierCommitment")?;
        let operation_id = java_kagemusha_fixed32(env, &operation_id, "operationId")?;
        let block_height = u64::try_from(block_height)
            .ok()
            .filter(|height| *height != 0)
            .ok_or_else(|| "blockHeight must be positive".to_owned())?;
        let mut local = KagemushaRecursiveSpendAppendLocalRequestV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
            previous_inputs: keyed_inputs
                .iter()
                .map(
                    |input| iroha_data_model::offline::KagemushaRecursiveSpendAppendInputV4 {
                        previous_bundle: input.bundle.clone(),
                        topup_provenance: input.topup_provenance.clone(),
                    },
                )
                .collect(),
            input_openings: keyed_inputs
                .iter()
                .map(|input| input.opening.clone())
                .collect(),
            input_membership_witnesses: keyed_inputs
                .iter()
                .map(|input| input.witness.clone())
                .collect(),
            change_opening: change_opening.as_ref().map(|opening| opening.value.clone()),
            output_artifact_binding: first_statement.artifact_binding.clone(),
            transfer_verifier_id: VerifyingKeyId::new(
                iroha_core::zk::ZK_BACKEND_HALO2_IPA,
                iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
            ),
            transfer_verifier_commitment: verifier_commitment,
            operation_id,
            block_height,
            output_membership,
        };
        local
            .validate_shape()
            .map_err(|_| "V4 append request fields are invalid".to_owned())?;
        if duplicate_single_input_for_negative_test {
            // Validate one exact spendable input first, then duplicate only its
            // already-bound branch material. The candidate-lab native append
            // boundary must be the component that observes and rejects reuse.
            local.previous_inputs.push(local.previous_inputs[0].clone());
            local.input_openings.push(local.input_openings[0].clone());
            local
                .input_membership_witnesses
                .push(local.input_membership_witnesses[0].clone());
        }
        let archive_result = norito::to_bytes(&local)
            .map_err(|error| format!("failed to encode V4 append request: {error}"));
        local.zeroize();
        let archive = Zeroizing::new(archive_result?);
        env.byte_array_from_slice(archive.as_slice())
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_build_append_request_v4(
    env: &mut jni::JNIEnv<'_>,
    bundles: jni::objects::JObjectArray<'_>,
    topup_provenances: jni::objects::JObjectArray<'_>,
    openings: jni::objects::JObjectArray<'_>,
    membership_witnesses: jni::objects::JObjectArray<'_>,
    change_opening: jni::objects::JByteArray<'_>,
    output_membership: jni::objects::JByteArray<'_>,
    verifier_commitment: jni::objects::JByteArray<'_>,
    operation_id: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_append_request_with_policy_v4(
        env,
        bundles,
        topup_provenances,
        openings,
        membership_witnesses,
        change_opening,
        output_membership,
        verifier_commitment,
        operation_id,
        block_height,
        false,
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum JavaKagemushaArtifactRegistryV4 {
    Production,
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    CandidateEvidenceLab,
}
pub(super) fn validate_java_kagemusha_topup_provenance_v4(
    bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
    provenance: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
    block_height: u64,
    registry: JavaKagemushaArtifactRegistryV4,
) -> BridgeResult<()> {
    match registry {
        JavaKagemushaArtifactRegistryV4::Production => {
            validate_kagemusha_topup_provenance_for_bundle_v4(bundle, provenance, block_height)
        }
        #[cfg(feature = "kagemusha-candidate-evidence-lab")]
        JavaKagemushaArtifactRegistryV4::CandidateEvidenceLab => {
            let installed = require_kagemusha_candidate_evidence_lab_artifact_binding_v4(
                &bundle.statement.artifact_binding,
            )?;
            validate_kagemusha_topup_provenance_for_bundle_against_installed_v4(
                bundle,
                provenance,
                block_height,
                &installed,
            )
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_build_verify_request_v4(
    env: &mut jni::JNIEnv<'_>,
    bundle: jni::objects::JByteArray<'_>,
    recipient_request: jni::objects::JByteArray<'_>,
    topup_provenance: jni::objects::JByteArray<'_>,
    maximum_hops: jni::sys::jint,
    block_height: jni::sys::jlong,
    verified_at_ms: jni::sys::jlong,
    registry: JavaKagemushaArtifactRegistryV4,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 verify request construction", |env| {
        let bundle = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
        >(env, &bundle, "bundle")?;
        validate_kagemusha_recursive_spend_bundle_shape_v4(&bundle)
            .map_err(|_| "bundle V4 binding is invalid".to_owned())?;
        let recipient_request = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
        >(
            env,
            &recipient_request,
            "recipientRequest",
            KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2,
        )?;
        let topup_provenance = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
        >(
            env,
            &topup_provenance,
            "topUpProvenance",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4,
        )?;
        let maximum_hops = u32::try_from(maximum_hops)
            .ok()
            .filter(|value| {
                *value > 0
                    && *value
                        <= iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            })
            .ok_or_else(|| "maximumHops is outside the protocol limit".to_owned())?;
        let block_height = u64::try_from(block_height)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "blockHeight must be positive".to_owned())?;
        let verified_at_ms = u64::try_from(verified_at_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "verifiedAtMilliseconds must be positive".to_owned())?;
        validate_java_kagemusha_topup_provenance_v4(
            &bundle,
            &topup_provenance,
            block_height,
            registry,
        )
        .map_err(|_| {
            "V4 top-up provenance does not match the bundle or installed release".to_owned()
        })?;
        let artifact_binding = bundle.statement.artifact_binding.clone();
        let request = iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV4 {
            bundle,
            recipient_request,
            topup_provenance,
            maximum_hops,
            artifact_binding,
            block_height,
            verified_at_ms,
        };
        let local = KagemushaRecursiveSpendVerifyLocalRequestV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
            request,
        };
        local
            .validate_shape()
            .map_err(|_| "V4 verify request fields are invalid".to_owned())?;
        let archive = norito::to_bytes(&local)
            .map_err(|error| format!("failed to encode V4 verify request: {error}"))?;
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_build_redeem_request_v4(
    env: &mut jni::JNIEnv<'_>,
    bundle: jni::objects::JByteArray<'_>,
    topup_provenance: jni::objects::JByteArray<'_>,
    opening: jni::objects::JByteArray<'_>,
    membership_witness: jni::objects::JByteArray<'_>,
    recipient: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    change_opening: jni::objects::JByteArray<'_>,
    change_output_membership: jni::objects::JByteArray<'_>,
    verifier_commitment: jni::objects::JByteArray<'_>,
    operation_id: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 redeem request construction", |env| {
        let bundle = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
        >(env, &bundle, "bundle")?;
        validate_kagemusha_recursive_spend_bundle_shape_v4(&bundle)
            .map_err(|_| "bundle V4 binding is invalid".to_owned())?;
        let topup_provenance = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
        >(
            env,
            &topup_provenance,
            "topUpProvenance",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4,
        )?;
        let opening = JavaKagemushaSensitiveOpeningV2 {
            value: java_kagemusha_decode_sensitive_archive::<KagemushaNoteOpeningV2>(
                env, &opening, "opening",
            )?,
        };
        let membership_witness = JavaKagemushaSensitiveMembershipWitnessV2 {
            value: java_kagemusha_decode_sensitive_archive::<KagemushaNoteMembershipWitnessV2>(
                env,
                &membership_witness,
                "membershipWitness",
            )?,
        };
        let chain_discriminant = u16::try_from(chain_discriminant)
            .map_err(|_| "chainDiscriminant must fit in u16".to_owned())?;
        let recipient = parse_account_id_for_chain(
            java_kagemusha_text(env, &recipient, "recipient")?,
            chain_discriminant,
        )
        .map_err(|_| "recipient must be a canonical account address".to_owned())?;
        let public_amount = java_kagemusha_amount(env, &atomic_units, scale)?;
        let change_opening =
            java_kagemusha_optional_opening(env, &change_opening, "changeOpening")?;
        let membership_bytes = Zeroizing::new(
            read_java_byte_array(env, &change_output_membership, "changeOutputMembership")
                .ok_or_else(|| "changeOutputMembership must be bytes".to_owned())?,
        );
        let change_output_membership = if membership_bytes.is_empty() {
            None
        } else {
            Some(
                decode_canonical_kagemusha_sensitive_archive::<KagemushaOutputMembershipPathsV4>(
                    &membership_bytes,
                )
                .map_err(|_| {
                    "changeOutputMembership is not a canonical typed archive".to_owned()
                })?,
            )
        };
        let verifier_commitment =
            java_kagemusha_fixed32(env, &verifier_commitment, "verifierCommitment")?;
        let operation_id = java_kagemusha_fixed32(env, &operation_id, "operationId")?;
        let block_height = u64::try_from(block_height)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "blockHeight must be positive".to_owned())?;
        let mut local = KagemushaRecursiveSpendRedeemLocalRequestV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
            bundle,
            topup_provenance,
            input_opening: opening.value.clone(),
            input_membership_witness: membership_witness.value.clone(),
            recipient,
            public_amount,
            change_opening: change_opening.as_ref().map(|opening| opening.value.clone()),
            unshield_verifier_id: VerifyingKeyId::new(
                iroha_core::zk::ZK_BACKEND_HALO2_IPA,
                iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
            ),
            unshield_verifier_commitment: verifier_commitment,
            block_height,
            operation_id,
            change_output_membership,
        };
        local
            .validate_shape()
            .map_err(|_| "V4 redeem request fields are invalid".to_owned())?;
        let archive_result = norito::to_bytes(&local)
            .map_err(|error| format!("failed to encode V4 redeem request: {error}"));
        local.zeroize();
        let archive = Zeroizing::new(archive_result?);
        env.byte_array_from_slice(archive.as_slice())
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_kagemusha_lower_hex_32(value: &str, field: &str) -> Result<Vec<u8>, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be canonical lowercase 32-byte hex"));
    }
    let digest = hex::decode(value).map_err(|_| format!("{field} is not hexadecimal"))?;
    if digest.iter().all(|byte| *byte == 0) {
        return Err(format!("{field} must be non-zero"));
    }
    Ok(digest)
}
pub(super) fn java_kagemusha_validate_active_verifier(
    verifier: &iroha_torii_shared::offline_api::OfflineActiveTransferVerifier,
    evaluated_height: u64,
    field: &str,
) -> Result<(), String> {
    if verifier.id.backend.is_empty()
        || verifier.id.name.is_empty()
        || verifier.circuit_id.is_empty()
        || verifier.version == 0
        || verifier.max_proof_bytes == 0
    {
        return Err(format!(
            "{field} has incomplete verifier identity or limits"
        ));
    }
    java_kagemusha_lower_hex_32(&verifier.commitment, &format!("{field}.commitment"))?;
    java_kagemusha_lower_hex_32(
        &verifier.public_inputs_schema_hash,
        &format!("{field}.publicInputsSchemaHash"),
    )?;
    if verifier.activation_height > evaluated_height
        || verifier
            .withdrawal_height
            .is_some_and(|withdrawal| withdrawal <= evaluated_height)
    {
        return Err(format!("{field} is not active at the readiness height"));
    }
    if verifier
        .withdrawal_height
        .is_some_and(|withdrawal| withdrawal <= verifier.activation_height)
    {
        return Err(format!("{field} has an invalid activation window"));
    }
    Ok(())
}
pub(super) fn java_kagemusha_readiness_verifier_archive(
    verifier: Option<&iroha_torii_shared::offline_api::OfflineActiveTransferVerifier>,
    evaluated_height: u64,
    field: &str,
) -> Result<Vec<u8>, String> {
    let Some(verifier) = verifier else {
        return Ok(Vec::new());
    };
    java_kagemusha_validate_active_verifier(verifier, evaluated_height, field)?;
    norito::to_bytes(verifier).map_err(|error| format!("failed to encode {field}: {error}"))
}
pub(super) fn java_kagemusha_authenticated_artifact_set_v4_fields(
    artifact_set: &iroha_torii_shared::offline_api::OfflineAuthenticatedArtifactSet,
) -> Result<Vec<Vec<u8>>, String> {
    if !iroha_data_model::offline::is_kagemusha_portable_identifier(&artifact_set.generation) {
        return Err("artifactSet.generation is not a portable V4 identifier".to_owned());
    }
    let manifest =
        java_kagemusha_lower_hex_32(&artifact_set.manifest_sha256, "artifactSet.manifestSha256")?;
    let release_policy = java_kagemusha_lower_hex_32(
        &artifact_set.release_policy_sha256,
        "artifactSet.releasePolicySha256",
    )?;
    let release_attestation = java_kagemusha_lower_hex_32(
        &artifact_set.release_attestation_sha256,
        "artifactSet.releaseAttestationSha256",
    )?;
    if manifest == release_policy
        || manifest == release_attestation
        || release_policy == release_attestation
    {
        return Err("artifactSet digests must be pairwise distinct".to_owned());
    }
    if artifact_set.activation_height == 0
        || artifact_set.withdrawal_height <= artifact_set.activation_height
    {
        return Err("artifactSet has an invalid activation window".to_owned());
    }
    if artifact_set.max_proof_bytes == 0
        || artifact_set.max_proof_bytes
            > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
    {
        return Err("artifactSet.maxProofBytes exceeds the ABI22 V4 release limit".to_owned());
    }
    if artifact_set.asset_scale > iroha_data_model::offline::KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
        return Err("artifactSet.assetScale exceeds the offline payment limit".to_owned());
    }
    Ok(vec![
        artifact_set.generation.as_bytes().to_vec(),
        manifest,
        release_policy,
        release_attestation,
        artifact_set.activation_height.to_string().into_bytes(),
        artifact_set.withdrawal_height.to_string().into_bytes(),
        artifact_set.max_proof_bytes.to_string().into_bytes(),
        artifact_set.asset_scale.to_string().into_bytes(),
    ])
}
pub(super) fn java_kagemusha_validate_exact_readiness_verifier_role(
    verifier: Option<&iroha_torii_shared::offline_api::OfflineActiveTransferVerifier>,
    field: &str,
    expected_name: &str,
    expected_circuit_id: &str,
) -> Result<(), String> {
    let Some(verifier) = verifier else {
        return Ok(());
    };
    if verifier.id.backend != iroha_core::zk::ZK_BACKEND_HALO2_IPA
        || verifier.id.name != expected_name
        || verifier.circuit_id != expected_circuit_id
    {
        return Err(format!(
            "{field} does not identify its exact production verifier role and circuit"
        ));
    }
    Ok(())
}
pub(super) fn java_kagemusha_project_readiness_v4_fields(
    readiness: iroha_torii_shared::offline_api::OfflineReadiness,
) -> Result<Vec<Vec<u8>>, String> {
    if readiness.cash_handoff_capability
        != iroha_data_model::offline::KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1
    {
        return Err(
            "cash handoff capability must be the exact cash_handoff_v1 contract".to_owned(),
        );
    }
    if readiness.required_bridge_abi_version
        != iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
    {
        return Err("required bridge ABI must be 22 for the ABI22/V4 contract".to_owned());
    }
    if readiness.max_hops != iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2 {
        return Err("maximum hop count does not match the protocol contract".to_owned());
    }
    let parsed_asset_definition =
        AssetDefinitionId::parse_address_literal(&readiness.asset_definition_id)
            .map_err(|_| "asset definition id is not a canonical address literal".to_owned())?;
    if parsed_asset_definition.to_string() != readiness.asset_definition_id {
        return Err("asset definition id is not a canonical address literal".to_owned());
    }
    let block_hash =
        java_kagemusha_lower_hex_32(&readiness.evaluated_block_hash, "evaluatedBlockHash")?;
    for (verifier, field, expected_name, expected_circuit_id) in [
        (
            readiness.active_transfer_verifier.as_ref(),
            "activeTransferVerifier",
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
            iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        ),
        (
            readiness.active_topup_shield_verifier.as_ref(),
            "activeTopUpShieldVerifier",
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
            iroha_core::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
        ),
        (
            readiness.active_unshield_verifier.as_ref(),
            "activeUnshieldVerifier",
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
            iroha_core::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
        ),
        (
            readiness.active_recursive_step_eq_verifier.as_ref(),
            "activeRecursiveStepEqVerifier",
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        ),
        (
            readiness.active_recursive_step_ep_verifier.as_ref(),
            "activeRecursiveStepEpVerifier",
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        ),
    ] {
        java_kagemusha_validate_exact_readiness_verifier_role(
            verifier,
            field,
            expected_name,
            expected_circuit_id,
        )?;
    }
    let transfer = java_kagemusha_readiness_verifier_archive(
        readiness.active_transfer_verifier.as_ref(),
        readiness.evaluated_block_height,
        "activeTransferVerifier",
    )?;
    let top_up = java_kagemusha_readiness_verifier_archive(
        readiness.active_topup_shield_verifier.as_ref(),
        readiness.evaluated_block_height,
        "activeTopUpShieldVerifier",
    )?;
    let unshield = java_kagemusha_readiness_verifier_archive(
        readiness.active_unshield_verifier.as_ref(),
        readiness.evaluated_block_height,
        "activeUnshieldVerifier",
    )?;
    let step_eq = java_kagemusha_readiness_verifier_archive(
        readiness.active_recursive_step_eq_verifier.as_ref(),
        readiness.evaluated_block_height,
        "activeRecursiveStepEqVerifier",
    )?;
    let step_ep = java_kagemusha_readiness_verifier_archive(
        readiness.active_recursive_step_ep_verifier.as_ref(),
        readiness.evaluated_block_height,
        "activeRecursiveStepEpVerifier",
    )?;
    let active_verifiers = [
        readiness.active_transfer_verifier.as_ref(),
        readiness.active_topup_shield_verifier.as_ref(),
        readiness.active_unshield_verifier.as_ref(),
        readiness.active_recursive_step_eq_verifier.as_ref(),
        readiness.active_recursive_step_ep_verifier.as_ref(),
    ];
    let mut verifier_ids = std::collections::BTreeSet::new();
    let mut commitments = std::collections::BTreeSet::new();
    let mut schema_hashes = std::collections::BTreeSet::new();
    for verifier in active_verifiers.into_iter().flatten() {
        if !verifier_ids.insert((verifier.id.backend.as_str(), verifier.id.name.as_str()))
            || !commitments.insert(verifier.commitment.as_str())
            || !schema_hashes.insert(verifier.public_inputs_schema_hash.as_str())
        {
            return Err("readiness reuses verifier identity across production roles".to_owned());
        }
    }
    let recursive_pair_present = !step_eq.is_empty() && !step_ep.is_empty();
    if step_eq.is_empty() != step_ep.is_empty() {
        return Err("ABI22 V4 recursive verifiers must be reported atomically".to_owned());
    }
    let artifact_set = match readiness.artifact_set.as_ref() {
        None => {
            if recursive_pair_present {
                return Err("artifactSet is required with the ABI22 V4 verifier pair".to_owned());
            }
            if readiness.proof_backend_available {
                return Err(
                    "proofBackendAvailable requires an authenticated artifactSet".to_owned(),
                );
            }
            Vec::new()
        }
        Some(artifact_set) => {
            if !recursive_pair_present {
                return Err("artifactSet requires the ABI22 V4 verifier pair".to_owned());
            }
            java_kagemusha_authenticated_artifact_set_v4_fields(artifact_set)?;
            if artifact_set.activation_height > readiness.evaluated_block_height
                || artifact_set.withdrawal_height <= readiness.evaluated_block_height
            {
                return Err("artifactSet is not active at the readiness height".to_owned());
            }
            if readiness.asset_scale != Some(artifact_set.asset_scale) {
                return Err("artifactSet.assetScale does not bind the live asset scale".to_owned());
            }
            for (field, verifier) in [
                (
                    "activeRecursiveStepEqVerifier",
                    readiness.active_recursive_step_eq_verifier.as_ref(),
                ),
                (
                    "activeRecursiveStepEpVerifier",
                    readiness.active_recursive_step_ep_verifier.as_ref(),
                ),
            ] {
                let verifier = verifier.ok_or_else(|| {
                    format!("{field} is required with the authenticated artifact set")
                })?;
                if verifier.max_proof_bytes != artifact_set.max_proof_bytes
                    || verifier.activation_height != artifact_set.activation_height
                    || verifier.withdrawal_height != Some(artifact_set.withdrawal_height)
                {
                    return Err(format!(
                        "{field} does not bind the artifactSet limit and activation window"
                    ));
                }
            }
            norito::to_bytes(artifact_set)
                .map_err(|error| format!("failed to encode artifactSet: {error}"))?
        }
    };
    let mut blocker_codes = std::collections::BTreeSet::new();
    for blocker in &readiness.blockers {
        let code = blocker.code.as_bytes();
        let code_is_canonical = (1..=64).contains(&code.len())
            && code[0].is_ascii_alphanumeric()
            && code
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_');
        if !code_is_canonical
            || blocker.message.is_empty()
            || blocker.message.trim() != blocker.message
            || blocker.message.chars().count() > 1024
            || blocker.message.chars().any(char::is_control)
            || !blocker_codes.insert(blocker.code.as_str())
        {
            return Err("readiness contains a malformed or duplicate blocker".to_owned());
        }
    }
    let blocker_matches_absence = |code: &str, absent: bool| blocker_codes.contains(code) == absent;
    if !blocker_matches_absence("asset_scale_unavailable", readiness.asset_scale.is_none())
        || !blocker_matches_absence(
            "asset_scale_unsupported",
            readiness.asset_scale.is_some_and(|scale| {
                scale > iroha_data_model::offline::KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            }),
        )
        || !blocker_matches_absence("transfer_verifier_unavailable", transfer.is_empty())
        || !blocker_matches_absence("topup_shield_verifier_unavailable", top_up.is_empty())
        || !blocker_matches_absence("unshield_verifier_unavailable", unshield.is_empty())
        || !blocker_matches_absence("recursive_step_eq_verifier_unavailable", step_eq.is_empty())
        || !blocker_matches_absence("recursive_step_ep_verifier_unavailable", step_ep.is_empty())
        || !blocker_matches_absence(
            "proof_backend_unavailable",
            !readiness.proof_backend_available,
        )
    {
        return Err("readiness availability fields contradict the blocker set".to_owned());
    }
    let registry_blocker_count = [
        "recursive_v4_registry_unavailable",
        "recursive_v4_registry_malformed",
    ]
    .into_iter()
    .filter(|code| blocker_codes.contains(code))
    .count();
    if (readiness.artifact_set.is_none() && registry_blocker_count != 1)
        || (readiness.artifact_set.is_some() && registry_blocker_count != 0)
    {
        return Err("artifactSet contradicts the ABI22 V4 registry blocker set".to_owned());
    }
    let expected_recursive_lineage_supported = readiness.proof_backend_available
        && readiness.artifact_set.is_some()
        && !step_eq.is_empty()
        && !step_ep.is_empty();
    if readiness.recursive_lineage_supported != expected_recursive_lineage_supported {
        return Err(
            "recursiveLineageSupported must equal the exact authenticated ABI22 lineage conjunction"
                .to_owned(),
        );
    }
    if readiness.recursive_lineage_supported
        == blocker_codes.contains("recursive_lineage_unavailable")
    {
        return Err("recursiveLineageSupported contradicts the blocker set".to_owned());
    }
    let expected_ready = readiness.proof_backend_available
        && readiness.recursive_lineage_supported
        && readiness.artifact_set.is_some()
        && readiness.asset_scale.is_some_and(|scale| {
            scale <= iroha_data_model::offline::KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
        })
        && !transfer.is_empty()
        && !top_up.is_empty()
        && !unshield.is_empty()
        && !step_eq.is_empty()
        && !step_ep.is_empty()
        && readiness.blockers.is_empty();
    if readiness.ready != expected_ready {
        return Err("ready must equal the complete ABI22 runtime conjunction".to_owned());
    }
    let mut fields = vec![
        readiness.cash_handoff_capability.into_bytes(),
        readiness
            .required_bridge_abi_version
            .to_string()
            .into_bytes(),
        readiness.max_hops.to_string().into_bytes(),
        readiness.asset_definition_id.into_bytes(),
        readiness
            .asset_scale
            .map(|scale| scale.to_string().into_bytes())
            .unwrap_or_default(),
        readiness.evaluated_block_height.to_string().into_bytes(),
        block_hash,
        vec![u8::from(readiness.proof_backend_available)],
        vec![u8::from(readiness.recursive_lineage_supported)],
        vec![u8::from(readiness.ready)],
        transfer,
        top_up,
        unshield,
        step_eq,
        step_ep,
        artifact_set,
        readiness.blockers.len().to_string().into_bytes(),
    ];
    for blocker in readiness.blockers {
        fields.push(blocker.code.into_bytes());
        fields.push(blocker.message.into_bytes());
    }
    Ok(fields)
}
pub(super) fn java_native_kagemusha_project_readiness_v4(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "readiness projection", |env| {
        let readiness = java_kagemusha_decode_archive::<
            iroha_torii_shared::offline_api::OfflineReadiness,
        >(env, &archive, "readiness")?;
        let fields = java_kagemusha_project_readiness_v4_fields(readiness)?;
        java_kagemusha_byte_arrays(env, &fields)
    })
}
pub(super) fn java_native_kagemusha_project_authenticated_artifact_set_v4(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "authenticated artifact-set projection", |env| {
        let artifact_set = java_kagemusha_decode_archive::<
            iroha_torii_shared::offline_api::OfflineAuthenticatedArtifactSet,
        >(env, &archive, "artifactSet")?;
        let fields = java_kagemusha_authenticated_artifact_set_v4_fields(&artifact_set)?;
        java_kagemusha_byte_arrays(env, &fields)
    })
}
pub(super) fn java_native_kagemusha_project_active_verifier_v2(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "active verifier projection", |env| {
        let verifier = java_kagemusha_decode_archive::<
            iroha_torii_shared::offline_api::OfflineActiveTransferVerifier,
        >(env, &archive, "activeVerifier")?;
        // Window membership is checked by projectReadiness; this projection still rejects a
        // structurally impossible window when used by future internal callers.
        if verifier
            .withdrawal_height
            .is_some_and(|withdrawal| withdrawal <= verifier.activation_height)
        {
            return Err("active verifier has an invalid activation window".to_owned());
        }
        let commitment = java_kagemusha_lower_hex_32(&verifier.commitment, "commitment")?;
        let schema = java_kagemusha_lower_hex_32(
            &verifier.public_inputs_schema_hash,
            "publicInputsSchemaHash",
        )?;
        java_kagemusha_byte_arrays(
            env,
            &[
                verifier.id.backend.into_bytes(),
                verifier.id.name.into_bytes(),
                verifier.version.to_string().into_bytes(),
                verifier.circuit_id.into_bytes(),
                commitment,
                schema,
                verifier.max_proof_bytes.to_string().into_bytes(),
                verifier.activation_height.to_string().into_bytes(),
                verifier
                    .withdrawal_height
                    .map(|height| height.to_string().into_bytes())
                    .unwrap_or_default(),
            ],
        )
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_prepare_authorization_v2(
    env: &mut jni::JNIEnv<'_>,
    authority: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    device_id: jni::objects::JByteArray<'_>,
    asset_definition_id: jni::objects::JByteArray<'_>,
    operation_id: jni::objects::JByteArray<'_>,
    issued_at_ms: jni::sys::jlong,
    expires_at_ms: jni::sys::jlong,
    nonce: jni::objects::JByteArray<'_>,
    payload_digest: jni::objects::JByteArray<'_>,
    registration_hash: jni::objects::JByteArray<'_>,
    hardware_assertion_platform: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "authorization preparation", |env| {
        let chain_discriminant = u16::try_from(chain_discriminant)
            .map_err(|_| "chainDiscriminant must fit in u16".to_owned())?;
        let authority = parse_account_id_for_chain(
            java_kagemusha_text(env, &authority, "authority")?,
            chain_discriminant,
        )
        .map_err(|_| "authority must be a canonical account address".to_owned())?;
        let device_id = java_kagemusha_text(env, &device_id, "deviceId")?;
        if device_id.len() > 128 {
            return Err("deviceId exceeds 128 bytes".to_owned());
        }
        let asset_definition_id = parse_asset_definition(java_kagemusha_text(
            env,
            &asset_definition_id,
            "assetDefinitionId",
        )?)
        .map_err(|_| "assetDefinitionId must be canonical".to_owned())?;
        let operation_id = java_kagemusha_fixed32(env, &operation_id, "operationId")?;
        let issued_at_ms = u64::try_from(issued_at_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "issuedAtMilliseconds must be positive".to_owned())?;
        let expires_at_ms = u64::try_from(expires_at_ms)
            .ok()
            .filter(|value| {
                *value > issued_at_ms
                    && *value - issued_at_ms
                        <= iroha_data_model::offline::KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2
            })
            .ok_or_else(|| "expiresAtMilliseconds is outside the authorization TTL".to_owned())?;
        let nonce = java_kagemusha_fixed32(env, &nonce, "nonce")?;
        let payload_digest = java_kagemusha_fixed32(env, &payload_digest, "payloadDigest")?;
        let registration_hash =
            java_kagemusha_fixed32(env, &registration_hash, "registrationHash")?;
        let platform = java_kagemusha_text(
            env,
            &hardware_assertion_platform,
            "hardwareAssertionPlatform",
        )?;
        let platform = KagemushaRequestAuthorizationPlatformV2::parse(&platform)
            .map_err(|_| "hardwareAssertionPlatform is unsupported".to_owned())?;
        let preparation = KagemushaRequestAuthorizationPreparationV2 {
            version: KAGEMUSHA_REQUEST_AUTHORIZATION_PREPARATION_VERSION_V2,
            authority,
            device_id,
            asset_definition_id,
            operation_id,
            issued_at_ms,
            expires_at_ms,
            nonce,
            payload_digest,
            registration_hash,
            platform,
        };
        preparation
            .validate()
            .map_err(|_| "authorization preparation fields are invalid".to_owned())?;
        let signing_bytes = preparation
            .signing_bytes()
            .map_err(|_| "failed to derive authorization signing bytes".to_owned())?;
        let preparation_archive = norito::to_bytes(&preparation)
            .map_err(|error| format!("failed to encode authorization preparation: {error}"))?;
        java_kagemusha_byte_arrays(
            env,
            &[
                preparation_archive,
                signing_bytes,
                operation_id.to_vec(),
                payload_digest.to_vec(),
                registration_hash.to_vec(),
            ],
        )
    })
}
pub(super) fn java_native_kagemusha_finalize_hardware_authorization_v2(
    env: &mut jni::JNIEnv<'_>,
    preparation: jni::objects::JByteArray<'_>,
    authenticator_data: jni::objects::JByteArray<'_>,
    signature_der: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "authorization signing", |env| {
        let preparation =
            java_kagemusha_decode_archive_bounded::<KagemushaRequestAuthorizationPreparationV2>(
                env,
                &preparation,
                "authorizationPreparation",
                KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
            )?;
        preparation
            .validate()
            .map_err(|_| "authorization preparation is invalid".to_owned())?;
        let authenticator_data =
            read_java_byte_array(env, &authenticator_data, "authenticatorData")
                .ok_or_else(|| "authenticatorData must be bytes".to_owned())?;
        if authenticator_data.len()
            > iroha_data_model::offline::KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MAX_BYTES_V1
        {
            return Err("authenticatorData exceeds the protocol bound".to_owned());
        }
        let signature_der = read_java_byte_array_bounded(env, &signature_der, "signatureDer", 72)
            .ok_or_else(|| "signatureDer must be strict DER bytes".to_owned())?;
        let signature = kagemusha_device_signature_from_strict_der_v2(&signature_der)
            .map_err(|_| "signatureDer is not strict P-256 DER".to_owned())?;
        let authorization = preparation
            .finalize(authenticator_data, signature)
            .map_err(|_| "authorization platform result or binding was rejected".to_owned())?;
        let archive = norito::to_bytes(&authorization)
            .map_err(|error| format!("failed to encode authorization: {error}"))?;
        java_kagemusha_byte_arrays(env, &[archive, signature.as_raw_bytes().to_vec()])
    })
}
pub(super) fn java_native_kagemusha_finalize_ios_app_attest_authorization_v2(
    env: &mut jni::JNIEnv<'_>,
    preparation: jni::objects::JByteArray<'_>,
    assertion_object: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "App Attest authorization finalization", |env| {
        let preparation = read_java_byte_array_bounded(
            env,
            &preparation,
            "authorizationPreparation",
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
        )
        .ok_or_else(|| "authorizationPreparation must be bounded bytes".to_owned())?;
        let assertion_object = read_java_byte_array_bounded(
            env,
            &assertion_object,
            "assertionObject",
            KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_OBJECT_MAX_BYTES_V1,
        )
        .ok_or_else(|| "assertionObject must be bounded bytes".to_owned())?;
        let (archive, signature_raw, authenticator_data) =
            kagemusha_finalize_ios_app_attest_authorization_archive_v2(
                &preparation,
                &assertion_object,
            )
            .map_err(|_| {
                "assertionObject is not an exact App Attest assertion for this preparation"
                    .to_owned()
            })?;
        java_kagemusha_byte_arrays(env, &[archive, signature_raw, authenticator_data])
    })
}
pub(super) fn java_native_kagemusha_finalize_top_up_v4(
    env: &mut jni::JNIEnv<'_>,
    unsigned: jni::objects::JByteArray<'_>,
    authorization: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "top-up finalization", |env| {
        let unsigned = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendTopUpUnsignedV4,
        >(
            env,
            &unsigned,
            "topUpUnsigned",
            KAGEMUSHA_RECURSIVE_SPEND_TOPUP_MAX_BYTES_V4,
        )?;
        let authorization = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRequestAuthorizationV2,
        >(
            env,
            &authorization,
            "authorization",
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
        )?;
        let request = unsigned
            .into_request(authorization)
            .map_err(|_| "authorization does not bind the top-up payload".to_owned())?;
        let archive = norito::to_bytes(&request)
            .map_err(|error| format!("failed to encode top-up request: {error}"))?;
        if archive.len() > KAGEMUSHA_RECURSIVE_SPEND_TOPUP_MAX_BYTES_V4 {
            return Err("top-up request exceeds the V4 archive limit".to_owned());
        }
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_finalize_redeem_v4(
    env: &mut jni::JNIEnv<'_>,
    build_result: jni::objects::JByteArray<'_>,
    authorization: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "redeem finalization", |env| {
        let build_result = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendRedeemBuildResultV4,
        >(
            env,
            &build_result,
            "redeemBuildResult",
            KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4,
        )?;
        let authorization = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRequestAuthorizationV2,
        >(
            env,
            &authorization,
            "authorization",
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
        )?;
        let result = build_result
            .into_redeem_result(authorization)
            .map_err(|_| "authorization does not bind the redeem payload".to_owned())?;
        // Do not make wallet code unwrap an opaque result archive: project the canonical Torii
        // request and stable idempotency key directly.
        let request = decode_canonical_kagemusha_recursive_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV4,
        >(&result.redeem_request_archive)
        .map_err(|_| "native redeem result contains a non-canonical request".to_owned())?;
        request
            .validate_public_binding()
            .map_err(|_| "finalized redeem request binding is invalid".to_owned())?;
        let request_archive = norito::to_bytes(&request)
            .map_err(|error| format!("failed to encode redeem request: {error}"))?;
        if request_archive.len()
            > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4
        {
            return Err("redeem request exceeds the V4 archive limit".to_owned());
        }
        java_kagemusha_byte_arrays(env, &[request_archive, result.operation_id.to_vec()])
    })
}
pub(super) fn java_kagemusha_bridge_failure(
    label: &str,
    error: BridgeError,
) -> JavaKagemushaLifecycleFailure {
    match error {
        BridgeError::KagemushaBusy => JavaKagemushaLifecycleFailure::Unavailable(format!(
            "Kagemusha {label} is busy; retry after the active proof completes"
        )),
        BridgeError::KagemushaRecursiveSpendV4Unavailable => {
            JavaKagemushaLifecycleFailure::Unavailable(format!(
                "Kagemusha {label} proof backend is unavailable"
            ))
        }
        BridgeError::KagemushaRecursiveSpendV4Artifact => {
            JavaKagemushaLifecycleFailure::Unavailable(format!(
                "Kagemusha {label} artifact set is unavailable or does not match the request"
            ))
        }
        _ => JavaKagemushaLifecycleFailure::Invalid(format!(
            "Kagemusha {label} request or proof binding was rejected"
        )),
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_prepare_top_up_v4(
    env: &mut jni::JNIEnv<'_>,
    network_id: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    asset_definition: jni::objects::JByteArray<'_>,
    payer: jni::objects::JByteArray<'_>,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    operation_id: jni::objects::JByteArray<'_>,
    spend_key: jni::objects::JByteArray<'_>,
    rho: jni::objects::JByteArray<'_>,
    diversifier: jni::objects::JByteArray<'_>,
    leaf_index: jni::sys::jint,
    flattened_siblings: jni::objects::JByteArray<'_>,
    directions: jni::objects::JByteArray<'_>,
    root: jni::objects::JByteArray<'_>,
    shield_verifier_commitment: jni::objects::JByteArray<'_>,
    artifact_binding: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, JavaKagemushaLifecycleFailure> {
        let _permit = try_preacquire_kagemusha_heavy_proof_permit_v4()
            .map_err(|error| java_kagemusha_bridge_failure("top-up", error))?;
        let invalid = |message: String| JavaKagemushaLifecycleFailure::Invalid(message);
        let network_id = java_network_id(env, &network_id).map_err(invalid)?;
        let (asset_definition, balance_scope) = parse_asset_definition_with_balance_scope(
            java_kagemusha_text(env, &asset_definition, "assetDefinitionId").map_err(invalid)?,
        )
        .map_err(|_| {
            JavaKagemushaLifecycleFailure::Invalid(
                "assetDefinitionId must be a canonical address with optional balance scope"
                    .to_owned(),
            )
        })?;
        let chain_discriminant = u16::try_from(chain_discriminant).map_err(|_| {
            JavaKagemushaLifecycleFailure::Invalid("chainDiscriminant must fit in u16".to_owned())
        })?;
        let payer = parse_account_id_for_chain(
            java_kagemusha_text(env, &payer, "payer").map_err(invalid)?,
            chain_discriminant,
        )
        .map_err(|_| {
            JavaKagemushaLifecycleFailure::Invalid(
                "payer must be a canonical account address".to_owned(),
            )
        })?;
        let asset = AssetId::with_scope(asset_definition, payer.clone(), balance_scope);
        let amount = java_kagemusha_amount(env, &atomic_units, scale).map_err(invalid)?;
        let operation_id =
            java_kagemusha_fixed32(env, &operation_id, "operationId").map_err(invalid)?;
        let mut opening =
            java_kagemusha_note_opening_v2(env, &spend_key, &rho, &diversifier).map_err(invalid)?;
        let opening_archive = Zeroizing::new(norito::to_bytes(&opening).map_err(|error| {
            JavaKagemushaLifecycleFailure::Invalid(format!(
                "failed to encode note opening: {error}"
            ))
        })?);
        let leaf_index = u32::try_from(leaf_index).map_err(|_| {
            JavaKagemushaLifecycleFailure::Invalid("leafIndex must be non-negative".to_owned())
        })?;
        let sibling_bytes = Zeroizing::new(
            read_java_byte_array(env, &flattened_siblings, "siblings").ok_or_else(|| {
                JavaKagemushaLifecycleFailure::Invalid("siblings must be bytes".to_owned())
            })?,
        );
        let tree_depth = iroha_data_model::offline::KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2;
        if sibling_bytes.len() != tree_depth * 32 {
            opening.zeroize();
            return Err(JavaKagemushaLifecycleFailure::Invalid(format!(
                "siblings must contain exactly {tree_depth} 32-byte nodes"
            )));
        }
        let siblings = Zeroizing::new(
            sibling_bytes
                .chunks_exact(32)
                .map(|chunk| <[u8; 32]>::try_from(chunk).expect("32-byte chunk"))
                .collect::<Vec<_>>(),
        );
        let directions = Zeroizing::new(
            read_java_byte_array(env, &directions, "directions").ok_or_else(|| {
                JavaKagemushaLifecycleFailure::Invalid("directions must be bytes".to_owned())
            })?,
        );
        let root = java_kagemusha_fixed32(env, &root, "root").map_err(invalid)?;
        let mut canonical_path = KagemushaConfidentialMerklePathV2 {
            siblings: siblings.to_vec(),
            directions: directions.to_vec(),
            root,
        };
        let path_validation = canonical_path
            .validate_for_leaf_index(leaf_index)
            .map_err(|_| {
                JavaKagemushaLifecycleFailure::Invalid(
                    "next-zero path shape, directions, or leaf index is invalid".to_owned(),
                )
            });
        canonical_path.siblings.zeroize();
        canonical_path.directions.zeroize();
        canonical_path.root.zeroize();
        path_validation?;
        let shield_verifier_commitment =
            java_kagemusha_fixed32(env, &shield_verifier_commitment, "shieldVerifierCommitment")
                .map_err(invalid)?;
        let artifact_binding = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
        >(env, &artifact_binding, "artifactBinding")
        .map_err(invalid)?;
        artifact_binding.validate().map_err(|_| {
            JavaKagemushaLifecycleFailure::Invalid("artifact binding is invalid".to_owned())
        })?;
        let mut request = KagemushaTopUpShieldBuildRequestV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
            network_id,
            asset,
            amount,
            payer,
            operation_id,
            opening,
            leaf_index,
            zero_path: KagemushaTopUpZeroPathV2 {
                siblings: siblings.to_vec(),
                directions: directions.to_vec(),
                root,
            },
            shield_verifier_id: VerifyingKeyId::new(
                iroha_core::zk::ZK_BACKEND_HALO2_IPA,
                iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
            ),
            shield_verifier_commitment,
            artifact_binding,
        };
        let request_archive = Zeroizing::new(norito::to_bytes(&request).map_err(|error| {
            JavaKagemushaLifecycleFailure::Invalid(format!(
                "failed to encode local top-up request: {error}"
            ))
        })?);
        request.zeroize();
        let unsigned = kagemusha_topup_shield_build_unsigned_from_archive_v4(&request_archive)
            .map_err(|error| java_kagemusha_bridge_failure("top-up", error))?;
        let payload_digest = unsigned.digest().map_err(|_| {
            JavaKagemushaLifecycleFailure::Invalid(
                "top-up unsigned payload digest failed".to_owned(),
            )
        })?;
        let unsigned_archive = norito::to_bytes(&unsigned).map_err(|error| {
            JavaKagemushaLifecycleFailure::Invalid(format!(
                "failed to encode unsigned top-up: {error}"
            ))
        })?;
        let mut fields = [
            unsigned_archive,
            payload_digest.to_vec(),
            opening_archive.to_vec(),
            unsigned.current_note.note_commitment.to_vec(),
            unsigned.current_note.spend_nullifier.to_vec(),
            unsigned.shield_evidence.initial_root.to_vec(),
            unsigned.shield_evidence.finalized_root.to_vec(),
            operation_id.to_vec(),
            amount.atomic_units.to_string().into_bytes(),
            amount.scale.to_string().into_bytes(),
            leaf_index.to_string().into_bytes(),
        ];
        java_kagemusha_secret_byte_arrays(env, &mut fields)
            .map_err(JavaKagemushaLifecycleFailure::Unavailable)
    })();
    match result {
        Ok(fields) => fields,
        Err(JavaKagemushaLifecycleFailure::Invalid(message)) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
        Err(JavaKagemushaLifecycleFailure::Unavailable(message)) => {
            throw_java_illegal_state(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_kagemusha_project_operation_status_v4(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "operation status projection", |env| {
        use iroha_torii_shared::offline_api::{
            OfflineOperationKind, OfflineOperationResult, OfflineOperationStatus,
        };
        let status = java_kagemusha_decode_archive::<OfflineOperationStatus>(
            env,
            &archive,
            "operationStatus",
        )?;
        let fields = match status {
            OfflineOperationStatus::Pending {
                operation_id,
                kind,
                transaction_hash,
                submitted_at_ms,
            } => vec![
                b"pending".to_vec(),
                match kind {
                    OfflineOperationKind::TopUp => b"top_up".to_vec(),
                    OfflineOperationKind::Redeem => b"redeem".to_vec(),
                },
                java_kagemusha_lower_hex_32(&operation_id, "operationId")?,
                java_kagemusha_lower_hex_32(&transaction_hash, "transactionHash")?,
                submitted_at_ms.to_string().into_bytes(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            OfflineOperationStatus::Applied {
                operation_id,
                result: OfflineOperationResult::TopUp(result),
            } => vec![
                b"applied".to_vec(),
                b"top_up".to_vec(),
                java_kagemusha_lower_hex_32(&operation_id, "operationId")?,
                java_kagemusha_lower_hex_32(&result.transaction_hash, "transactionHash")?,
                result.finalized_block_height.to_string().into_bytes(),
                result.server_time_ms.to_string().into_bytes(),
                norito::to_bytes(&result.anchor).map_err(|error| {
                    format!("failed to encode finalized top-up anchor: {error}")
                })?,
                norito::to_bytes(&result.finality_proof)
                    .map_err(|error| format!("failed to encode top-up finality proof: {error}"))?,
                Vec::new(),
                Vec::new(),
            ],
            OfflineOperationStatus::Applied {
                operation_id,
                result: OfflineOperationResult::Redeem(result),
            } => vec![
                b"applied".to_vec(),
                b"redeem".to_vec(),
                java_kagemusha_lower_hex_32(&operation_id, "operationId")?,
                java_kagemusha_lower_hex_32(&result.transaction_hash, "transactionHash")?,
                result.finalized_block_height.to_string().into_bytes(),
                result.server_time_ms.to_string().into_bytes(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            OfflineOperationStatus::Rejected {
                operation_id,
                kind,
                transaction_hash,
                error,
            } => {
                if error.code.is_empty()
                    || error.code.trim() != error.code
                    || error.code.chars().any(char::is_control)
                    || error.message.is_empty()
                    || error.message.trim() != error.message
                    || error.message.chars().any(char::is_control)
                {
                    return Err("rejected status contains a malformed error".to_owned());
                }
                vec![
                    b"rejected".to_vec(),
                    match kind {
                        OfflineOperationKind::TopUp => b"top_up".to_vec(),
                        OfflineOperationKind::Redeem => b"redeem".to_vec(),
                    },
                    java_kagemusha_lower_hex_32(&operation_id, "operationId")?,
                    java_kagemusha_lower_hex_32(&transaction_hash, "transactionHash")?,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    error.code.into_bytes(),
                    error.message.into_bytes(),
                ]
            }
        };
        java_kagemusha_byte_arrays(env, &fields)
    })
}
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
