// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import org.hyperledger.iroha.sdk.address.PublicKeyPayload
import org.hyperledger.iroha.sdk.address.compactPublicKeyPayload
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.offline.KagemushaP256Codec

/** Closed recursive JSON projection for the two governed KAGEMUSHA verifier-release proposals. */
internal object KagemushaVerifierProposalValidatorV1 {
    private val EXACT_JSON_MAX = BigInteger("9007199254740991")
    private val U32_MAX = BigInteger("4294967295")
    private val U8_MAX = BigInteger.valueOf(255)
    private val U16_MAX = BigInteger("65535")

    fun install(value: Map<String, Any?>) {
        exact(value, setOf("proposal_operator", "network_id", "expected_predecessor", "manifest", "receipt", "attestation"), "KagemushaVerifierReleaseInstall")
        account(value["proposal_operator"], "KagemushaVerifierReleaseInstall.proposal_operator")
        NetworkId.parse(string(value["network_id"], "KagemushaVerifierReleaseInstall.network_id"))
        val predecessor = governedVerifierRegistry(value["expected_predecessor"], "KagemushaVerifierReleaseInstall.expected_predecessor")
        require(predecessor["authority_policy"] != null) { "release install requires a governed signer policy" }
        releaseManifest(value["manifest"], "KagemushaVerifierReleaseInstall.manifest")
        internalValidationReceipt(value["receipt"], "KagemushaVerifierReleaseInstall.receipt")
        releaseAttestation(value["attestation"], "KagemushaVerifierReleaseInstall.attestation")
    }

    fun activate(value: Map<String, Any?>) {
        exact(value, setOf("proposal_operator", "network_id", "expected_predecessor", "successor_release_id"), "KagemushaVerifierReleaseActivate")
        account(value["proposal_operator"], "KagemushaVerifierReleaseActivate.proposal_operator")
        NetworkId.parse(string(value["network_id"], "KagemushaVerifierReleaseActivate.network_id"))
        val predecessor = governedVerifierRegistry(value["expected_predecessor"], "KagemushaVerifierReleaseActivate.expected_predecessor")
        val successor = bytes32(value["successor_release_id"], "KagemushaVerifierReleaseActivate.successor_release_id")
        val releases = array(predecessor["releases"], "KagemushaVerifierReleaseActivate.expected_predecessor.releases")
        require(predecessor["authority_policy"] != null && predecessor["active_release_id"] == null && releases.size == 1) {
            "first activation requires an inactive governed registry with one standby release"
        }
        val row = objectValue(releases[0], "KagemushaVerifierReleaseActivate.expected_predecessor.releases[0]")
        require(bytes32(row["release_id"], "successor release id").contentEquals(successor) &&
            uint(row["status"], BigInteger.valueOf(3), "successor status") == BigInteger.valueOf(2)) {
            "first activation requires its sole installed standby target"
        }
    }

