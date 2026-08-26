package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KaigiInstructionValidationTest {

    @Test
    fun `usage duration above signed int range round trips`() {
        val durationMs = 3_000_000_000L
        val instruction = RecordKaigiUsageInstruction(
            callId = KaigiInstructionUtils.CallId("wonderland", "sync"),
            durationMs = durationMs,
        )

        val decoded = RecordKaigiUsageInstruction.fromArguments(instruction.arguments)

        assertEquals(durationMs, decoded.durationMs)
        assertEquals(instruction, decoded)
    }

    @Test
    fun `relay manifest parser rejects oversized and sparse hop indices`() {
        val oversized = linkedMapOf(
            "action" to "SetKaigiRelayManifest",
            "call.domain_id" to "wonderland",
            "call.call_name" to "sync",
            "relay_manifest.expiry_ms" to "100",
            "relay_manifest.hop.${Int.MAX_VALUE}.relay_id" to "relay-a",
        )
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.fromArguments(oversized)
        }

        val sparse = linkedMapOf(
            "action" to "SetKaigiRelayManifest",
            "call.domain_id" to "wonderland",
            "call.call_name" to "sync",
            "relay_manifest.expiry_ms" to "100",
            "relay_manifest.hop.0.relay_id" to "relay-a",
            "relay_manifest.hop.0.hpke_public_key" to key(1),
            "relay_manifest.hop.0.weight" to "1",
            "relay_manifest.hop.2.relay_id" to "relay-c",
            "relay_manifest.hop.2.hpke_public_key" to key(3),
            "relay_manifest.hop.2.weight" to "1",
        )
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.fromArguments(sparse)
        }
    }

    @Test
    fun `relay registration requires a nonzero explicit bandwidth class`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterKaigiRelayInstruction("relay-a", key(1), 0)
        }
    }

    @Test
    fun `relay manifests require expiry three unique hops valid keys and positive weights`() {
        val valid = SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "sync")
            .setRelayManifestExpiryMs(100)
            .addRelayManifestHop("relay-a", key(1), 1)
            .addRelayManifestHop("relay-b", key(2), 2)
            .addRelayManifestHop("relay-c", key(3), 255)
            .build()
        assertEquals(valid, SetKaigiRelayManifestInstruction.fromArguments(valid.arguments))

        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "sync")
                .setRelayManifestExpiryMs(100)
                .addRelayManifestHop("relay-a", key(1), 1)
                .addRelayManifestHop("relay-b", key(2), 1)
                .build()
        }
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "sync")
                .addRelayManifestHop("relay-a", key(1), 1)
                .addRelayManifestHop("relay-b", key(2), 1)
                .addRelayManifestHop("relay-c", key(3), 1)
                .build()
        }
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "sync")
                .setRelayManifestExpiryMs(100)
                .addRelayManifestHop("relay-a", key(1), 1)
                .addRelayManifestHop("relay-a", key(2), 1)
                .addRelayManifestHop("relay-c", key(3), 1)
                .build()
        }
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "sync")
                .setRelayManifestExpiryMs(100)
                .addRelayManifestHop("relay-a", key(1), 0)
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.create(
                callId = KaigiInstructionUtils.CallId("wonderland", "sync"),
                host = "host",
                relayManifest = KaigiInstructionUtils.RelayManifest(
                    100,
                    listOf(
                        KaigiInstructionUtils.RelayManifestHop("relay-a", key(1), 1),
                        KaigiInstructionUtils.RelayManifestHop("relay-b", "", 1),
                        KaigiInstructionUtils.RelayManifestHop("relay-c", key(3), 1),
                    ),
                ),
            )
        }
    }

    @Test
    fun `kaigi privacy artifacts preserve only ledger safe fields`() {
        val callId = KaigiInstructionUtils.CallId("wonderland", "sync")
        val proof = key(9)
        val create = CreateKaigiInstruction.create(
            callId = callId,
            host = "host",
            roomPolicy = KaigiInstructionUtils.RoomPolicy("Public", null),
            commitment = "commitment-literal",
            nullifierDigest = "nullifier-literal",
            nullifierIssuedAtMs = 0,
            rosterRoot = "roster-literal",
            proofBase64 = proof,
        )
        val join = JoinKaigiInstruction(
            callId = callId,
            participant = "participant",
            commitment = "commitment-literal",
            nullifierDigest = "nullifier-literal",
            nullifierIssuedAtMs = 0,
            rosterRoot = "roster-literal",
            proofBase64 = proof,
        )
        val leave = LeaveKaigiInstruction(
            callId = callId,
            participant = "participant",
            commitment = "commitment-literal",
            nullifierDigest = "nullifier-literal",
            nullifierIssuedAtMs = 0,
            rosterRoot = "roster-literal",
            proofBase64 = proof,
        )
        val end = EndKaigiInstruction(
            callId = callId,
            endedAtMs = 84,
            commitment = "commitment-literal",
            nullifierDigest = "nullifier-literal",
            nullifierIssuedAtMs = 0,
            rosterRoot = "roster-literal",
            proofBase64 = proof,
        )

        for (instruction in listOf(create, join, leave, end)) {
            assertEquals(null, instruction.arguments["commitment.alias_tag"])
            assertEquals("0", instruction.arguments["nullifier.issued_at_ms"])
        }
        assertEquals("Public", create.arguments["room_policy.policy"])
        assertEquals("roster-literal", create.arguments["roster_root"])
        assertEquals(proof, create.arguments["proof"])
        assertEquals(create, CreateKaigiInstruction.fromArguments(create.arguments))
        assertEquals(join, JoinKaigiInstruction.fromArguments(join.arguments))
        assertEquals(leave, LeaveKaigiInstruction.fromArguments(leave.arguments))
        assertEquals(end, EndKaigiInstruction.fromArguments(end.arguments))

        val createWithoutIssuedAt = CreateKaigiInstruction.create(
            callId = callId,
            host = "host",
            nullifierDigest = "nullifier-literal",
        )
        val joinWithoutIssuedAt = JoinKaigiInstruction(
            callId = callId,
            participant = "participant",
            nullifierDigest = "nullifier-literal",
        )
        val leaveWithoutIssuedAt = LeaveKaigiInstruction(
            callId = callId,
            participant = "participant",
            nullifierDigest = "nullifier-literal",
        )
        val endWithoutIssuedAt = EndKaigiInstruction(
            callId = callId,
            nullifierDigest = "nullifier-literal",
        )
        for (instruction in listOf(
            createWithoutIssuedAt,
            joinWithoutIssuedAt,
            leaveWithoutIssuedAt,
            endWithoutIssuedAt,
        )) {
            assertEquals(null, instruction.arguments["nullifier.issued_at_ms"])
        }
        assertEquals(
            createWithoutIssuedAt,
            CreateKaigiInstruction.fromArguments(createWithoutIssuedAt.arguments),
        )
        assertEquals(
            joinWithoutIssuedAt,
            JoinKaigiInstruction.fromArguments(joinWithoutIssuedAt.arguments),
        )
        assertEquals(
            leaveWithoutIssuedAt,
            LeaveKaigiInstruction.fromArguments(leaveWithoutIssuedAt.arguments),
        )
        assertEquals(
            endWithoutIssuedAt,
            EndKaigiInstruction.fromArguments(endWithoutIssuedAt.arguments),
        )
    }

    @Test
    fun `kaigi builders and parsers reject clear privacy identity hints`() {
        val callId = KaigiInstructionUtils.CallId("wonderland", "sync")
        val create = CreateKaigiInstruction.create(callId = callId, host = "host")
        val join = JoinKaigiInstruction(callId = callId, participant = "participant")
        val leave = LeaveKaigiInstruction(callId = callId, participant = "participant")
        val end = EndKaigiInstruction(callId = callId)

        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.create(
                callId = callId,
                host = "host",
                commitmentAliasTag = "host-alias",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            JoinKaigiInstruction(callId, "participant", commitmentAliasTag = "participant-alias")
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction(callId, "participant", commitmentAliasTag = "participant-alias")
        }
        assertFailsWith<IllegalArgumentException> {
            EndKaigiInstruction(callId, commitmentAliasTag = "host-alias")
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.create(callId, "host", nullifierIssuedAtMs = 1)
        }
        assertFailsWith<IllegalArgumentException> {
            JoinKaigiInstruction(callId, "participant", nullifierIssuedAtMs = 1)
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction(callId, "participant", nullifierIssuedAtMs = 1)
        }
        assertFailsWith<IllegalArgumentException> {
            EndKaigiInstruction(callId, nullifierIssuedAtMs = 1)
        }

        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("commitment.alias_tag" to "host-alias"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            JoinKaigiInstruction.fromArguments(
                join.arguments + ("commitment.alias_tag" to "participant-alias"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction.fromArguments(
                leave.arguments + ("commitment.alias_tag" to "participant-alias"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            EndKaigiInstruction.fromArguments(
                end.arguments + ("commitment.alias_tag" to "host-alias"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("nullifier.issued_at_ms" to "1"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            JoinKaigiInstruction.fromArguments(
                join.arguments + ("nullifier.issued_at_ms" to "1"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction.fromArguments(
                leave.arguments + ("nullifier.issued_at_ms" to "1"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            EndKaigiInstruction.fromArguments(
                end.arguments + ("nullifier.issued_at_ms" to "1"),
            )
        }
    }

    private fun key(value: Int): String = Base64.getEncoder().encodeToString(byteArrayOf(value.toByte()))
}
