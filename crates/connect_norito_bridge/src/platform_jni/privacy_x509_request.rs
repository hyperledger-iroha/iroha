//! Bounded proof-free X509 request inspection and original-action materialization.

use super::{SubmitPrivacyZkX509CertificateV1, catch_unwind_to_java};
use iroha_data_model::{
    privacy::{
        PrivacyCompiledProfileSnapshotV1, PrivacyConsensusLimitsV1, PrivacyProtocolLimitsV1,
        PrivacyStatementV1,
        TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
    },
    transaction::signed_attachments::{
        ActionAttachmentRefV1, ActionIntentV1, OrderedIntentDigestV1,
        SIGNED_ATTACHMENTS_MAX_ACTIONS_V1, SIGNED_ATTACHMENTS_MAX_CANONICAL_FRAME_BYTES_V1,
    },
};
use zeroize::Zeroizing;

/// Largest outer JNI input among the two explicitly selected instruction routes.
pub(super) fn construction_frame_limit() -> usize {
    crate::privacy_construction::construction_frame_limit()
}

fn decode_limits(bytes: usize) -> norito::DecodeLimits {
    let base = norito::canonical_decode_limits(bytes);
    norito::DecodeLimits::new(
        base.max_sequence_elements(),
        base.max_field_bytes(),
        base.max_total_elements(),
        base.max_total_allocated_bytes(),
        64,
    )
}

fn decode_request(archive: &[u8]) -> Option<SubmitPrivacyZkX509CertificateV1> {
    if archive.is_empty()
        || archive.len() > SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1()
    {
        return None;
    }
    let request = norito::decode_canonical_with_limits::<SubmitPrivacyZkX509CertificateV1>(
        archive,
        decode_limits(archive.len()),
    )
    .ok()?;
    request
        .validate_proof_free_v1(&PrivacyConsensusLimitsV1::taira_default())
        .ok()?;
    (usize::try_from(request.proof_ref.action_index).ok()? < SIGNED_ATTACHMENTS_MAX_ACTIONS_V1)
        .then_some(request)
}

fn create_request(statement: &[u8], attachment_action_index: i64) -> Option<Vec<u8>> {
    let action_index = u32::try_from(attachment_action_index).ok()?;
    if usize::try_from(action_index).ok()? >= SIGNED_ATTACHMENTS_MAX_ACTIONS_V1
        || statement.is_empty()
        || statement.len()
            > TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1 as usize
    {
        return None;
    }
    let tagged = norito::decode_canonical_with_limits::<PrivacyStatementV1>(
        statement,
        decode_limits(statement.len()),
    )
    .ok()?;
    let PrivacyStatementV1::IrohaZkX509StarkP256V1(statement) = tagged else {
        return None;
    };
    let request = SubmitPrivacyZkX509CertificateV1::new(
        statement,
        ActionAttachmentRefV1 {
            action_index,
            slot_index: 0,
        },
    );
    request
        .validate_proof_free_v1(&PrivacyConsensusLimitsV1::taira_default())
        .ok()?;
    let frame = norito::encode_canonical(&request).ok()?;
    (frame.len() <= SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1())
        .then_some(frame)
}

/// Inspect only real request fields: frame, attachment ordinal, digest, network and five identities.
/// This result carries no execution authority or native proof-validity claim.
pub(super) fn inspect_request(archive: &[u8]) -> Option<Vec<Vec<u8>>> {
    let request = decode_request(archive)?;
    let context = &request.statement.context;
    Some(vec![
        archive.to_vec(),
        request.proof_ref.action_index.to_le_bytes().to_vec(),
        request.public_inputs_digest_v1().ok()?.as_bytes().to_vec(),
        context.network_id.as_bytes().to_vec(),
        context.parameter_id.as_bytes().to_vec(),
        context.parameter_digest.as_bytes().to_vec(),
        context.verifier_digest.as_bytes().to_vec(),
        context.statement_schema_digest.as_bytes().to_vec(),
        context.engine_manifest_digest.as_bytes().to_vec(),
    ])
}