    private fun acceptanceCaseEvidence(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("case", "validator_count", "report"), label)
        acceptanceCase(record["case"], "$label.case")
        uint(record["validator_count"], U8_MAX, "$label.validator_count")
        evidenceFile(record["report"], "$label.report")
    }

    private fun acceptanceCase(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("case", "value"), label)
        require(string(record["case"], "$label.case") in setOf(
            "receiver_inbox_pressure", "sender_outbox_capacity_exhaustion", "crash_during_prepare", "crash_after_prepare_before_proof",
            "crash_during_proof", "crash_after_proof_before_candidate_persistence", "crash_during_candidate_persistence", "crash_after_candidate_persistence_before_verification",
            "crash_during_candidate_verification", "crash_after_candidate_verification_before_hardware_commit", "crash_during_hardware_commit", "crash_after_hardware_commit_before_terminal_authorization",
            "crash_during_terminal_authorization", "crash_after_terminal_authorization_before_final_envelope_persistence", "crash_during_final_envelope_persistence", "crash_after_final_envelope_persistence_before_exposure",
            "crash_during_exposure", "crash_during_transport", "crash_after_transport_before_inbox_stage", "crash_during_inbox_stage",
            "crash_after_inbox_stage_before_ack", "crash_during_ack_persistence", "crash_after_ack_persistence_before_exposure", "crash_during_ack_exposure",
            "crash_during_ack_recovery", "ack_recovery_idempotence", "crash_during_recovery", "recovery_idempotence",
            "missing_sender_authorization", "forged_sender_authorization", "replayed_sender_authorization", "cross_release_sender_authorization",
            "missing_mint_authorization", "forged_mint_authorization", "replayed_mint_authorization", "cross_release_mint_authorization",
            "shuffled_concurrent_requests", "delayed_delivery_after_request_expiry", "delayed_delivery_across_ordinary_suite_rotation", "delayed_delivery_across_credential_rotation",
            "positive_exact_request", "recipient_key_binding", "request_amount_mismatch_rejection", "committed_payment_after_request_expiry",
            "exact_amount_binding", "distinct_payments_same_request", "shuffled_concurrent_payments_same_request", "invoice_deduplication_application_policy",
            "duplicate_transport", "exact_duplicate_durable_ack", "conflicting_credit_id_bytes", "same_credit_replay",
            "stale_state", "two_successors_from_one_predecessor", "rollback", "clock_rollback",
            "monotonic_lease_expiry", "counter_reuse_or_skip", "forged_epoch_rotation", "hardware_epoch_rollover",
            "hardware_counter_rollover", "ordinary_verifier_rotation", "emergency_suspension_online_recovery", "arithmetic_overflow",
            "proof_output_substitution", "transcript_unlinkability", "x25519_low_order_public_key_rejection", "x25519_zero_dh_rejection",
            "aead_ciphertext_substitution", "aead_associated_data_substitution", "deterministic_encryption_injected_randomness_kat", "receive_fold_single_credit",
            "receive_fold_replay_atomicity", "pending_credit_backlog_no_count_rejection", "reserve_underflow", "duplicate_redemption",
            "concurrent_redemption", "top_up_recovery", "full_redemption", "partial_redemption",
            "zero_balance_continuation", "animated_qr_loss_recovery", "animated_qr_reordering_recovery", "static_qr_size_guard",
            "four_peer_activation_restart_replay", "physical_airplane_mode", "physical_restart", "physical_power_loss",
            "physical_clock_rollback", "physical_backup_restore_rejection", "physical_memory_and_latency", "physical_thermal_folding",
            "no_software_fallback", "native_fixture_swift", "native_fixture_kotlin", "native_fixture_java",
            "native_fixture_java_script", "native_fixture_python", "native_fixture_c_sharp", "native_fixture_jni",
            "native_fixture_qr", "native_fixture_nfc",
        )) { "$label.case has unknown tag" }
        require(record["value"] == null) { "$label.value must be explicit null" }
    }

    private fun aggregateBalanceQualification(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("independent_payments", "folded_credits", "spend_payments", "report"), label)
        uint(record["independent_payments"], U32_MAX, "$label.independent_payments")
        uint(record["folded_credits"], U32_MAX, "$label.folded_credits")
        uint(record["spend_payments"], U32_MAX, "$label.spend_payments")
        evidenceFile(record["report"], "$label.report")
    }

    private fun artifactBinding(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("role", "sha256", "byte_len"), label)
        artifactRole(record["role"], "$label.role")
        bytes32(record["sha256"], "$label.sha256")
        uint(record["byte_len"], EXACT_JSON_MAX, "$label.byte_len")
    }

    private fun artifactRole(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("role", "value"), label)
        require(string(record["role"], "$label.role") in setOf(
            "params_eq", "params_ep", "inner_state_pk_eq", "inner_state_vk_eq",
            "inner_state_pk_ep", "inner_state_vk_ep", "state_pk_eq", "state_vk_eq",
            "state_pk_ep", "state_vk_ep", "mint_authorization_pk_eq", "mint_authorization_vk_eq",
            "mint_authorization_pk_ep", "mint_authorization_vk_ep", "mint_credit_pk_eq", "mint_credit_vk_eq",
            "mint_credit_pk_ep", "mint_credit_vk_ep", "platform_credential_pk_eq", "platform_credential_vk_eq",
            "platform_credential_pk_ep", "platform_credential_vk_ep", "guard_bundle_pk_eq", "guard_bundle_vk_eq",
            "guard_bundle_pk_ep", "guard_bundle_vk_ep", "terminal_authorization_pk_eq", "terminal_authorization_vk_eq",
            "terminal_authorization_pk_ep", "terminal_authorization_vk_ep", "commit_wrapper_pk_eq", "commit_wrapper_vk_eq",
            "commit_wrapper_pk_ep", "commit_wrapper_vk_ep", "inner_mint_authorization_pk_eq", "inner_mint_authorization_vk_eq",
            "inner_mint_authorization_pk_ep", "inner_mint_authorization_vk_ep", "inner_mint_credit_pk_eq", "inner_mint_credit_vk_eq",
            "inner_mint_credit_pk_ep", "inner_mint_credit_vk_ep", "mint_hash_shard_pk_eq", "mint_hash_shard_vk_eq",
            "mint_hash_shard_pk_ep", "mint_hash_shard_vk_ep", "mint_hash_claim_pk_eq", "mint_hash_claim_vk_eq",
            "mint_hash_claim_pk_ep", "mint_hash_claim_vk_ep",
        )) { "$label.role has unknown tag" }
        require(record["value"] == null) { "$label.value must be explicit null" }
    }

    private fun enabledProfile(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("hardware_profile", "hardware_profile_id", "suite_id", "vk_digest", "qualification_digest", "policy_epoch", "qualification_report"), label)
        hardwareProfile(record["hardware_profile"], "$label.hardware_profile")
        bytes32(record["hardware_profile_id"], "$label.hardware_profile_id")
        bytes32(record["suite_id"], "$label.suite_id")
        bytes32(record["vk_digest"], "$label.vk_digest")
        bytes32(record["qualification_digest"], "$label.qualification_digest")
        uint(record["policy_epoch"], EXACT_JSON_MAX, "$label.policy_epoch")
        evidenceFile(record["qualification_report"], "$label.qualification_report")
    }

    private fun envelopeQualification(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("raw_complete_exchange_bytes", "text_complete_exchange_bytes", "handoff_p95_ms", "report"), label)
        uint(record["raw_complete_exchange_bytes"], U32_MAX, "$label.raw_complete_exchange_bytes")
        uint(record["text_complete_exchange_bytes"], U32_MAX, "$label.text_complete_exchange_bytes")
        uint(record["handoff_p95_ms"], U32_MAX, "$label.handoff_p95_ms")
        evidenceFile(record["report"], "$label.report")
    }

    private fun evidenceClosure(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("evidence_manifest", "observer_policy", "verification_records_digest", "candidate_context_digest", "verification_record_count", "total_evidence_bytes", "total_transcript_bytes", "total_command_input_bytes", "total_observed_duration_ms", "total_observed_cpu_ms"), label)
        evidenceFile(record["evidence_manifest"], "$label.evidence_manifest")
        evidenceFile(record["observer_policy"], "$label.observer_policy")
        bytes32(record["verification_records_digest"], "$label.verification_records_digest")
        bytes32(record["candidate_context_digest"], "$label.candidate_context_digest")
        uint(record["verification_record_count"], U32_MAX, "$label.verification_record_count")
        uint(record["total_evidence_bytes"], EXACT_JSON_MAX, "$label.total_evidence_bytes")
        uint(record["total_transcript_bytes"], EXACT_JSON_MAX, "$label.total_transcript_bytes")
        uint(record["total_command_input_bytes"], EXACT_JSON_MAX, "$label.total_command_input_bytes")
        uint(record["total_observed_duration_ms"], EXACT_JSON_MAX, "$label.total_observed_duration_ms")
        uint(record["total_observed_cpu_ms"], EXACT_JSON_MAX, "$label.total_observed_cpu_ms")
    }

    private fun evidenceFile(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("sha256", "byte_len"), label)
        bytes32(record["sha256"], "$label.sha256")
        uint(record["byte_len"], EXACT_JSON_MAX, "$label.byte_len")
    }

    private fun governedVerifierRelease(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("release_id", "status", "profile_digest", "artifact_manifest_digest", "receipt_digest", "attestation_digest", "authority_policy_digest", "hardware_policy_digest", "native_profile_digest", "provider_policy_root", "suite_id", "vk_set_digest"), label)
        bytes32(record["release_id"], "$label.release_id", nonzero = true)
        require(uint(record["status"], U8_MAX, "$label.status") in setOf(BigInteger.ONE, BigInteger.valueOf(2), BigInteger.valueOf(3))) {
            "$label.status has unknown status"
        }
        bytes32(record["profile_digest"], "$label.profile_digest", nonzero = true)
        bytes32(record["artifact_manifest_digest"], "$label.artifact_manifest_digest", nonzero = true)
        bytes32(record["receipt_digest"], "$label.receipt_digest", nonzero = true)
        bytes32(record["attestation_digest"], "$label.attestation_digest", nonzero = true)
        bytes32(record["authority_policy_digest"], "$label.authority_policy_digest", nonzero = true)
        bytes32(record["hardware_policy_digest"], "$label.hardware_policy_digest", nonzero = true)
        bytes32(record["native_profile_digest"], "$label.native_profile_digest", nonzero = true)
        bytes32(record["provider_policy_root"], "$label.provider_policy_root", nonzero = true)
        bytes32(record["suite_id"], "$label.suite_id", nonzero = true)
        bytes32(record["vk_set_digest"], "$label.vk_set_digest", nonzero = true)
    }

    private fun hardwarePlatformClass(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("class", "value"), label)
        require(string(record["class"], "$label.class") in setOf("android_oem_service", "apple_oem_service", "dedicated_secure_element", "other_qualified")) { "$label.class has unknown tag" }
        require(record["value"] == null) { "$label.value must be explicit null" }
    }

    private fun hardwareProfile(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("version", "protocol_version", "hardware_profile_id", "provider_id", "platform_class", "product_class_digest", "firmware_policy_digest", "enrollment_attestation_verifier_digest", "attestation_trust_roots_digest", "allowed_suite_commitment", "policy_epoch", "governance_credential_public_key", "capability_mask", "qualification_report_digest", "valid_from_ms", "expires_at_ms"), label)
        require(uint(record["version"], EXACT_JSON_MAX, "$label.version") == BigInteger("1")) { "$label.version must be 1" }
        require(uint(record["protocol_version"], EXACT_JSON_MAX, "$label.protocol_version") == BigInteger("1")) { "$label.protocol_version must be 1" }
        bytes32(record["hardware_profile_id"], "$label.hardware_profile_id")
        bytes32(record["provider_id"], "$label.provider_id")
        hardwarePlatformClass(record["platform_class"], "$label.platform_class")
        bytes32(record["product_class_digest"], "$label.product_class_digest")
        bytes32(record["firmware_policy_digest"], "$label.firmware_policy_digest")
        bytes32(record["enrollment_attestation_verifier_digest"], "$label.enrollment_attestation_verifier_digest")
        bytes32(record["attestation_trust_roots_digest"], "$label.attestation_trust_roots_digest")
        bytes32(record["allowed_suite_commitment"], "$label.allowed_suite_commitment")
        uint(record["policy_epoch"], EXACT_JSON_MAX, "$label.policy_epoch")
        devicePublicKey(record["governance_credential_public_key"], "$label.governance_credential_public_key")
        uint(record["capability_mask"], U16_MAX, "$label.capability_mask")
        bytes32(record["qualification_report_digest"], "$label.qualification_report_digest")
        uint(record["valid_from_ms"], EXACT_JSON_MAX, "$label.valid_from_ms")
        uint(record["expires_at_ms"], EXACT_JSON_MAX, "$label.expires_at_ms")
    }

    private fun helperProtocol(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("helper", "eq_protocol_digest", "ep_protocol_digest", "eq_proof_bytes", "ep_proof_bytes"), label)
        qualifiedHelperCircuit(record["helper"], "$label.helper")
        bytes32(record["eq_protocol_digest"], "$label.eq_protocol_digest")
        bytes32(record["ep_protocol_digest"], "$label.ep_protocol_digest")
        uint(record["eq_proof_bytes"], U32_MAX, "$label.eq_proof_bytes")
        uint(record["ep_proof_bytes"], U32_MAX, "$label.ep_proof_bytes")
    }

    private fun helperQualification(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("helper", "eq_protocol_digest", "ep_protocol_digest", "eq_verifying_key", "ep_verifying_key", "eq_circuit_rows", "ep_circuit_rows", "eq_proof_bytes", "ep_proof_bytes", "complete_proof_bytes", "prove_p95_ms", "verify_p95_ms", "process_rss_bytes", "operation_energy_millijoules", "report"), label)
        qualifiedHelperCircuit(record["helper"], "$label.helper")
        bytes32(record["eq_protocol_digest"], "$label.eq_protocol_digest")
        bytes32(record["ep_protocol_digest"], "$label.ep_protocol_digest")
        artifactBinding(record["eq_verifying_key"], "$label.eq_verifying_key")
        artifactBinding(record["ep_verifying_key"], "$label.ep_verifying_key")
        uint(record["eq_circuit_rows"], U32_MAX, "$label.eq_circuit_rows")
        uint(record["ep_circuit_rows"], U32_MAX, "$label.ep_circuit_rows")
        uint(record["eq_proof_bytes"], U32_MAX, "$label.eq_proof_bytes")
        uint(record["ep_proof_bytes"], U32_MAX, "$label.ep_proof_bytes")
        uint(record["complete_proof_bytes"], U32_MAX, "$label.complete_proof_bytes")
        uint(record["prove_p95_ms"], U32_MAX, "$label.prove_p95_ms")
        uint(record["verify_p95_ms"], U32_MAX, "$label.verify_p95_ms")
        uint(record["process_rss_bytes"], EXACT_JSON_MAX, "$label.process_rss_bytes")
        uint(record["operation_energy_millijoules"], EXACT_JSON_MAX, "$label.operation_energy_millijoules")
        evidenceFile(record["report"], "$label.report")
    }

    private fun internalValidationReceipt(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("version", "source_tree_digest", "cargo_lock_digest", "profile_digest", "native_profile_digest", "eq_protocol_digest", "ep_protocol_digest", "artifact_set_digest", "hardware_policy_digest", "provider_policy_root", "provider_policy", "evidence_closure", "circuit_shape_report", "security_review_report", "kat_report", "fuzz_report", "resource_report", "profile_qualifications", "helper_protocols", "reproducible_builds", "fuzz_cases"), label)
        require(uint(record["version"], EXACT_JSON_MAX, "$label.version") == BigInteger("1")) { "$label.version must be 1" }
        bytes32(record["source_tree_digest"], "$label.source_tree_digest")
        bytes32(record["cargo_lock_digest"], "$label.cargo_lock_digest")
        bytes32(record["profile_digest"], "$label.profile_digest")
        bytes32(record["native_profile_digest"], "$label.native_profile_digest")
        bytes32(record["eq_protocol_digest"], "$label.eq_protocol_digest")
        bytes32(record["ep_protocol_digest"], "$label.ep_protocol_digest")
        bytes32(record["artifact_set_digest"], "$label.artifact_set_digest")
        bytes32(record["hardware_policy_digest"], "$label.hardware_policy_digest")
        bytes32(record["provider_policy_root"], "$label.provider_policy_root")
        array(record["provider_policy"], "$label.provider_policy").also { items ->
            items.forEachIndexed { index, item ->
                providerPolicyEntry(item, "$label.provider_policy[$index]")
            }
        }
        evidenceClosure(record["evidence_closure"], "$label.evidence_closure")
        evidenceFile(record["circuit_shape_report"], "$label.circuit_shape_report")
        evidenceFile(record["security_review_report"], "$label.security_review_report")
        evidenceFile(record["kat_report"], "$label.kat_report")
        evidenceFile(record["fuzz_report"], "$label.fuzz_report")
        evidenceFile(record["resource_report"], "$label.resource_report")
        array(record["profile_qualifications"], "$label.profile_qualifications").also { items ->
            items.forEachIndexed { index, item ->
                profileQualification(item, "$label.profile_qualifications[$index]")
            }
        }
        array(record["helper_protocols"], "$label.helper_protocols").also { items ->
            items.forEachIndexed { index, item ->
                helperProtocol(item, "$label.helper_protocols[$index]")
            }
        }
        array(record["reproducible_builds"], "$label.reproducible_builds").also { items ->
            items.forEachIndexed { index, item ->
                reproducibleBuild(item, "$label.reproducible_builds[$index]")
            }
        }
        uint(record["fuzz_cases"], EXACT_JSON_MAX, "$label.fuzz_cases")
    }

    private fun profileQualification(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("profile", "relations", "helper_circuits", "recursive_depths", "aggregate_balance", "thermal", "envelope", "acceptance_cases"), label)
        enabledProfile(record["profile"], "$label.profile")
        array(record["relations"], "$label.relations").also { items ->
            items.forEachIndexed { index, item ->
                relationQualification(item, "$label.relations[$index]")
            }
        }
        array(record["helper_circuits"], "$label.helper_circuits").also { items ->
            items.forEachIndexed { index, item ->
                helperQualification(item, "$label.helper_circuits[$index]")
            }
        }
        array(record["recursive_depths"], "$label.recursive_depths").also { items ->
            items.forEachIndexed { index, item ->
                recursiveDepthQualification(item, "$label.recursive_depths[$index]")
            }
        }
        aggregateBalanceQualification(record["aggregate_balance"], "$label.aggregate_balance")
        thermalQualification(record["thermal"], "$label.thermal")
        envelopeQualification(record["envelope"], "$label.envelope")
        array(record["acceptance_cases"], "$label.acceptance_cases").also { items ->
            items.forEachIndexed { index, item ->
                acceptanceCaseEvidence(item, "$label.acceptance_cases[$index]")
            }
        }
    }

    private fun providerPolicyEntry(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("hardware_profile_id", "provider_authority_commitment", "provider_profile_index", "issuer_signature"), label)
        bytes32(record["hardware_profile_id"], "$label.hardware_profile_id")
        bytes32(record["provider_authority_commitment"], "$label.provider_authority_commitment")
        uint(record["provider_profile_index"], U16_MAX, "$label.provider_profile_index")
        deviceSignature(record["issuer_signature"], "$label.issuer_signature")
    }

    private fun qualifiedHelperCircuit(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("helper", "value"), label)
        require(string(record["helper"], "$label.helper") in setOf("mint_authorization", "mint_credit", "platform_credential", "guard_bundle", "mint_hash_shard", "mint_hash_claim")) { "$label.helper has unknown tag" }
        require(record["value"] == null) { "$label.value must be explicit null" }
    }

    private fun qualifiedRelation(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("relation", "value"), label)
        require(string(record["relation"], "$label.relation") in setOf("bootstrap", "mint_fold", "send_split", "receive_fold", "redeem_split", "rotate", "terminal_authorization", "commit_wrapper")) { "$label.relation has unknown tag" }
        require(record["value"] == null) { "$label.value must be explicit null" }
    }

    private fun recursiveDepthQualification(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("depth", "verified_handoffs", "complete_proof_bytes", "raw_complete_exchange_bytes", "text_complete_exchange_bytes", "report"), label)
        uint(record["depth"], U32_MAX, "$label.depth")
        uint(record["verified_handoffs"], U32_MAX, "$label.verified_handoffs")
        uint(record["complete_proof_bytes"], U32_MAX, "$label.complete_proof_bytes")
        uint(record["raw_complete_exchange_bytes"], U32_MAX, "$label.raw_complete_exchange_bytes")
        uint(record["text_complete_exchange_bytes"], U32_MAX, "$label.text_complete_exchange_bytes")
        evidenceFile(record["report"], "$label.report")
    }

    private fun relationQualification(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("relation", "eq_protocol_digest", "ep_protocol_digest", "eq_verifying_key", "ep_verifying_key", "eq_circuit_rows", "ep_circuit_rows", "complete_proof_bytes", "prove_p95_ms", "verify_p95_ms", "process_rss_bytes", "operation_energy_millijoules", "report"), label)
        qualifiedRelation(record["relation"], "$label.relation")
        bytes32(record["eq_protocol_digest"], "$label.eq_protocol_digest")
        bytes32(record["ep_protocol_digest"], "$label.ep_protocol_digest")
        artifactBinding(record["eq_verifying_key"], "$label.eq_verifying_key")
        artifactBinding(record["ep_verifying_key"], "$label.ep_verifying_key")
        uint(record["eq_circuit_rows"], U32_MAX, "$label.eq_circuit_rows")
        uint(record["ep_circuit_rows"], U32_MAX, "$label.ep_circuit_rows")
        uint(record["complete_proof_bytes"], U32_MAX, "$label.complete_proof_bytes")
        uint(record["prove_p95_ms"], U32_MAX, "$label.prove_p95_ms")
        uint(record["verify_p95_ms"], U32_MAX, "$label.verify_p95_ms")
        uint(record["process_rss_bytes"], EXACT_JSON_MAX, "$label.process_rss_bytes")
        uint(record["operation_energy_millijoules"], EXACT_JSON_MAX, "$label.operation_energy_millijoules")
        evidenceFile(record["report"], "$label.report")
    }

    private fun releaseAttestationSubject(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("version", "authority_policy_digest", "release_id", "manifest_digest", "validation_receipt_digest", "artifact_set_digest"), label)
        require(uint(record["version"], EXACT_JSON_MAX, "$label.version") == BigInteger("1")) { "$label.version must be 1" }
        bytes32(record["authority_policy_digest"], "$label.authority_policy_digest")
        bytes32(record["release_id"], "$label.release_id")
        bytes32(record["manifest_digest"], "$label.manifest_digest")
        bytes32(record["validation_receipt_digest"], "$label.validation_receipt_digest")
        bytes32(record["artifact_set_digest"], "$label.artifact_set_digest")
    }

    private fun releaseAttestation(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("version", "subject", "approvals"), label)
        require(uint(record["version"], EXACT_JSON_MAX, "$label.version") == BigInteger("1")) { "$label.version must be 1" }
        releaseAttestationSubject(record["subject"], "$label.subject")
        array(record["approvals"], "$label.approvals").also { items ->
            items.forEachIndexed { index, item ->
                releaseApproval(item, "$label.approvals[$index]")
            }
        }
    }

    private fun releaseManifest(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("version", "release_id", "source_tree_digest", "cargo_lock_digest", "profile_digest", "eq_protocol_digest", "ep_protocol_digest", "hardware_policy_digest", "validation_receipt_digest", "halo2_k", "helper_protocols", "enabled_profiles", "artifacts"), label)
        require(uint(record["version"], EXACT_JSON_MAX, "$label.version") == BigInteger("1")) { "$label.version must be 1" }
        bytes32(record["release_id"], "$label.release_id")
        bytes32(record["source_tree_digest"], "$label.source_tree_digest")
        bytes32(record["cargo_lock_digest"], "$label.cargo_lock_digest")
        bytes32(record["profile_digest"], "$label.profile_digest")
        bytes32(record["eq_protocol_digest"], "$label.eq_protocol_digest")
        bytes32(record["ep_protocol_digest"], "$label.ep_protocol_digest")
        bytes32(record["hardware_policy_digest"], "$label.hardware_policy_digest")
        bytes32(record["validation_receipt_digest"], "$label.validation_receipt_digest")
        uint(record["halo2_k"], U32_MAX, "$label.halo2_k")
        array(record["helper_protocols"], "$label.helper_protocols").also { items ->
            items.forEachIndexed { index, item ->
                helperProtocol(item, "$label.helper_protocols[$index]")
            }
        }
        array(record["enabled_profiles"], "$label.enabled_profiles").also { items ->
            items.forEachIndexed { index, item ->
                enabledProfile(item, "$label.enabled_profiles[$index]")
            }
        }
        array(record["artifacts"], "$label.artifacts").also { items ->
            items.forEachIndexed { index, item ->
                artifactBinding(item, "$label.artifacts[$index]")
            }
        }
    }

    private fun reproducibleBuild(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("builder_id", "artifact_set_digest", "report"), label)
        bytes32(record["builder_id"], "$label.builder_id")
        bytes32(record["artifact_set_digest"], "$label.artifact_set_digest")
        evidenceFile(record["report"], "$label.report")
    }

    private fun thermalQualification(value: Any?, label: String) {
        val record = objectValue(value, label)
        exact(record, setOf("folded_credits", "fold_p95_ms", "process_rss_bytes", "operation_energy_millijoules", "report"), label)
        uint(record["folded_credits"], U32_MAX, "$label.folded_credits")
        uint(record["fold_p95_ms"], U32_MAX, "$label.fold_p95_ms")
        uint(record["process_rss_bytes"], EXACT_JSON_MAX, "$label.process_rss_bytes")
        uint(record["operation_energy_millijoules"], EXACT_JSON_MAX, "$label.operation_energy_millijoules")
        evidenceFile(record["report"], "$label.report")
    }

    private fun authorityPolicy(value: Any?, label: String) {
        val policy = objectValue(value, label)
        exact(policy, setOf("version", "authority_set_id", "threshold", "authorized_signers"), label)
        require(uint(policy["version"], U16_MAX, "$label.version") == BigInteger.ONE) { "$label.version must be 1" }
        bytes32(policy["authority_set_id"], "$label.authority_set_id", nonzero = true)
        val signers = array(policy["authorized_signers"], "$label.authorized_signers")
        require(signers.size in 1..32) { "$label.authorized_signers requires 1..32 signers" }
        val threshold = uint(policy["threshold"], U16_MAX, "$label.threshold")
        require(threshold >= BigInteger.ONE && threshold <= BigInteger.valueOf(signers.size.toLong())) {
            "$label.threshold exceeds the signer set"
        }
        var previous: ByteArray? = null
        signers.forEachIndexed { index, signer ->
            val parsed = canonicalPublicKey(signer, "$label.authorized_signers[$index]")
            val current = compactPublicKeyPayload(parsed.curveId, parsed.keyBytes)
            previous?.let { require(compareUnsignedBytes(it, current) < 0) { "$label.authorized_signers must be strictly ordered" } }
            previous = current
        }
    }

    private fun governedVerifierRegistry(value: Any?, label: String): Map<String, Any?> {
        val registry = objectValue(value, label)
        exact(registry, setOf("version", "authority_policy", "active_release_id", "releases"), label)
        require(uint(registry["version"], U16_MAX, "$label.version") == BigInteger.ONE) { "$label.version must be 1" }
        registry["authority_policy"]?.let { authorityPolicy(it, "$label.authority_policy") }
        val active = registry["active_release_id"]?.let { uppercaseHex32(it, "$label.active_release_id") }
        val releases = array(registry["releases"], "$label.releases")
        require(releases.size <= 64) { "$label.releases exceeds 64" }
        require(registry["authority_policy"] != null || (active == null && releases.isEmpty())) {
            "$label has releases without a signer policy"
        }
        var previous: ByteArray? = null
        var activeCount = 0
        releases.forEachIndexed { index, item ->
            val rowLabel = "$label.releases[$index]"
            governedVerifierRelease(item, rowLabel)
            val row = objectValue(item, rowLabel)
            val id = bytes32(row["release_id"], "$rowLabel.release_id", nonzero = true)
            previous?.let { require(compareUnsignedBytes(it, id) < 0) { "$label.releases must be strictly ordered" } }
            previous = id
            val status = uint(row["status"], BigInteger.valueOf(3), "$rowLabel.status")
            if (status == BigInteger.ONE) {
                activeCount += 1
                require(active != null && active.contentEquals(id)) { "$rowLabel active pointer differs" }
            } else if (active == null) {
                require(status == BigInteger.valueOf(2)) { "$rowLabel inactive registry requires standby rows" }
            }
        }
        require(activeCount == if (active == null) 0 else 1) { "$label requires exactly one active row" }
        return registry
    }

    private fun releaseApproval(value: Any?, label: String) {
        val approval = objectValue(value, label)
        exact(approval, setOf("public_key", "signature"), label)
        canonicalPublicKey(approval["public_key"], "$label.public_key")
        canonicalSignature(approval["signature"], "$label.signature")
    }

    private fun devicePublicKey(value: Any?, label: String) {
        val tuple = array(value, label)
        require(tuple.size == 1) { "$label must contain one P-256 public key" }
        KagemushaP256Codec.requireUncompressedPublicKey(uppercaseHex(tuple[0], 65, "$label[0]"))
    }

    private fun deviceSignature(value: Any?, label: String) {
        val tuple = array(value, label)
        require(tuple.size == 1) { "$label must contain one P-256 signature" }
        KagemushaP256Codec.requireRawLowSSignature(uppercaseHex(tuple[0], 64, "$label[0]"))
    }

    private fun canonicalPublicKey(value: Any?, label: String): PublicKeyPayload {
        val literal = string(value, label)
        val parsed = requireNotNull(decodePublicKeyLiteral(literal)) { "$label must be a canonical public-key multihash" }
        require(encodePublicKeyMultihash(parsed.curveId, parsed.keyBytes) == literal) { "$label has noncanonical public-key spelling" }
        return parsed
    }

    private fun canonicalSignature(value: Any?, label: String) {
        val literal = string(value, label)
        require(literal.length >= 2 && literal.length % 2 == 0 && Regex("[0-9A-F]+").matches(literal) &&
            literal.chunked(2).any { it != "00" }) { "$label must be nonzero uppercase signature hex" }
    }

    private fun account(value: Any?, label: String) {
        requireCanonicalI105Address(string(value, label), label)
    }

    private fun bytes32(value: Any?, label: String, nonzero: Boolean = false): ByteArray {
        val values = array(value, label)
        require(values.size == 32) { "$label must have exactly 32 bytes" }
        val bytes = ByteArray(32)
        values.forEachIndexed { index, item -> bytes[index] = uint(item, U8_MAX, "$label[$index]").toByte() }
        require(!nonzero || bytes.any { it.toInt() != 0 }) { "$label must be nonzero" }
        return bytes
    }

    private fun uppercaseHex32(value: Any?, label: String): ByteArray = uppercaseHex(value, 32, label)

    private fun uppercaseHex(value: Any?, byteCount: Int, label: String): ByteArray {
        val literal = string(value, label)
        require(literal.length == byteCount * 2 && Regex("[0-9A-F]+").matches(literal)) {
            "$label must be exact uppercase hexadecimal"
        }
        return ByteArray(byteCount) { index -> literal.substring(index * 2, index * 2 + 2).toInt(16).toByte() }
    }

    private fun uint(value: Any?, maximum: BigInteger, label: String): BigInteger {
        require(value is Number) { "$label must be an unsigned JSON integer" }
        val literal = value.toString()
        require(Regex("0|[1-9][0-9]*").matches(literal)) { "$label must be an unsigned JSON integer" }
        val parsed = BigInteger(literal)
        require(parsed <= EXACT_JSON_MAX && parsed <= maximum) { "$label exceeds its exact integer bound" }
        return parsed
    }

    private fun compareUnsignedBytes(left: ByteArray, right: ByteArray): Int {
        for (index in 0 until minOf(left.size, right.size)) {
            val comparison = (left[index].toInt() and 0xff).compareTo(right[index].toInt() and 0xff)
            if (comparison != 0) return comparison
        }
        return left.size.compareTo(right.size)
    }

    private fun string(value: Any?, label: String): String =
        value as? String ?: throw IllegalArgumentException("$label must be a string")

    private fun array(value: Any?, label: String): List<*> =
        value as? List<*> ?: throw IllegalArgumentException("$label must be an array")

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, label: String): Map<String, Any?> {
        require(value is Map<*, *> && value.keys.all { it is String }) { "$label must be an object" }
        return value as Map<String, Any?>
    }

    private fun exact(value: Map<String, Any?>, fields: Set<String>, label: String) {
        require(value.keys == fields) { "$label contains unknown, aliased, or missing fields" }
    }
}
