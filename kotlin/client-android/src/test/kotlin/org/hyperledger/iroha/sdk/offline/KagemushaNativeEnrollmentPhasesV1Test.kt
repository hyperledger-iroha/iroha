// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertSame

class KagemushaNativeEnrollmentPhasesV1Test {
    private val account = AccountAddress.fromAccount(
        hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
        "ed25519",
    ).toI105(0)

    @Test
    fun `selection and qualified challenge remain one process-owned native attempt`() {
        val endpoint = Endpoint()
        val native = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/enrollment", endpoint)
        val phases = native.initialEnrollment()
        assertSame(phases, native.initialEnrollment())
        assertFailsWith<IllegalArgumentException> { phases.begin("i105example") }
        assertEquals(0, endpoint.calls)

        val selected = phases.begin(account)
        assertEquals(account, selected.accountI105)
        assertSame(selected, phases.recoverExactSelection(account))
        selected.clientNonce().fill(0)
        assertContentEquals(ByteArray(32) { 1 }, selected.clientNonce())
        assertFailsWith<IllegalStateException> { phases.begin(account) }
        assertEquals(1, endpoint.calls)

        val accepted = accept(phases, selected)
        assertContentEquals(ByteArray(32) { 7 }, accepted.challengeId())
        assertContentEquals(ByteArray(32) { 8 }, accepted.signingMessage())
        assertSame(accepted, accept(phases, selected))
        val calls = endpoint.calls
        assertFailsWith<IllegalStateException> {
            phases.acceptQualifiedChallenge(selected, preparation(selected), byteArrayOf(3), byteArrayOf(9),
                byteArrayOf(5), ByteArray(32) { 7 }, ByteArray(32) { 8 }, ByteArray(32) { 7 },
                byteArrayOf(6), 120_000)
        }
        assertEquals(calls, endpoint.calls)

        val proof = phases.prepareProof(accepted, ByteArray(64) { 11 }, byteArrayOf(12))
        assertContentEquals(byteArrayOf(99), proof.canonicalProof())
        assertSame(proof, phases.recoverExactProof(accepted))
        val id = phases.complete(proof, byteArrayOf(13))
        assertContentEquals(ByteArray(32) { 15 }, id)
        id.fill(0)
        assertContentEquals(ByteArray(32) { 15 }, phases.complete(proof, byteArrayOf(13)))
        phases.cancel(selected)
        assertNull(phases.recoverExactSelection(account))
        assertFailsWith<IllegalStateException> { phases.recoverExactProof(accepted) }
    }

    @Test
    fun `lost phase one response never dispatches a second native selection`() {
        val endpoint = Endpoint().apply { loseBegin = true }
        val phases = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/enrollment", endpoint)
            .initialEnrollment()
        assertFailsWith<IllegalStateException> { phases.begin(account) }
        assertNull(phases.recoverExactSelection(account))
        assertFailsWith<IllegalStateException> { phases.begin(account) }
        assertEquals(1, endpoint.calls)
    }

    @Test
    fun `closing the native owner revokes cached enrollment selection before another phase`() {
        val endpoint = Endpoint()
        val native = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/enrollment", endpoint)
        val phases = native.initialEnrollment()
        val selection = phases.begin(account)
        assertSame(selection, phases.recoverExactSelection(account))
        native.close()
        assertNull(phases.recoverExactSelection(account))
        assertFailsWith<IllegalStateException> { accept(phases, selection) }
        assertEquals(1, endpoint.calls)
    }

    @Test
    fun `lost proof response reads exact retained proof without redispatching device frame`() {
        val endpoint = Endpoint().apply { loseProof = true }
        val phases = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/enrollment", endpoint)
            .initialEnrollment()
        val accepted = accept(phases, phases.begin(account))
        assertFailsWith<IllegalStateException> {
            phases.prepareProof(accepted, ByteArray(64) { 11 }, byteArrayOf(12))
        }
        assertEquals(1, endpoint.prepareCalls)
        assertContentEquals(byteArrayOf(99), phases.recoverExactProof(accepted).canonicalProof())
        assertEquals(1, endpoint.prepareCalls)
    }

