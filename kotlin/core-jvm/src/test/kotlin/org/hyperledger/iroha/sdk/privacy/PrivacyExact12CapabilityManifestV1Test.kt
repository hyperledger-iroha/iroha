// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.core.model.NetworkId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue

class PrivacyExact12CapabilityManifestV1Test {
    @Test
    fun runtimeManifestRejectsDocumentaryQualificationFields() {
        val canonical = inspection()
        parseInspection(canonical)
        for (evidence in listOf("null", qualification(), qualification(pgcActivationHeight = 4, convergenceHeight = 5))) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(canonical.replace("\"protocols\":[", "\"qualification\":$evidence,\"protocols\":["))
            }
        }
        for (field in listOf("release_manifest", "deployment_qualification", "production_qualification")) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(canonical.replace("\"protocols\":[", "\"$field\":null,\"protocols\":["))
            }
        }
    }

    @Test
    fun retiredDocumentaryCatalogMutationsCannotEnterRuntimeManifest() {
        val canonical = "5bcf366fb18d2e4361486c015da5b41a3fb00edf05611042ccc452120d176eb0"
            .chunked(2).map { it.toInt(16) }
        assertEquals(32, canonical.size)
        val original = inspection().replace(
            "\"protocols\":[", "\"qualification\":${qualification()},\"protocols\":[",
        )
        val hostile = listOf(
            canonical.dropLast(1), canonical + 0, canonical + List(16) { 0 },
            "e037f13904a0307c00db15d85cfb406bd79772d20144a949def0f3fda78e342e747f65787cbfbffac94f11c369e2bbff"
                .chunked(2).map { it.toInt(16) },
        ) + canonical.indices.map { index ->
            canonical.toMutableList().also { it[index] = it[index] xor 1 }
        }
        val needle = "\"catalog_commitment\":" + catalogCommitment()
        for (bytes in hostile) {
            val replacement = "\"catalog_commitment\":" + bytes.joinToString(",", "[", "]")
            assertFailsWith<IllegalArgumentException> {
                parseInspection(original.replaceFirst(needle, replacement))
            }
            val first = original.indexOf(needle)
            val second = original.indexOf(needle, first + needle.length)
            check(first >= 0 && second > first)
            assertFailsWith<IllegalArgumentException> {
                parseInspection(original.substring(0, second) +
                    original.substring(second).replaceFirst(needle, replacement))
            }
        }
    }

    @Test
    fun decodedArchivesNeverGrantAdmissionIncludingAvailableRows() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        for (manifest in listOf(
            parseInspection(inspection()),
            parseInspection(inspection(mapOf(protocol to availablePolicyRow()))),
        )) {
            val error = assertFailsWith<IllegalArgumentException> {
                PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(manifest, protocol)
            }
            assertEquals(
                "Exact12 admission requires an authenticated Torii response; decoded archives are inspection-only",
                error.message,
            )
        }
    }

    @Test
    fun authoritativeNetworkIsMandatoryEvenWhenEveryProtocolIsUnavailable() {
        val expected = NetworkId.fromBytes(ByteArray(32) { 0xD1.toByte() })
        val other = NetworkId.fromBytes(ByteArray(32) { 0xD3.toByte() })
        for (input in listOf(inspection(), inspection(mapOf(PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 to availablePolicyRow())))) {
            val parsed = parseInspection(input)
            assertEquals(expected, parsed.networkId)
            parsed.requireNetwork(expected)
            assertFailsWith<IllegalArgumentException> { parsed.requireNetwork(other) }
            for (invalid in listOf("kotlin-test-network", expected.literal.lowercase(), "")) {
                assertFailsWith<IllegalArgumentException> { parseInspection(input.replace(expected.literal, invalid)) }
            }
            assertFailsWith<IllegalArgumentException> {
                parseInspection(input.replace("\"network_id\":\"${expected.literal}\",", ""))
            }
            assertFailsWith<IllegalArgumentException> {
                parseInspection(input.replace("\"network_id\":\"${expected.literal}\"", "\"network_id\":null"))
            }
        }
    }

    @Test
    fun pendingUpdateAcceptsNextBlockAndRejectsInvalidSnapshotHeights() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val cases = listOf(
            listOf("42", "43", "42", "true"),
            listOf("42", "44", "42", "true"),
            listOf("42", "44", "43", "true"),
            listOf("18446744073709551614", "18446744073709551615", "18446744073709551614", "true"),
            listOf("-1", "43", "42", "false"),
            listOf("0", "43", "42", "false"),
            listOf("42", "-1", "42", "false"),
            listOf("42", "0", "42", "false"),
            listOf("42", "42", "42", "false"),
            listOf("42", "41", "42", "false"),
            listOf("43", "44", "42", "false"),
            listOf("42", "43", "43", "false"),
            listOf("42", "43", "44", "false"),
            listOf("18446744073709551615", "18446744073709551615", "18446744073709551615", "false"),
            listOf("18446744073709551615", "18446744073709551616", "18446744073709551615", "false"),
            listOf("18446744073709551616", "18446744073709551617", "42", "false"),
        )
        for (consensus in listOf(true, false)) {
            for ((scheduled, effective, committed, accepted) in cases) {
                val next = if (consensus) consensusLimits().replace("2048", "1024") else
                    """{"protocol":"${protocol.canonicalLabel}","limits":{"max_anonymity_set_size":32,"max_recipient_count":8}}"""
                val pending = """{"scheduled_at_height":$scheduled,"effective_at_height":$effective,"next_limits":$next}"""
                var input = inspection(mapOf(protocol to availableRow(
                    "available", policy(),
                ))).replace("\"committed_height\":42", "\"committed_height\":$committed")
                input = if (consensus) input.replaceFirst("\"pending_update\":null", "\"pending_update\":$pending")
                    else input.replace("\"policy\":${policy()}", "\"policy\":${policy(pending = pending)}")
                if (accepted == "true") {
                    val manifest = parseInspection(input)
                    assertEquals(committed.toBigInteger(), manifest.committedHeight)
                    val parsedSchedule = if (consensus) {
                        manifest.consensusPolicy.pendingUpdate?.let {
                            it.scheduledAtHeight to it.effectiveAtHeight
                        }
                    } else {
                        manifest.rowFor(protocol).policy?.pendingUpdate?.let {
                            it.scheduledAtHeight to it.effectiveAtHeight
                        }
                    }
                    assertEquals(scheduled.toBigInteger() to effective.toBigInteger(), parsedSchedule)
                    assertFailsWith<IllegalArgumentException> {
                        PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(manifest, protocol)
                    }
                } else {
                    assertFailsWith<IllegalArgumentException> { parseInspection(input) }
                }
            }
        }
    }

    @Test
    fun parsesSevenFieldRowsAndPreservesCanonicalManifestState() {
        val manifest = parseInspection(inspection())

        assertEquals(PrivacyExact12CapabilityManifestV1.VERSION, manifest.version)
        assertEquals(42.toBigInteger(), manifest.committedHeight)
        assertEquals(2048, manifest.consensusPolicy.currentLimits.retainedRootCount)
        assertEquals(PrivacyProtocolIdV1.values().toList(), manifest.protocols.map { it.protocolId })
        assertEquals(7, exactRowKeys().size)
        assertEquals(NetworkId.fromBytes(ByteArray(32) { 0xD1.toByte() }), manifest.networkId)

        val readiness = assertIs<PrivacyCapabilityReadinessV1.Unavailable>(
            manifest.protocols.first().readiness,
        )
        val reason = assertIs<PrivacyCapabilityUnavailableReasonV1.CompiledProfile>(
            readiness.reason,
        )
        assertEquals(
            PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
            reason.failure.reason,
        )
        assertNull(manifest.protocols.first().policy)
        assertFalse(manifest.protocols.first().isNetworkAvailable())
    }

    @Test
    fun absenceOfPolicyUsesCanonicalLimitsAtEveryCommittedHeight() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        for (height in listOf("0", "1", "42", "18446744073709551615")) {
            val row = parseInspection(inspection(mapOf(protocol to availableRow("available", null)))
                .replace("\"committed_height\":42", "\"committed_height\":$height")).rowFor(protocol)
            assertNull(row.policy)
            assertTrue(row.isNetworkAvailable())
            assertEquals(64, row.effectiveProtocolLimits()!!.values!!.getValue("max_anonymity_set_size"))
        }
    }

    @Test
    fun retiredLifecycleAndRegistrationReasonsHaveNoDecodeFallback() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        for (reason in listOf("not-registered", "proposed", "active", "suspended", "retired")) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(inspection(mapOf(protocol to availableRow(reason, null))))
            }
        }
        val canonical = inspection(mapOf(protocol to availableRow("available", null)))
        for (state in listOf("proposed", "active", "suspended", "retired")) {
            val old = "{\"state\":\"$state\",\"record\":{\"proposed_at_height\":1,\"activated_at_height\":1,\"state_since_height\":2}}"
            assertFailsWith<IllegalArgumentException> {
                parseInspection(canonical.replaceFirst("\"policy\":null", "\"activation\":$old"))
            }
            assertFailsWith<IllegalArgumentException> {
                parseInspection(canonical.replaceFirst("\"policy\":null", "\"policy\":$old"))
            }
        }
    }

    @Test
    fun policyAllowsIncreasesWithinCanonicalBoundsAndNeverEnablesUnavailableRows() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val current = protocolLimits().replace(":64", ":32")
        val pending = "{\"scheduled_at_height\":42,\"effective_at_height\":43,\"next_limits\":${protocolLimits()}}"
        val ordinary = policy(current, pending)
        val row = parseInspection(inspection(mapOf(protocol to availableRow("available", ordinary)))).rowFor(protocol)
        assertTrue(row.isNetworkAvailable())
        assertEquals(32, row.effectiveProtocolLimits()!!.values!!.getValue("max_anonymity_set_size"))
        assertEquals(64, row.policy!!.pendingUpdate!!.nextLimits.values!!.getValue("max_anonymity_set_size"))
        val unavailable = unavailableRow(protocol).replace("\"policy\":null", "\"policy\":$ordinary")
        val closed = parseInspection(inspection(mapOf(protocol to unavailable))).rowFor(protocol)
        assertFalse(closed.isNetworkAvailable())
        assertNull(closed.effectiveProtocolLimits())
        for (bad in listOf(
            policy(protocolLimits(), pending),
            ordinary.replace("\"max_recipient_count\":8", "\"max_recipient_count\":9"),
            ordinary.replace("\"max_anonymity_set_size\":32", "\"max_anonymity_set_size\":48"),
        )) {
            assertFailsWith<IllegalArgumentException> { parseInspection(inspection(mapOf(protocol to availableRow("available", bad)))) }
        }
        val lowered = consensusLimits().replace("2048", "1024")
        val update = "{\"scheduled_at_height\":42,\"effective_at_height\":43,\"next_limits\":${consensusLimits()}}"
        val input = inspection().replace("\"current_limits\":${consensusLimits()},\"pending_update\":null",
            "\"current_limits\":$lowered,\"pending_update\":$update")
        assertEquals(2048, parseInspection(input).consensusPolicy.pendingUpdate!!.nextLimits.retainedRootCount)
    }

    @Test
    fun policyWireRejectsCrossProtocolMissingFieldsAndRetiredUpdateLayouts() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val ordinary = policy()
        val other = PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1
        for (bad in listOf(
            ordinary.replace(protocolTag(protocol), protocolTag(other)),
            ordinary.replace("\"current_limits\":${protocolLimits()},", ""),
            ordinary.replace("\"pending_update\":null", "\"pending_protocol_limits_tightening\":null"),
            ordinary.replace("\"pending_update\":null", "\"pending_update\":null,\"lifecycle\":null"),
        )) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(inspection(mapOf(protocol to availableRow("available", bad))))
            }
        }
        assertFailsWith<IllegalArgumentException> {
            parseInspection(inspection().replaceFirst("\"pending_update\":null", "\"pending_tightening\":null"))
        }
    }

    @Test
    fun rejectsRetiredRowReadinessAndActivationShapes() {
        val canonical = inspection()
        val readiness = compiledUnavailableReadiness()
        val hostile = listOf(
            canonical.replaceFirst(
                "\"policy\":null}",
                "\"policy\":null,\"activation_state\":{\"activation_state\":\"not-registered\",\"detail\":null}}",
            ),
            canonical.replaceFirst(
                "\"policy\":null}",
                "\"policy\":null,\"limitation\":null}",
            ),
            canonical.replaceFirst(readiness, "{\"readiness\":\"available\",\"detail\":null}"),
            canonical.replaceFirst(
                readiness,
                "{\"readiness\":\"available-experimental\",\"detail\":null}",
            ),
            inspection(
                mapOf(
                    PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 to
                        availableRow("available", policy()),
                ),
            ).replaceFirst(
                "\"pending_update\":null}",
                "\"pending_update\":null,\"production_qualification\":null}",
            ),
            canonical.replaceFirst("\"version\":1,", "\"version\":1,\"qualification\":null,"),
        )

        hostile.forEachIndexed { index, candidate ->
            assertFailsWith<IllegalArgumentException>("pre-hard-cut shape $index must fail") {
                parseInspection(candidate)
            }
        }
    }

    @Test
    fun implementationAvailabilityDoesNotAcceptRetiredDocumentaryReadiness() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val active = availablePolicyRow()
        assertTrue(parseInspection(inspection(mapOf(protocol to active))).rowFor(protocol).isNetworkAvailable())
        for (retired in listOf("production-qualified", "available-experimental")) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(inspection(mapOf(protocol to active.replace("\"readiness\":\"available\"", "\"readiness\":\"$retired\""))))
            }
        }
        for (reason in listOf("missing-production-qualification", "invalid-production-qualification")) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(inspection(mapOf(protocol to availableRow(reason, policy()))))
            }
        }
    }

    @Test
    fun oneSupportedProtocolIsAvailableWithoutMakingOtherRowsAvailable() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val manifest = parseInspection(inspection(mapOf(protocol to availablePolicyRow())))
        assertIs<PrivacyCapabilityReadinessV1.Available>(manifest.rowFor(protocol).readiness)
        assertTrue(manifest.rowFor(protocol).isNetworkAvailable())
        for (row in manifest.protocols.filter { it.protocolId != protocol }) {
            assertIs<PrivacyCapabilityReadinessV1.Unavailable>(row.readiness)
            assertFalse(row.isNetworkAvailable())
        }
        val x509 = manifest.rowFor(PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1)
        assertIs<PrivacyCompiledProfileResultV1.Unavailable>(x509.compiledProfile)
        assertFalse(x509.isNetworkAvailable())
        assertEquals(12, manifest.protocols.size)
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(manifest, protocol)
        }
    }

    @Test
    fun x509JointSecurityReductionRemainsUnavailableAndClosed() {
        val protocol = PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1
        val rowJson = unavailableRow(protocol)
            .replace("engine-unavailable", "joint-security-reduction-incomplete")
        val manifest = parseInspection(inspection(mapOf(protocol to rowJson)))
        val row = manifest.rowFor(protocol)
        val compiled = assertIs<PrivacyCompiledProfileResultV1.Unavailable>(row.compiledProfile)
        assertEquals(
            PrivacyCompiledProfileUnavailableReasonV1.JOINT_SECURITY_REDUCTION_INCOMPLETE,
            compiled.reason,
        )
        assertNull(compiled.statementSchemaError)
        val readiness = assertIs<PrivacyCapabilityReadinessV1.Unavailable>(row.readiness)
        assertEquals(compiled, assertIs<PrivacyCapabilityUnavailableReasonV1.CompiledProfile>(readiness.reason).failure)
        assertFalse(row.isNetworkAvailable())
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(manifest, protocol)
        }
        for (reason in listOf("unsupported", "joint-security-reduction-complete", "unknown")) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(inspection(mapOf(protocol to rowJson.replace("joint-security-reduction-incomplete", reason))))
            }
        }
        val extraDetail = rowJson.replace(
            "\"reason\":\"joint-security-reduction-incomplete\",\"detail\":null",
            "\"reason\":\"joint-security-reduction-incomplete\",\"detail\":{}",
        )
        assertFailsWith<IllegalArgumentException> {
            parseInspection(inspection(mapOf(protocol to extraDetail)))
        }
        val forgedAvailable = rowJson.replace(
            compiledUnavailableReadiness().replace("engine-unavailable", "joint-security-reduction-incomplete"),
            "{\"readiness\":\"available\",\"detail\":null}",
        )
        assertFailsWith<IllegalArgumentException> {
            parseInspection(inspection(mapOf(protocol to forgedAvailable)))
        }
    }

    @Test
    fun exactTwelveRowsAndLocalMatchCardinalityRemainWireRequirements() {
        val canonical = inspection()
        val rows = PrivacyProtocolIdV1.values().map { unavailableRow(it) }
        val rowList = rows.joinToString(",")
        val matches = List(12) { "false" }.joinToString(",")
        for (hostileRows in listOf(rows.dropLast(1), rows + rows.last(), rows.reversed())) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(canonical.replace(rowList, hostileRows.joinToString(",")))
            }
        }
        for (hostileMatches in listOf(
            List(11) { "false" }, List(13) { "false" }, List(11) { "false" } + "null",
            List(11) { "false" } + "0",
        )) {
            assertFailsWith<IllegalArgumentException> {
                parseInspection(canonical.replace(
                    "\"local_compiled_tuple_matches\":[$matches]",
                    "\"local_compiled_tuple_matches\":[${hostileMatches.joinToString(",")}]",
                ))
            }
        }
    }

    private fun parseInspection(value: String): PrivacyExact12CapabilityManifestV1 =
        PrivacyExact12CapabilityManifestInspectionV1.parse(
            byteArrayOf(1),
            value.toByteArray(StandardCharsets.UTF_8),
        )

    private fun inspection(
        overrides: Map<PrivacyProtocolIdV1, String> = emptyMap(),
    ): String {
        val rows = PrivacyProtocolIdV1.values().joinToString(",") { protocol ->
            overrides[protocol] ?: unavailableRow(protocol)
        }
        val matches = PrivacyProtocolIdV1.values().joinToString(",") { "false" }
        return """{"manifest":{"version":1,"network_id":"${NetworkId.fromBytes(ByteArray(32) { 0xD1.toByte() }).literal}","committed_height":42,"consensus_policy":{"current_limits":${consensusLimits()},"pending_update":null},"protocols":[$rows],"manifest_digest":${bytes(9, 32)}},"local_compiled_tuple_matches":[$matches]}"""
    }

    private fun unavailableRow(protocol: PrivacyProtocolIdV1): String =
        """{"protocol_id":${protocolTag(protocol)},"operation_schema":{"operation_schema":"${expectedOperationSchema(protocol).canonicalLabel}","value":null},"execution_mode":{"execution_mode":"${expectedExecutionMode(protocol).canonicalLabel}","value":null},"privacy_feature_mask":${expectedFeatureMask(protocol)},"compiled_profile":${compiledUnavailable()},"readiness":${compiledUnavailableReadiness()},"policy":null}"""

    private fun availableRow(reason: String, policy: String?): String {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val profile = profileFields(protocol)
        return """{"protocol_id":${protocolTag(protocol)},"operation_schema":{"operation_schema":"${expectedOperationSchema(protocol).canonicalLabel}","value":null},"execution_mode":{"execution_mode":"${expectedExecutionMode(protocol).canonicalLabel}","value":null},"privacy_feature_mask":${expectedFeatureMask(protocol)},"compiled_profile":{"status":"available","value":{$profile}},"readiness":${if (reason == "available") "{\"readiness\":\"available\",\"detail\":null}" else unavailableReadiness(reason)},"policy":${policy ?: "null"}}"""
    }

    private fun availablePolicyRow(): String = availableRow("available", policy())

    private fun protocolLimits(): String =
        """{"protocol":"${PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1.canonicalLabel}","limits":{"max_anonymity_set_size":64,"max_recipient_count":8}}"""

    private fun policy(current: String = protocolLimits(), pending: String = "null"): String =
        """{"protocol_id":${protocolTag(PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1)},"current_limits":$current,"pending_update":$pending}"""

    private fun profileFields(protocol: PrivacyProtocolIdV1): String =
        """"protocol_id":${protocolTag(protocol)},"proof_system_id":{"proof_system":"${protocol.expectedProofSystem.canonicalLabel}","value":null},"engine_id":{"engine":"${protocol.expectedEngine.canonicalLabel}","value":null},"parameter_id":${bytes(1, 32)},"parameter_digest":${bytes(2, 32)},"verifier_digest":${bytes(3, 32)},"statement_schema_digest":${bytes(4, 32)},"engine_manifest_digest":${bytes(5, 32)},"protocol_limits":{"protocol":"${protocol.canonicalLabel}","limits":{"max_anonymity_set_size":64,"max_recipient_count":8}}"""

    private fun compiledUnavailable(): String =
        """{"status":"unavailable","value":{"reason":"engine-unavailable","detail":null}}"""

    private fun compiledUnavailableReadiness(): String =
        """{"readiness":"unavailable","detail":{"reason":"compiled-profile","detail":{"reason":"engine-unavailable","detail":null}}}"""

    private fun unavailableReadiness(reason: String): String =
        """{"readiness":"unavailable","detail":{"reason":"$reason","detail":null}}"""

    private fun protocolTag(protocol: PrivacyProtocolIdV1): String =
        """{"protocol":"${protocol.canonicalLabel}","value":null}"""

    private fun exactRowKeys(): Set<String> = setOf(
        "protocol_id",
        "operation_schema",
        "execution_mode",
        "privacy_feature_mask",
        "compiled_profile",
        "readiness",
        "policy",
    )

    private fun qualification(pgcActivationHeight: Int = 2, convergenceHeight: Int = 3): String {
        val releaseDigest = bytes(177, 32)
        val bindings = PrivacyProtocolIdV1.values().joinToString(",") { protocol ->
            releaseBinding(protocol)
        }
        val activations = PrivacyProtocolIdV1.values().joinToString(",") { protocol ->
            val height = if (protocol == PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1) {
                pgcActivationHeight
            } else {
                2
            }
            """{"protocol_id":${protocolTag(protocol)},"activation_height":$height}"""
        }
        val release = """{"version":1,"catalog_id":"iroha-privacy-exact12-v1","catalog_commitment":${catalogCommitment()},"source":{"source_tree_digest":${bytes(161, 32)},"source_tree_clean":true,"toolchain_id":"kotlin-test-toolchain","toolchain_digest":${bytes(162, 32)},"cargo_lock_digest":${bytes(163, 32)}},"abi_version":1,"abi_hash":${bytes(164, 32)},"syscall_list_digest":${bytes(165, 32)},"executables":[],"protocols":[$bindings],"stage_receipts":[],"proof_artifacts":[],"sdk_packages":[],"hardware_results":[],"release_artifact_set_digest":${bytes(166, 32)},"audits":[],"audit_bundle_digest":${bytes(226, 32)},"release_signatures":[],"manifest_digest":$releaseDigest}"""
        val deployment = """{"version":1,"chain_id":"kotlin-test-chain","network_id":"${NetworkId.fromBytes(ByteArray(32) { 0xD1.toByte() }).literal}","genesis_hash":${bytes(209, 32)},"release_manifest_digest":$releaseDigest,"activation_transaction_digest":${bytes(209, 32)},"activations":[$activations],"validator_roster_digest":${bytes(210, 32)},"endpoint_version":"v1","convergence_height":$convergenceHeight,"converged_state_digest":${bytes(211, 32)},"validator_canaries":[],"validator_signatures":[],"qualification_digest":${bytes(228, 32)}}"""
        return """{"release_manifest":$release,"deployment_qualification":$deployment}"""
    }

    private fun releaseBinding(protocol: PrivacyProtocolIdV1): String {
        val index = protocol.ordinal
        val parameterId = if (index == 1) 1 else 10 + index
        val parameterDigest = if (index == 1) 2 else 30 + index
        val verifierDigest = if (index == 1) 3 else 50 + index
        val statementDigest = if (index == 1) 4 else 70 + index
        val engineDigest = if (index == 1) 5 else 90 + index
        val claim = """{"catalog_commitment":${catalogCommitment()},"protocol_id":${protocolTag(protocol)},"security_model":{"security_model":"${expectedSecurityModelV1(protocol).canonicalLabel}","value":null},"target_security_bits":128,"achieved_security_bits":128,"parameter_digest":${bytes(parameterDigest, 32)},"verifier_digest":${bytes(verifierDigest, 32)},"reduction_digest":${bytes(120 + index, 32)},"audit_bundle_digest":${bytes(226, 32)}}"""
        return """{"protocol_id":${protocolTag(protocol)},"proof_system_id":{"proof_system":"${protocol.expectedProofSystem.canonicalLabel}","value":null},"engine_id":{"engine":"${protocol.expectedEngine.canonicalLabel}","value":null},"parameter_id":${bytes(parameterId, 32)},"parameter_digest":${bytes(parameterDigest, 32)},"verifier_digest":${bytes(verifierDigest, 32)},"statement_schema_digest":${bytes(statementDigest, 32)},"engine_manifest_digest":${bytes(engineDigest, 32)},"security_claim":$claim,"security_claim_digest":${bytes(140 + index, 32)}}"""
    }

    private fun catalogCommitment(): String {
        val hex = "5bcf366fb18d2e4361486c015da5b41a3fb00edf05611042ccc452120d176eb0"
        return hex.chunked(2).map { it.toInt(16) }.joinToString(",", "[", "]")
    }

    private fun bytes(value: Int, count: Int): String =
        List(count) { value }.joinToString(",", "[", "]")

    private fun consensusLimits(): String =
        """{"max_actions_per_transaction":1,"max_actions_per_block":2,"max_proof_bytes_per_action":9437184,"max_action_bytes":9699840,"max_privacy_bytes_per_transaction":9699840,"max_privacy_bytes_per_block":16711680,"max_statement_and_encrypted_output_bytes_per_transaction":262144,"max_nullifiers_per_action":8,"max_commitments_per_action":8,"retained_root_count":2048}"""
}
