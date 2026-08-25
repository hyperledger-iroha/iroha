package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class GovernanceSelectorV1Test {
    @Test
    fun `governance ballot selectors accept exact v1 boundaries`() {
        val selectors = listOf(
            "a",
            "A9_selector~with.dots",
            "a".repeat(128),
        )

        selectors.forEach { selector ->
            assertEquals(selector, plainBallot(selector).referendumId)
            assertEquals(selector, zkBallot(selector).electionId())
        }
    }

    @Test
    fun `governance ballot selectors reject noncanonical v1 values`() {
        val selectors = listOf(
            "",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a%2Fb",
            "has space",
            "投票",
            "a".repeat(129),
        )

        selectors.forEach { selector ->
            assertFailsWith<IllegalArgumentException>("plain ballot accepted '$selector'") {
                plainBallot(selector)
            }
            assertFailsWith<IllegalArgumentException>("ZK ballot accepted '$selector'") {
                zkBallot(selector)
            }
        }
    }

    @Test
    fun `governance selector argument factories share the strict validator`() {
        val invalid = ".hidden"

        assertFailsWith<IllegalArgumentException> {
            CastPlainBallotInstruction.fromArguments(
                mapOf(
                    "action" to "CastPlainBallot",
                    "referendum_id" to invalid,
                    "owner" to "owner",
                    "amount" to "1",
                    "duration_blocks" to "1",
                    "direction" to "0",
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CastZkBallotInstruction.fromArguments(
                mapOf(
                    "action" to "CastZkBallot",
                    "election_id" to invalid,
                    "proof_b64" to "AA==",
                    "public_inputs_json" to "{}",
                ),
            )
        }
    }

    private fun plainBallot(selector: String): CastPlainBallotInstruction =
        CastPlainBallotInstruction(
            referendumId = selector,
            ownerAccountId = "owner",
            amount = "1",
            durationBlocks = 1,
            direction = 0,
        )

    private fun zkBallot(selector: String): CastZkBallotInstruction =
        CastZkBallotInstruction.builder()
            .setElectionId(selector)
            .setProofBase64("AA==")
            .setPublicInputsJson("{}")
            .build()
}
