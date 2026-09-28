// SPDX-License-Identifier: Apache-2.0

// Generated from the closed Torii OpenAPI V1 release-schema closure.
// The JavaScript fixture test checks this projection against the canonical artifact.
export const KAGEMUSHA_RELEASE_SCHEMAS_V1 = deepFreeze({
  "GovernanceKagemushaAcceptanceCaseEvidenceV1": {
    "additionalProperties": false,
    "properties": {
      "case": {
        "$ref": "#/components/schemas/GovernanceKagemushaAcceptanceCaseV1"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "validator_count": {
        "format": "uint8",
        "maximum": 255,
        "minimum": 0,
        "type": "integer"
      }
    },
    "required": [
      "case",
      "validator_count",
      "report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaAcceptanceCaseV1": {
    "additionalProperties": false,
    "properties": {
      "case": {
        "enum": [
          "receiver_inbox_pressure",
          "sender_outbox_capacity_exhaustion",
          "crash_during_prepare",
          "crash_after_prepare_before_proof",
          "crash_during_proof",
          "crash_after_proof_before_candidate_persistence",
          "crash_during_candidate_persistence",
          "crash_after_candidate_persistence_before_verification",
          "crash_during_candidate_verification",
          "crash_after_candidate_verification_before_hardware_commit",
          "crash_during_hardware_commit",
          "crash_after_hardware_commit_before_terminal_authorization",
          "crash_during_terminal_authorization",
          "crash_after_terminal_authorization_before_final_envelope_persistence",
          "crash_during_final_envelope_persistence",
          "crash_after_final_envelope_persistence_before_exposure",
          "crash_during_exposure",
          "crash_during_transport",
          "crash_after_transport_before_inbox_stage",
          "crash_during_inbox_stage",
          "crash_after_inbox_stage_before_ack",
          "crash_during_ack_persistence",
          "crash_after_ack_persistence_before_exposure",
          "crash_during_ack_exposure",
          "crash_during_ack_recovery",
          "ack_recovery_idempotence",
          "crash_during_recovery",
          "recovery_idempotence",
          "missing_sender_authorization",
          "forged_sender_authorization",
          "replayed_sender_authorization",
          "cross_release_sender_authorization",
          "missing_mint_authorization",
          "forged_mint_authorization",
          "replayed_mint_authorization",
          "cross_release_mint_authorization",
          "shuffled_concurrent_requests",
          "delayed_delivery_after_request_expiry",
          "delayed_delivery_across_ordinary_suite_rotation",
          "delayed_delivery_across_credential_rotation",
          "positive_exact_request",
          "recipient_key_binding",
          "request_amount_mismatch_rejection",
          "committed_payment_after_request_expiry",
          "exact_amount_binding",
          "distinct_payments_same_request",
          "shuffled_concurrent_payments_same_request",
          "invoice_deduplication_application_policy",
          "duplicate_transport",
          "exact_duplicate_durable_ack",
          "conflicting_credit_id_bytes",
          "same_credit_replay",
          "stale_state",
          "two_successors_from_one_predecessor",
          "rollback",
          "clock_rollback",
          "monotonic_lease_expiry",
          "counter_reuse_or_skip",
          "forged_epoch_rotation",
          "hardware_epoch_rollover",
          "hardware_counter_rollover",
          "ordinary_verifier_rotation",
          "emergency_suspension_online_recovery",
          "arithmetic_overflow",
          "proof_output_substitution",
          "transcript_unlinkability",
          "x25519_low_order_public_key_rejection",
          "x25519_zero_dh_rejection",
          "aead_ciphertext_substitution",
          "aead_associated_data_substitution",
          "deterministic_encryption_injected_randomness_kat",
          "receive_fold_single_credit",
          "receive_fold_replay_atomicity",
          "pending_credit_backlog_no_count_rejection",
          "reserve_underflow",
          "duplicate_redemption",
          "concurrent_redemption",
          "top_up_recovery",
          "full_redemption",
          "partial_redemption",
          "zero_balance_continuation",
          "animated_qr_loss_recovery",
          "animated_qr_reordering_recovery",
          "static_qr_size_guard",
          "four_peer_activation_restart_replay",
          "physical_airplane_mode",
          "physical_restart",
          "physical_power_loss",
          "physical_clock_rollback",
          "physical_backup_restore_rejection",
          "physical_memory_and_latency",
          "physical_thermal_folding",
          "no_software_fallback",
          "native_fixture_swift",
          "native_fixture_kotlin",
          "native_fixture_java",
          "native_fixture_java_script",
          "native_fixture_python",
          "native_fixture_c_sharp",
          "native_fixture_jni",
          "native_fixture_qr",
          "native_fixture_nfc"
        ],
        "type": "string"
      },
      "value": {
        "type": "null"
      }
    },
    "required": [
      "case",
      "value"
    ],
    "type": "object"
  },
  "GovernanceKagemushaAggregateBalanceQualificationV1": {
    "additionalProperties": false,
    "properties": {
      "folded_credits": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "independent_payments": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "spend_payments": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      }
    },
    "required": [
      "independent_payments",
      "folded_credits",
      "spend_payments",
      "report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaArtifactBindingV1": {
    "additionalProperties": false,
    "properties": {
      "byte_len": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "role": {
        "$ref": "#/components/schemas/GovernanceKagemushaArtifactRoleV1"
      },
      "sha256": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      }
    },
    "required": [
      "role",
      "sha256",
      "byte_len"
    ],
    "type": "object"
  },
  "GovernanceKagemushaArtifactRoleV1": {
    "additionalProperties": false,
    "properties": {
      "role": {
        "enum": [
          "params_eq",
          "params_ep",
          "inner_state_pk_eq",
          "inner_state_vk_eq",
          "inner_state_pk_ep",
          "inner_state_vk_ep",
          "state_pk_eq",
          "state_vk_eq",
          "state_pk_ep",
          "state_vk_ep",
          "mint_authorization_pk_eq",
          "mint_authorization_vk_eq",
          "mint_authorization_pk_ep",
          "mint_authorization_vk_ep",
          "mint_credit_pk_eq",
          "mint_credit_vk_eq",
          "mint_credit_pk_ep",
          "mint_credit_vk_ep",
          "platform_credential_pk_eq",
          "platform_credential_vk_eq",
          "platform_credential_pk_ep",
          "platform_credential_vk_ep",
          "guard_bundle_pk_eq",
          "guard_bundle_vk_eq",
          "guard_bundle_pk_ep",
          "guard_bundle_vk_ep",
          "terminal_authorization_pk_eq",
          "terminal_authorization_vk_eq",
          "terminal_authorization_pk_ep",
          "terminal_authorization_vk_ep",
          "commit_wrapper_pk_eq",
          "commit_wrapper_vk_eq",
          "commit_wrapper_pk_ep",
          "commit_wrapper_vk_ep",
          "inner_mint_authorization_pk_eq",
          "inner_mint_authorization_vk_eq",
          "inner_mint_authorization_pk_ep",
          "inner_mint_authorization_vk_ep",
          "inner_mint_credit_pk_eq",
          "inner_mint_credit_vk_eq",
          "inner_mint_credit_pk_ep",
          "inner_mint_credit_vk_ep",
          "mint_hash_shard_pk_eq",
          "mint_hash_shard_vk_eq",
          "mint_hash_shard_pk_ep",
          "mint_hash_shard_vk_ep",
          "mint_hash_claim_pk_eq",
          "mint_hash_claim_vk_eq",
          "mint_hash_claim_pk_ep",
          "mint_hash_claim_vk_ep"
        ],
        "type": "string"
      },
      "value": {
        "type": "null"
      }
    },
    "required": [
      "role",
      "value"
    ],
    "type": "object"
  },
  "GovernanceKagemushaBytes32V1": {
    "items": {
      "format": "uint8",
      "maximum": 255,
      "minimum": 0,
      "type": "integer"
    },
    "maxItems": 32,
    "minItems": 32,
    "type": "array"
  },
  "GovernanceKagemushaDevicePublicKeyV1": {
    "items": {
      "maxLength": 130,
      "minLength": 130,
      "pattern": "^04[0-9A-F]{128}$",
      "type": "string"
    },
    "maxItems": 1,
    "minItems": 1,
    "type": "array"
  },
  "GovernanceKagemushaDeviceSignatureV1": {
    "items": {
      "maxLength": 128,
      "minLength": 128,
      "pattern": "^[0-9A-F]{128}$",
      "type": "string"
    },
    "maxItems": 1,
    "minItems": 1,
    "type": "array"
  },
  "GovernanceKagemushaEnabledProfileV1": {
    "additionalProperties": false,
    "properties": {
      "hardware_profile": {
        "$ref": "#/components/schemas/GovernanceKagemushaHardwareProfileV1"
      },
      "hardware_profile_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "policy_epoch": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "qualification_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "qualification_report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "suite_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "vk_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      }
    },
    "required": [
      "hardware_profile",
      "hardware_profile_id",
      "suite_id",
      "vk_digest",
      "qualification_digest",
      "policy_epoch",
      "qualification_report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaEnvelopeQualificationV1": {
    "additionalProperties": false,
    "properties": {
      "handoff_p95_ms": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "raw_complete_exchange_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "text_complete_exchange_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      }
    },
    "required": [
      "raw_complete_exchange_bytes",
      "text_complete_exchange_bytes",
      "handoff_p95_ms",
      "report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaEvidenceClosureV1": {
    "additionalProperties": false,
    "properties": {
      "candidate_context_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "evidence_manifest": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "observer_policy": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "total_command_input_bytes": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "total_evidence_bytes": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "total_observed_cpu_ms": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "total_observed_duration_ms": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "total_transcript_bytes": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "verification_record_count": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "verification_records_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      }
    },
    "required": [
      "evidence_manifest",
      "observer_policy",
      "verification_records_digest",
      "candidate_context_digest",
      "verification_record_count",
      "total_evidence_bytes",
      "total_transcript_bytes",
      "total_command_input_bytes",
      "total_observed_duration_ms",
      "total_observed_cpu_ms"
    ],
    "type": "object"
  },
  "GovernanceKagemushaEvidenceFileV1": {
    "additionalProperties": false,
    "properties": {
      "byte_len": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "sha256": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      }
    },
    "required": [
      "sha256",
      "byte_len"
    ],
    "type": "object"
  },
  "GovernanceKagemushaGovernedVerifierRegistryV1": {
    "additionalProperties": false,
    "properties": {
      "active_release_id": {
        "oneOf": [
          {
            "maxLength": 64,
            "minLength": 64,
            "pattern": "^[0-9A-F]{64}$",
            "type": "string"
          },
          {
            "type": "null"
          }
        ]
      },
      "authority_policy": {
        "oneOf": [
          {
            "$ref": "#/components/schemas/GovernanceKagemushaReleaseAuthorityPolicyV1"
          },
          {
            "type": "null"
          }
        ]
      },
      "releases": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaGovernedVerifierReleaseV1"
        },
        "maxItems": 64,
        "type": "array"
      },
      "version": {
        "const": 1,
        "type": "integer"
      }
    },
    "required": [
      "version",
      "authority_policy",
      "active_release_id",
      "releases"
    ],
    "type": "object"
  },
  "GovernanceKagemushaGovernedVerifierReleaseV1": {
    "additionalProperties": false,
    "properties": {
      "artifact_manifest_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "attestation_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "authority_policy_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "hardware_policy_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "native_profile_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "profile_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "provider_policy_root": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "receipt_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "release_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "status": {
        "enum": [
          1,
          2,
          3
        ],
        "type": "integer"
      },
      "suite_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "vk_set_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      }
    },
    "required": [
      "release_id",
      "status",
      "profile_digest",
      "artifact_manifest_digest",
      "receipt_digest",
      "attestation_digest",
      "authority_policy_digest",
      "hardware_policy_digest",
      "native_profile_digest",
      "provider_policy_root",
      "suite_id",
      "vk_set_digest"
    ],
    "type": "object"
  },
  "GovernanceKagemushaHardwarePlatformClassV1": {
    "additionalProperties": false,
    "properties": {
      "class": {
        "enum": [
          "android_oem_service",
          "apple_oem_service",
          "dedicated_secure_element",
          "other_qualified"
        ],
        "type": "string"
      },
      "value": {
        "type": "null"
      }
    },
    "required": [
      "class",
      "value"
    ],
    "type": "object"
  },
  "GovernanceKagemushaHardwareProfileV1": {
    "additionalProperties": false,
    "properties": {
      "allowed_suite_commitment": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "attestation_trust_roots_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "capability_mask": {
        "format": "uint16",
        "maximum": 65535,
        "minimum": 0,
        "type": "integer"
      },
      "enrollment_attestation_verifier_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "expires_at_ms": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "firmware_policy_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "governance_credential_public_key": {
        "$ref": "#/components/schemas/GovernanceKagemushaDevicePublicKeyV1"
      },
      "hardware_profile_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "platform_class": {
        "$ref": "#/components/schemas/GovernanceKagemushaHardwarePlatformClassV1"
      },
      "policy_epoch": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "product_class_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "protocol_version": {
        "const": 1,
        "type": "integer"
      },
      "provider_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "qualification_report_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "valid_from_ms": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "version": {
        "const": 1,
        "type": "integer"
      }
    },
    "required": [
      "version",
      "protocol_version",
      "hardware_profile_id",
      "provider_id",
      "platform_class",
      "product_class_digest",
      "firmware_policy_digest",
      "enrollment_attestation_verifier_digest",
      "attestation_trust_roots_digest",
      "allowed_suite_commitment",
      "policy_epoch",
      "governance_credential_public_key",
      "capability_mask",
      "qualification_report_digest",
      "valid_from_ms",
      "expires_at_ms"
    ],
    "type": "object"
  },
  "GovernanceKagemushaHelperProtocolV1": {
    "additionalProperties": false,
    "properties": {
      "ep_proof_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "ep_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "eq_proof_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "eq_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "helper": {
        "$ref": "#/components/schemas/GovernanceKagemushaQualifiedHelperCircuitV1"
      }
    },
    "required": [
      "helper",
      "eq_protocol_digest",
      "ep_protocol_digest",
      "eq_proof_bytes",
      "ep_proof_bytes"
    ],
    "type": "object"
  },
  "GovernanceKagemushaHelperQualificationV1": {
    "additionalProperties": false,
    "properties": {
      "complete_proof_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "ep_circuit_rows": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "ep_proof_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "ep_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "ep_verifying_key": {
        "$ref": "#/components/schemas/GovernanceKagemushaArtifactBindingV1"
      },
      "eq_circuit_rows": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "eq_proof_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "eq_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "eq_verifying_key": {
        "$ref": "#/components/schemas/GovernanceKagemushaArtifactBindingV1"
      },
      "helper": {
        "$ref": "#/components/schemas/GovernanceKagemushaQualifiedHelperCircuitV1"
      },
      "operation_energy_millijoules": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "process_rss_bytes": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "prove_p95_ms": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "verify_p95_ms": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      }
    },
    "required": [
      "helper",
      "eq_protocol_digest",
      "ep_protocol_digest",
      "eq_verifying_key",
      "ep_verifying_key",
      "eq_circuit_rows",
      "ep_circuit_rows",
      "eq_proof_bytes",
      "ep_proof_bytes",
      "complete_proof_bytes",
      "prove_p95_ms",
      "verify_p95_ms",
      "process_rss_bytes",
      "operation_energy_millijoules",
      "report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaInternalValidationReceiptV1": {
    "additionalProperties": false,
    "properties": {
      "artifact_set_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "cargo_lock_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "circuit_shape_report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "ep_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "eq_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "evidence_closure": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceClosureV1"
      },
      "fuzz_cases": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "fuzz_report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "hardware_policy_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "helper_protocols": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaHelperProtocolV1"
        },
        "type": "array"
      },
      "kat_report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "native_profile_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "profile_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "profile_qualifications": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaProfileQualificationV1"
        },
        "type": "array"
      },
      "provider_policy": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaProviderPolicyEntryV1"
        },
        "type": "array"
      },
      "provider_policy_root": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "reproducible_builds": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaReproducibleBuildV1"
        },
        "type": "array"
      },
      "resource_report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "security_review_report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "source_tree_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "version": {
        "const": 1,
        "type": "integer"
      }
    },
    "required": [
      "version",
      "source_tree_digest",
      "cargo_lock_digest",
      "profile_digest",
      "native_profile_digest",
      "eq_protocol_digest",
      "ep_protocol_digest",
      "artifact_set_digest",
      "hardware_policy_digest",
      "provider_policy_root",
      "provider_policy",
      "evidence_closure",
      "circuit_shape_report",
      "security_review_report",
      "kat_report",
      "fuzz_report",
      "resource_report",
      "profile_qualifications",
      "helper_protocols",
      "reproducible_builds",
      "fuzz_cases"
    ],
    "type": "object"
  },
  "GovernanceKagemushaProfileQualificationV1": {
    "additionalProperties": false,
    "properties": {
      "acceptance_cases": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaAcceptanceCaseEvidenceV1"
        },
        "type": "array"
      },
      "aggregate_balance": {
        "$ref": "#/components/schemas/GovernanceKagemushaAggregateBalanceQualificationV1"
      },
      "envelope": {
        "$ref": "#/components/schemas/GovernanceKagemushaEnvelopeQualificationV1"
      },
      "helper_circuits": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaHelperQualificationV1"
        },
        "type": "array"
      },
      "profile": {
        "$ref": "#/components/schemas/GovernanceKagemushaEnabledProfileV1"
      },
      "recursive_depths": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaRecursiveDepthQualificationV1"
        },
        "type": "array"
      },
      "relations": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaRelationQualificationV1"
        },
        "type": "array"
      },
      "thermal": {
        "$ref": "#/components/schemas/GovernanceKagemushaThermalQualificationV1"
      }
    },
    "required": [
      "profile",
      "relations",
      "helper_circuits",
      "recursive_depths",
      "aggregate_balance",
      "thermal",
      "envelope",
      "acceptance_cases"
    ],
    "type": "object"
  },
  "GovernanceKagemushaProviderPolicyEntryV1": {
    "additionalProperties": false,
    "properties": {
      "hardware_profile_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "issuer_signature": {
        "$ref": "#/components/schemas/GovernanceKagemushaDeviceSignatureV1"
      },
      "provider_authority_commitment": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "provider_profile_index": {
        "format": "uint16",
        "maximum": 65535,
        "minimum": 0,
        "type": "integer"
      }
    },
    "required": [
      "hardware_profile_id",
      "provider_authority_commitment",
      "provider_profile_index",
      "issuer_signature"
    ],
    "type": "object"
  },
  "GovernanceKagemushaQualifiedHelperCircuitV1": {
    "additionalProperties": false,
    "properties": {
      "helper": {
        "enum": [
          "mint_authorization",
          "mint_credit",
          "platform_credential",
          "guard_bundle",
          "mint_hash_shard",
          "mint_hash_claim"
        ],
        "type": "string"
      },
      "value": {
        "type": "null"
      }
    },
    "required": [
      "helper",
      "value"
    ],
    "type": "object"
  },
  "GovernanceKagemushaQualifiedRelationV1": {
    "additionalProperties": false,
    "properties": {
      "relation": {
        "enum": [
          "bootstrap",
          "mint_fold",
          "send_split",
          "receive_fold",
          "redeem_split",
          "rotate",
          "terminal_authorization",
          "commit_wrapper"
        ],
        "type": "string"
      },
      "value": {
        "type": "null"
      }
    },
    "required": [
      "relation",
      "value"
    ],
    "type": "object"
  },
  "GovernanceKagemushaRecursiveDepthQualificationV1": {
    "additionalProperties": false,
    "properties": {
      "complete_proof_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "depth": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "raw_complete_exchange_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "text_complete_exchange_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "verified_handoffs": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      }
    },
    "required": [
      "depth",
      "verified_handoffs",
      "complete_proof_bytes",
      "raw_complete_exchange_bytes",
      "text_complete_exchange_bytes",
      "report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaRelationQualificationV1": {
    "additionalProperties": false,
    "properties": {
      "complete_proof_bytes": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "ep_circuit_rows": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "ep_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "ep_verifying_key": {
        "$ref": "#/components/schemas/GovernanceKagemushaArtifactBindingV1"
      },
      "eq_circuit_rows": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "eq_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "eq_verifying_key": {
        "$ref": "#/components/schemas/GovernanceKagemushaArtifactBindingV1"
      },
      "operation_energy_millijoules": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "process_rss_bytes": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "prove_p95_ms": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "relation": {
        "$ref": "#/components/schemas/GovernanceKagemushaQualifiedRelationV1"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      },
      "verify_p95_ms": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      }
    },
    "required": [
      "relation",
      "eq_protocol_digest",
      "ep_protocol_digest",
      "eq_verifying_key",
      "ep_verifying_key",
      "eq_circuit_rows",
      "ep_circuit_rows",
      "complete_proof_bytes",
      "prove_p95_ms",
      "verify_p95_ms",
      "process_rss_bytes",
      "operation_energy_millijoules",
      "report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaReleaseApprovalV1": {
    "additionalProperties": false,
    "properties": {
      "public_key": {
        "minLength": 1,
        "type": "string"
      },
      "signature": {
        "minLength": 2,
        "pattern": "^[0-9A-F]+$",
        "type": "string"
      }
    },
    "required": [
      "public_key",
      "signature"
    ],
    "type": "object"
  },
  "GovernanceKagemushaReleaseAttestationSubjectV1": {
    "additionalProperties": false,
    "properties": {
      "artifact_set_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "authority_policy_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "manifest_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "release_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "validation_receipt_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "version": {
        "const": 1,
        "type": "integer"
      }
    },
    "required": [
      "version",
      "authority_policy_digest",
      "release_id",
      "manifest_digest",
      "validation_receipt_digest",
      "artifact_set_digest"
    ],
    "type": "object"
  },
  "GovernanceKagemushaReleaseAttestationV1": {
    "additionalProperties": false,
    "properties": {
      "approvals": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaReleaseApprovalV1"
        },
        "type": "array"
      },
      "subject": {
        "$ref": "#/components/schemas/GovernanceKagemushaReleaseAttestationSubjectV1"
      },
      "version": {
        "const": 1,
        "type": "integer"
      }
    },
    "required": [
      "version",
      "subject",
      "approvals"
    ],
    "type": "object"
  },
  "GovernanceKagemushaReleaseAuthorityPolicyV1": {
    "additionalProperties": false,
    "properties": {
      "authority_set_id": {
        "items": {
          "format": "uint8",
          "maximum": 255,
          "minimum": 0,
          "type": "integer"
        },
        "maxItems": 32,
        "minItems": 32,
        "not": {
          "const": [
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
          ]
        },
        "type": "array"
      },
      "authorized_signers": {
        "items": {
          "minLength": 1,
          "type": "string"
        },
        "maxItems": 32,
        "minItems": 1,
        "type": "array",
        "uniqueItems": true
      },
      "threshold": {
        "format": "uint16",
        "maximum": 32,
        "minimum": 1,
        "type": "integer"
      },
      "version": {
        "const": 1,
        "type": "integer"
      }
    },
    "required": [
      "version",
      "authority_set_id",
      "threshold",
      "authorized_signers"
    ],
    "type": "object"
  },
  "GovernanceKagemushaReleaseManifestV1": {
    "additionalProperties": false,
    "properties": {
      "artifacts": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaArtifactBindingV1"
        },
        "type": "array"
      },
      "cargo_lock_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "enabled_profiles": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaEnabledProfileV1"
        },
        "type": "array"
      },
      "ep_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "eq_protocol_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "halo2_k": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "hardware_policy_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "helper_protocols": {
        "items": {
          "$ref": "#/components/schemas/GovernanceKagemushaHelperProtocolV1"
        },
        "type": "array"
      },
      "profile_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "release_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "source_tree_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "validation_receipt_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "version": {
        "const": 1,
        "type": "integer"
      }
    },
    "required": [
      "version",
      "release_id",
      "source_tree_digest",
      "cargo_lock_digest",
      "profile_digest",
      "eq_protocol_digest",
      "ep_protocol_digest",
      "hardware_policy_digest",
      "validation_receipt_digest",
      "halo2_k",
      "helper_protocols",
      "enabled_profiles",
      "artifacts"
    ],
    "type": "object"
  },
  "GovernanceKagemushaReproducibleBuildV1": {
    "additionalProperties": false,
    "properties": {
      "artifact_set_digest": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "builder_id": {
        "$ref": "#/components/schemas/GovernanceKagemushaBytes32V1"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      }
    },
    "required": [
      "builder_id",
      "artifact_set_digest",
      "report"
    ],
    "type": "object"
  },
  "GovernanceKagemushaThermalQualificationV1": {
    "additionalProperties": false,
    "properties": {
      "fold_p95_ms": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "folded_credits": {
        "format": "uint32",
        "maximum": 4294967295,
        "minimum": 0,
        "type": "integer"
      },
      "operation_energy_millijoules": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "process_rss_bytes": {
        "format": "uint64",
        "maximum": 9007199254740991,
        "minimum": 0,
        "type": "integer"
      },
      "report": {
        "$ref": "#/components/schemas/GovernanceKagemushaEvidenceFileV1"
      }
    },
    "required": [
      "folded_credits",
      "fold_p95_ms",
      "process_rss_bytes",
      "operation_energy_millijoules",
      "report"
    ],
    "type": "object"
  }
});

function deepFreeze(value) {
  if (value !== null && typeof value === "object") {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}
