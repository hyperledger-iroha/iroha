//! Bounded typed privacy instruction projection for Kotlin ordered-intent hashing.

use super::*;

/// Decode one complete canonical instruction and produce only its proof-free preimage.
fn project_instruction(archive: &[u8]) -> Option<Vec<u8>> {
    let maximum = SubmitPrivacyProofV1::canonical_frame_byte_limit_v1();
    if archive.is_empty() || archive.len() > maximum {
        return None;
    }
    let base = norito::canonical_decode_limits(archive.len());
    let limits = norito::DecodeLimits::new(
        base.max_sequence_elements(),
        base.max_field_bytes(),
        base.max_total_elements(),
        base.max_total_allocated_bytes(),
        64,
    );
    let instruction =
        norito::decode_canonical_with_limits::<SubmitPrivacyProofV1>(archive, limits).ok()?;
    if instruction.envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1
        || instruction.envelope.proof.protocol_id()
            == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1
        || matches!(
            &instruction.envelope.statement,
            iroha_data_model::privacy::PrivacyStatementV1::IrohaZkX509StarkP256V1(_)
        )
    {
        return None;
    }
    let projection = instruction.ordered_intent_projection_bytes_v1().ok()?;
    (projection.len() <= maximum).then_some(projection)
}

/// Return an intent-only instruction projection; null means malformed or unavailable input.
pub(super) fn java_native_submit_proof_intent_projection(
    env: &mut jni::JNIEnv<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    if archive.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(length) = env.get_array_length(&archive) else {
        return std::ptr::null_mut();
    };
    if length <= 0
        || usize::try_from(length).map_or(true, |length| {
            length > SubmitPrivacyProofV1::canonical_frame_byte_limit_v1()
        })
    {
        return std::ptr::null_mut();
    }
    let Ok(mut input) = env.convert_byte_array(&archive) else {
        return std::ptr::null_mut();
    };
    let projection = project_instruction(&input);
    input.fill(0);
    let Some(mut projection) = projection else {
        return std::ptr::null_mut();
    };
    let result = env
        .byte_array_from_slice(&projection)
        .map_or(std::ptr::null_mut(), |array| array.into_raw());
    projection.fill(0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_exact12_projections_match_the_typed_model_and_reject_malformed_frames() {
        let fixtures =
            iroha_data_model::privacy::privacy_exact12_fixture_bundle_v1().expect("fixtures");
        for row in fixtures.rows {
            let input = row.submit_proof_instruction_norito;
            let x509 = row.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
            let project = |archive: &[u8]| {
                if x509 {
                    privacy_x509_request::inspect_request(archive).map(|fields| fields[0].clone())
                } else {
                    project_instruction(archive)
                }
            };
            if x509 {
                let typed = norito::decode_canonical::<SubmitPrivacyZkX509CertificateV1>(&input)
                    .expect("typed proof-free X509 request");
                assert_eq!(
                    project(&input),
                    Some(norito::encode_canonical(&typed).unwrap())
                );
                assert!(project_instruction(&input).is_none());
                let envelope = norito::decode_canonical(&row.envelope_norito).unwrap();
                let retired = norito::encode_canonical(&SubmitPrivacyProofV1::new(envelope)).unwrap();
                assert!(project_instruction(&retired).is_none());
            } else {
                let typed = norito::decode_canonical::<SubmitPrivacyProofV1>(&input)
                    .expect("typed instruction");
                assert_eq!(
                    project(&input),
                    Some(typed.ordered_intent_projection_bytes_v1().expect("projection"))
                );
            }
            assert!(project(&input[..input.len() - 1]).is_none());
            let mut trailing = input.clone();
            trailing.push(0);
            assert!(project(&trailing).is_none());
            let mut damaged = input;
            *damaged.last_mut().expect("frame") ^= 1;
            assert!(project(&damaged).is_none());
            assert!(project(&row.envelope_norito).is_none());
        }
        assert!(project_instruction(&[]).is_none());
        assert!(
            project_instruction(&vec![
                0;
                SubmitPrivacyProofV1::canonical_frame_byte_limit_v1()
                    + 1
            ])
            .is_none()
        );
    }

    #[test]
    fn generic_projection_refuses_x509_in_each_typed_envelope_component() {
        let fixtures =
            iroha_data_model::privacy::privacy_exact12_fixture_bundle_v1().expect("fixtures");
        let x509 = fixtures
            .rows
            .iter()
            .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
            .unwrap();
        let x509: iroha_data_model::privacy::PrivacyProofEnvelopeV1 =
            norito::decode_canonical(&x509.envelope_norito).unwrap();
        let generic: SubmitPrivacyProofV1 =
            norito::decode_canonical(&fixtures.rows[0].submit_proof_instruction_norito)
                .unwrap();
        for component in 0..3 {
            let mut mixed = generic.clone();
            match component {
                0 => mixed.envelope.protocol_id = x509.protocol_id,
                1 => mixed.envelope.proof = x509.proof.clone(),
                _ => mixed.envelope.statement = x509.statement.clone(),
            }
            assert!(
                project_instruction(&norito::encode_canonical(&mixed).unwrap()).is_none()
            );
        }
    }

    #[test]
    fn instruction_wrapper_accepts_a_ceiling_envelope_and_rejects_one_over() {
        let fixtures =
            iroha_data_model::privacy::privacy_exact12_fixture_bundle_v1().expect("fixtures");
        let instruction: SubmitPrivacyProofV1 =
            norito::decode_canonical(&fixtures.rows[0].submit_proof_instruction_norito)
                .expect("typed instruction");
        let mut envelope = instruction.envelope().clone();
        let envelope_limit = iroha_data_model::privacy::TAIRA_PRIVACY_MAX_ACTION_BYTES_V1 as usize;
        envelope.proof.bytes_mut().bytes = vec![0x5a; envelope_limit];
        let overhead = norito::canonical_frame_len(&envelope).expect("frame") - envelope_limit;
        envelope
            .proof
            .bytes_mut()
            .bytes
            .truncate(envelope_limit - overhead);
        assert_eq!(
            norito::canonical_frame_len(&envelope).unwrap(),
            envelope_limit
        );
        let instruction = SubmitPrivacyProofV1::new(envelope.clone());
        let exact = norito::encode_canonical(&instruction).expect("ceiling instruction");
        assert_eq!(
            exact.len(),
            SubmitPrivacyProofV1::canonical_frame_byte_limit_v1()
        );
        assert!(project_instruction(&exact).is_some());
        envelope.proof.bytes_mut().bytes.push(0x5a);
        let one_over = norito::encode_canonical(&SubmitPrivacyProofV1::new(envelope))
            .expect("one-over instruction");
        assert_eq!(one_over.len(), exact.len() + 1);
        assert!(project_instruction(&one_over).is_none());
    }
}