    @Test
    fun `foreign selection and changed proof identity stop before issuer completion`() {
        val first = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/first", Endpoint())
            .initialEnrollment()
        val secondEndpoint = Endpoint().apply { wrongProofChallenge = true }
        val second = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/second", secondEndpoint)
            .initialEnrollment()
        val foreign = first.begin(account)
        second.begin(account)
        assertFailsWith<IllegalStateException> { accept(second, foreign) }
        assertEquals(1, secondEndpoint.calls)
        val accepted = accept(second, second.recoverExactSelection(account)!!)
        assertFailsWith<IllegalStateException> {
            second.prepareProof(accepted, ByteArray(64) { 11 }, byteArrayOf(12))
        }
        assertNull(second.recoverExactSelection(account))
        assertFailsWith<IllegalStateException> { second.recoverExactProof(accepted) }
    }

    @Test
    fun `changed retained proof poisons issuer completion before native dispatch`() {
        val endpoint = Endpoint()
        val phases = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/proof-poison", endpoint)
            .initialEnrollment()
        val accepted = accept(phases, phases.begin(account))
        val original = phases.prepareProof(accepted, ByteArray(64) { 11 }, byteArrayOf(12))
        endpoint.wrongProofChallenge = true
        assertFailsWith<IllegalStateException> { phases.recoverExactProof(accepted) }
        val calls = endpoint.calls
        assertFailsWith<IllegalStateException> { phases.complete(original, byteArrayOf(13)) }
        assertEquals(calls, endpoint.calls)
    }

    private fun accept(phases: KagemushaNativeEnrollmentPhasesV1,
        selection: KagemushaNativeEnrollmentPhasesV1.Selection): KagemushaNativeEnrollmentPhasesV1.AcceptedChallenge =
        phases.acceptQualifiedChallenge(selection, preparation(selection), byteArrayOf(3), byteArrayOf(4),
            byteArrayOf(5), ByteArray(32) { 7 }, ByteArray(32) { 8 }, ByteArray(32) { 7 },
            byteArrayOf(6), 120_000)

    private fun preparation(selection: KagemushaNativeEnrollmentPhasesV1.Selection): ByteArray =
        ByteArray(273).also { bytes ->
            bytes[0] = 1
            putU64(bytes, 1, 1_000)
            putU64(bytes, 9, 120_000)
            selection.clientNonce().copyInto(bytes, 17)
            ByteArray(32) { 2 }.copyInto(bytes, 49)
            selection.releaseId().copyInto(bytes, 81)
            selection.profileId().copyInto(bytes, 113)
            selection.laneId().copyInto(bytes, 177)
            bytes.fill(3, 209)
        }

    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        var calls = 0
        var prepareCalls = 0
        var loseBegin = false
        var loseProof = false
        var wrongProofChallenge = false
        private var challengeId = ByteArray(32) { 7 }
        override fun contract() = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 12)
        override fun open(storagePath: String) = 31L
        override fun close(handle: Long) = 0
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            calls++
            assertEquals(31L, handle)
            assertEquals(12, method)
            val ticket = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(17).array()
            return when (ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int) {
                1 -> if (loseBegin) null else arrayOf(ticket, ByteArray(32) { 1 }, ByteArray(32) { 3 },
                    ByteArray(32) { 4 }, ByteArray(32) { 5 })
                2 -> {
                    challengeId = fields[6].copyOf()
                    arrayOf(fields[1], fields[7], fields[8], fields[9])
                }
                3 -> {
                    prepareCalls++
                    if (loseProof) null else proof(fields[1])
                }
                4 -> proof(fields[1])
                5 -> arrayOf(fields[1], ByteArray(32) { 15 })
                6 -> emptyArray()
                else -> error("Unexpected enrollment phase")
            }
        }

        private fun proof(ticket: ByteArray) = arrayOf(ticket,
            if (wrongProofChallenge) ByteArray(32) { 22 } else challengeId, byteArrayOf(99))
    }

    private fun putU64(bytes: ByteArray, offset: Int, value: Long) {
        ByteBuffer.wrap(bytes, offset, 8).order(ByteOrder.LITTLE_ENDIAN).putLong(value)
    }

    private fun hex(value: String): ByteArray = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
