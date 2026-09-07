package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

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
    fun `bounded participants and full width u64 fields round trip through JVM carriers`() {
        val callId = KaigiInstructionUtils.CallId("wonderland", "unsigned-boundary")
        val u64Max = -1L
        val u64MaxText = "18446744073709551615"
        val maxParticipants = CreateKaigiInstruction.MAX_PARTICIPANTS_V1
        val maxParticipantsText = "4096"

        val create = CreateKaigiInstruction.create(
            callId = callId,
            host = "host",
            maxParticipants = maxParticipants,
            gasRatePerMinute = u64Max,
            scheduledStartMs = u64Max,
        )
        assertEquals(maxParticipantsText, create.arguments["max_participants"])
        assertEquals(u64MaxText, create.arguments["gas_rate_per_minute"])
        assertEquals(u64MaxText, create.arguments["scheduled_start_ms"])
        assertEquals(create, CreateKaigiInstruction.fromArguments(create.arguments))

        val end = EndKaigiInstruction(callId, endedAtMs = u64Max)
        assertEquals(u64MaxText, end.arguments["ended_at_ms"])
        assertEquals(end, EndKaigiInstruction.fromArguments(end.arguments))

        val usage = RecordKaigiUsageInstruction(callId, durationMs = u64Max, billedGas = u64Max)
        assertEquals(u64MaxText, usage.arguments["duration_ms"])
        assertEquals(u64MaxText, usage.arguments["billed_gas"])
        assertEquals(usage, RecordKaigiUsageInstruction.fromArguments(usage.arguments))

        val health = ReportKaigiRelayHealthInstruction(
            callId = callId,
            relayId = "relay",
            status = ReportKaigiRelayHealthInstruction.Status.HEALTHY,
            reportedAtMs = u64Max,
        )
        assertEquals(u64MaxText, health.arguments["reported_at_ms"])
        assertEquals(health, ReportKaigiRelayHealthInstruction.fromArguments(health.arguments))

        val manifest = SetKaigiRelayManifestInstruction.builder()
            .setCallId(callId)
            .setRelayManifestExpiryMs(u64Max)
            .addRelayManifestHop("relay-a", key(1), 1)
            .addRelayManifestHop("relay-b", key(2), 1)
            .addRelayManifestHop("relay-c", key(3), 1)
            .build()
        assertEquals(u64MaxText, manifest.arguments["relay_manifest.expiry_ms"])
        assertEquals(manifest, SetKaigiRelayManifestInstruction.fromArguments(manifest.arguments))

        assertFailsWith<IllegalArgumentException> {
            RecordKaigiUsageInstruction.fromArguments(
                usage.arguments + ("duration_ms" to "01"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            RecordKaigiUsageInstruction.fromArguments(
                usage.arguments + ("billed_gas" to "18446744073709551616"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("scheduled_start_ms" to ""),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("max_participants" to "4294967296"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.create(
                callId = callId,
                host = "host",
                maxParticipants = maxParticipants + 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.create(
                callId = callId,
                host = "host",
                maxParticipants = -1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("max_participants" to "4097"),
            )
        }
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

        val nonCanonical = linkedMapOf(
            "action" to "SetKaigiRelayManifest",
            "call.domain_id" to "wonderland",
            "call.call_name" to "sync",
            "relay_manifest.expiry_ms" to "100",
            "relay_manifest.hop.00.relay_id" to "relay-a",
        )
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.fromArguments(nonCanonical)
        }
    }

    @Test
    fun `typed Kaigi parsers require their exact action discriminator`() {
        val usage = RecordKaigiUsageInstruction(
            KaigiInstructionUtils.CallId("wonderland", "sync"),
            durationMs = 1,
        )
        assertFailsWith<IllegalArgumentException> {
            RecordKaigiUsageInstruction.fromArguments(
                usage.arguments + ("action" to "EndKaigi"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            RecordKaigiUsageInstruction.fromArguments(usage.arguments - "action")
        }
    }

    @Test
    fun `parsers reject unknown fields and rebuild immutable canonical maps`() {
        val rawScalar = "01".repeat(32)
        val shuffled = linkedMapOf(
            "metadata.z" to "\"last\"", "commitment.commitment" to rawScalar,
            "host" to "host", "call.call_name" to "sync", "call.domain_id" to "wonderland",
            "action" to "CreateKaigi", "metadata.a" to "\"first\"",
            "privacy.mode" to "ZkRosterV1", "nullifier.digest" to "02".repeat(32),
            "roster_root" to hash(3), "proof" to key(1),
        )
        val parsedCreate = CreateKaigiInstruction.fromArguments(shuffled)
        assertEquals(listOf(
            "action", "call.domain_id", "call.call_name", "host", "gas_rate_per_minute",
            "metadata.a", "metadata.z", "privacy.mode", "room_policy.policy",
            "commitment.commitment", "nullifier.digest", "roster_root", "proof",
        ), parsedCreate.arguments.keys.toList())
        assertEquals(rawScalar, parsedCreate.arguments["commitment.commitment"])
        val lowercaseLiteral = KaigiInstructionUtils.canonicalizeHash(hash(0xAB)).lowercase()
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(shuffled + ("commitment.commitment" to lowercaseLiteral))
        }
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (parsedCreate.arguments as MutableMap<String, String>)["host"] = "changed"
        }
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (parsedCreate.metadata as MutableMap<String, String>)["new"] = "value"
        }

        val mutableHops = mutableListOf(
            KaigiInstructionUtils.RelayManifestHop("relay-a", key(1), 1),
            KaigiInstructionUtils.RelayManifestHop("relay-b", key(2), 1),
            KaigiInstructionUtils.RelayManifestHop("relay-c", key(3), 1),
        )
        val manifestSnapshot = KaigiInstructionUtils.RelayManifest(100, mutableHops)
        val manifestCreate = CreateKaigiInstruction.create(
            callId = KaigiInstructionUtils.CallId("wonderland", "manifest-snapshot"),
            host = "host",
            relayManifest = manifestSnapshot,
        )
        val manifestArguments = manifestCreate.arguments.toMap()
        mutableHops.clear()
        assertEquals(3, manifestSnapshot.hops.size)
        assertEquals(3, manifestCreate.relayManifest!!.hops.size)
        assertEquals(manifestArguments, manifestCreate.arguments)
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (manifestSnapshot.hops as MutableList<KaigiInstructionUtils.RelayManifestHop>).clear()
        }

        val callId = KaigiInstructionUtils.CallId("wonderland", "sync")
        val instructions = listOf<InstructionTemplate>(
            parsedCreate,
            JoinKaigiInstruction(callId, "participant"),
            LeaveKaigiInstruction(callId, "participant"),
            EndKaigiInstruction(callId),
            RecordKaigiUsageInstruction(callId, 1),
            RegisterKaigiRelayInstruction("relay", key(1), 1),
            UnregisterKaigiRelayInstruction("relay"),
            SetKaigiRelayManifestInstruction.builder().setCallId(callId).build(),
            ReportKaigiRelayHealthInstruction(
                callId,
                "relay",
                ReportKaigiRelayHealthInstruction.Status.HEALTHY,
                1,
            ),
        )
        val parsers = listOf<(Map<String, String>) -> Unit>(
            { CreateKaigiInstruction.fromArguments(it) },
            { JoinKaigiInstruction.fromArguments(it) },
            { LeaveKaigiInstruction.fromArguments(it) },
            { EndKaigiInstruction.fromArguments(it) },
            { RecordKaigiUsageInstruction.fromArguments(it) },
            { RegisterKaigiRelayInstruction.fromArguments(it) },
            { UnregisterKaigiRelayInstruction.fromArguments(it) },
            { SetKaigiRelayManifestInstruction.fromArguments(it) },
            { ReportKaigiRelayHealthInstruction.fromArguments(it) },
        )
        for ((instruction, parser) in instructions.zip(parsers)) {
            assertFailsWith<IllegalArgumentException> {
                parser(instruction.arguments + ("unknown" to "value"))
            }
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(shuffled + ("metadata." to "malformed"))
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(shuffled + ("commitment.commitment" to ""))
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterKaigiRelayInstruction("relay", "AQ", 1)
        }
        val unregistration = UnregisterKaigiRelayInstruction("relay")
        assertEquals(unregistration, UnregisterKaigiRelayInstruction.fromArguments(unregistration.arguments))
        assertFailsWith<IllegalArgumentException> { UnregisterKaigiRelayInstruction(" ") }
        assertEquals(rawScalar, parsedCreate.arguments["commitment.commitment"])
        assertTrue(parsedCreate.arguments["roster_root"]!!.startsWith("hash:"))
    }

    @Test
    fun `relay health reports validate status notes and canonical maps`() {
        val maxNotes = "\uD83D\uDE00".repeat(512)
        val report = ReportKaigiRelayHealthInstruction(
            callId = KaigiInstructionUtils.CallId("wonderland", "sync"),
            relayId = "relay",
            status = ReportKaigiRelayHealthInstruction.Status.DEGRADED,
            reportedAtMs = -1,
            notes = maxNotes,
        )

        assertEquals(
            listOf(
                "action",
                "call.domain_id",
                "call.call_name",
                "relay_id",
                "status",
                "reported_at_ms",
                "notes",
            ),
            report.arguments.keys.toList(),
        )
        assertEquals("Degraded", report.arguments["status"])
        assertEquals("18446744073709551615", report.arguments["reported_at_ms"])
        assertEquals(report, ReportKaigiRelayHealthInstruction.fromArguments(report.arguments))
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (report.arguments as MutableMap<String, String>)["status"] = "Healthy"
        }

        val zeroTimestamp = ReportKaigiRelayHealthInstruction(
            callId = report.callId,
            relayId = report.relayId,
            status = ReportKaigiRelayHealthInstruction.Status.HEALTHY,
            reportedAtMs = 0,
        )
        assertEquals("0", zeroTimestamp.arguments["reported_at_ms"])

        val emptyNotes = ReportKaigiRelayHealthInstruction.fromArguments(
            report.arguments + ("notes" to ""),
        )
        assertEquals("", emptyNotes.notes)
        assertTrue("notes" in emptyNotes.arguments)

        assertFailsWith<IllegalArgumentException> {
            ReportKaigiRelayHealthInstruction.fromArguments(
                report.arguments + ("status" to "degraded"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ReportKaigiRelayHealthInstruction.fromArguments(
                report.arguments + ("reported_at_ms" to "01"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ReportKaigiRelayHealthInstruction.fromArguments(report.arguments - "relay_id")
        }
        assertFailsWith<IllegalArgumentException> {
            ReportKaigiRelayHealthInstruction(
                callId = report.callId,
                relayId = report.relayId,
                status = report.status,
                reportedAtMs = report.reportedAtMs,
                notes = "x".repeat(513),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ReportKaigiRelayHealthInstruction(
                callId = report.callId,
                relayId = report.relayId,
                status = report.status,
                reportedAtMs = report.reportedAtMs,
                notes = "\uD800",
            )
        }
    }

    @Test
    fun `unit enum payloads and call identifiers reject malformed state`() {
        assertFailsWith<IllegalArgumentException> {
            KaigiInstructionUtils.CallId("", "sync")
        }
        assertFailsWith<IllegalArgumentException> {
            KaigiInstructionUtils.CallId("wonderland", " ")
        }
        assertFailsWith<IllegalArgumentException> {
            KaigiInstructionUtils.PrivacyMode("Transparent", "unexpected")
        }
        assertFailsWith<IllegalArgumentException> {
            KaigiInstructionUtils.RoomPolicy("Public", "unexpected")
        }

        val create = CreateKaigiInstruction.create(
            KaigiInstructionUtils.CallId("wonderland", "sync"),
            "host",
        )
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("privacy.state" to "unexpected"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("room_policy.state" to "unexpected"),
            )
        }
    }

    @Test
    fun `relay registration requires a nonzero explicit bandwidth class`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterKaigiRelayInstruction("relay-a", key(1), 0)
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterKaigiRelayInstruction("relay-a", "not!base64", 1)
        }
        assertFailsWith<IllegalArgumentException> {
            RecordKaigiUsageInstruction(
                KaigiInstructionUtils.CallId("wonderland", "sync"),
                durationMs = 1,
                proofBase64 = "not!base64",
            )
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
    fun `relay manifests accept eight hops and reject nine in builders and parsers`() {
        val setBuilder = SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "eight-hop-limit")
            .setRelayManifestExpiryMs(100)
        repeat(KaigiInstructionUtils.KAIGI_RELAY_MANIFEST_MAX_HOPS_V1) { index ->
            setBuilder.addRelayManifestHop("relay-$index", key(index + 1), 1)
        }
        val setAtLimit = setBuilder.build()
        assertEquals(
            KaigiInstructionUtils.KAIGI_RELAY_MANIFEST_MAX_HOPS_V1,
            setAtLimit.relayManifest!!.hops.size,
        )
        assertEquals(
            setAtLimit,
            SetKaigiRelayManifestInstruction.fromArguments(setAtLimit.arguments),
        )

        assertFailsWith<IllegalArgumentException> {
            setBuilder.addRelayManifestHop("relay-8", key(9), 1)
        }

        val setWithNineArguments = LinkedHashMap(setAtLimit.arguments).apply {
            this["relay_manifest.hop.8.relay_id"] = "relay-8"
            this["relay_manifest.hop.8.hpke_public_key"] = key(9)
            this["relay_manifest.hop.8.weight"] = "1"
        }
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.fromArguments(setWithNineArguments)
        }

        val createAtLimit = CreateKaigiInstruction.create(
            callId = KaigiInstructionUtils.CallId("wonderland", "eight-hop-limit"),
            host = "host",
            relayManifest = setAtLimit.relayManifest,
        )
        assertEquals(createAtLimit, CreateKaigiInstruction.fromArguments(createAtLimit.arguments))

        val createWithNineArguments = LinkedHashMap(createAtLimit.arguments).apply {
            this["relay_manifest.hop.8.relay_id"] = "relay-8"
            this["relay_manifest.hop.8.hpke_public_key"] = key(9)
            this["relay_manifest.hop.8.weight"] = "1"
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(createWithNineArguments)
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.create(
                callId = KaigiInstructionUtils.CallId("wonderland", "nine-hop-limit"),
                host = "host",
                relayManifest = KaigiInstructionUtils.RelayManifest(
                    100,
                    (0..KaigiInstructionUtils.KAIGI_RELAY_MANIFEST_MAX_HOPS_V1).map { index ->
                        KaigiInstructionUtils.RelayManifestHop(
                            "relay-$index",
                            key(index + 1),
                            1,
                        )
                    },
                ),
            )
        }
    }

    @Test
    fun `relay HPKE keys accept 4096 decoded bytes and reject 4097`() {
        val maxKey = keyWithSize(KaigiInstructionUtils.KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1)
        val oversizedKey = keyWithSize(
            KaigiInstructionUtils.KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1,
        )

        val registration = RegisterKaigiRelayInstruction("relay-a", maxKey, 1)
        assertEquals(
            registration,
            RegisterKaigiRelayInstruction.fromArguments(registration.arguments),
        )
        assertFailsWith<IllegalArgumentException> {
            RegisterKaigiRelayInstruction("relay-a", oversizedKey, 1)
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterKaigiRelayInstruction.fromArguments(
                registration.arguments + ("relay.hpke_public_key" to oversizedKey),
            )
        }

        val manifest = SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "hpke-key-limit")
            .setRelayManifestExpiryMs(100)
            .addRelayManifestHop("relay-a", maxKey, 1)
            .addRelayManifestHop("relay-b", key(2), 1)
            .addRelayManifestHop("relay-c", key(3), 1)
            .build()
        assertEquals(manifest, SetKaigiRelayManifestInstruction.fromArguments(manifest.arguments))
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "hpke-key-limit")
                .setRelayManifestExpiryMs(100)
                .addRelayManifestHop("relay-a", oversizedKey, 1)
        }
        val maxKeyBytes = ByteArray(
            KaigiInstructionUtils.KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1,
        ) { 0xA5.toByte() }
        SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "hpke-key-limit-bytes")
            .setRelayManifestExpiryMs(100)
            .addRelayManifestHop("relay-a", maxKeyBytes, 1)
            .addRelayManifestHop("relay-b", byteArrayOf(2), 1)
            .addRelayManifestHop("relay-c", byteArrayOf(3), 1)
            .build()
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "hpke-key-limit-bytes")
                .setRelayManifestExpiryMs(100)
                .addRelayManifestHop("relay-a", ByteArray(maxKeyBytes.size + 1), 1)
        }
        assertFailsWith<IllegalArgumentException> {
            SetKaigiRelayManifestInstruction.fromArguments(
                manifest.arguments +
                    ("relay_manifest.hop.0.hpke_public_key" to oversizedKey),
            )
        }

        val create = CreateKaigiInstruction.create(
            callId = KaigiInstructionUtils.CallId("wonderland", "hpke-key-limit"),
            host = "host",
            relayManifest = manifest.relayManifest,
        )
        assertEquals(create, CreateKaigiInstruction.fromArguments(create.arguments))
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.create(
                callId = KaigiInstructionUtils.CallId("wonderland", "hpke-key-limit"),
                host = "host",
                relayManifest = KaigiInstructionUtils.RelayManifest(
                    100,
                    listOf(
                        KaigiInstructionUtils.RelayManifestHop("relay-a", oversizedKey, 1),
                        KaigiInstructionUtils.RelayManifestHop("relay-b", key(2), 1),
                        KaigiInstructionUtils.RelayManifestHop("relay-c", key(3), 1),
                    ),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments +
                    ("relay_manifest.hop.0.hpke_public_key" to oversizedKey),
            )
        }
    }

    @Test
    fun `kaigi private actions require complete scalar authorization and contain no clear hints`() {
        val callId = KaigiInstructionUtils.CallId("wonderland", "sync")
        val commitment = KaigiAuthorizationScalarV1.fromLeBytes(ByteArray(32) { 1 })
        val nullifier = KaigiAuthorizationScalarV1.fromLeBytes(ByteArray(32) { 2 })
        val create = CreateKaigiInstruction.create(
            callId, "host", privacyMode = KaigiInstructionUtils.PrivacyMode("ZkRosterV1", null),
            roomPolicy = KaigiInstructionUtils.RoomPolicy("Public", null),
            commitment = commitment, nullifierDigest = nullifier, rosterRoot = hash(3), proofBase64 = key(9),
        )
        val join = JoinKaigiInstruction(callId, "participant", commitment, nullifier, hash(3), key(9))
        val leave = LeaveKaigiInstruction(callId, "participant", commitment, nullifier, hash(3), key(9))
        val end = EndKaigiInstruction(callId, 84, commitment, nullifier, hash(3), key(9))
        for (instruction in listOf(create, join, leave, end)) {
            assertEquals(null, instruction.arguments["commitment.alias_tag"])
            assertEquals(null, instruction.arguments["nullifier.issued_at_ms"])
            assertEquals(commitment.toHex(), instruction.arguments["commitment.commitment"])
            assertEquals(nullifier.toHex(), instruction.arguments["nullifier.digest"])
        }
        assertEquals("Public", create.arguments["room_policy.policy"])
        assertEquals(KaigiInstructionUtils.canonicalizeHash(hash(3)), create.arguments["roster_root"])
        assertEquals(key(9), create.arguments["proof"])
        assertEquals(create, CreateKaigiInstruction.fromArguments(create.arguments))
        assertEquals(join, JoinKaigiInstruction.fromArguments(join.arguments))
        assertEquals(leave, LeaveKaigiInstruction.fromArguments(leave.arguments))
        assertEquals(end, EndKaigiInstruction.fromArguments(end.arguments))
        for (mask in 0..15) {
            val c = commitment.takeIf { mask and 1 != 0 }
            val n = nullifier.takeIf { mask and 2 != 0 }
            val root = hash(3).takeIf { mask and 4 != 0 }
            val proof = key(9).takeIf { mask and 8 != 0 }
            val operations = listOf<() -> KaigiWireInstructionV1>(
                { JoinKaigiInstruction(callId, "participant", c, n, root, proof) },
                { LeaveKaigiInstruction(callId, "participant", c, n, root, proof) },
                { EndKaigiInstruction(callId, null, c, n, root, proof) },
            )
            for (operation in operations) {
                if (mask == 0 || mask == 15) operation() else assertFailsWith<IllegalArgumentException> { operation() }
            }
            if (mask != 15) assertFailsWith<IllegalArgumentException> {
                CreateKaigiInstruction.create(callId, "host", privacyMode = KaigiInstructionUtils.PrivacyMode("ZkRosterV1", null),
                    commitment = c, nullifierDigest = n, rosterRoot = root, proofBase64 = proof)
            }
            if (mask != 0) assertFailsWith<IllegalArgumentException> {
                CreateKaigiInstruction.create(callId, "host", commitment = c, nullifierDigest = n, rosterRoot = root, proofBase64 = proof)
            }
        }
    }

    @Test
    fun `kaigi parsers reject all retired clear hint keys and malformed partial artifacts`() {
        val callId = KaigiInstructionUtils.CallId("wonderland", "sync")
        val instructions = listOf(
            CreateKaigiInstruction.create(callId, "host"), JoinKaigiInstruction(callId, "participant"),
            LeaveKaigiInstruction(callId, "participant"), EndKaigiInstruction(callId),
        )
        val parsers = listOf<(Map<String, String>) -> KaigiWireInstructionV1>(
            CreateKaigiInstruction::fromArguments, JoinKaigiInstruction::fromArguments,
            LeaveKaigiInstruction::fromArguments, EndKaigiInstruction::fromArguments,
        )
        for ((instruction, parser) in instructions.zip(parsers)) {
            for ((key, value) in listOf(
                "commitment.alias_tag" to "host-alias", "commitment.alias_tag" to "",
                "nullifier.issued_at_ms" to "0", "nullifier.issued_at_ms" to "1",
                "commitment.commitment" to "commitment", "nullifier.digest" to "nullifier",
                "roster_root" to "root", "proof" to key(1),
                "nullifier.digest" to "02".repeat(32),
            )) {
                assertFailsWith<IllegalArgumentException> { parser(instruction.arguments + (key to value)) }
            }
        }
    }

    private fun key(value: Int): String = Base64.getEncoder().encodeToString(byteArrayOf(value.toByte()))

    private fun keyWithSize(size: Int): String =
        Base64.getEncoder().encodeToString(ByteArray(size) { 0xA5.toByte() })

    private fun hash(value: Int): String = "%02x".format(value and 0xFF).repeat(32)
}
