    #[test]
    fn soracloud_fhe_public_input_schema_hashes_are_stable() {
        for (label, actual, direct) in [
            (
                "input admission",
                soracloud_fhe_input_admission_public_inputs_schema_hash_v1(),
                <[u8; 32]>::from(Hash::new(
                    SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1,
                )),
            ),
            (
                "public-key proof",
                soracloud_fhe_public_key_proof_public_inputs_schema_hash_v1(),
                <[u8; 32]>::from(Hash::new(
                    SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
                )),
            ),
            (
                "bootstrap-key proof",
                soracloud_fhe_bootstrap_key_proof_public_inputs_schema_hash_v1(),
                <[u8; 32]>::from(Hash::new(
                    SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
                )),
            ),
            (
                "full-bootstrap material proof",
                soracloud_fhe_full_bootstrap_material_proof_public_inputs_schema_hash_v1(),
                <[u8; 32]>::from(Hash::new(
                    SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
                )),
            ),
            (
                "full-bootstrap execution proof",
                soracloud_fhe_full_bootstrap_execution_proof_public_inputs_schema_hash_v1(),
                <[u8; 32]>::from(Hash::new(
                    SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
                )),
            ),
        ] {
            assert_eq!(actual, direct, "{label} schema hash helper drifted");
        }

        assert_eq!(
            hex::encode(soracloud_fhe_input_admission_public_inputs_schema_hash_v1()),
            "3a4ea767a17590fa97da2f481630673ca1f492c6ccddf982d37c203f31bb3f6b",
            "input admission public-input schema hash drifted"
        );
        assert_eq!(
            hex::encode(soracloud_fhe_public_key_proof_public_inputs_schema_hash_v1()),
            "c208cb0bd5df814bb7c2d382a288633e34f004dbd66832fd495659f861afb45f",
            "public-key proof public-input schema hash drifted"
        );
        assert_eq!(
            hex::encode(soracloud_fhe_bootstrap_key_proof_public_inputs_schema_hash_v1()),
            "47f9c35097833abe736254b49544d15fd3f47dd22abac78c0d5fbc46b69520a3",
            "bootstrap-key proof public-input schema hash drifted"
        );
        assert_eq!(
            hex::encode(soracloud_fhe_full_bootstrap_material_proof_public_inputs_schema_hash_v1()),
            "0273accdd8ff0242212a9695275baef1e441b40d6df6079bac4aeb557b5ae5e5",
            "full-bootstrap material proof public-input schema hash drifted"
        );
        assert_eq!(
            hex::encode(
                soracloud_fhe_full_bootstrap_execution_proof_public_inputs_schema_hash_v1()
            ),
            "2df6d711dfec113250c004dbf1904db999c07fc3f5dfcf7f53c17204538d1c1f",
            "full-bootstrap execution proof public-input schema hash drifted"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn soracloud_fhe_input_admission_schema_advertises_backend() {
        let schema = std::str::from_utf8(SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1)
            .expect("input-admission proof schema is valid UTF-8");
        let exact_ciphertext_statement_domain =
            std::str::from_utf8(iroha_crypto::fhe_bfv::BFV_CIPHERTEXT_PROOF_STATEMENT_DOMAIN)
                .expect("exact ciphertext statement digest domain is valid UTF-8");
        let bounded_ciphertext_statement_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_CIPHERTEXT_PROOF_STATEMENT_DOMAIN,
        )
        .expect("bounded ciphertext statement digest domain is valid UTF-8");
        let exact_ciphertext_proof_input_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_CIPHERTEXT_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("exact ciphertext proof input digest domain is valid UTF-8");
        let bounded_ciphertext_proof_input_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_CIPHERTEXT_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("bounded ciphertext proof input digest domain is valid UTF-8");
        assert!(
            schema.contains("\"proof_backend\":\"stark/fri/sha256-goldilocks\""),
            "public schema must advertise the canonical BFV STARK/FRI backend"
        );
        assert!(
            schema.contains("residual_multiple_bound,bound_mode"),
            "public schema must advertise input-admission bound fields"
        );
        for required in [
            "\"bound_contract\"",
            "\"modes\":[\"exact_residual_multiple\",\"bounded_noise\"]",
            "\"validates_exact_residual_capacity\":true",
            "\"validates_bounded_noise_capacity\":true",
            "\"ciphertext_statement_digest_domains\"",
            "iroha.crypto.fhe.bfv.ciphertext_proof_statement.v1",
            "iroha.crypto.fhe.bfv.bounded_noise_ciphertext_proof_statement.v1",
            "\"separates_exact_and_bounded\":true",
            "\"ciphertext_statement_material\"",
            "\"version\":1",
            "\"field_count\":8",
            "\"binds_params\":true",
            "\"binds_public_key\":true",
            "\"binds_public_key_digest\":true",
            "\"binds_ciphertext\":true",
            "\"binds_ciphertext_digest\":true",
            "\"binds_declared_bound\":true",
            "\"validates_exact_seeded_encryption_capacity\":true",
            "\"rejects_all_zero_ciphertext\":true",
            "\"ciphertext_generation\"",
            "\"resamples_all_zero_ephemeral_mask\":true",
            "\"resamples_all_zero_exact_error\":true",
            "\"resamples_all_zero_bounded_noise\":true",
            "\"ciphertext_proof_input_material\"",
            "iroha.crypto.fhe.bfv.exact_residual_ciphertext_proof_input_material_digest.v1",
            "iroha.crypto.fhe.bfv.bounded_noise_ciphertext_proof_input_material_digest.v1",
            "\"binds_secret_key_witness\":true",
            "\"binds_plaintext\":true",
            "\"binds_scaled_coefficients\":true",
            "\"binds_residual_multiples\":true",
            "\"binds_noise_polynomial\":true",
            "\"binds_actual_residual_max\":true",
            "\"binds_actual_noise_max\":true",
            "\"rejects_zero_residual_witness\":true",
            "\"rejects_zero_noise_witness\":true",
            "\"hashes_proof_input_material\":true",
        ] {
            assert!(
                schema.contains(required),
                "public schema must advertise input-admission bound contract {required}"
            );
        }
        #[cfg(feature = "json")]
        {
            let schema_value: Value =
                json::from_slice(SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1)
                    .expect("input-admission proof schema must parse as Norito JSON");
            let bound_contract =
                assert_schema_object(&schema_value, "/bound_contract", "input-admission schema");
            let modes = bound_contract
                .get("modes")
                .and_then(Value::as_array)
                .expect("input-admission schema must advertise bound modes")
                .iter()
                .map(|mode| mode.as_str().expect("bound mode must be a string"))
                .collect::<Vec<_>>();
            assert_eq!(
                modes,
                ["exact_residual_multiple", "bounded_noise"],
                "input-admission schema bound modes drifted"
            );
            assert_schema_bool_field(
                bound_contract,
                "validates_exact_residual_capacity",
                true,
                "input-admission schema",
            );
            assert_schema_bool_field(
                bound_contract,
                "validates_bounded_noise_capacity",
                true,
                "input-admission schema",
            );

            let domains = assert_schema_object(
                &schema_value,
                "/ciphertext_statement_digest_domains",
                "input-admission ciphertext statement digest domains",
            );
            assert_schema_string_field(
                domains,
                "exact",
                exact_ciphertext_statement_domain,
                "input-admission schema",
            );
            assert_schema_string_field(
                domains,
                "bounded",
                bounded_ciphertext_statement_domain,
                "input-admission schema",
            );
            assert_schema_bool_field(
                domains,
                "separates_exact_and_bounded",
                true,
                "input-admission schema",
            );

            let material = assert_schema_object(
                &schema_value,
                "/ciphertext_statement_material",
                "input-admission ciphertext statement material",
            );
            assert_schema_u64_field(
                material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_CIPHERTEXT_PROOF_STATEMENT_MATERIAL_VERSION_V1,
                ),
                "input-admission schema",
            );
            assert_schema_u64_field(
                material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_CIPHERTEXT_PROOF_STATEMENT_MATERIAL_FIELD_COUNT_V1,
                ),
                "input-admission schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
                "binds_ciphertext",
                "binds_ciphertext_digest",
                "binds_declared_bound",
                "validates_exact_seeded_encryption_capacity",
                "rejects_all_zero_ciphertext",
            ] {
                assert_schema_bool_field(material, field, true, "input-admission schema");
            }

            let ciphertext_generation = assert_schema_object(
                &schema_value,
                "/ciphertext_generation",
                "input-admission ciphertext generation policy",
            );
            for field in [
                "resamples_all_zero_ephemeral_mask",
                "resamples_all_zero_exact_error",
                "resamples_all_zero_bounded_noise",
            ] {
                assert_schema_bool_field(
                    ciphertext_generation,
                    field,
                    true,
                    "input-admission schema",
                );
            }

            let proof_input = assert_schema_object(
                &schema_value,
                "/ciphertext_proof_input_material",
                "input-admission ciphertext proof input material",
            );
            let proof_input_domains = assert_schema_object(
                &schema_value,
                "/ciphertext_proof_input_material/digest_domains",
                "input-admission ciphertext proof input digest domains",
            );
            assert_schema_string_field(
                proof_input_domains,
                "exact",
                exact_ciphertext_proof_input_domain,
                "input-admission schema",
            );
            assert_schema_string_field(
                proof_input_domains,
                "bounded",
                bounded_ciphertext_proof_input_domain,
                "input-admission schema",
            );
            assert_schema_bool_field(
                proof_input_domains,
                "separates_exact_and_bounded",
                true,
                "input-admission schema",
            );
            assert_schema_bool_field(
                proof_input,
                "hashes_proof_input_material",
                true,
                "input-admission schema",
            );

            let exact_material = assert_schema_object(
                &schema_value,
                "/ciphertext_proof_input_material/exact",
                "exact ciphertext proof input material",
            );
            assert_schema_u64_field(
                exact_material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_CIPHERTEXT_PROOF_INPUT_MATERIAL_VERSION_V1,
                ),
                "input-admission schema",
            );
            assert_schema_u64_field(
                exact_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_CIPHERTEXT_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
                ),
                "input-admission schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
                "binds_ciphertext",
                "binds_ciphertext_digest",
                "binds_secret_key_witness",
                "binds_plaintext",
                "binds_scaled_coefficients",
                "binds_residual_multiples",
                "binds_declared_bound",
                "binds_actual_residual_max",
                "rejects_zero_residual_witness",
                "binds_statement_hash",
            ] {
                assert_schema_bool_field(exact_material, field, true, "input-admission schema");
            }

            let bounded_material = assert_schema_object(
                &schema_value,
                "/ciphertext_proof_input_material/bounded",
                "bounded ciphertext proof input material",
            );
            assert_schema_u64_field(
                bounded_material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_CIPHERTEXT_PROOF_INPUT_MATERIAL_VERSION_V1,
                ),
                "input-admission schema",
            );
            assert_schema_u64_field(
                bounded_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_CIPHERTEXT_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
                ),
                "input-admission schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
                "binds_ciphertext",
                "binds_ciphertext_digest",
                "binds_secret_key_witness",
                "binds_plaintext",
                "binds_scaled_coefficients",
                "binds_noise_polynomial",
                "binds_declared_bound",
                "binds_actual_noise_max",
                "rejects_zero_noise_witness",
                "binds_statement_hash",
            ] {
                assert_schema_bool_field(bounded_material, field, true, "input-admission schema");
            }
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn soracloud_fhe_public_key_schema_advertises_statement_material() {
        let schema = std::str::from_utf8(SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1)
            .expect("public-key proof schema is valid UTF-8");
        let exact_public_key_statement_domain =
            std::str::from_utf8(iroha_crypto::fhe_bfv::BFV_PUBLIC_KEY_PROOF_STATEMENT_DOMAIN)
                .expect("exact public-key statement digest domain is valid UTF-8");
        let bounded_public_key_statement_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_PUBLIC_KEY_PROOF_STATEMENT_DOMAIN,
        )
        .expect("bounded public-key statement digest domain is valid UTF-8");
        let exact_public_key_proof_input_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_PUBLIC_KEY_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("exact public-key proof input digest domain is valid UTF-8");
        let bounded_public_key_proof_input_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_PUBLIC_KEY_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("bounded public-key proof input digest domain is valid UTF-8");
        for required in [
            "\"proof_backend\":\"stark/fri/sha256-goldilocks\"",
            "public_key_proof_statement_digest(version,field_count,params,public_key,public_key_digest)",
            "\"statement_digest_domains\"",
            "iroha.crypto.fhe.bfv.public_key_proof_statement.v1",
            "iroha.crypto.fhe.bfv.bounded_noise_public_key_proof_statement.v1",
            "\"separates_exact_and_bounded\":true",
            "\"proof_statement_material\"",
            "\"version\":1",
            "\"field_count\":5",
            "\"binds_params\":true",
            "\"binds_public_key\":true",
            "\"binds_public_key_digest\":true",
            "\"key_generation\"",
            "\"resamples_all_zero_public_key_a\":true",
            "\"resamples_all_zero_exact_error\":true",
            "\"resamples_all_zero_bounded_noise\":true",
            "\"proof_input_material\"",
            "iroha.crypto.fhe.bfv.exact_residual_public_key_proof_input_material_digest.v1",
            "iroha.crypto.fhe.bfv.bounded_noise_public_key_proof_input_material_digest.v1",
            "\"binds_secret_key_witness\":true",
            "\"binds_public_key_residual\":true",
            "\"binds_residual_multiples\":true",
            "\"binds_residual_max\":true",
            "\"binds_public_key_noise\":true",
            "\"binds_noise_max\":true",
            "\"rejects_zero_residual_witness\":true",
            "\"rejects_zero_noise_witness\":true",
            "\"hashes_proof_input_material\":true",
        ] {
            assert!(
                schema.contains(required),
                "public schema must advertise public-key proof term {required}"
            );
        }
        #[cfg(feature = "json")]
        {
            let schema_value: Value =
                json::from_slice(SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1)
                    .expect("public-key proof schema must parse as Norito JSON");
            let domains = assert_schema_object(
                &schema_value,
                "/statement_digest_domains",
                "public-key proof statement digest domains",
            );
            assert_schema_string_field(
                domains,
                "exact",
                exact_public_key_statement_domain,
                "public-key proof schema",
            );
            assert_schema_string_field(
                domains,
                "bounded",
                bounded_public_key_statement_domain,
                "public-key proof schema",
            );
            assert_schema_bool_field(
                domains,
                "separates_exact_and_bounded",
                true,
                "public-key proof schema",
            );

            let material = assert_schema_object(
                &schema_value,
                "/proof_statement_material",
                "public-key proof statement material",
            );
            assert_schema_u64_field(
                material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_PUBLIC_KEY_PROOF_STATEMENT_MATERIAL_VERSION_V1,
                ),
                "public-key proof schema",
            );
            assert_schema_u64_field(
                material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_PUBLIC_KEY_PROOF_STATEMENT_MATERIAL_FIELD_COUNT_V1,
                ),
                "public-key proof schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
            ] {
                assert_schema_bool_field(material, field, true, "public-key proof schema");
            }

            let key_generation = assert_schema_object(
                &schema_value,
                "/key_generation",
                "public-key proof key generation policy",
            );
            for field in [
                "resamples_all_zero_public_key_a",
                "resamples_all_zero_exact_error",
                "resamples_all_zero_bounded_noise",
            ] {
                assert_schema_bool_field(key_generation, field, true, "public-key proof schema");
            }

            let proof_input = assert_schema_object(
                &schema_value,
                "/proof_input_material",
                "public-key proof input material",
            );
            let proof_input_domains = assert_schema_object(
                &schema_value,
                "/proof_input_material/digest_domains",
                "public-key proof input digest domains",
            );
            assert_schema_string_field(
                proof_input_domains,
                "exact",
                exact_public_key_proof_input_domain,
                "public-key proof schema",
            );
            assert_schema_string_field(
                proof_input_domains,
                "bounded",
                bounded_public_key_proof_input_domain,
                "public-key proof schema",
            );
            assert_schema_bool_field(
                proof_input_domains,
                "separates_exact_and_bounded",
                true,
                "public-key proof schema",
            );
            assert_schema_bool_field(
                proof_input,
                "hashes_proof_input_material",
                true,
                "public-key proof schema",
            );

            let exact_material = assert_schema_object(
                &schema_value,
                "/proof_input_material/exact",
                "exact public-key proof input material",
            );
            assert_schema_u64_field(
                exact_material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_PUBLIC_KEY_PROOF_INPUT_MATERIAL_VERSION_V1,
                ),
                "public-key proof schema",
            );
            assert_schema_u64_field(
                exact_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_PUBLIC_KEY_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
                ),
                "public-key proof schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
                "binds_secret_key_witness",
                "binds_public_key_residual",
                "binds_residual_multiples",
                "binds_residual_max",
                "rejects_zero_residual_witness",
                "binds_statement_hash",
            ] {
                assert_schema_bool_field(exact_material, field, true, "public-key proof schema");
            }

            let bounded_material = assert_schema_object(
                &schema_value,
                "/proof_input_material/bounded",
                "bounded public-key proof input material",
            );
            assert_schema_u64_field(
                bounded_material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_PUBLIC_KEY_PROOF_INPUT_MATERIAL_VERSION_V1,
                ),
                "public-key proof schema",
            );
            assert_schema_u64_field(
                bounded_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_PUBLIC_KEY_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
                ),
                "public-key proof schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
                "binds_secret_key_witness",
                "binds_public_key_noise",
                "binds_noise_max",
                "rejects_zero_noise_witness",
                "binds_statement_hash",
            ] {
                assert_schema_bool_field(bounded_material, field, true, "public-key proof schema");
            }
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn soracloud_fhe_public_key_schema_advertises_proof_input_material() {
        let schema = std::str::from_utf8(SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1)
            .expect("public-key proof schema is valid UTF-8");
        let exact_public_key_proof_input_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_PUBLIC_KEY_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("exact public-key proof input digest domain is valid UTF-8");
        let bounded_public_key_proof_input_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_PUBLIC_KEY_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("bounded public-key proof input digest domain is valid UTF-8");
        for required in [
            "\"proof_input_material\"",
            exact_public_key_proof_input_domain,
            bounded_public_key_proof_input_domain,
            "\"binds_secret_key_witness\":true",
            "\"binds_public_key_residual\":true",
            "\"binds_residual_multiples\":true",
            "\"binds_residual_max\":true",
            "\"binds_public_key_noise\":true",
            "\"binds_noise_max\":true",
            "\"rejects_zero_residual_witness\":true",
            "\"rejects_zero_noise_witness\":true",
            "\"binds_statement_hash\":true",
            "\"hashes_proof_input_material\":true",
        ] {
            assert!(
                schema.contains(required),
                "public schema must advertise public-key proof input term {required}"
            );
        }
        #[cfg(feature = "json")]
        {
            let schema_value: Value =
                json::from_slice(SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1)
                    .expect("public-key proof schema must parse as Norito JSON");
            let proof_input = assert_schema_object(
                &schema_value,
                "/proof_input_material",
                "public-key proof input material",
            );
            let proof_input_domains = assert_schema_object(
                &schema_value,
                "/proof_input_material/digest_domains",
                "public-key proof input digest domains",
            );
            assert_schema_string_field(
                proof_input_domains,
                "exact",
                exact_public_key_proof_input_domain,
                "public-key proof schema",
            );
            assert_schema_string_field(
                proof_input_domains,
                "bounded",
                bounded_public_key_proof_input_domain,
                "public-key proof schema",
            );
            assert_schema_bool_field(
                proof_input_domains,
                "separates_exact_and_bounded",
                true,
                "public-key proof schema",
            );
            assert_schema_bool_field(
                proof_input,
                "hashes_proof_input_material",
                true,
                "public-key proof schema",
            );

            let exact_material = assert_schema_object(
                &schema_value,
                "/proof_input_material/exact",
                "exact public-key proof input material",
            );
            assert_schema_u64_field(
                exact_material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_PUBLIC_KEY_PROOF_INPUT_MATERIAL_VERSION_V1,
                ),
                "public-key proof schema",
            );
            assert_schema_u64_field(
                exact_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_EXACT_RESIDUAL_PUBLIC_KEY_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
                ),
                "public-key proof schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
                "binds_secret_key_witness",
                "binds_public_key_residual",
                "binds_residual_multiples",
                "binds_residual_max",
                "rejects_zero_residual_witness",
                "binds_statement_hash",
            ] {
                assert_schema_bool_field(exact_material, field, true, "public-key proof schema");
            }

            let bounded_material = assert_schema_object(
                &schema_value,
                "/proof_input_material/bounded",
                "bounded public-key proof input material",
            );
            assert_schema_u64_field(
                bounded_material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_PUBLIC_KEY_PROOF_INPUT_MATERIAL_VERSION_V1,
                ),
                "public-key proof schema",
            );
            assert_schema_u64_field(
                bounded_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_PUBLIC_KEY_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
                ),
                "public-key proof schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "binds_public_key_digest",
                "binds_secret_key_witness",
                "binds_public_key_noise",
                "binds_noise_max",
                "rejects_zero_noise_witness",
                "binds_statement_hash",
            ] {
                assert_schema_bool_field(bounded_material, field, true, "public-key proof schema");
            }
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "schema golden test keeps audited contract terms inline"
    )]
    fn soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary() {
        let schema = std::str::from_utf8(SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1)
            .expect("bootstrap-key proof schema is valid UTF-8");
        let round_refresh_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_ROUND_REFRESH_SUMMARY_DIGEST_DOMAIN,
        )
        .expect("round-refresh digest domain is valid UTF-8");
        let zero_refresh_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_ZERO_REFRESH_SUMMARY_DIGEST_DOMAIN,
        )
        .expect("zero-refresh digest domain is valid UTF-8");
        let exact_refresh_transcript_domain =
            std::str::from_utf8(iroha_crypto::fhe_bfv::BFV_REFRESH_TRANSCRIPT_DIGEST_DOMAIN)
                .expect("exact refresh-transcript digest domain is valid UTF-8");
        let bounded_refresh_transcript_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_REFRESH_TRANSCRIPT_DIGEST_DOMAIN,
        )
        .expect("bounded refresh-transcript digest domain is valid UTF-8");
        let exact_rotation_seed_domain =
            std::str::from_utf8(iroha_crypto::fhe_bfv::BFV_ENCRYPT_SEED_DERIVATION_DOMAIN)
                .expect("exact rotation seed-derivation domain is valid UTF-8");
        let bounded_rotation_seed_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_ENCRYPT_SEED_DERIVATION_DOMAIN,
        )
        .expect("bounded rotation seed-derivation domain is valid UTF-8");
        let exact_bootstrap_round_seed_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_REFRESH_ROUND_SEED_DERIVATION_DOMAIN,
        )
        .expect("exact bootstrap refresh-round seed-derivation domain is valid UTF-8");
        let bounded_bootstrap_round_seed_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_BOOTSTRAP_REFRESH_ROUND_SEED_DERIVATION_DOMAIN,
        )
        .expect("bounded bootstrap refresh-round seed-derivation domain is valid UTF-8");
        let exact_raw_statement_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_ZERO_REFRESH_PROOF_STATEMENT_DOMAIN,
        )
        .expect("exact raw statement digest domain is valid UTF-8");
        let bounded_raw_statement_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_BOOTSTRAP_KEY_ZERO_REFRESH_PROOF_STATEMENT_DOMAIN,
        )
        .expect("bounded raw statement digest domain is valid UTF-8");
        let exact_transcript_statement_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_TRANSCRIPT_ZERO_REFRESH_PROOF_STATEMENT_DOMAIN,
        )
        .expect("exact transcript statement digest domain is valid UTF-8");
        let bounded_transcript_statement_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_BOUNDED_NOISE_BOOTSTRAP_KEY_TRANSCRIPT_ZERO_REFRESH_PROOF_STATEMENT_DOMAIN,
        )
        .expect("bounded transcript statement digest domain is valid UTF-8");

        assert!(
            schema.contains(
                "bootstrap_key_transcript_zero_refresh_proof_statement_digest(version,field_count,"
            ),
            "public schema must advertise the self-describing bootstrap statement header"
        );
        assert!(
            schema.contains(&format!(
                "\"version\":{}",
                iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_TRANSCRIPT_PROOF_STATEMENT_MATERIAL_VERSION_V1
            )),
            "public schema must advertise the crypto statement-material version"
        );
        assert!(
            schema.contains(&format!(
                "\"field_count\":{}",
                iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_TRANSCRIPT_PROOF_STATEMENT_MATERIAL_FIELD_COUNT_V1
            )),
            "public schema must advertise the crypto statement-material field count"
        );
        assert!(
            schema.contains(&format!(
                "\"version\":{}",
                iroha_crypto::fhe_bfv::BFV_REFRESH_TRANSCRIPT_DIGEST_MATERIAL_VERSION_V1
            )),
            "public schema must advertise the crypto refresh-transcript material version"
        );
        assert!(
            schema.contains(&format!(
                "\"field_count\":{}",
                iroha_crypto::fhe_bfv::BFV_REFRESH_TRANSCRIPT_DIGEST_MATERIAL_FIELD_COUNT_V1
            )),
            "public schema must advertise the crypto refresh-transcript material field count"
        );
        for required in [
            "\"proof_backend\":\"stark/fri/sha256-goldilocks\"",
            "\"statement_digest_domains\"",
            "\"separates_exact_and_bounded\":true",
            "\"separates_raw_and_transcript\":true",
            exact_raw_statement_domain,
            bounded_raw_statement_domain,
            exact_transcript_statement_domain,
            bounded_transcript_statement_domain,
            "\"refresh_transcript_digest_domains\"",
            exact_refresh_transcript_domain,
            bounded_refresh_transcript_domain,
            "\"refresh_transcript_seed_derivation_domains\"",
            exact_rotation_seed_domain,
            bounded_rotation_seed_domain,
            exact_bootstrap_round_seed_domain,
            bounded_bootstrap_round_seed_domain,
            "\"separates_rotation_and_bootstrap_round\":true",
            "\"refresh_transcript_material\"",
            "\"binds_params\":true",
            "\"binds_public_key\":true",
            "\"rejects_all_zero_public_key\":true",
            "\"binds_rotation_transcripts\":true",
            "\"rejects_all_zero_transcript_seeds\":true",
            "\"proof_statement_material\"",
            "\"binds_evaluation_key_digest\":true",
            "\"binds_refresh_transcript_digest\":true",
            "\"binds_bootstrap_transcript\":true",
            "\"binds_bootstrap_round_count\":true",
            "\"binds_zero_refresh_digest\":true",
            "\"binds_round_refresh_digests\":true",
            "\"binds_bootstrap_key\":true",
            "\"round_refresh_digest_domain\"",
            "\"zero_refresh_digest_domain\"",
            round_refresh_digest_domain,
            zero_refresh_digest_domain,
        ] {
            assert!(
                schema.contains(required),
                "public schema must advertise bootstrap refresh summary term {required}"
            );
        }
        #[cfg(feature = "json")]
        {
            let schema_value: Value =
                json::from_slice(SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1)
                    .expect("bootstrap-key proof schema must parse as Norito JSON");
            let seed_domains = schema_value
                .get("refresh_transcript_seed_derivation_domains")
                .and_then(Value::as_object)
                .expect("bootstrap-key proof schema must carry seed-derivation domains");
            for (field, expected) in [
                ("exact_rotation", exact_rotation_seed_domain),
                ("bounded_rotation", bounded_rotation_seed_domain),
                ("exact_bootstrap_round", exact_bootstrap_round_seed_domain),
                (
                    "bounded_bootstrap_round",
                    bounded_bootstrap_round_seed_domain,
                ),
            ] {
                assert_eq!(
                    seed_domains.get(field).and_then(Value::as_str),
                    Some(expected),
                    "bootstrap-key proof schema seed-domain field `{field}` drifted"
                );
            }
            assert_eq!(
                seed_domains
                    .get("separates_exact_and_bounded")
                    .and_then(Value::as_bool),
                Some(true),
                "bootstrap-key proof schema must advertise exact/bounded seed separation"
            );
            assert_eq!(
                seed_domains
                    .get("separates_rotation_and_bootstrap_round")
                    .and_then(Value::as_bool),
                Some(true),
                "bootstrap-key proof schema must advertise rotation/bootstrap seed separation"
            );
            assert_ne!(
                seed_domains.get("exact_rotation").and_then(Value::as_str),
                seed_domains
                    .get("exact_bootstrap_round")
                    .and_then(Value::as_str),
                "exact rotation and bootstrap-round domains must be distinct"
            );
            assert_ne!(
                seed_domains.get("bounded_rotation").and_then(Value::as_str),
                seed_domains
                    .get("bounded_bootstrap_round")
                    .and_then(Value::as_str),
                "bounded rotation and bootstrap-round domains must be distinct"
            );

            let statement_domains = assert_schema_object(
                &schema_value,
                "/statement_digest_domains",
                "bootstrap-key proof statement digest domains",
            );
            for (field, expected) in [
                ("exact_raw", exact_raw_statement_domain),
                ("bounded_raw", bounded_raw_statement_domain),
                ("exact_transcript", exact_transcript_statement_domain),
                ("bounded_transcript", bounded_transcript_statement_domain),
            ] {
                assert_schema_string_field(
                    statement_domains,
                    field,
                    expected,
                    "bootstrap-key proof schema",
                );
            }
            assert_schema_bool_field(
                statement_domains,
                "separates_exact_and_bounded",
                true,
                "bootstrap-key proof schema",
            );
            assert_schema_bool_field(
                statement_domains,
                "separates_raw_and_transcript",
                true,
                "bootstrap-key proof schema",
            );

            let refresh_domains = assert_schema_object(
                &schema_value,
                "/refresh_transcript_digest_domains",
                "bootstrap-key refresh transcript digest domains",
            );
            for (field, expected) in [
                ("exact", exact_refresh_transcript_domain),
                ("bounded", bounded_refresh_transcript_domain),
            ] {
                assert_schema_string_field(
                    refresh_domains,
                    field,
                    expected,
                    "bootstrap-key proof schema",
                );
            }
            assert_schema_bool_field(
                refresh_domains,
                "separates_exact_and_bounded",
                true,
                "bootstrap-key proof schema",
            );

            let refresh_material = assert_schema_object(
                &schema_value,
                "/refresh_transcript_material",
                "bootstrap-key refresh transcript material",
            );
            assert_schema_u64_field(
                refresh_material,
                "version",
                u64::from(iroha_crypto::fhe_bfv::BFV_REFRESH_TRANSCRIPT_DIGEST_MATERIAL_VERSION_V1),
                "bootstrap-key proof schema",
            );
            assert_schema_u64_field(
                refresh_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_REFRESH_TRANSCRIPT_DIGEST_MATERIAL_FIELD_COUNT_V1,
                ),
                "bootstrap-key proof schema",
            );
            for field in [
                "binds_params",
                "binds_public_key",
                "rejects_all_zero_public_key",
                "binds_evaluation_key_digest",
                "binds_rotation_transcripts",
                "binds_bootstrap_transcript",
                "rejects_all_zero_transcript_seeds",
            ] {
                assert_schema_bool_field(
                    refresh_material,
                    field,
                    true,
                    "bootstrap-key proof schema",
                );
            }

            let proof_material = assert_schema_object(
                &schema_value,
                "/proof_statement_material",
                "bootstrap-key proof statement material",
            );
            assert_schema_u64_field(
                proof_material,
                "version",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_TRANSCRIPT_PROOF_STATEMENT_MATERIAL_VERSION_V1,
                ),
                "bootstrap-key proof schema",
            );
            assert_schema_u64_field(
                proof_material,
                "field_count",
                u64::from(
                    iroha_crypto::fhe_bfv::BFV_BOOTSTRAP_KEY_TRANSCRIPT_PROOF_STATEMENT_MATERIAL_FIELD_COUNT_V1,
                ),
                "bootstrap-key proof schema",
            );
            assert_schema_string_field(
                proof_material,
                "round_refresh_digest_domain",
                round_refresh_digest_domain,
                "bootstrap-key proof schema",
            );
            assert_schema_string_field(
                proof_material,
                "zero_refresh_digest_domain",
                zero_refresh_digest_domain,
                "bootstrap-key proof schema",
            );
            for field in [
                "binds_evaluation_key_digest",
                "binds_refresh_transcript_digest",
                "binds_bootstrap_transcript",
                "binds_bootstrap_round_count",
                "binds_zero_refresh_digest",
                "binds_round_refresh_digests",
                "binds_bootstrap_key",
            ] {
                assert_schema_bool_field(proof_material, field, true, "bootstrap-key proof schema");
            }
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn soracloud_fhe_full_bootstrap_material_schema_advertises_statement_header() {
        let schema = std::str::from_utf8(
            SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
        )
        .expect("full-bootstrap material schema is valid UTF-8");
        let statement_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_MATERIAL_PROOF_STATEMENT_DOMAIN,
        )
        .expect("material proof statement digest domain is valid UTF-8");
        let proof_input_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_MATERIAL_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("material proof input digest domain is valid UTF-8");
        let execution_proof_input_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("execution proof input material digest domain is valid UTF-8");
        let prover_input_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("execution prover input material digest domain is valid UTF-8");
        let air_evaluation_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic AIR evaluation material digest domain is valid UTF-8");
        let public_opening_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace public-opening material digest domain is valid UTF-8");
        let trace_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace material digest domain is valid UTF-8");
        let air_constraint_system_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_DIGEST_DOMAIN,
        )
        .expect("arithmetic AIR constraint-system digest domain is valid UTF-8");
        let proof_key_material_commitment_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_COMMITMENT_DOMAIN,
        )
        .expect("proof-key material commitment domain is valid UTF-8");
        let proof_key_pair_commitment_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_PAIR_COMMITMENT_DOMAIN,
        )
        .expect("proof-key pair commitment domain is valid UTF-8");
        let circuit_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("full-bootstrap circuit-material digest domain is valid UTF-8");
        let evaluator_artifact_set_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EVALUATOR_ARTIFACT_SET_DIGEST_DOMAIN,
        )
        .expect("full-bootstrap evaluator-artifact-set digest domain is valid UTF-8");
        let circuit_artifact_bundle_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ARTIFACT_BUNDLE_DIGEST_DOMAIN,
        )
        .expect("full-bootstrap circuit-artifact-bundle digest domain is valid UTF-8");
        assert!(
            schema.contains("full_bootstrap_material_proof_statement_digest(version,field_count,"),
            "public schema must advertise the self-describing material statement header"
        );
        assert!(
            schema.contains("full_bootstrap_material_digest"),
            "public schema must advertise the governed material digest"
        );
        for expected in [
            "\"proof_backend\":\"stark/fri/sha256-goldilocks\"",
            "\"statement_digest_domain\"",
            statement_digest_domain,
            "\"material_proof_input\"",
            "\"proof_input_material_version\":1",
            "\"proof_input_material_field_count\":6",
            "\"proof_input_material_digest_domain\"",
            "\"hashes_proof_input_material\":true",
            "\"requires_full_bootstrap_material\":true",
            "\"binds_public_key\":true",
            "\"binds_governed_evaluation_keys\":true",
            "\"binds_concrete_artifact_bundle\":true",
            "\"binds_statement_hash\":true",
            "\"native_air_envelope\"",
            "\"native_air_envelope\":{\"validates_stark_parameter_profile\":true",
            "\"binds_domain_tag_to_statement_hash_and_material_digest\":true",
            "\"rejects_auxiliary_composition_value_commitments\":true",
            "\"requires_canonical_material_transcript_label\":true",
            "\"release_audit_evidence\"",
            "\"release_audit_evidence\":{\"version\":1,\"field_count\":23",
            "full_bootstrap_release_audit_evidence_digest.v1",
            "\"proof_profile_field_count\":58",
            "\"proof_profile_public_opening_material_version\":1",
            "\"proof_profile_public_opening_material_field_count\":13",
            "\"proof_profile_release_prover_digest_domains\"",
            execution_proof_input_material_digest_domain,
            prover_input_material_digest_domain,
            air_evaluation_material_digest_domain,
            public_opening_material_digest_domain,
            trace_material_digest_domain,
            air_constraint_system_digest_domain,
            "\"proof_profile_proof_key_commitment_domains\"",
            proof_key_material_commitment_domain,
            proof_key_pair_commitment_domain,
            "\"proof_profile_validates_air_evaluation_material_digest\":true",
            "\"proof_profile_validates_air_evaluation_trace_material_digest\":true",
            "\"proof_profile_requires_zero_air_composition_values\":true",
            "\"proof_profile_validates_artifact_bound_prover_input\":true",
            "\"proof_profile_rejects_stale_galois_key_set_replay\":true",
            "\"proof_profile_rejects_stale_proof_key_artifacts\":true",
            "\"proof_profile_derives_opening_schedule_from_statement_hash\":true",
            "\"proof_profile_derives_opening_schedule_from_trace_material_digest\":true",
            "\"proof_profile_bounds_opening_schedule_rejection_sampling\":true",
            "\"proof_profile_validates_transcript_public_padding_openings\":true",
            "\"proof_profile_validates_transcript_public_opening_material\":true",
            "\"proof_profile_requires_verifier_owned_trace_material_digest\":true",
            "\"proof_profile_requires_canonical_base_transcript_label\":true",
            "\"proof_profile_rejects_suffixed_transcript_label_aliases\":true",
            "\"proof_profile_validates_merkle_path_shape\":true",
            "\"proof_profile_validates_merkle_path_roots\":true",
            "\"proof_profile_validates_fri_query_chain\":true",
            "\"proof_profile_binds_first_fri_values_to_opened_air_values\":true",
            "\"proof_profile_binds_fri_queries_to_air_commitment_roots\":true",
            "\"proof_profile_binds_centered_scale_round_source_chain_digest\":true",
            "\"key_evidence_field_count\":11",
            "\"key_evidence_binds_centered_scale_round_source_chain_digest\":true",
            "\"artifact_digest_domains\"",
            circuit_material_digest_domain,
            evaluator_artifact_set_digest_domain,
            circuit_artifact_bundle_digest_domain,
            "\"separates_material_set_and_bundle\":true",
            "\"binds_artifact_bundle_digest\":true",
            "\"binds_evaluator_artifact_set_digest\":true",
            "\"binds_coefficient_to_slot_key_digest\":true",
            "\"binds_slot_to_coefficient_key_digest\":true",
            "\"binds_blind_rotation_key_digest\":true",
            "\"binds_sample_extraction_key_digest\":true",
            "\"binds_accumulator_digest\":true",
            "\"binds_proof_public_input_schema_digest\":true",
            "\"binds_arithmetic_trace_profile_digest\":true",
            "\"binds_arithmetic_air_constraint_system_digest\":true",
            "\"binds_arithmetic_air_constraint_system_artifact_digest\":true",
            "\"binds_generated_proof_key_pair\":true",
            "\"proof_key_commitment_domains\"",
            proof_key_material_commitment_domain,
            proof_key_pair_commitment_domain,
            "\"separates_material_and_pair\":true",
            "\"binds_generated_circuit_body_digest\":true",
            "\"requires_nonzero_generated_circuit_body\":true",
            "\"binds_native_payload_digests\":true",
            "\"rejects_inert_native_payload_digest_sentinels\":true",
            "\"rejects_non_production_native_payload_digest_sentinels\":true",
            "\"rejects_whitespace_prefixed_non_production_native_payload_digest_sentinels\":true",
            "\"rejects_case_decorated_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_whitespace_prefixed_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_padded_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_delayed_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_binary_decorated_placeholder_native_payload_digest_sentinels\":true",
            "\"binds_native_circuit_fingerprint\":true",
            "\"binds_registered_profile_digests\":true",
            "\"binds_canonical_profile_artifact_digests\":true",
            "\"binds_centered_scale_round_source_chain_digest\":true",
            "\"requires_distinct_evidence_commitments\":true",
            "\"release_audit_signoff\"",
            "\"release_audit_signoff\":{\"version\":1,\"field_count\":4",
            "\"payload_version\":1",
            "\"payload_field_count\":18",
            "\"reviewer_id_max_bytes\":128",
            "\"binds_release_audit_evidence_digest\":true",
            "\"binds_release_audit_evidence_digest\":true,\"binds_generated_circuit_body_digest\":true",
            "\"binds_centered_scale_round_source_chain_digest\":true",
            "\"binds_prover_native_payload_digest\":true",
            "\"binds_verifier_native_payload_digest\":true",
            "\"binds_external_audit_report_digest\":true",
            "\"binds_evidence_archive_digest\":true",
            "\"requires_external_audit_digests_distinct_from_signed_commitments\":true",
            "\"requires_reviewer_signature\":true",
            "\"requires_trusted_reviewer_public_key\":true",
            "\"requires_ed25519_reviewer_public_key\":true",
            "\"rejects_empty_reviewer_public_key\":true",
            "\"rejects_all_zero_reviewer_public_key\":true",
            "\"release_audit_record\"",
            "\"release_audit_record\":{\"version\":1,\"field_count\":4",
            "full_bootstrap_release_audit_record_digest.v1",
            "\"packages_release_audit_evidence\":true",
            "\"packages_release_audit_signoff\":true",
            "\"validates_record_against_governed_artifacts\":true",
            "\"release_audit_manifest\"",
            "\"release_audit_manifest\":{\"version\":1,\"field_count\":23",
            "full_bootstrap_release_audit_manifest_digest.v1",
            "\"scope\":\"iroha.crypto.fhe.bfv.full_bootstrap.release_audit.v1\"",
            "\"requires_approved_verdict\":true",
            "\"binds_release_audit_record_digest\":true",
            "\"binds_release_audit_evidence_digest\":true",
            "\"binds_centered_scale_round_source_chain_digest\":true",
            "\"binds_proof_key_pair_commitment\":true",
            "\"binds_prover_native_payload_digest\":true",
            "\"binds_verifier_native_payload_digest\":true",
            "\"binds_native_circuit_fingerprint\":true,\"binds_generated_circuit_body_digest\":true",
            "\"requires_external_audit_digests_distinct_from_signed_commitments\":true",
            "\"binds_trusted_reviewer_public_key\":true",
            "\"binds_ed25519_trusted_reviewer_public_key\":true",
            "\"rejects_empty_trusted_reviewer_public_key\":true",
            "\"rejects_all_zero_trusted_reviewer_public_key\":true",
            "\"release_audit_package\"",
            "\"release_audit_package\":{\"version\":1,\"field_count\":8",
            "full_bootstrap_release_audit_package_digest.v1",
            "\"audit_report_body_min_bytes\":64",
            "\"audit_archive_body_min_bytes\":64",
            "\"validates_release_audit_manifest_digest\":true",
            "\"validates_machine_checkable_release_verdict\":true",
            "\"validates_external_audit_report_bytes\":true",
            "\"validates_evidence_archive_bytes\":true",
            "\"validates_evidence_archive_bytes\":true,\"requires_audit_report_body_release_evidence_digest\":true,\"requires_audit_report_body_proof_profile_field_count\":true,\"requires_audit_report_body_proof_profile_canonical_base_transcript_label_obligation\":true,\"requires_audit_report_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true,\"requires_audit_report_body_proof_profile_release_prover_digest_domains\":true,\"requires_audit_report_body_proof_profile_proof_key_commitment_domains\":true,\"requires_evidence_archive_body_artifact_bundle_digest\":true,\"requires_evidence_archive_body_evaluator_artifact_set_digest\":true,\"requires_evidence_archive_body_centered_scale_round_source_chain_digest\":true,\"requires_evidence_archive_body_arithmetic_trace_profile_digest\":true,\"requires_evidence_archive_body_arithmetic_air_constraint_system_digest\":true,\"requires_evidence_archive_body_proof_profile_field_count\":true,\"requires_evidence_archive_body_proof_profile_canonical_base_transcript_label_obligation\":true,\"requires_evidence_archive_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true,\"requires_evidence_archive_body_proof_profile_release_prover_digest_domains\":true,\"requires_evidence_archive_body_proof_profile_proof_key_commitment_domains\":true,\"requires_evidence_archive_body_generated_circuit_body_digest\":true,\"requires_evidence_archive_body_generated_circuit_body_byte_length\":true,\"requires_evidence_archive_body_generated_circuit_body_hex\":true,\"requires_evidence_archive_body_coefficient_to_slot_key_artifact_hex\":true,\"requires_evidence_archive_body_slot_to_coefficient_key_artifact_hex\":true,\"requires_evidence_archive_body_blind_rotation_key_artifact_hex\":true,\"requires_evidence_archive_body_extraction_key_artifact_hex\":true,\"requires_evidence_archive_body_accumulator_artifact_hex\":true,\"requires_evidence_archive_body_proof_public_input_schema_artifact_hex\":true,\"requires_evidence_archive_body_arithmetic_air_constraint_system_artifact_hex\":true,\"requires_evidence_archive_body_native_circuit_fingerprint\":true,\"requires_evidence_archive_body_proof_key_pair_commitment\":true,\"requires_evidence_archive_body_prover_key_digest\":true,\"requires_evidence_archive_body_verifier_key_digest\":true,\"requires_evidence_archive_body_native_prover_payload_hex\":true,\"requires_evidence_archive_body_native_verifier_payload_hex\":true,\"requires_evidence_archive_body_prover_native_payload_digest\":true,\"requires_evidence_archive_body_verifier_native_payload_digest\":true,\"requires_evidence_archive_body_prover_key_artifact_hex\":true,\"requires_evidence_archive_body_verifier_key_artifact_hex\":true,\"requires_signed_commitment_standalone_label_tokens\":true,\"requires_signed_commitment_value_separator\":true,\"requires_signed_commitment_same_field_values\":true,\"requires_signed_commitment_standalone_value_tokens\":true,\"requires_signed_commitment_equals_separator\":true,\"requires_lowercase_signed_commitment_labels\":true,\"requires_lowercase_signed_commitment_hex_values\":true,\"rejects_raw_byte_signed_commitment_values\":true,\"rejects_colon_separated_signed_commitments\":true,\"rejects_uppercase_signed_commitment_labels\":true,\"rejects_uppercase_signed_commitment_values\":true,\"rejects_relabelled_signed_commitments\":true,\"rejects_cross_field_signed_commitment_replay\":true,\"rejects_punctuated_signed_commitment_values\":true,\"rejects_conflicting_duplicate_signed_commitment_labels\":true,\"rejects_duplicate_signed_commitment_labels\":true,\"rejects_same_value_duplicate_signed_commitments\":true,\"rejects_header_only_external_audit_digests\":true",
            "\"requires_evidence_archive_body_coefficient_to_slot_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_slot_to_coefficient_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_blind_rotation_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_extraction_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_accumulator_artifact_hex\":true",
            "\"requires_evidence_archive_body_proof_public_input_schema_artifact_hex\":true",
            "\"requires_evidence_archive_body_arithmetic_air_constraint_system_artifact_hex\":true",
            "\"requires_evidence_archive_body_arithmetic_trace_profile_digest\":true",
            "\"requires_evidence_archive_body_arithmetic_air_constraint_system_digest\":true",
            "\"requires_audit_report_body_proof_profile_field_count\":true",
            "\"requires_audit_report_body_proof_profile_canonical_base_transcript_label_obligation\":true",
            "\"requires_audit_report_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true",
            "\"requires_audit_report_body_proof_profile_release_prover_digest_domains\":true",
            "\"requires_audit_report_body_proof_profile_proof_key_commitment_domains\":true",
            "\"requires_evidence_archive_body_proof_profile_field_count\":true",
            "\"requires_evidence_archive_body_proof_profile_canonical_base_transcript_label_obligation\":true",
            "\"requires_evidence_archive_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true",
            "\"requires_evidence_archive_body_proof_profile_release_prover_digest_domains\":true",
            "\"requires_evidence_archive_body_proof_profile_proof_key_commitment_domains\":true",
            "\"rejects_nested_header_external_audit_digests\":true",
            "\"rejects_header_only_external_audit_digests\":true,\"rejects_nested_header_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_nested_header_external_audit_digests\":true",
            "\"rejects_zero_body_external_audit_digests\":true",
            "\"rejects_blank_body_external_audit_digests\":true",
            "\"rejects_padded_zero_body_external_audit_digests\":true",
            "\"rejects_padded_blank_body_external_audit_digests\":true",
            "\"rejects_case_decorated_placeholder_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_placeholder_external_audit_digests\":true",
            "\"rejects_delayed_placeholder_external_audit_digests\":true",
            "\"rejects_placeholder_external_audit_digests\":true",
            "\"rejects_all_zero_audit_artifacts\":true",
            "\"requires_canonical_audit_artifact_headers\":true",
            "\"requires_nonempty_audit_artifact_bodies\":true",
            "\"requires_minimum_audit_artifact_body_bytes\":true",
            "\"rejects_blank_audit_artifact_bodies\":true",
            "\"rejects_zero_body_audit_artifacts\":true",
            "\"rejects_nested_audit_artifact_bodies\":true",
            "\"rejects_whitespace_prefixed_nested_audit_artifact_bodies\":true",
            "\"rejects_delayed_nested_audit_artifact_bodies\":true",
            "\"rejects_placeholder_audit_artifact_bodies\":true",
            "\"rejects_delayed_placeholder_audit_artifact_bodies\":true",
            "\"rejects_delayed_non_production_audit_artifact_bodies\":true",
            "\"scans_entire_placeholder_audit_artifact_bodies\":true",
            "\"requires_distinct_audit_artifact_bodies\":true",
            "\"requires_external_review_report_marker\":true",
            "\"requires_external_review_archive_marker\":true",
            "\"requires_byte_leading_external_review_report_marker\":true",
            "\"requires_byte_leading_external_review_archive_marker\":true",
            "\"requires_lowercase_external_review_markers\":true",
            "\"requires_external_review_marker_colon_separator\":true",
            "\"requires_external_review_marker_statement\":true",
            "\"requires_external_review_marker_statement_text\":true",
            "\"requires_printable_ascii_external_review_marker_statement_text\":true",
            "\"requires_distinct_external_review_marker_statements\":true",
            "\"requires_external_review_marker_reviewer_id\":true",
            "\"requires_external_review_marker_reviewer_id_label\":true",
            "\"requires_single_external_review_marker_reviewer_id_label\":true",
            "\"rejects_conflicting_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_duplicate_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_case_drifted_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_separator_alias_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_missing_external_review_marker_colon_separator\":true",
            "\"rejects_padded_colon_external_review_marker_aliases\":true",
            "\"rejects_empty_external_review_marker_statement\":true",
            "\"rejects_generic_external_review_marker_statement\":true",
            "\"rejects_uppercase_external_review_markers\":true",
            "\"rejects_whitespace_prefixed_external_review_markers\":true",
            "\"rejects_machine_generated_audit_artifact_bodies\":true",
            "\"rejects_separator_obfuscated_machine_generated_audit_artifact_bodies\":true",
            "\"validates_package_against_governed_artifacts\":true",
            "\"rejects_placeholder_reviewer_ids\":true",
            "\"validates_trusted_reviewer_public_key\":true",
            "\"requires_ed25519_trusted_reviewer_public_key\":true",
            "\"rejects_empty_trusted_reviewer_public_key\":true",
            "\"rejects_all_zero_trusted_reviewer_public_key\":true",
            "\"requires_caller_pinned_package_digest\":true",
            "\"validates_caller_pinned_package_digest\":true",
            "\"rejects_placeholder_caller_pinned_package_digest\":true",
            "\"rejects_binary_framed_placeholder_caller_pinned_package_digest\":true",
            "\"rejects_placeholder_package_record_digest\":true",
            "\"rejects_placeholder_package_manifest_digest\":true",
            "\"rejects_caller_pinned_record_digest_alias\":true",
            "\"rejects_caller_pinned_manifest_digest_alias\":true",
            "\"rejects_caller_pinned_signed_commitment_digest_alias\":true",
            "\"rejects_caller_pinned_centered_scale_round_source_chain_digest_alias\":true",
        ] {
            assert!(
                schema.contains(expected),
                "public schema must advertise material proof input contract `{expected}`"
            );
        }
        assert!(
            schema.contains(proof_input_material_digest_domain),
            "public schema must advertise the material proof input digest domain"
        );
        assert_eq!(
            schema.matches("\"proof_key_commitment_domains\"").count(),
            1,
            "material schema must advertise proof-key commitment domains in release-audit evidence"
        );
        assert_eq!(
            schema.matches("\"artifact_digest_domains\"").count(),
            1,
            "material schema must advertise artifact digest domains in release-audit evidence"
        );
        #[cfg(feature = "json")]
        {
            let schema_value: Value = json::from_slice(
                SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            )
            .expect("full-bootstrap material proof schema must parse as Norito JSON");
            assert_schema_string_field(
                &schema_value,
                "statement_digest_domain",
                statement_digest_domain,
                "full-bootstrap material proof schema",
            );
            assert_soracloud_proof_key_commitment_domains(
                &schema_value,
                "/release_audit_evidence/proof_key_commitment_domains",
                "full-bootstrap material release-audit evidence schema",
                proof_key_material_commitment_domain,
                proof_key_pair_commitment_domain,
            );
            assert_soracloud_artifact_digest_domains(
                &schema_value,
                "/release_audit_evidence/artifact_digest_domains",
                "full-bootstrap material release-audit evidence schema",
                circuit_material_digest_domain,
                evaluator_artifact_set_digest_domain,
                circuit_artifact_bundle_digest_domain,
            );
            assert_soracloud_material_schema_sections(
                &schema_value,
                "full-bootstrap material proof schema",
            );
            assert_soracloud_release_audit_schema_sections(
                &schema_value,
                "full-bootstrap material proof schema",
            );
        }
        assert!(
            !schema.contains("rejects_delayed_nested_header_external_audit_digests"),
            "digest-only material schema must not advertise arbitrary delayed nested-header rejection without audit artifact bytes"
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "schema golden test keeps audited contract terms inline"
    )]
    fn soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest() {
        let schema = std::str::from_utf8(
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
        )
        .expect("full-bootstrap execution schema is valid UTF-8");
        let statement_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_STATEMENT_DOMAIN,
        )
        .expect("execution proof statement digest domain is valid UTF-8");
        let proof_input_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("execution proof input digest domain is valid UTF-8");
        let prover_input_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("execution prover input digest domain is valid UTF-8");
        let air_evaluation_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic AIR evaluation digest domain is valid UTF-8");
        let public_opening_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace public-opening material digest domain is valid UTF-8");
        let trace_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace material digest domain is valid UTF-8");
        let air_constraint_system_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_DIGEST_DOMAIN,
        )
        .expect("arithmetic AIR constraint-system digest domain is valid UTF-8");
        let proof_key_material_commitment_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_COMMITMENT_DOMAIN,
        )
        .expect("proof-key material commitment domain is valid UTF-8");
        let proof_key_pair_commitment_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_PAIR_COMMITMENT_DOMAIN,
        )
        .expect("proof-key pair commitment domain is valid UTF-8");
        let circuit_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("full-bootstrap circuit-material digest domain is valid UTF-8");
        let evaluator_artifact_set_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EVALUATOR_ARTIFACT_SET_DIGEST_DOMAIN,
        )
        .expect("full-bootstrap evaluator-artifact-set digest domain is valid UTF-8");
        let circuit_artifact_bundle_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ARTIFACT_BUNDLE_DIGEST_DOMAIN,
        )
        .expect("full-bootstrap circuit-artifact-bundle digest domain is valid UTF-8");
        assert!(
            schema.contains("execution_witness_digest"),
            "public schema must advertise the execution witness digest bound by the typed claim"
        );
        assert!(
            schema.contains("galois_key_set_digest,execution_witness_digest"),
            "public schema must advertise the claim-level Galois key-set digest before the witness digest"
        );
        assert!(
            schema.contains("claim.galois_key_set_digest matches supplied galois_keys"),
            "public schema must advertise artifact-aware claim Galois-key preflight"
        );
        assert!(
            schema.contains("full_bootstrap_execution_proof_statement_digest(version,field_count,"),
            "public schema must advertise the self-describing statement material header"
        );
        assert!(
            schema.contains("full_bootstrap_execution_witness_digest.v1"),
            "public schema must advertise the execution witness digest domain"
        );
        for required in [
            "\"material_version\":1",
            "\"material_field_count\":15",
            "\"trace_field_count\":7",
            "\"trace_bounds_field_count\":6",
            "\"binds_galois_key_set_digest\":true",
            "\"binds_trace\":true",
            "\"binds_trace_bounds\":true",
        ] {
            assert!(
                schema.contains(required),
                "public schema must advertise witness layout term {required}"
            );
        }
        for required in [
            "\"proof_backend\":\"stark/fri/sha256-goldilocks\"",
            "\"statement_digest_domain\"",
            statement_digest_domain,
            "\"arithmetic_trace_profile\"",
            "\"field_count\":34",
            "\"row_width\":34",
            "\"private_row_count\":64",
            "\"private_row_kind\":1",
            "\"public_row_kind\":0",
            "\"forbids_unmasked_private_row_openings\":true",
            "\"forbids_duplicate_openings\":true",
            "\"arithmetic_air_contract\"",
            "\"arithmetic_air_contract\":{\"version\":1,\"field_count\":37",
            "\"binds_constraint_system_digest\":true",
            "\"enforces_goldilocks_field_canonicality\":true",
            "\"enforces_row_kind_partition\":true",
            "\"enforces_active_rows_match_witness_material\":true",
            "\"enforces_full_bootstrap_arithmetic_constraints\":true",
            "\"enforces_public_padding_rows\":true",
            "\"enforces_statement_hash_nonzero\":true",
            "\"enforces_trace_output_matches_claim\":true",
            "\"enforces_trace_bound_matches_claim\":true",
            "\"enforces_no_unmasked_private_row_openings\":true",
            "\"enforces_duplicate_free_openings\":true",
            "\"derives_opening_schedule_from_statement_hash\":true",
            "\"derives_opening_schedule_from_trace_material_digest\":true",
            "\"bounds_opening_schedule_rejection_sampling\":true",
            "\"validates_transcript_public_padding_openings\":true",
            "\"composition_challenge_domain\":\"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_air_composition_challenge.v1\"",
            "\"composition_challenge_digest_bytes\":32",
            "\"binds_composition_challenges_to_statement_hash\":true",
            "\"binds_composition_challenges_to_trace_material_digest\":true",
            "\"binds_composition_challenges_to_row_index\":true",
            "\"binds_composition_challenges_to_column_index\":true",
            "\"maps_zero_composition_challenge_to_one\":true",
            "\"native_air_envelope\"",
            "\"native_air_envelope\":{\"validates_stark_parameter_profile\":true",
            "\"binds_domain_tag_to_statement_hash\":true",
            "\"requires_public_padding_context\":true",
            "\"requires_verifier_owned_trace_material_digest\":true",
            "\"rejects_auxiliary_composition_value_commitments\":true",
            "\"binds_trace_root_to_governed_arithmetic_trace\":true",
            "\"binds_composition_root_to_governed_air_evaluation\":true",
            "\"binds_opened_rows_to_governed_arithmetic_trace\":true",
            "\"requires_zero_public_padding_composition_values\":true",
            "\"validates_merkle_path_roots\":true",
            "\"validates_fri_query_chain\":true",
            "\"binds_first_fri_values_to_opened_air_values\":true",
            "\"binds_fri_queries_to_air_commitment_roots\":true",
            "\"requires_canonical_base_transcript_label\":true",
            "\"rejects_suffixed_transcript_label_aliases\":true",
            "\"rejects_blank_native_envelope_bytes\":true",
            "\"rejects_placeholder_native_envelope_text\":true",
            "\"artifact_bundle\"",
            "\"artifact_digest_count\":9",
            "\"binds_arithmetic_air_constraint_system_artifact\":true",
            "\"validates_arithmetic_air_constraint_system_material\":true",
            "\"release_prover_input\"",
            "\"proof_input_material_version\":1",
            "\"proof_input_material_field_count\":5",
            "\"proof_input_material_digest_domain\"",
            "\"release_prover_digest_domains\"",
            proof_input_material_digest_domain,
            prover_input_material_digest_domain,
            air_evaluation_material_digest_domain,
            public_opening_material_digest_domain,
            trace_material_digest_domain,
            air_constraint_system_digest_domain,
            "\"separates_release_prover_material_domains\":true",
            "\"hashes_proof_input_material\":true",
            "\"prover_input_material_version\":1",
            "\"prover_input_material_field_count\":13",
            "\"air_evaluation_material_version\":1",
            "\"air_evaluation_material_field_count\":8",
            "\"public_opening_material_version\":1",
            "\"public_opening_material_field_count\":13",
            "\"binds_release_prover_arithmetic_air_constraint_system_digest\":true",
            "\"binds_release_prover_arithmetic_air_constraint_system_artifact_digest\":true",
            "\"binds_release_prover_arithmetic_air_evaluation_material_digest\":true",
            "\"binds_arithmetic_air_evaluation_trace_material_digest\":true",
            "\"requires_zero_arithmetic_air_composition_values\":true",
            "\"binds_arithmetic_trace_material_digest\":true",
            "\"binds_trace_proof_input_consistency\":true",
            "\"binds_generated_proof_key_pair\":true",
            "\"proof_key_commitment_domains\"",
            proof_key_material_commitment_domain,
            proof_key_pair_commitment_domain,
            "\"separates_material_and_pair\":true",
            "\"binds_release_prover_verifier_key\":true",
            "\"validates_artifact_bound_prover_input\":true",
            "\"rejects_stale_galois_key_set_replay\":true",
            "\"rejects_stale_proof_key_artifacts\":true",
            "\"derives_opening_schedule_from_statement_hash\":true",
            "\"derives_opening_schedule_from_trace_material_digest\":true",
            "\"bounds_opening_schedule_rejection_sampling\":true",
            "\"validates_transcript_public_padding_openings\":true",
            "\"validates_transcript_public_opening_material\":true",
            "\"requires_verifier_owned_trace_material_digest\":true",
            "\"requires_canonical_base_transcript_label\":true",
            "\"rejects_suffixed_transcript_label_aliases\":true",
            "\"release_audit_evidence\"",
            "\"release_audit_evidence\":{\"version\":1,\"field_count\":23",
            "full_bootstrap_release_audit_evidence_digest.v1",
            "\"proof_profile_field_count\":58",
            "\"proof_profile_air_evaluation_material_version\":1",
            "\"proof_profile_air_evaluation_material_field_count\":8",
            "\"proof_profile_public_opening_material_version\":1",
            "\"proof_profile_public_opening_material_field_count\":13",
            "\"proof_profile_release_prover_digest_domains\"",
            proof_input_material_digest_domain,
            prover_input_material_digest_domain,
            air_evaluation_material_digest_domain,
            public_opening_material_digest_domain,
            trace_material_digest_domain,
            air_constraint_system_digest_domain,
            "\"proof_profile_proof_key_commitment_domains\"",
            proof_key_material_commitment_domain,
            proof_key_pair_commitment_domain,
            "\"proof_profile_validates_air_evaluation_material_digest\":true",
            "\"proof_profile_validates_air_evaluation_trace_material_digest\":true",
            "\"proof_profile_requires_zero_air_composition_values\":true",
            "\"proof_profile_validates_artifact_bound_prover_input\":true",
            "\"proof_profile_rejects_stale_galois_key_set_replay\":true",
            "\"proof_profile_rejects_stale_proof_key_artifacts\":true",
            "\"proof_profile_derives_opening_schedule_from_statement_hash\":true",
            "\"proof_profile_derives_opening_schedule_from_trace_material_digest\":true",
            "\"proof_profile_bounds_opening_schedule_rejection_sampling\":true",
            "\"proof_profile_validates_transcript_public_padding_openings\":true",
            "\"proof_profile_validates_transcript_public_opening_material\":true",
            "\"proof_profile_requires_verifier_owned_trace_material_digest\":true",
            "\"proof_profile_requires_canonical_base_transcript_label\":true",
            "\"proof_profile_rejects_suffixed_transcript_label_aliases\":true",
            "\"proof_profile_validates_merkle_path_shape\":true",
            "\"proof_profile_validates_merkle_path_roots\":true",
            "\"proof_profile_validates_fri_query_chain\":true",
            "\"proof_profile_binds_first_fri_values_to_opened_air_values\":true",
            "\"proof_profile_binds_fri_queries_to_air_commitment_roots\":true",
            "\"proof_profile_binds_centered_scale_round_source_chain_digest\":true",
            "\"key_evidence_field_count\":11",
            "\"key_evidence_binds_centered_scale_round_source_chain_digest\":true",
            "\"artifact_digest_domains\"",
            circuit_material_digest_domain,
            evaluator_artifact_set_digest_domain,
            circuit_artifact_bundle_digest_domain,
            "\"separates_material_set_and_bundle\":true",
            "\"binds_artifact_bundle_digest\":true",
            "\"binds_evaluator_artifact_set_digest\":true",
            "\"binds_coefficient_to_slot_key_digest\":true",
            "\"binds_slot_to_coefficient_key_digest\":true",
            "\"binds_blind_rotation_key_digest\":true",
            "\"binds_sample_extraction_key_digest\":true",
            "\"binds_accumulator_digest\":true",
            "\"binds_proof_public_input_schema_digest\":true",
            "\"binds_arithmetic_trace_profile_digest\":true",
            "\"binds_arithmetic_air_constraint_system_digest\":true",
            "\"binds_arithmetic_air_constraint_system_artifact_digest\":true",
            "\"binds_native_payload_digests\":true",
            "\"binds_generated_circuit_body_digest\":true",
            "\"requires_nonzero_generated_circuit_body\":true",
            "\"rejects_inert_native_payload_digest_sentinels\":true",
            "\"rejects_non_production_native_payload_digest_sentinels\":true",
            "\"rejects_whitespace_prefixed_non_production_native_payload_digest_sentinels\":true",
            "\"rejects_case_decorated_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_whitespace_prefixed_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_padded_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_delayed_placeholder_native_payload_digest_sentinels\":true",
            "\"rejects_binary_decorated_placeholder_native_payload_digest_sentinels\":true",
            "\"binds_native_circuit_fingerprint\":true",
            "\"binds_registered_profile_digests\":true",
            "\"binds_canonical_profile_artifact_digests\":true",
            "\"binds_centered_scale_round_source_chain_digest\":true",
            "\"requires_distinct_evidence_commitments\":true",
            "\"release_audit_signoff\"",
            "\"release_audit_signoff\":{\"version\":1,\"field_count\":4",
            "\"payload_version\":1",
            "\"payload_field_count\":18",
            "\"reviewer_id_max_bytes\":128",
            "\"rejects_placeholder_reviewer_ids\":true",
            "\"binds_release_audit_evidence_digest\":true",
            "\"binds_release_audit_evidence_digest\":true,\"binds_generated_circuit_body_digest\":true",
            "\"binds_centered_scale_round_source_chain_digest\":true",
            "\"binds_prover_native_payload_digest\":true",
            "\"binds_verifier_native_payload_digest\":true",
            "\"binds_external_audit_report_digest\":true",
            "\"binds_evidence_archive_digest\":true",
            "\"requires_external_audit_digests_distinct_from_signed_commitments\":true",
            "\"rejects_header_only_external_audit_digests\":true",
            "\"rejects_nested_header_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_nested_header_external_audit_digests\":true",
            "\"rejects_zero_body_external_audit_digests\":true",
            "\"rejects_blank_body_external_audit_digests\":true",
            "\"rejects_padded_zero_body_external_audit_digests\":true",
            "\"rejects_padded_blank_body_external_audit_digests\":true",
            "\"rejects_case_decorated_placeholder_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_placeholder_external_audit_digests\":true",
            "\"rejects_delayed_placeholder_external_audit_digests\":true",
            "\"rejects_placeholder_external_audit_digests\":true",
            "\"requires_reviewer_signature\":true",
            "\"requires_trusted_reviewer_public_key\":true",
            "\"requires_ed25519_reviewer_public_key\":true",
            "\"rejects_empty_reviewer_public_key\":true",
            "\"rejects_all_zero_reviewer_public_key\":true",
            "\"release_audit_record\"",
            "\"release_audit_record\":{\"version\":1,\"field_count\":4",
            "full_bootstrap_release_audit_record_digest.v1",
            "\"packages_release_audit_evidence\":true",
            "\"packages_release_audit_signoff\":true",
            "\"validates_record_against_governed_artifacts\":true",
            "\"release_audit_manifest\"",
            "\"release_audit_manifest\":{\"version\":1,\"field_count\":23",
            "full_bootstrap_release_audit_manifest_digest.v1",
            "\"scope\":\"iroha.crypto.fhe.bfv.full_bootstrap.release_audit.v1\"",
            "\"requires_approved_verdict\":true",
            "\"rejects_placeholder_reviewer_ids\":true",
            "\"binds_release_audit_record_digest\":true",
            "\"binds_release_audit_evidence_digest\":true",
            "\"binds_centered_scale_round_source_chain_digest\":true",
            "\"binds_artifact_bundle_digest\":true",
            "\"binds_evaluator_artifact_set_digest\":true",
            "\"binds_proof_key_pair_commitment\":true",
            "\"binds_prover_native_payload_digest\":true",
            "\"binds_verifier_native_payload_digest\":true",
            "\"binds_native_circuit_fingerprint\":true,\"binds_generated_circuit_body_digest\":true",
            "\"binds_external_audit_report_digest\":true",
            "\"binds_evidence_archive_digest\":true",
            "\"requires_external_audit_digests_distinct_from_signed_commitments\":true",
            "\"rejects_header_only_external_audit_digests\":true",
            "\"rejects_nested_header_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_nested_header_external_audit_digests\":true",
            "\"rejects_zero_body_external_audit_digests\":true",
            "\"rejects_blank_body_external_audit_digests\":true",
            "\"rejects_padded_zero_body_external_audit_digests\":true",
            "\"rejects_padded_blank_body_external_audit_digests\":true",
            "\"rejects_case_decorated_placeholder_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_placeholder_external_audit_digests\":true",
            "\"rejects_placeholder_external_audit_digests\":true",
            "\"binds_trusted_reviewer_public_key\":true",
            "\"binds_ed25519_trusted_reviewer_public_key\":true",
            "\"rejects_empty_trusted_reviewer_public_key\":true",
            "\"rejects_all_zero_trusted_reviewer_public_key\":true",
            "\"release_audit_package\"",
            "\"release_audit_package\":{\"version\":1,\"field_count\":8",
            "full_bootstrap_release_audit_package_digest.v1",
            "\"audit_report_max_bytes\":16777216",
            "\"audit_archive_max_bytes\":134217728",
            "\"audit_report_body_min_bytes\":64",
            "\"audit_archive_body_min_bytes\":64",
            "\"packages_release_audit_record\":true",
            "\"packages_release_audit_manifest\":true",
            "\"validates_release_audit_manifest_digest\":true",
            "\"validates_machine_checkable_release_verdict\":true",
            "\"validates_external_audit_report_bytes\":true",
            "\"validates_evidence_archive_bytes\":true",
            "\"validates_evidence_archive_bytes\":true,\"requires_audit_report_body_release_evidence_digest\":true,\"requires_audit_report_body_proof_profile_field_count\":true,\"requires_audit_report_body_proof_profile_canonical_base_transcript_label_obligation\":true,\"requires_audit_report_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true,\"requires_audit_report_body_proof_profile_release_prover_digest_domains\":true,\"requires_audit_report_body_proof_profile_proof_key_commitment_domains\":true,\"requires_evidence_archive_body_artifact_bundle_digest\":true,\"requires_evidence_archive_body_evaluator_artifact_set_digest\":true,\"requires_evidence_archive_body_centered_scale_round_source_chain_digest\":true,\"requires_evidence_archive_body_arithmetic_trace_profile_digest\":true,\"requires_evidence_archive_body_arithmetic_air_constraint_system_digest\":true,\"requires_evidence_archive_body_proof_profile_field_count\":true,\"requires_evidence_archive_body_proof_profile_canonical_base_transcript_label_obligation\":true,\"requires_evidence_archive_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true,\"requires_evidence_archive_body_proof_profile_release_prover_digest_domains\":true,\"requires_evidence_archive_body_proof_profile_proof_key_commitment_domains\":true,\"requires_evidence_archive_body_generated_circuit_body_digest\":true,\"requires_evidence_archive_body_generated_circuit_body_byte_length\":true,\"requires_evidence_archive_body_generated_circuit_body_hex\":true,\"requires_evidence_archive_body_coefficient_to_slot_key_artifact_hex\":true,\"requires_evidence_archive_body_slot_to_coefficient_key_artifact_hex\":true,\"requires_evidence_archive_body_blind_rotation_key_artifact_hex\":true,\"requires_evidence_archive_body_extraction_key_artifact_hex\":true,\"requires_evidence_archive_body_accumulator_artifact_hex\":true,\"requires_evidence_archive_body_proof_public_input_schema_artifact_hex\":true,\"requires_evidence_archive_body_arithmetic_air_constraint_system_artifact_hex\":true,\"requires_evidence_archive_body_native_circuit_fingerprint\":true,\"requires_evidence_archive_body_proof_key_pair_commitment\":true,\"requires_evidence_archive_body_prover_key_digest\":true,\"requires_evidence_archive_body_verifier_key_digest\":true,\"requires_evidence_archive_body_native_prover_payload_hex\":true,\"requires_evidence_archive_body_native_verifier_payload_hex\":true,\"requires_evidence_archive_body_prover_native_payload_digest\":true,\"requires_evidence_archive_body_verifier_native_payload_digest\":true,\"requires_evidence_archive_body_prover_key_artifact_hex\":true,\"requires_evidence_archive_body_verifier_key_artifact_hex\":true,\"requires_signed_commitment_standalone_label_tokens\":true,\"requires_signed_commitment_value_separator\":true,\"requires_signed_commitment_same_field_values\":true,\"requires_signed_commitment_standalone_value_tokens\":true,\"requires_signed_commitment_equals_separator\":true,\"requires_lowercase_signed_commitment_labels\":true,\"requires_lowercase_signed_commitment_hex_values\":true,\"rejects_raw_byte_signed_commitment_values\":true,\"rejects_colon_separated_signed_commitments\":true,\"rejects_uppercase_signed_commitment_labels\":true,\"rejects_uppercase_signed_commitment_values\":true,\"rejects_relabelled_signed_commitments\":true,\"rejects_cross_field_signed_commitment_replay\":true,\"rejects_punctuated_signed_commitment_values\":true,\"rejects_conflicting_duplicate_signed_commitment_labels\":true,\"rejects_duplicate_signed_commitment_labels\":true,\"rejects_same_value_duplicate_signed_commitments\":true,\"rejects_header_only_external_audit_digests\":true",
            "\"requires_evidence_archive_body_coefficient_to_slot_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_slot_to_coefficient_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_blind_rotation_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_extraction_key_artifact_hex\":true",
            "\"requires_evidence_archive_body_accumulator_artifact_hex\":true",
            "\"requires_evidence_archive_body_proof_public_input_schema_artifact_hex\":true",
            "\"requires_evidence_archive_body_arithmetic_air_constraint_system_artifact_hex\":true",
            "\"requires_evidence_archive_body_arithmetic_trace_profile_digest\":true",
            "\"requires_evidence_archive_body_arithmetic_air_constraint_system_digest\":true",
            "\"requires_audit_report_body_proof_profile_field_count\":true",
            "\"requires_audit_report_body_proof_profile_canonical_base_transcript_label_obligation\":true",
            "\"requires_audit_report_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true",
            "\"requires_audit_report_body_proof_profile_release_prover_digest_domains\":true",
            "\"requires_audit_report_body_proof_profile_proof_key_commitment_domains\":true",
            "\"requires_evidence_archive_body_proof_profile_field_count\":true",
            "\"requires_evidence_archive_body_proof_profile_canonical_base_transcript_label_obligation\":true",
            "\"requires_evidence_archive_body_proof_profile_rejects_suffixed_transcript_label_aliases\":true",
            "\"requires_evidence_archive_body_proof_profile_release_prover_digest_domains\":true",
            "\"requires_evidence_archive_body_proof_profile_proof_key_commitment_domains\":true",
            "\"rejects_header_only_external_audit_digests\":true,\"rejects_nested_header_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_nested_header_external_audit_digests\":true",
            "\"rejects_zero_body_external_audit_digests\":true",
            "\"rejects_blank_body_external_audit_digests\":true",
            "\"rejects_padded_zero_body_external_audit_digests\":true",
            "\"rejects_padded_blank_body_external_audit_digests\":true",
            "\"rejects_case_decorated_placeholder_external_audit_digests\":true",
            "\"rejects_whitespace_prefixed_placeholder_external_audit_digests\":true",
            "\"rejects_placeholder_external_audit_digests\":true",
            "\"rejects_all_zero_audit_artifacts\":true",
            "\"requires_canonical_audit_artifact_headers\":true",
            "\"requires_nonempty_audit_artifact_bodies\":true",
            "\"requires_minimum_audit_artifact_body_bytes\":true",
            "\"rejects_blank_audit_artifact_bodies\":true",
            "\"rejects_zero_body_audit_artifacts\":true",
            "\"rejects_nested_audit_artifact_bodies\":true",
            "\"rejects_whitespace_prefixed_nested_audit_artifact_bodies\":true",
            "\"rejects_delayed_nested_audit_artifact_bodies\":true",
            "\"rejects_placeholder_audit_artifact_bodies\":true",
            "\"rejects_delayed_placeholder_audit_artifact_bodies\":true",
            "\"rejects_delayed_non_production_audit_artifact_bodies\":true",
            "\"scans_entire_placeholder_audit_artifact_bodies\":true",
            "\"requires_distinct_audit_artifact_bodies\":true",
            "\"requires_external_review_report_marker\":true",
            "\"requires_external_review_archive_marker\":true",
            "\"requires_byte_leading_external_review_report_marker\":true",
            "\"requires_byte_leading_external_review_archive_marker\":true",
            "\"requires_lowercase_external_review_markers\":true",
            "\"requires_external_review_marker_colon_separator\":true",
            "\"requires_external_review_marker_statement\":true",
            "\"requires_external_review_marker_statement_text\":true",
            "\"requires_printable_ascii_external_review_marker_statement_text\":true",
            "\"requires_distinct_external_review_marker_statements\":true",
            "\"requires_external_review_marker_reviewer_id\":true",
            "\"requires_external_review_marker_reviewer_id_label\":true",
            "\"requires_single_external_review_marker_reviewer_id_label\":true",
            "\"rejects_conflicting_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_duplicate_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_case_drifted_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_separator_alias_external_review_marker_reviewer_id_labels\":true",
            "\"rejects_missing_external_review_marker_colon_separator\":true",
            "\"rejects_padded_colon_external_review_marker_aliases\":true",
            "\"rejects_empty_external_review_marker_statement\":true",
            "\"rejects_generic_external_review_marker_statement\":true",
            "\"rejects_uppercase_external_review_markers\":true",
            "\"rejects_whitespace_prefixed_external_review_markers\":true",
            "\"rejects_machine_generated_audit_artifact_bodies\":true",
            "\"rejects_separator_obfuscated_machine_generated_audit_artifact_bodies\":true",
            "\"validates_package_against_governed_artifacts\":true",
            "\"rejects_placeholder_reviewer_ids\":true",
            "\"validates_trusted_reviewer_public_key\":true",
            "\"requires_ed25519_trusted_reviewer_public_key\":true",
            "\"rejects_empty_trusted_reviewer_public_key\":true",
            "\"rejects_all_zero_trusted_reviewer_public_key\":true",
            "\"requires_caller_pinned_package_digest\":true",
            "\"validates_caller_pinned_package_digest\":true",
            "\"rejects_placeholder_caller_pinned_package_digest\":true",
            "\"rejects_binary_framed_placeholder_caller_pinned_package_digest\":true",
            "\"rejects_placeholder_package_record_digest\":true",
            "\"rejects_placeholder_package_manifest_digest\":true",
            "\"rejects_caller_pinned_record_digest_alias\":true",
            "\"rejects_caller_pinned_manifest_digest_alias\":true",
            "\"rejects_caller_pinned_signed_commitment_digest_alias\":true",
            "\"rejects_caller_pinned_centered_scale_round_source_chain_digest_alias\":true",
        ] {
            assert!(
                schema.contains(required),
                "public schema must advertise release-prover trace/key term {required}"
            );
        }
        assert!(
            schema.contains(proof_input_material_digest_domain),
            "public schema must advertise the execution proof input digest domain"
        );
        assert_eq!(
            schema.matches("\"proof_key_commitment_domains\"").count(),
            2,
            "execution schema must advertise proof-key commitment domains in release-prover input and release-audit evidence"
        );
        assert_eq!(
            schema.matches("\"artifact_digest_domains\"").count(),
            1,
            "execution schema must advertise artifact digest domains in release-audit evidence"
        );
        #[cfg(feature = "json")]
        {
            let schema_value: Value = json::from_slice(
                SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            )
            .expect("full-bootstrap execution proof schema must parse as Norito JSON");
            assert_schema_string_field(
                &schema_value,
                "statement_digest_domain",
                statement_digest_domain,
                "full-bootstrap execution proof schema",
            );
            for (pointer, context) in [
                (
                    "/release_prover_input/proof_key_commitment_domains",
                    "full-bootstrap execution release-prover input schema",
                ),
                (
                    "/release_audit_evidence/proof_key_commitment_domains",
                    "full-bootstrap execution release-audit evidence schema",
                ),
            ] {
                assert_soracloud_proof_key_commitment_domains(
                    &schema_value,
                    pointer,
                    context,
                    proof_key_material_commitment_domain,
                    proof_key_pair_commitment_domain,
                );
            }
            assert_soracloud_artifact_digest_domains(
                &schema_value,
                "/release_audit_evidence/artifact_digest_domains",
                "full-bootstrap execution release-audit evidence schema",
                circuit_material_digest_domain,
                evaluator_artifact_set_digest_domain,
                circuit_artifact_bundle_digest_domain,
            );
            assert_soracloud_execution_schema_sections(
                &schema_value,
                "full-bootstrap execution proof schema",
            );
            assert_soracloud_release_audit_schema_sections(
                &schema_value,
                "full-bootstrap execution proof schema",
            );
        }
        assert!(
            !schema.contains("rejects_delayed_nested_header_external_audit_digests"),
            "digest-only execution schema must not advertise arbitrary delayed nested-header rejection without audit artifact bytes"
        );
        assert!(
            !schema.contains("output_bound))"),
            "public schema must not advertise the pre-witness execution claim layout"
        );
    }

    #[test]
    fn soracloud_fhe_proof_validate_rejects_zero_prehash_statement_hashes() {
        let zero_statement = zero_prehash_statement_hash();

        let mut admission = sample_fhe_input_admission_proof();
        admission.statement_hash = zero_statement;
        let envelope =
            open_verify_envelope_with_statement(&admission.proof.proof.bytes, zero_statement);
        replace_fhe_input_admission_open_verify_envelope(&mut admission, &envelope);
        let err = admission
            .validate()
            .expect_err("input admission proof must reject zero statement sentinel");
        assert_zero_statement_hash_error(&err);

        let mut bootstrap = sample_fhe_bootstrap_key_proof();
        bootstrap.statement_hash = zero_statement;
        let envelope =
            open_verify_envelope_with_statement(&bootstrap.proof.proof.bytes, zero_statement);
        replace_fhe_bootstrap_key_open_verify_envelope(&mut bootstrap, &envelope);
        let err = bootstrap
            .validate()
            .expect_err("bootstrap-key proof must reject zero statement sentinel");
        assert_zero_statement_hash_error(&err);

        let mut material = sample_fhe_full_bootstrap_material_proof();
        material.statement_hash = zero_statement;
        let envelope =
            open_verify_envelope_with_statement(&material.proof.proof.bytes, zero_statement);
        replace_fhe_full_bootstrap_material_open_verify_envelope(&mut material, &envelope);
        let err = material
            .validate()
            .expect_err("full-bootstrap material proof must reject zero statement sentinel");
        assert_zero_statement_hash_error(&err);

        let mut execution = sample_fhe_full_bootstrap_execution_proof();
        execution.statement_hash = zero_statement;
        let envelope =
            open_verify_envelope_with_statement(&execution.proof.proof.bytes, zero_statement);
        replace_fhe_full_bootstrap_execution_open_verify_envelope(&mut execution, &envelope);
        let err = execution
            .validate()
            .expect_err("full-bootstrap execution proof must reject zero statement sentinel");
        assert_zero_statement_hash_error(&err);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn soracloud_fhe_proof_validate_rejects_textual_placeholder_native_envelope_only() {
        let placeholder_native = b"placeholder native STARK envelope".to_vec();

        let mut admission = sample_fhe_input_admission_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &admission.proof.proof.bytes,
            placeholder_native.clone(),
        );
        replace_fhe_input_admission_open_verify_envelope(&mut admission, &envelope);
        let err = admission
            .validate()
            .expect_err("input-admission proof must reject placeholder native envelope text");
        assert_native_envelope_error(&err, "placeholder or non-production text");

        let mut public_key = sample_fhe_public_key_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &public_key.proof.proof.bytes,
            placeholder_native.clone(),
        );
        replace_fhe_public_key_open_verify_envelope(&mut public_key, &envelope);
        let err = public_key
            .validate()
            .expect_err("public-key proof must reject placeholder native envelope text");
        assert_native_envelope_error(&err, "placeholder or non-production text");

        let mut bootstrap = sample_fhe_bootstrap_key_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &bootstrap.proof.proof.bytes,
            placeholder_native.clone(),
        );
        replace_fhe_bootstrap_key_open_verify_envelope(&mut bootstrap, &envelope);
        let err = bootstrap
            .validate()
            .expect_err("bootstrap-key proof must reject placeholder native envelope text");
        assert_native_envelope_error(&err, "placeholder or non-production text");

        let mut material = sample_fhe_full_bootstrap_material_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &material.proof.proof.bytes,
            placeholder_native.clone(),
        );
        replace_fhe_full_bootstrap_material_open_verify_envelope(&mut material, &envelope);
        let err = material.validate().expect_err(
            "full-bootstrap material proof must reject placeholder native envelope text",
        );
        assert_native_envelope_error(&err, "placeholder or non-production text");

        let mut binary_fragmented_material = sample_fhe_full_bootstrap_material_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &binary_fragmented_material.proof.proof.bytes,
            b"native verifier metadata\xffoperator your.proof payload".to_vec(),
        );
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut binary_fragmented_material,
            &envelope,
        );
        binary_fragmented_material.validate().expect(
            "full-bootstrap material proof must accept opaque binary native envelope bytes",
        );

        let mut marker_split_material = sample_fhe_full_bootstrap_material_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &marker_split_material.proof.proof.bytes,
            b"native verifier metadata operator your\xffproof payload".to_vec(),
        );
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut marker_split_material,
            &envelope,
        );
        marker_split_material.validate().expect(
            "full-bootstrap material proof must not collapse placeholder text across binary proof bytes",
        );

        let mut execution = sample_fhe_full_bootstrap_execution_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &execution.proof.proof.bytes,
            placeholder_native,
        );
        replace_fhe_full_bootstrap_execution_open_verify_envelope(&mut execution, &envelope);
        let err = execution.validate().expect_err(
            "full-bootstrap execution proof must reject placeholder native envelope text",
        );
        assert_native_envelope_error(&err, "placeholder or non-production text");

        let mut binary_decorated = sample_fhe_full_bootstrap_execution_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &binary_decorated.proof.proof.bytes,
            b"\x00\xff\x80replace before production native stark\x81\xfe\x00".to_vec(),
        );
        replace_fhe_full_bootstrap_execution_open_verify_envelope(&mut binary_decorated, &envelope);
        binary_decorated.validate().expect(
            "full-bootstrap execution proof must accept opaque binary native envelope bytes",
        );

        let mut binary_fragmented = sample_fhe_full_bootstrap_execution_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &binary_fragmented.proof.proof.bytes,
            b"native verifier metadata\xffoperator your.proof payload".to_vec(),
        );
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut binary_fragmented,
            &envelope,
        );
        binary_fragmented
            .validate()
            .expect("full-bootstrap execution proof must accept binary native envelope fragments");

        let mut marker_split = sample_fhe_full_bootstrap_execution_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &marker_split.proof.proof.bytes,
            b"native verifier metadata operator your\xffproof payload".to_vec(),
        );
        replace_fhe_full_bootstrap_execution_open_verify_envelope(&mut marker_split, &envelope);
        marker_split.validate().expect(
            "full-bootstrap execution proof must not collapse placeholder text across binary proof bytes",
        );

        let mut blank_native = sample_fhe_full_bootstrap_execution_proof();
        let envelope = open_verify_envelope_with_native_envelope_bytes(
            &blank_native.proof.proof.bytes,
            b" \n\t\r\n".to_vec(),
        );
        replace_fhe_full_bootstrap_execution_open_verify_envelope(&mut blank_native, &envelope);
        let err = blank_native
            .validate()
            .expect_err("full-bootstrap execution proof must reject blank native envelope text");
        assert_native_envelope_error(&err, "blank");
    }

    #[test]
    fn soracloud_fhe_native_envelope_placeholder_scan_is_text_only() {
        for text in [
            b"placeholder native STARK envelope".as_slice(),
            b"replace-before-production native stark",
            b"operator YOUR.proof payload",
            b"todo pending",
            b"draft only",
        ] {
            assert!(
                soracloud_fhe_stark_native_envelope_bytes_are_placeholder_text(text),
                "textual native envelope placeholder must be detected: {text:?}"
            );
        }

        for binary in [
            b"\0placeholder native STARK envelope".as_slice(),
            b"native verifier metadata\xffoperator your.proof payload",
            b"todo\xffpending",
            b"\x80replace before production native stark",
        ] {
            assert!(
                !soracloud_fhe_stark_native_envelope_bytes_are_placeholder_text(binary),
                "opaque binary native envelope bytes must not be scanned as text: {binary:?}"
            );
        }
    }

    #[test]
    fn fhe_input_admission_proof_validate_requires_vk_commitment_and_matching_envelope_hash() {
        let mut admission = sample_fhe_input_admission_proof();
        admission.proof.vk_commitment = None;

        let err = admission
            .validate()
            .expect_err("missing vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        admission.proof.vk_commitment = Some([0x42; 32]);
        admission.proof.envelope_hash = None;
        let err = admission
            .validate()
            .expect_err("missing envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.envelope_hash",
                ..
            }
        ));

        admission.proof.envelope_hash =
            Some(<[u8; 32]>::from(Hash::new(&admission.proof.proof.bytes)));
        admission
            .validate()
            .expect("matching envelope hash must be accepted");

        let mut forged_commitment = admission.clone();
        forged_commitment.proof.vk_commitment = Some([0x24; 32]);
        let err = forged_commitment
            .validate()
            .expect_err("forged vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        let mut forged_hash = admission.proof.envelope_hash.expect("matching hash");
        forged_hash[0] ^= 0x01;
        admission.proof.envelope_hash = Some(forged_hash);
        let err = admission
            .validate()
            .expect_err("forged envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField { field: "proof", .. }
        ));
    }

    #[test]
    fn fhe_input_admission_proof_validate_requires_public_key_and_ciphertext_digests() {
        let mut missing_public_key = sample_fhe_input_admission_proof();
        missing_public_key.public_key = None;
        let err = missing_public_key
            .validate()
            .expect_err("input-admission proof must carry the ciphertext public key");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "public_key",
                ..
            }
        ));

        let mut all_zero_public_key = sample_fhe_input_admission_proof();
        let params = ram_lfe_bfv_parameters_v1();
        let degree = usize::from(params.polynomial_degree);
        all_zero_public_key.public_key = Some(BfvPublicKey {
            b: vec![0; degree],
            a: vec![0; degree],
        });
        let err = all_zero_public_key
            .validate()
            .expect_err("input-admission proof must reject inert all-zero public keys");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "public_key",
                ..
            }
        ));
        assert!(
            err.to_string().contains("all zero"),
            "unexpected error: {err}"
        );

        let mut empty_digests = sample_fhe_input_admission_proof();
        empty_digests.ciphertext_proof_statement_digests.clear();
        let err = empty_digests
            .validate()
            .expect_err("input-admission proof must carry at least one ciphertext digest");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "ciphertext_proof_statement_digests",
                ..
            }
        ));

        let mut too_many_digests = sample_fhe_input_admission_proof();
        too_many_digests.ciphertext_proof_statement_digests =
            vec![sample_hash(11); RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT + 1];
        let err = too_many_digests
            .validate()
            .expect_err("input-admission proof must cap ciphertext digest count");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "ciphertext_proof_statement_digests",
                ..
            }
        ));

        let mut zero_digest = sample_fhe_input_admission_proof();
        zero_digest.ciphertext_proof_statement_digests = vec![zero_prehash_statement_hash()];
        let err = zero_digest
            .validate()
            .expect_err("input-admission proof must reject all-zero ciphertext digests");
        assert_zero_prehash_digest_error(&err, "ciphertext_proof_statement_digests");
    }

    #[test]
    fn fhe_input_admission_proof_validate_rejects_over_capacity_bounds() {
        let mut exact = sample_fhe_input_admission_proof();
        exact.residual_multiple_bound = u128::MAX;
        exact.bound_mode = BfvCiphertextBoundModeV1::ExactResidualMultiple;
        let err = exact
            .validate()
            .expect_err("over-capacity exact residual bound must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "residual_multiple_bound",
                ..
            }
        ));
        assert!(
            err.to_string().contains("exact residual"),
            "unexpected error: {err}"
        );

        let mut bounded = sample_fhe_input_admission_proof();
        bounded.residual_multiple_bound = u128::MAX;
        bounded.bound_mode = BfvCiphertextBoundModeV1::BoundedNoise;
        let err = bounded
            .validate()
            .expect_err("over-capacity bounded-noise bound must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "residual_multiple_bound",
                ..
            }
        ));
        assert!(
            err.to_string().contains("bounded-noise"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_input_admission_proof_validate_preflights_attachment_metadata_before_bounds() {
        let mut wrong_backend = sample_fhe_input_admission_proof();
        wrong_backend.residual_multiple_bound = u128::MAX;
        wrong_backend.proof.proof.backend = "stark/fri/other".into();
        let err = wrong_backend
            .validate()
            .expect_err("proof backend mismatch must be rejected before bound capacity");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.backend",
                ..
            }
        ));

        let mut wrong_vk_backend = sample_fhe_input_admission_proof();
        wrong_vk_backend.residual_multiple_bound = u128::MAX;
        wrong_vk_backend.proof.vk_ref.backend = "stark/fri/other".into();
        let err = wrong_vk_backend
            .validate()
            .expect_err("verifier-key backend mismatch must be rejected before bound capacity");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.backend",
                ..
            }
        ));

        let mut wrong_vk_ref = sample_fhe_input_admission_proof();
        wrong_vk_ref.residual_multiple_bound = u128::MAX;
        wrong_vk_ref.proof.vk_ref.name = "soracloud_fhe_input_admission_alias_v1".to_string();
        let err = wrong_vk_ref
            .validate()
            .expect_err("verifier id metadata must be rejected before bound capacity");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.name",
                ..
            }
        ));

        let mut missing_vk_commitment = sample_fhe_input_admission_proof();
        missing_vk_commitment.residual_multiple_bound = u128::MAX;
        missing_vk_commitment.proof.vk_commitment = None;
        let err = missing_vk_commitment
            .validate()
            .expect_err("verifier-key commitment metadata must be rejected before bound capacity");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        let mut missing_envelope_hash = sample_fhe_input_admission_proof();
        missing_envelope_hash.residual_multiple_bound = u128::MAX;
        missing_envelope_hash.proof.envelope_hash = None;
        let err = missing_envelope_hash
            .validate()
            .expect_err("envelope hash metadata must be rejected before bound capacity");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.envelope_hash",
                ..
            }
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_input_admission_proof_validate_rejects_open_verify_envelope_drift() {
        let sample = sample_fhe_input_admission_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");

        let mut malformed = sample.clone();
        malformed.proof.proof.bytes = vec![0xA5];
        malformed.proof.envelope_hash =
            Some(<[u8; 32]>::from(Hash::new(&malformed.proof.proof.bytes)));
        let err = malformed
            .validate()
            .expect_err("malformed OpenVerify bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_backend = sample.clone();
        let mut wrong_backend_envelope = envelope.clone();
        wrong_backend_envelope.backend = BackendTag::Halo2IpaPasta;
        replace_fhe_input_admission_open_verify_envelope(
            &mut wrong_backend,
            &wrong_backend_envelope,
        );
        let err = wrong_backend
            .validate()
            .expect_err("OpenVerify backend drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_circuit = sample.clone();
        let mut wrong_circuit_envelope = envelope.clone();
        wrong_circuit_envelope.circuit_id = "soracloud_fhe_input_admission_v2".to_string();
        replace_fhe_input_admission_open_verify_envelope(
            &mut wrong_circuit,
            &wrong_circuit_envelope,
        );
        let err = wrong_circuit
            .validate()
            .expect_err("OpenVerify circuit id drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string()
                .contains("OpenVerifyEnvelope circuit id must be canonical v1"),
            "unexpected error: {err}"
        );

        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut wrong_wrapper_version = sample.clone();
        let mut wrong_wrapper_version_envelope = envelope.clone();
        let mut version_drift = open_proof.clone();
        version_drift.version = 2;
        wrong_wrapper_version_envelope.proof_bytes =
            norito::to_bytes(&version_drift).expect("encode version-drifted STARK wrapper");
        replace_fhe_input_admission_open_verify_envelope(
            &mut wrong_wrapper_version,
            &wrong_wrapper_version_envelope,
        );
        let err = wrong_wrapper_version
            .validate()
            .expect_err("STARK wrapper version drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut empty_native_envelope = sample.clone();
        let mut empty_native_open_verify = envelope.clone();
        let mut empty_native_proof = open_proof.clone();
        empty_native_proof.envelope_bytes.clear();
        empty_native_open_verify.proof_bytes =
            norito::to_bytes(&empty_native_proof).expect("encode empty-native STARK wrapper");
        replace_fhe_input_admission_open_verify_envelope(
            &mut empty_native_envelope,
            &empty_native_open_verify,
        );
        let err = empty_native_envelope
            .validate()
            .expect_err("empty native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("native envelope bytes"),
            "unexpected error: {err}"
        );

        let mut all_zero_native_envelope = sample.clone();
        let mut all_zero_native_open_verify = envelope.clone();
        let mut all_zero_native_proof = open_proof.clone();
        all_zero_native_proof.envelope_bytes = vec![0; 32];
        all_zero_native_open_verify.proof_bytes =
            norito::to_bytes(&all_zero_native_proof).expect("encode all-zero STARK wrapper");
        replace_fhe_input_admission_open_verify_envelope(
            &mut all_zero_native_envelope,
            &all_zero_native_open_verify,
        );
        let err = all_zero_native_envelope
            .validate()
            .expect_err("all-zero native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("all-zero"),
            "unexpected error: {err}"
        );

        let mut wrong_statement = sample.clone();
        let mut wrong_statement_envelope = envelope.clone();
        let mut statement_drift = open_proof;
        statement_drift.public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(sample_hash(99))]];
        wrong_statement_envelope.proof_bytes =
            norito::to_bytes(&statement_drift).expect("encode statement-drifted STARK wrapper");
        replace_fhe_input_admission_open_verify_envelope(
            &mut wrong_statement,
            &wrong_statement_envelope,
        );
        let err = wrong_statement
            .validate()
            .expect_err("STARK wrapper statement drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_schema = sample;
        let mut wrong_schema_envelope = envelope;
        wrong_schema_envelope.public_inputs =
            b"soracloud:fhe-input-admission:public-inputs:v2".to_vec();
        replace_fhe_input_admission_open_verify_envelope(&mut wrong_schema, &wrong_schema_envelope);
        let err = wrong_schema
            .validate()
            .expect_err("OpenVerify public-input schema drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
    }

