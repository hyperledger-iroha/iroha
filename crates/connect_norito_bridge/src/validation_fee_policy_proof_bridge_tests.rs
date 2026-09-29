#[cfg(test)]
mod validation_fee_policy_proof_bridge_tests {
    use super::*;
    use iroha_data_model::{
        hijiri::{FeeMultiplierBand, HijiriFeePolicy, HijiriParametersV1, Q16},
        validation_fee::VALIDATION_FEE_DS_SCALE,
    };
    use iroha_torii_shared::validation_fee_api::{
        VALIDATION_FEE_BASE_MINOR_UNITS_V1,
        VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1, ValidationFeeHijiriQuoteBaseV1,
        evaluate_hijiri_quote_v1,
    };
    use std::ptr;

    fn hijiri_quote_request() -> ValidationFeeHijiriQuoteRequestV1 {
        let key_pair = KeyPair::try_from_seed(vec![0x37; 32], Algorithm::Ed25519)
            .expect("derive Hijiri quote account");
        ValidationFeeHijiriQuoteRequestV1 {
            version: VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1,
            account_id: AccountId::new(key_pair.public_key().clone()),
            qualifying_transfer_count: 2,
        }
    }

    fn hijiri_quote_response(
        request: &ValidationFeeHijiriQuoteRequestV1,
    ) -> ValidationFeeHijiriQuoteResponseV1 {
        let policy = HijiriFeePolicy::new(
            vec![FeeMultiplierBand::new(Q16::ONE, Q16::ONE).expect("valid fee band")],
            Q16::ONE,
        )
        .expect("valid fee policy");
        let parameters = HijiriParametersV1::try_new(1, None, policy, Q16::ZERO)
            .expect("valid Hijiri parameters");
        let fee_asset = AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .expect("valid fee asset");
        let base = ValidationFeeHijiriQuoteBaseV1::try_new(
            42,
            43,
            1,
            [0x03; 32],
            fee_asset.to_string(),
            request.account_id.to_string(),
            VALIDATION_FEE_DS_SCALE,
            VALIDATION_FEE_BASE_MINOR_UNITS_V1,
        )
        .expect("valid quote base");
        evaluate_hijiri_quote_v1(
            base,
            &request.account_id,
            &parameters,
            None,
            request.qualifying_transfer_count,
        )
        .expect("evaluate quote")
    }
    fn native_checkpoint(height: u64) -> &'static [u8] {
        match height {
            1 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/sumeragi/native-finality/genesis-checkpoint.nrt"
            )),
            2 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/sumeragi/native-finality/height-2-checkpoint.nrt"
            )),
            _ => panic!("fixture checkpoint height"),
        }
    }
    #[test]
    fn request_encoder_derives_height_from_complete_native_checkpoint() {
        for height in 1..=2 {
            let checkpoint = native_checkpoint(height);
            let request = validation_fee_current_policy_proof_request_v1(checkpoint).unwrap();
            let decoded: ValidationFeeCurrentPolicyProofRequestV1 =
                decode_from_bytes(&request).unwrap();
            assert_eq!(
                decoded,
                ValidationFeeCurrentPolicyProofRequestV1 {
                    version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
                    trusted_checkpoint_height: height,
                }
            );
            let mut output = ptr::null_mut();
            let mut length = 0;
            let code = unsafe {
                connect_norito_validation_fee_current_policy_proof_request_v1(
                    checkpoint.as_ptr(),
                    checkpoint.len() as c_ulong,
                    &mut output,
                    &mut length,
                )
            };
            assert_eq!(code, 0);
            assert_eq!(
                unsafe { slice::from_raw_parts(output, length as usize) },
                request
            );
            connect_norito_free(output);
            let mut suffix = checkpoint.to_vec();
            suffix.push(0);
            assert!(validation_fee_current_policy_proof_request_v1(&suffix).is_err());
        }
        for malformed in [
            &[][..],
            &[0; 32],
            &[1; 32],
            &[2; 32],
            &native_checkpoint(1)[..32],
        ] {
            assert!(validation_fee_current_policy_proof_request_v1(malformed).is_err());
        }
    }
    #[test]
    fn proof_verifier_rejects_malformed_archive_and_scalar_checkpoint() {
        let checkpoint =
            iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint::decode_canonical(
                native_checkpoint(1),
            )
            .unwrap();
        for archive in [&[][..], b"not norito"] {
            for trust in [native_checkpoint(1), &[5; 32][..]] {
                assert!(
                    validation_fee_current_policy_proof_verify_v1(
                        archive,
                        checkpoint.network_id(),
                        [3; 32],
                        trust,
                    )
                    .is_err()
                );
            }
        }
    }
    #[test]
    fn fee_checkpoint_c_boundaries_clear_every_output_on_refusal() {
        let checkpoint = native_checkpoint(1);
        let network =
            iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint::decode_canonical(
                checkpoint,
            )
            .unwrap()
            .network_id();
        for (input, length) in [
            (ptr::null(), checkpoint.len() as c_ulong),
            (checkpoint.as_ptr(), 0),
            (
                checkpoint.as_ptr(),
                (iroha_data_model::sumeragi_finality::MAX_FINALITY_CHECKPOINT_BYTES + 1) as c_ulong,
            ),
        ] {
            let mut output = ptr::dangling_mut();
            let mut output_len = 99;
            let code = unsafe {
                connect_norito_validation_fee_current_policy_proof_request_v1(
                    input,
                    length,
                    &mut output,
                    &mut output_len,
                )
            };
            assert_ne!(code, 0);
            assert!(output.is_null());
            assert_eq!(output_len, 0);
        }
        let mut projection = ptr::dangling_mut();
        let mut projection_len = 99;
        let mut promoted = ptr::dangling_mut();
        let mut promoted_len = 99;
        let code = unsafe {
            connect_norito_validation_fee_current_policy_proof_verify_v1(
                b"not norito".as_ptr(),
                10,
                network.as_bytes().as_ptr(),
                32,
                [3_u8; 32].as_ptr(),
                32,
                checkpoint.as_ptr(),
                checkpoint.len() as c_ulong,
                &mut projection,
                &mut projection_len,
                &mut promoted,
                &mut promoted_len,
            )
        };
        assert_ne!(code, 0);
        assert!(projection.is_null());
        assert!(promoted.is_null());
        assert_eq!((projection_len, promoted_len), (0, 0));
    }

    #[test]
    fn hijiri_quote_bridge_roundtrips_native_request_and_bound_response() {
        let request = hijiri_quote_request();
        let account_literal = request.account_id.to_string();
        let mut request_ptr: *mut c_uchar = ptr::null_mut();
        let mut request_len: c_ulong = 0;
        let request_status = unsafe {
            connect_norito_validation_fee_hijiri_quote_request_v1(
                account_literal.as_ptr(),
                account_literal.len() as c_ulong,
                request.qualifying_transfer_count,
                &mut request_ptr,
                &mut request_len,
            )
        };
        assert_eq!(request_status, 0);
        assert!(!request_ptr.is_null());
        let request_archive =
            unsafe { slice::from_raw_parts(request_ptr, request_len as usize).to_vec() };
        connect_norito_free(request_ptr);
        let decoded_request: ValidationFeeHijiriQuoteRequestV1 =
            decode_from_bytes(&request_archive).expect("decode request archive");
        assert_eq!(decoded_request, request);

        let expected = hijiri_quote_response(&request);
        let response_archive = norito::to_bytes(&expected).expect("encode response archive");
        let mut projection_ptr: *mut c_uchar = ptr::null_mut();
        let mut projection_len: c_ulong = 0;
        let response_status = unsafe {
            connect_norito_validation_fee_hijiri_quote_response_verify_v1(
                response_archive.as_ptr(),
                response_archive.len() as c_ulong,
                request_archive.as_ptr(),
                request_archive.len() as c_ulong,
                &mut projection_ptr,
                &mut projection_len,
            )
        };
        assert_eq!(response_status, 0);
        assert!(!projection_ptr.is_null());
        let projection =
            unsafe { slice::from_raw_parts(projection_ptr, projection_len as usize).to_vec() };
        connect_norito_free(projection_ptr);
        let decoded_projection: ValidationFeeHijiriQuoteResponseV1 =
            norito::json::from_slice(&projection).expect("decode typed projection JSON");
        assert_eq!(decoded_projection, expected);
    }

    #[test]
    fn hijiri_quote_bridge_rejects_invalid_counts_and_response_substitution() {
        let request = hijiri_quote_request();
        let account_literal = request.account_id.to_string();
        assert!(
            validation_fee_hijiri_quote_request_v1(&account_literal, 0).is_err(),
            "zero qualifying transfers must fail closed"
        );
        assert!(
            validation_fee_hijiri_quote_request_v1(
                &account_literal,
                VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1 + 1,
            )
            .is_err(),
            "an oversized transfer count must fail closed"
        );
        let request_archive = norito::to_bytes(&request).expect("encode request");
        let response_archive =
            norito::to_bytes(&hijiri_quote_response(&request)).expect("encode response");
        let mut substituted_request = request.clone();
        substituted_request.qualifying_transfer_count = 1;
        let substituted_request_archive =
            norito::to_bytes(&substituted_request).expect("encode substituted request");
        assert!(
            validation_fee_hijiri_quote_response_verify_v1(
                &response_archive,
                &substituted_request_archive,
            )
            .is_err(),
            "response must remain bound to the exact request"
        );
        let mut trailing_request = request_archive.clone();
        trailing_request.push(0);
        assert!(
            validation_fee_hijiri_quote_response_verify_v1(&response_archive, &trailing_request,)
                .is_err(),
            "non-canonical request bytes must fail closed"
        );
        let mut trailing_response = response_archive;
        trailing_response.push(0);
        assert!(
            validation_fee_hijiri_quote_response_verify_v1(&trailing_response, &request_archive,)
                .is_err(),
            "non-canonical response bytes must fail closed"
        );
    }

    #[test]
    fn hijiri_quote_c_abi_clears_outputs_and_reports_stable_errors() {
        let request = hijiri_quote_request();
        let account_literal = request.account_id.to_string();
        let sentinel = ptr::dangling_mut::<c_uchar>();

        let mut request_ptr = sentinel;
        let mut request_len = c_ulong::MAX;
        let invalid_account_status = unsafe {
            connect_norito_validation_fee_hijiri_quote_request_v1(
                ptr::null(),
                1,
                request.qualifying_transfer_count,
                &mut request_ptr,
                &mut request_len,
            )
        };
        assert_eq!(invalid_account_status, ERR_VALIDATION_FEE_HIJIRI_QUOTE);
        assert!(request_ptr.is_null());
        assert_eq!(request_len, 0);

        request_len = c_ulong::MAX;
        let missing_output_pointer_status = unsafe {
            connect_norito_validation_fee_hijiri_quote_request_v1(
                account_literal.as_ptr(),
                account_literal.len() as c_ulong,
                request.qualifying_transfer_count,
                ptr::null_mut(),
                &mut request_len,
            )
        };
        assert_eq!(missing_output_pointer_status, ERR_NULL_PTR);
        assert_eq!(request_len, 0);

        request_ptr = sentinel;
        let missing_output_length_status = unsafe {
            connect_norito_validation_fee_hijiri_quote_request_v1(
                account_literal.as_ptr(),
                account_literal.len() as c_ulong,
                request.qualifying_transfer_count,
                &mut request_ptr,
                ptr::null_mut(),
            )
        };
        assert_eq!(missing_output_length_status, ERR_NULL_PTR);
        assert!(request_ptr.is_null());

        let request_archive = norito::to_bytes(&request).expect("encode request");
        let malformed_response = [0_u8];
        let mut projection_ptr = sentinel;
        let mut projection_len = c_ulong::MAX;
        let malformed_response_status = unsafe {
            connect_norito_validation_fee_hijiri_quote_response_verify_v1(
                malformed_response.as_ptr(),
                malformed_response.len() as c_ulong,
                request_archive.as_ptr(),
                request_archive.len() as c_ulong,
                &mut projection_ptr,
                &mut projection_len,
            )
        };
        assert_eq!(malformed_response_status, ERR_VALIDATION_FEE_HIJIRI_QUOTE);
        assert!(projection_ptr.is_null());
        assert_eq!(projection_len, 0);

        connect_norito_free(ptr::null_mut());
    }
}
