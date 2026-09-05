// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KagemushaCoreCoordinatorFrameV1Test {
    @Test
    fun `all native methods agree with the shared current schema vectors`() {
        val cases = fixtures()
        assertEquals((1..10).toSet(), cases.map { it.method.code }.toSet())
        assertEquals(14, cases.size)
        cases.forEach { case ->
            val request = KagemushaCoreCoordinatorFrameV1.decodeRequest(case.method, case.request)
            val response = KagemushaCoreCoordinatorFrameV1.decodeResponse(case.method, case.request, case.response)
            assertContentEquals(case.request, KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, request), case.name)
            assertContentEquals(case.response, KagemushaCoreCoordinatorFrameV1.encodeResponse(case.method, case.request, response), case.name)
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
            "release-send" to 3, "release-redeem" to 3)
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
            KagemushaCoreCoordinatorFrameV1.encodeRequest(case.method, valid.take(2) + listOf(ByteArray(65537)))
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
