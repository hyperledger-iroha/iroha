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
    fun `full width unsigned Kaigi fields round trip through signed JVM carriers`() {
        val callId = KaigiInstructionUtils.CallId("wonderland", "unsigned-boundary")
        val u64Max = -1L
        val u32Max = -1
        val u64MaxText = "18446744073709551615"
        val u32MaxText = "4294967295"

        val create = CreateKaigiInstruction.create(
            callId = callId,
            host = "host",
            maxParticipants = u32Max,
            gasRatePerMinute = u64Max,
            scheduledStartMs = u64Max,
        )
        assertEquals(u32MaxText, create.arguments["max_participants"])
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
        val rawHash = hash(1)
        val shuffled = linkedMapOf(
            "metadata.z" to "last",
            "commitment.commitment" to rawHash,
            "host" to "host",
            "call.call_name" to "sync",
            "call.domain_id" to "wonderland",
            "action" to "CreateKaigi",
            "metadata.a" to "first",
        )
        val parsedCreate = CreateKaigiInstruction.fromArguments(shuffled)
        assertEquals(
            listOf(
                "action",
                "call.domain_id",
                "call.call_name",
                "host",
                "gas_rate_per_minute",
                "metadata.a",
                "metadata.z",
                "privacy.mode",
                "room_policy.policy",
                "commitment.commitment",
            ),
            parsedCreate.arguments.keys.toList(),
        )
        assertEquals(
            KaigiInstructionUtils.canonicalizeHash(rawHash),
            parsedCreate.arguments["commitment.commitment"],
        )
        val lowercaseLiteral = KaigiInstructionUtils.canonicalizeHash(hash(0xAB)).lowercase()
        val parsedLowercaseLiteral = CreateKaigiInstruction.fromArguments(
            shuffled + ("commitment.commitment" to lowercaseLiteral),
        )
        assertEquals(
            KaigiInstructionUtils.canonicalizeHash(hash(0xAB)),
            parsedLowercaseLiteral.arguments["commitment.commitment"],
        )
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
        assertTrue(parsedCreate.arguments["commitment.commitment"]!!.startsWith("hash:"))
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
    fun `kaigi privacy artifacts preserve only ledger safe fields`() {
        val callId = KaigiInstructionUtils.CallId("wonderland", "sync")
        val proof = key(9)
        val commitment = hash(1)
        val nullifier = hash(2)
        val rosterRoot = hash(3)
        val create = CreateKaigiInstruction.create(
            callId = callId,
            host = "host",
            roomPolicy = KaigiInstructionUtils.RoomPolicy("Public", null),
            commitment = commitment,
            nullifierDigest = nullifier,
            nullifierIssuedAtMs = 0,
            rosterRoot = rosterRoot,
            proofBase64 = proof,
        )
        val join = JoinKaigiInstruction(
            callId = callId,
            participant = "participant",
            commitment = commitment,
            nullifierDigest = nullifier,
            nullifierIssuedAtMs = 0,
            rosterRoot = rosterRoot,
            proofBase64 = proof,
        )
        val leave = LeaveKaigiInstruction(
            callId = callId,
            participant = "participant",
        )
        val end = EndKaigiInstruction(
            callId = callId,
            endedAtMs = 84,
            commitment = commitment,
            nullifierDigest = nullifier,
            nullifierIssuedAtMs = 0,
            rosterRoot = rosterRoot,
            proofBase64 = proof,
        )

        for (instruction in listOf(create, join, end)) {
            assertEquals(null, instruction.arguments["commitment.alias_tag"])
            assertEquals("0", instruction.arguments["nullifier.issued_at_ms"])
        }
        for (key in listOf(
            "commitment.commitment",
            "commitment.alias_tag",
            "nullifier.digest",
            "nullifier.issued_at_ms",
            "roster_root",
            "proof",
        )) {
            assertEquals(null, leave.arguments[key])
        }
        assertEquals("Public", create.arguments["room_policy.policy"])
        assertEquals(KaigiInstructionUtils.canonicalizeHash(rosterRoot), create.arguments["roster_root"])
        assertEquals(proof, create.arguments["proof"])
        assertEquals(create, CreateKaigiInstruction.fromArguments(create.arguments))
        assertEquals(join, JoinKaigiInstruction.fromArguments(join.arguments))
        assertEquals(leave, LeaveKaigiInstruction.fromArguments(leave.arguments))
        assertEquals(end, EndKaigiInstruction.fromArguments(end.arguments))

        val createWithoutIssuedAt = CreateKaigiInstruction.create(
            callId = callId,
            host = "host",
            nullifierDigest = nullifier,
        )
        val joinWithoutIssuedAt = JoinKaigiInstruction(
            callId = callId,
            participant = "participant",
            nullifierDigest = nullifier,
        )
        val leaveWithoutIssuedAt = LeaveKaigiInstruction(
            callId = callId,
            participant = "participant",
        )
        val endWithoutIssuedAt = EndKaigiInstruction(
            callId = callId,
            nullifierDigest = nullifier,
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
            LeaveKaigiInstruction(callId, "participant", commitment = "commitment")
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction(callId, "participant", nullifierDigest = "nullifier")
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction(callId, "participant", nullifierIssuedAtMs = 0)
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction(callId, "participant", rosterRoot = "root")
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction(callId, "participant", proofBase64 = key(1))
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
            CreateKaigiInstruction.create(callId, "host", nullifierIssuedAtMs = 0)
        }
        assertFailsWith<IllegalArgumentException> {
            JoinKaigiInstruction(callId, "participant", nullifierIssuedAtMs = 0)
        }
        assertFailsWith<IllegalArgumentException> {
            EndKaigiInstruction(callId, nullifierIssuedAtMs = 0)
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
            LeaveKaigiInstruction.fromArguments(
                leave.arguments + ("commitment.commitment" to "commitment"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction.fromArguments(
                leave.arguments + ("nullifier.issued_at_ms" to "0"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction.fromArguments(
                leave.arguments + ("roster_root" to "root"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            LeaveKaigiInstruction.fromArguments(
                leave.arguments + ("proof" to key(1)),
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
        assertFailsWith<IllegalArgumentException> {
            CreateKaigiInstruction.fromArguments(
                create.arguments + ("nullifier.issued_at_ms" to "0"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            JoinKaigiInstruction.fromArguments(
                join.arguments + ("nullifier.issued_at_ms" to "0"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            EndKaigiInstruction.fromArguments(
                end.arguments + ("nullifier.issued_at_ms" to "0"),
            )
        }
    }

    private fun key(value: Int): String = Base64.getEncoder().encodeToString(byteArrayOf(value.toByte()))

    private fun keyWithSize(size: Int): String =
        Base64.getEncoder().encodeToString(ByteArray(size) { 0xA5.toByte() })

    private fun hash(value: Int): String = "%02x".format(value and 0xFF).repeat(32)
}
