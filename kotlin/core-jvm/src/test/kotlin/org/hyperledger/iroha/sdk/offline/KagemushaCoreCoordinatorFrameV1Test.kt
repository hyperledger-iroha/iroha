// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KagemushaCoreCoordinatorFrameV1Test {
    @Test
    fun `release acceptance requires its exact original frame and retires ten fields`() {
        // Public framing specimen; this is not hardware or monetary qualification.
        val id = ByteArray(32) { 7 }
        val signature = ByteArray(64).also { it[31] = 1; it[63] = 1 }
        val reply = byteArrayOf(0x12)
        fun u32(value: Int) = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()
        val original = ByteBuffer.allocate(116 + reply.size + signature.size).order(ByteOrder.LITTLE_ENDIAN)
            .put("IKGMJRS1".toByteArray(Charsets.US_ASCII)).putShort(1).put(12).put(0).put(id)
            .putInt(reply.size).putInt(signature.size).put(MessageDigest.getInstance("SHA-256").digest(reply))
            .put(MessageDigest.getInstance("SHA-256").digest(signature)).put(reply).put(signature).array()
        val retired = listOf(u32(12), id, byteArrayOf(1), reply, signature,
            u32(1), id, byteArrayOf(3), byteArrayOf(4), u32(0xffff))
        val method = KagemushaCoreCoordinatorMethodV1.ACCEPT_AUTHENTICATED_REPLY
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, retired) }
        val fields = retired + listOf(original)
        val frame = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields)
        assertContentEquals(original, KagemushaCoreCoordinatorFrameV1.decodeRequest(method, frame)[10])
        listOf(1, 3, 4, 10).forEach { index ->
            val changed = fields.map { it.copyOf() }.toMutableList()
            changed[index] = if (index == 10) changed[index].copyOf(changed[index].size - 1)
                else changed[index].also { it[0] = (it[0].toInt() xor 1).toByte() }
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, changed) }
        }
    }

    @Test
    fun `coordinator methods agree with the shared current schema vectors`() {
        val cases = fixtures()
        assertEquals((1..21).toSet(), cases.map { it.method.code }.toSet())
        assertEquals(28, cases.size)
        cases.forEach { case ->
            val request = KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, case.request)
            val response = KagemushaCoreCoordinatorFrameV1.decodeResponse(case.method, case.request, case.response)
            assertContentEquals(case.request, KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, request), case.name)
            assertContentEquals(case.response, KagemushaCoreCoordinatorFrameV1.encodeResponse(case.method, case.request, response), case.name)
        }
    }

    @Test fun `native observation fixtures match actual canonical Kotlin read commands`() {
        val commands = mapOf(
            "observation-credential" to KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential,
            "observation-time" to KagemushaDeviceControlCommandV1.ReadTrustedTimeOrLease,
            "observation-watermark" to KagemushaDeviceControlCommandV1.ReadPendingCreditWatermark(null, KagemushaPendingCreditTargetV1.DrainAll),
            "observation-wallet" to KagemushaDeviceControlCommandV1.RecoverWalletSnapshot,
        )
        for (fixture in fixtures().filter { it.name in commands }) {
            val command = commands.getValue(fixture.name)
            val canonical = KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
            assertContentEquals(fixture.request, KagemushaCoreCoordinatorFrameV1.encodeRequest(
                KagemushaCoreCoordinatorMethodV1.BEGIN_OBSERVATION,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(command.operation), canonical)))
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeRequest(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID,
                    listOf(KagemushaCoreCoordinatorFrameV1.u32(command.operation), ByteArray(32) { 1 }, canonical))
            }
        }
    }

    @Test
    fun `truncation trailing bytes retired schemas and invalid lengths fail closed`() {
        fixtures().forEach { case ->
            for (size in case.request.indices) {
                assertFailsWith<IllegalArgumentException>(case.name) {
                    KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, case.request.copyOf(size))
                }
            }
            listOf(
                case.request + byteArrayOf(0),
                case.request.copyOf().apply { this[8] = 1 },
                case.request.copyOf().apply { this[12] = 1 },
                case.request.copyOf().apply { this[10] = 19 },
                case.request.copyOf().apply {
                    if (size >= 20) fill(-1, 16, 20) else this[10] = 1
                },
            ).forEach { malformed ->
                assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, malformed) }
            }
            for (size in case.response.indices) {
                assertFailsWith<IllegalArgumentException>(case.name) {
                    KagemushaCoreCoordinatorFrameV1.decodeResponse(case.method, case.request, case.response.copyOf(size))
                }
            }
        }
    }

    @Test
    fun `every closed field inventory rejects extra or missing fields`() {
        fixtures().forEach { case ->
            val fields = KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, case.request)
            if (fields.isNotEmpty()) assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, fields.dropLast(1)) }
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, fields + listOf(byteArrayOf(1))) }
            val response = KagemushaCoreCoordinatorFrameV1.decodeResponse(case.method, case.request, case.response)
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeResponse(case.method, case.request, response + listOf(byteArrayOf(1))) }
        }
    }

    @Test
    fun `response identity or envelope substitution fails for every correlated method`() {
        val indexes = mapOf("reserve" to 0, "begin-send" to 0, "begin-redeem" to 0,
            "installed-terminal" to 0, "recover-sender" to 0, "recover-terminal" to 1,
            "release-send" to 3, "release-redeem" to 3, "app-attest-ack" to 0,
            "outgoing-state-proof-export" to 0)
        fixtures().forEach { case ->
            val index = when (case.method) {
                KagemushaCoreCoordinatorMethodV1.PREPARE_INCOMING_FOLD -> 1
                KagemushaCoreCoordinatorMethodV1.COMPLETE_INCOMING_FOLD,
                KagemushaCoreCoordinatorMethodV1.STAGE_INCOMING_ORIGINAL -> 0
                else -> indexes[case.name] ?: return@forEach
            }
            val fields = KagemushaCoreCoordinatorFrameV1.decodeResponse(case.method, case.request, case.response)
            fields[index][0] = (fields[index][0].toInt() xor 1).toByte()
            assertFailsWith<IllegalArgumentException>(case.name) {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(case.method, case.request, fields)
            }
        }
    }

    @Test
    fun `outgoing State archive export requires an original operation and bounded pair`() {
        val method = KagemushaCoreCoordinatorMethodV1.EXPORT_OUTGOING_STATE_PROOF
        val operation = ByteArray(32) { 0x66 }
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(operation))
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(ByteArray(32)))
        }
        val pair = listOf(operation, byteArrayOf(1), byteArrayOf(2))
        KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, pair)
        for (invalid in listOf(
            listOf(ByteArray(32) { 0x67 }, pair[1], pair[2]),
            listOf(operation, ByteArray(4 * 1024 + 1), pair[2]),
            listOf(operation, pair[1], ByteArray(6_529)),
            listOf(operation, byteArrayOf(), pair[2]),
        )) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, invalid)
            }
        }
    }

    @Test
    fun `bounds and detached output copies preserve untrusted input isolation`() {
        val case = fixtures().first()
        val fields = KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, case.request)
        val encoded = KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, fields)
        fields[1].fill(0)
        assertContentEquals(case.request, encoded)
        val valid = KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, case.request)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method,
                valid.take(2) + listOf(ByteArray(KagemushaCoreCoordinatorFrameV1.MAXIMUM_FIELD_BYTES + 1)))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, ByteArray(262145))
        }
        val install = fixtures().single { it.name == "installed-terminal" }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(install.method, List(5) { ByteArray(65536) })
        }
        val prove = fixtures().single { it.name == "prove" }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.decodeResponse(prove.method, prove.request, ByteArray(131073))
        }
    }

    @Test
    fun `initial enrollment bounds full device response and correlates native ticket`() {
        val method = KagemushaCoreCoordinatorMethodV1.INITIAL_ENROLLMENT
        val ticket = KagemushaCoreCoordinatorFrameV1.u32(7) + KagemushaCoreCoordinatorFrameV1.u32(0)
        val begin = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(1), "i105example".toByteArray(Charsets.UTF_8)))
        val response = listOf(ticket, ByteArray(32) { 0x44 }, ByteArray(32) { 0x45 },
            ByteArray(32) { 0x46 }, ByteArray(32) { 0x47 }, ByteArray(32) { 0x48 },
            java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(120_007).array())
        val responseFrame = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin, response)
        val decoded = KagemushaCoreCoordinatorFrameV1.decodeResponse(method, begin, responseFrame)
        decoded.zip(response).forEach { (left, right) -> assertContentEquals(right, left) }
        val readSelection = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(7), "i105example".toByteArray(Charsets.UTF_8)))
        val retained = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, readSelection, response)
        KagemushaCoreCoordinatorFrameV1.decodeResponse(method, readSelection, retained)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin,
                listOf(ticket, response[1], response[2], response[3], byteArrayOf(0x45)))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin, response.take(5))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin,
                response.dropLast(1) + listOf(ByteArray(8)))
        }
        val challenge = listOf(KagemushaCoreCoordinatorFrameV1.u32(2), ticket,
            ByteArray(273) { 0x51 }, byteArrayOf(0x52), byteArrayOf(0x53), byteArrayOf(0x54),
            ByteArray(32) { 0x55 }, ByteArray(32) { 0x56 }, ByteArray(32) { 0x57 },
            byteArrayOf(0x58), ticket)
        KagemushaCoreCoordinatorFrameV1.encodeRequest(method, challenge)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
                challenge.take(2) + listOf(ByteArray(272) { 0x51 }) + challenge.drop(3))
        }
        val prepare = listOf(KagemushaCoreCoordinatorFrameV1.u32(3), ticket,
            ByteArray(64) { 0x46 }, ByteArray(65_716) { 0x47 })
        KagemushaCoreCoordinatorFrameV1.encodeRequest(method, prepare)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
                prepare.dropLast(1) + listOf(ByteArray(65_717) { 0x47 }))
        }
        val read = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(4), ticket))
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, read,
                listOf(ByteArray(8), ByteArray(32) { 1 }, byteArrayOf(1)))
        }
    }

    @Test
    fun `recovered enrollment preserves exact attempt and bounded original challenge and signatures`() {
        val method = KagemushaCoreCoordinatorMethodV1.INITIAL_ENROLLMENT
        val ticket = KagemushaCoreCoordinatorFrameV1.u32(7) + KagemushaCoreCoordinatorFrameV1.u32(0)
        val begin = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(9)))
        val reply = listOf(ticket, ByteArray(16 * 1024) { 1 }, ByteArray(32) { 2 },
            ByteArray(2 * 1024) { 3 }, ByteArray(32) { 4 })
        KagemushaCoreCoordinatorFrameV1.decodeResponse(method, begin,
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin, reply))
        for ((index, changed) in listOf(0 to ByteArray(8), 1 to ByteArray(16 * 1024 + 1),
            2 to ByteArray(31), 3 to ByteArray(2 * 1024 + 1), 4 to ByteArray(31))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin,
                    reply.mapIndexed { i, original -> if (i == index) changed else original })
            }
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(9), ticket))
        }
        val proof = listOf(KagemushaCoreCoordinatorFrameV1.u32(10), ticket,
            ByteArray(64) { 5 }, ByteArray(65_716) { 6 })
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, proof)
        KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request,
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, listOf(ticket)))
        for ((index, changed) in listOf(1 to ByteArray(8), 2 to ByteArray(63), 3 to ByteArray(65_717))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
                    proof.mapIndexed { i, original -> if (i == index) changed else original })
            }
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(8) + KagemushaCoreCoordinatorFrameV1.u32(0)))
        }
        val cancel = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(11), ticket))
        KagemushaCoreCoordinatorFrameV1.decodeResponse(method, cancel,
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, cancel, emptyList()))
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, cancel, listOf(ticket))
        }
    }

    @Test
    fun `App Attest commit acknowledgment binds every original byte and the actual monotonic counter`() {
        val method = KagemushaCoreCoordinatorMethodV1.ACKNOWLEDGE_COMMITTED_APP_ATTEST
        val domain = "iroha:kagemusha:v1:hardware-transition-selection\u0000".toByteArray(Charsets.US_ASCII)
        val selection = domain + ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(403).array() +
            ByteArray(403) { 0x42 }
        selection[57] = 1
        selection[58] = 0
        selection.fill(0, 283, 291)
        selection[283] = 1
        selection.fill(0, 323, 331)
        selection[323] = 1
        selection[331] = 2
        selection.fill(0, 428, 460)
        selection[428] = 70
        selection[444] = 71
        val request = listOf(ByteArray(32) { 0x11 }, "app-attest-key".toByteArray(Charsets.UTF_8),
            selection, originalAppAttestAssertion(11u), KagemushaCoreCoordinatorFrameV1.u32(4),
            ByteArray(32) { 0x33 }, ByteArray(32) { 0x44 })
        // The wallet's u128 index may be far beyond the unrelated signed u32 hardware counter.
        val wideIndex = request.map { it.copyOf() }.toMutableList()
        wideIndex[2][443] = 1
        wideIndex[2][459] = 1
        KagemushaCoreCoordinatorFrameV1.encodeRequest(method, wideIndex)
        val financialOverflow = request.map { it.copyOf() }.toMutableList()
        financialOverflow[2].fill(0xff.toByte(), 428, 444)
        financialOverflow[2].fill(0, 444, 460)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, financialOverflow)
        }
        val skippedIndex = request.map { it.copyOf() }.toMutableList()
        skippedIndex[2][444] = 72
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, skippedIndex)
        }
        val enrollment = request.map { it.copyOf() }.toMutableList()
        enrollment[2][331] = 0
        enrollment[2].fill(0, 364, 396)
        enrollment[2].fill(0, 396, 428)
        enrollment[2].fill(0, 428, 460)
        KagemushaCoreCoordinatorFrameV1.encodeRequest(method, enrollment)
        enrollment[2][428] = 1
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, enrollment)
        }
        val encoded = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request)
        val response = listOf(request[0], MessageDigest.getInstance("SHA-256").digest(request[1]),
            MessageDigest.getInstance("SHA-256").digest(request[2]),
            MessageDigest.getInstance("SHA-256").digest(request[3]),
            KagemushaCoreCoordinatorFrameV1.u32(11), request[5], request[6])
        val reply = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, encoded, response)
        KagemushaCoreCoordinatorFrameV1.decodeResponse(method, encoded, reply)
        val derivedCounter = response.map { it.copyOf() }.toMutableList()
        derivedCounter[4] = KagemushaCoreCoordinatorFrameV1.u32(5)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, encoded, derivedCounter)
        }
        val assertion = originalAppAttestAssertion(11u)
        val unknownKey = assertion.copyOf().also { it[2] = 'x'.code.toByte() }
        val noncanonicalMap = byteArrayOf(0xb8.toByte(), 2) + assertion.copyOfRange(1, assertion.size)
        val indefiniteMap = assertion.copyOf().also { it[0] = 0xbf.toByte() }
        val duplicateKey = byteArrayOf(0xa2.toByte()) + assertion.copyOfRange(1, 58) + assertion.copyOfRange(1, 58)
        val malformedUtf8 = assertion.copyOf().also { it[2] = 0xff.toByte() }
        val wrongAuthenticatorType = assertion.copyOf().also { it[19] = 0x78 }
        val noncanonicalAuthenticatorLength = assertion.copyOfRange(0, 19) +
            byteArrayOf(0x59, 0, 37) + assertion.copyOfRange(21, assertion.size)
        for (malformed in listOf(byteArrayOf(0xa2.toByte(), 1, 2), assertion + byteArrayOf(0),
                assertion.copyOf(assertion.size - 1), unknownKey, noncanonicalMap, indefiniteMap, duplicateKey,
                malformedUtf8, wrongAuthenticatorType, noncanonicalAuthenticatorLength,
                originalAppAttestAssertion(4u), originalAppAttestAssertion(3u))) {
            val invalid = request.map { it.copyOf() }.toMutableList()
            invalid[3] = malformed
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeRequest(method, invalid)
            }
        }

        for (offset in listOf(57, 331, 428, 444)) {
            val changed = request.map { it.copyOf() }.toMutableList()
            changed[2][offset] = (changed[2][offset].toInt() xor 1).toByte()
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeRequest(method, changed)
            }
        }
        val otherCounter = request.map { it.copyOf() }.toMutableList()
        otherCounter[4] = KagemushaCoreCoordinatorFrameV1.u32(11)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, otherCounter)
        }
        response.indices.forEach { index ->
            val changed = response.map { it.copyOf() }
            changed[index][0] = (changed[index][0].toInt() xor 1).toByte()
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(method, encoded, changed)
            }
        }
        request.indices.forEach { index ->
            val changed = request.map { it.copyOf() }.toMutableList()
            changed[index] = ByteArray(0)
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeRequest(method, changed)
            }
        }
        val exhausted = request.map { it.copyOf() }.toMutableList()
        exhausted[4] = KagemushaCoreCoordinatorFrameV1.u32(-1)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, exhausted)
        }
    }

    /** Canonical original-byte shape only; no real App Attest signature or hardware authority. */
    private fun originalAppAttestAssertion(counter: UInt): ByteArray =
        byteArrayOf(0xa2.toByte(), 0x71) + "authenticatorData".toByteArray(Charsets.US_ASCII) +
            byteArrayOf(0x58, 37) + ByteArray(32) { 0x42 } + byteArrayOf(0x40) +
            ByteBuffer.allocate(4).order(ByteOrder.BIG_ENDIAN).putInt(counter.toInt()).array() +
            byteArrayOf(0x69) + "signature".toByteArray(Charsets.US_ASCII) +
            byteArrayOf(0x48, 0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01)

    private class Fixture(val name: String, val method: KagemushaCoreCoordinatorMethodV1, val request: ByteArray, val response: ByteArray)

    private fun fixtures(): List<Fixture> {
        var directory = Paths.get("").toAbsolutePath().normalize()
        while (directory != null) {
            val path = directory.resolve("fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv")
            if (Files.isRegularFile(path)) return Files.readAllLines(path, Charsets.UTF_8)
                .filter { !it.startsWith("#") && it.isNotEmpty() }.map { line ->
                    val columns = line.split('\t')
                    require(columns.size == 4)
                    Fixture(columns[0], KagemushaCoreCoordinatorMethodV1.values().single { it.code == columns[1].toInt() }, hex(columns[2]), hex(columns[3]))
                }
            directory = directory.parent
        }
        error("missing coordinator frame fixture")
    }

    private fun hex(value: String): ByteArray = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
