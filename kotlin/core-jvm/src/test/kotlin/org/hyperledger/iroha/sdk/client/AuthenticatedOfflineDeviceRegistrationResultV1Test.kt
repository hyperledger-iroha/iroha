package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceEligibilityOutcomeV1
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceEligibilityReasonV1

class AuthenticatedOfflineDeviceRegistrationResultV1Test {
    @Test
    fun exactAppliedAndEligibilityRejectedShapesDecode() {
        val applied = parse(registrationResultJson())
        assertEquals(OfflineDeviceRegistrationTerminalStateV1.APPLIED, applied.terminalState)
        assertEquals(BigInteger.valueOf(7), applied.committedBlockHeight)
        assertNull(applied.eligibilityOutcome)
        assertNull(applied.rejectionMessage)

        val rejected = parse(
            registrationResultJson(
                terminalState = "eligibility_rejected",
                eligibilityOutcome = "\"drain_only\"",
                eligibilityReason = "\"vulnerable_firmware\"",
                matchedRules = "[\"CVE-2026-21046\",\"samsung-keymaster-2021\"]",
                rejectionCode = "\"offline_device_eligibility\"",
                rejectionMessage = "\"device firmware is governed drain-only\"",
                height = "18446744073709551615",
            ),
        )
        assertEquals(
            OfflineDeviceRegistrationTerminalStateV1.ELIGIBILITY_REJECTED,
            rejected.terminalState,
        )
        assertEquals(OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY, rejected.eligibilityOutcome)
        assertEquals(
            OfflineDeviceEligibilityReasonV1.VULNERABLE_FIRMWARE,
            rejected.eligibilityReason,
        )
        assertEquals(
            listOf("CVE-2026-21046", "samsung-keymaster-2021"),
            rejected.matchedRuleIds,
        )
        assertEquals(BigInteger("18446744073709551615"), rejected.committedBlockHeight)
    }

    @Test
    fun cryptographicAndOtherRejectionsRemainDistinct() {
        val cryptographic = parse(
            registrationResultJson(
                terminalState = "eligibility_rejected",
                eligibilityOutcome = "\"cryptographically_rejected\"",
                eligibilityReason = "\"cryptographic_attestation_rejected\"",
                rejectionCode = "\"offline_device_eligibility\"",
                rejectionMessage = "\"attestation chain was rejected\"",
            ),
        )
        assertEquals(
            OfflineDeviceEligibilityOutcomeV1.CRYPTOGRAPHICALLY_REJECTED,
            cryptographic.eligibilityOutcome,
        )
        assertEquals(emptyList(), cryptographic.matchedRuleIds)

        val other = parse(
            registrationResultJson(
                terminalState = "other_rejected",
                rejectionCode = "\"validation\"",
                rejectionMessage = "\"authority is not permitted\"",
            ),
        )
        assertEquals(OfflineDeviceRegistrationTerminalStateV1.OTHER_REJECTED, other.terminalState)
        assertNull(other.eligibilityOutcome)
        assertEquals("validation", other.rejectionCode)
    }

    @Test
    fun parserRejectsFieldAndTerminalShapeSubstitution() {
        val hostile = listOf(
            registrationResultJson(version = "2"),
            registrationResultJson(extraField = ",\"unexpected\":true"),
            registrationResultJson(height = "07"),
            registrationResultJson(height = "18446744073709551616"),
            registrationResultJson(transactionHash = "AB".repeat(32)),
            registrationResultJson(rejectionMessage = "\"contradiction\""),
            registrationResultJson(
                terminalState = "eligibility_rejected",
                eligibilityOutcome = "\"eligible\"",
                eligibilityReason = "\"policy_satisfied\"",
                rejectionCode = "\"offline_device_eligibility\"",
                rejectionMessage = "\"contradiction\"",
            ),
            registrationResultJson(
                terminalState = "eligibility_rejected",
                eligibilityOutcome = "\"drain_only\"",
                eligibilityReason = "\"vulnerable_firmware\"",
                rejectionCode = "\"offline_device_eligibility\"",
                rejectionMessage = "\"missing matched rule\"",
            ),
            registrationResultJson(
                terminalState = "eligibility_rejected",
                eligibilityOutcome = "\"drain_only\"",
                eligibilityReason = "\"policy_not_fresh\"",
                matchedRules = "[\"unexpected-rule\"]",
                rejectionCode = "\"offline_device_eligibility\"",
                rejectionMessage = "\"unexpected matched rule\"",
            ),
            registrationResultJson(
                terminalState = "eligibility_rejected",
                eligibilityOutcome = "\"drain_only\"",
                eligibilityReason = "\"vulnerable_firmware\"",
                matchedRules = "[\"z-rule\",\"a-rule\"]",
                rejectionCode = "\"offline_device_eligibility\"",
                rejectionMessage = "\"unsorted rules\"",
            ),
            registrationResultJson(
                terminalState = "other_rejected",
                rejectionCode = "\"invented\"",
                rejectionMessage = "\"unknown code\"",
            ),
        )
        hostile.forEachIndexed { index, json ->
            assertFailsWith<IllegalStateException>("accepted hostile projection $index") {
                parse(json)
            }
        }
    }

    private fun parse(json: String): AuthenticatedOfflineDeviceRegistrationResultV1 =
        AuthenticatedOfflineDeviceRegistrationResultV1.parseNativeJson(json.toByteArray())

    private fun registrationResultJson(
        version: String = "1",
        transactionHash: String = "ab".repeat(32),
        height: String = "7",
        terminalState: String = "applied",
        eligibilityOutcome: String = "null",
        eligibilityReason: String = "null",
        matchedRules: String = "[]",
        rejectionCode: String = "null",
        rejectionMessage: String = "null",
        extraField: String = "",
    ): String = """
        {"version":$version,"transaction_hash_hex":"$transactionHash","transaction_authority":"alice@wonderland","block_hash_hex":"${"cd".repeat(32)}","result_hash_hex":"${"ef".repeat(32)}","committed_block_height":"$height","terminal_state":"$terminalState","eligibility_outcome":$eligibilityOutcome,"eligibility_reason":$eligibilityReason,"matched_rule_ids":$matchedRules,"rejection_code":$rejectionCode,"rejection_message":$rejectionMessage$extraField}
    """.trimIndent()
}