/// Check a proof-free request against the already admitted current compiled/committed tuple.
pub(super) fn request_admitted(
    archive: &[u8],
    profile: &PrivacyCompiledProfileSnapshotV1,
    protocol_limits: &PrivacyProtocolLimitsV1,
    limits: &PrivacyConsensusLimitsV1,
    expected_network: &[u8; 32],
) -> bool {
    let Some(request) = decode_request(archive) else {
        return false;
    };
    crate::privacy_construction::request_matches_profile(
        &request,
        profile,
        protocol_limits,
        limits,
        expected_network,
    )
}

fn materialize_statement(request: &[u8], action: &[u8], intent: &[u8]) -> Option<Vec<u8>> {
    let request = decode_request(request)?;
    if action.is_empty() || action.len() > SIGNED_ATTACHMENTS_MAX_CANONICAL_FRAME_BYTES_V1 {
        return None;
    }
    let action =
        norito::decode_canonical_with_limits::<ActionIntentV1>(action, decode_limits(action.len()))
            .ok()?;
    let intent = OrderedIntentDigestV1::new(intent.try_into().ok()?);
    // Kotlin derives intent from the original payload and passes that payload's actual action.
    // The model checks operation, role, slot and independent inputs; this grants no authority.
    let statement = request
        .materialize_statement_v1(&action, intent, &PrivacyConsensusLimitsV1::taira_default())
        .ok()?;
    norito::encode_canonical(&PrivacyStatementV1::IrohaZkX509StarkP256V1(statement)).ok()
}

fn read_bounded(
    env: &mut jni::JNIEnv<'_>,
    input: &jni::objects::JByteArray<'_>,
    maximum: usize,
) -> Option<Zeroizing<Vec<u8>>> {
    if input.is_null() {
        return None;
    }
    let length = usize::try_from(env.get_array_length(input).ok()?).ok()?;
    if length == 0 || length > maximum {
        return None;
    }
    Some(Zeroizing::new(env.convert_byte_array(input).ok()?))
}

pub(super) fn java_create_request(
    env: &mut jni::JNIEnv<'_>,
    statement: jni::objects::JByteArray<'_>,
    attachment_action_index: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let Some(statement) = read_bounded(
        env,
        &statement,
        TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1 as usize,
    ) else {
        return std::ptr::null_mut();
    };
    let Some(Some(request)) = catch_unwind_to_java(env, "X509 request construction", || {
        create_request(&statement, attachment_action_index)
    }) else {
        return std::ptr::null_mut();
    };
    env.byte_array_from_slice(&request)
        .map_or(std::ptr::null_mut(), |array| array.into_raw())
}

pub(super) fn java_inspect_request(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let Some(input) = read_bounded(
        env,
        &request,
        SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1(),
    ) else {
        return std::ptr::null_mut();
    };
    let Some(Some(fields)) =
        catch_unwind_to_java(env, "X509 request inspection", || inspect_request(&input))
    else {
        return std::ptr::null_mut();
    };
    (|| -> jni::errors::Result<jni::sys::jobjectArray> {
        let result = env.new_object_array(9, "[B", jni::objects::JObject::null())?;
        for (index, bytes) in fields.iter().enumerate() {
            let value = env.byte_array_from_slice(bytes)?;
            env.set_object_array_element(&result, index as i32, &value)?;
        }
        Ok(result.into_raw())
    })()
    .unwrap_or(std::ptr::null_mut())
}

pub(super) fn java_materialize_statement(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    action: jni::objects::JByteArray<'_>,
    intent: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let Some(request) = read_bounded(
        env,
        &request,
        SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1(),
    ) else {
        return std::ptr::null_mut();
    };
    let Some(action) = read_bounded(
        env,
        &action,
        SIGNED_ATTACHMENTS_MAX_CANONICAL_FRAME_BYTES_V1,
    ) else {
        return std::ptr::null_mut();
    };
    let Some(intent) = read_bounded(env, &intent, 32) else {
        return std::ptr::null_mut();
    };
    let Some(Some(statement)) = catch_unwind_to_java(env, "X509 statement materialization", || {
        materialize_statement(&request, &action, &intent)
    }) else {
        return std::ptr::null_mut();
    };
    env.byte_array_from_slice(&statement)
        .map_or(std::ptr::null_mut(), |array| array.into_raw())
}

#[cfg(test)]
#[path = "privacy_x509_request_tests.rs"]
mod tests;
