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
    fun `coordinator methods agree with the shared current schema vectors`() {
        val cases = fixtures()
        assertEquals((1..13).toSet(), cases.map { it.method.code }.toSet())
        assertEquals(20, cases.size)
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
                case.request.copyOf().apply { this[10] = 17 },
                case.request.copyOf().apply { fill(-1, 16, 20) },
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
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, fields.dropLast(1)) }
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, fields + listOf(byteArrayOf(1))) }
            val response = KagemushaCoreCoordinatorFrameV1.decodeResponse(case.method, case.request, case.response)
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeResponse(case.method, case.request, response + listOf(byteArrayOf(1))) }
        }
    }

    @Test
    fun `response identity or envelope substitution fails for every correlated method`() {
        val indexes = mapOf("reserve" to 0, "begin-send" to 0, "begin-redeem" to 0,
            "installed-terminal" to 0, "recover-sender" to 0, "recover-terminal" to 1,
            "release-send" to 3, "release-redeem" to 3, "app-attest-ack" to 0)
        fixtures().filter { it.name in indexes }.forEach { case ->
            val fields = KagemushaCoreCoordinatorFrameV1.decodeResponse(case.method, case.request, case.response)
            fields[indexes.getValue(case.name)][0] = 0x7f
            assertFailsWith<IllegalArgumentException>(case.name) {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(case.method, case.request, fields)
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
            ByteArray(32) { 0x46 }, ByteArray(32) { 0x47 })
        val responseFrame = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin, response)
        val decoded = KagemushaCoreCoordinatorFrameV1.decodeResponse(method, begin, responseFrame)
        decoded.zip(response).forEach { (left, right) -> assertContentEquals(right, left) }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, begin,
                listOf(ticket, response[1], response[2], response[3], byteArrayOf(0x45)))
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
    fun `App Attest commit acknowledgment binds every original byte and exact next counter`() {
        val method = KagemushaCoreCoordinatorMethodV1.ACKNOWLEDGE_COMMITTED_APP_ATTEST
        val domain = "iroha:kagemusha:v1:hardware-transition-selection\u0000".toByteArray(Charsets.US_ASCII)
        val selection = domain + ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(403).array() +
            ByteArray(403) { 0x42 }
        val request = listOf(ByteArray(32) { 0x11 }, "app-attest-key".toByteArray(Charsets.UTF_8),
            selection, byteArrayOf(0xa2.toByte(), 1, 2), KagemushaCoreCoordinatorFrameV1.u32(4),
            ByteArray(32) { 0x33 }, ByteArray(32) { 0x44 })
        val encoded = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request)
        val response = listOf(request[0], MessageDigest.getInstance("SHA-256").digest(request[1]),
            MessageDigest.getInstance("SHA-256").digest(request[2]),
            MessageDigest.getInstance("SHA-256").digest(request[3]),
            KagemushaCoreCoordinatorFrameV1.u32(5), request[5], request[6])
        val reply = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, encoded, response)
        KagemushaCoreCoordinatorFrameV1.decodeResponse(method, encoded, reply)
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
