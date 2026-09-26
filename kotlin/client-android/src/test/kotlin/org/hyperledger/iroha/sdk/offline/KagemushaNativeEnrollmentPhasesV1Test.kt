// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.junit.jupiter.api.Tag
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertSame

/** Scripted enrollment phases require the ABI-24 host validator for the universal account ID. */
@Tag("host-native")
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
        assertContentEquals(ByteArray(32) { 6 }, selected.ownerScope())
        assertEquals(120_007L, selected.nativeDeadlineContinuousMS())
        selected.clientNonce().fill(0)
        assertContentEquals(ByteArray(32) { 1 }, selected.clientNonce())
        assertFailsWith<IllegalStateException> { phases.begin(account) }
        assertEquals(2, endpoint.calls)
        assertEquals(1, endpoint.readSelectionCalls)

        val accepted = accept(phases, selected)
        assertContentEquals(ByteArray(32) { 7 }, accepted.challengeId())
        assertContentEquals(ByteArray(32) { 8 }, accepted.signingMessage())
        assertSame(accepted, accept(phases, selected))
        val calls = endpoint.calls
        assertFailsWith<IllegalStateException> {
            phases.acceptQualifiedChallenge(selected, preparation(selected), byteArrayOf(3), byteArrayOf(9),
                byteArrayOf(5), ByteArray(32) { 7 }, ByteArray(32) { 8 }, ByteArray(32) { 7 },
                byteArrayOf(6), 121_000)
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
    fun `lost phase one response reads the exact native selection without dispatching another selection`() {
        val endpoint = Endpoint().apply { loseBegin = true }
        val phases = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/enrollment", endpoint)
            .initialEnrollment()
        assertFailsWith<IllegalStateException> { phases.begin(account) }
        val selected = phases.recoverExactSelection(account)!!
        assertSame(selected, phases.recoverExactSelection(account))
        assertContentEquals(ByteArray(32) { 1 }, selected.clientNonce())
        assertFailsWith<IllegalStateException> { phases.begin(account) }
        assertEquals(3, endpoint.calls)
        assertEquals(1, endpoint.selectionCalls)
        assertEquals(2, endpoint.readSelectionCalls)
    }

    @Test
    fun `android preparation shape rejects wrong lifetime and preselected key before dispatch`() {
        val endpoint = Endpoint()
        val phases = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/preparation", endpoint)
            .initialEnrollment()
        val selected = phases.begin(account)
        val shortLived = preparation(selected).also { putU64(it, 1, 1_001) }
        assertFailsWith<IllegalArgumentException> {
            phases.acceptQualifiedChallenge(selected, shortLived, byteArrayOf(3), byteArrayOf(4),
                byteArrayOf(5), ByteArray(32) { 7 }, ByteArray(32) { 8 }, ByteArray(32) { 7 },
                byteArrayOf(6), 121_000)
        }
        val preselectedKey = preparation(selected).also { it[145] = 1 }
        assertFailsWith<IllegalArgumentException> {
            phases.acceptQualifiedChallenge(selected, preselectedKey, byteArrayOf(3), byteArrayOf(4),
                byteArrayOf(5), ByteArray(32) { 7 }, ByteArray(32) { 8 }, ByteArray(32) { 7 },
                byteArrayOf(6), 121_000)
        }
        assertEquals(1, endpoint.calls)
        accept(phases, selected)
        assertEquals(2, endpoint.calls)
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
        assertEquals(2, endpoint.calls)
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
    fun `cached selection is rechecked natively and every changed recovered field poisons the owner`() {
        for (changedField in 0..6) {
            val endpoint = Endpoint()
            val phases = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint(
                "/durable/recheck-$changedField", endpoint,
            ).initialEnrollment()
            val selected = phases.begin(account)
            endpoint.changedSelectionField = changedField
            assertFailsWith<IllegalStateException> { phases.recoverExactSelection(account) }
            assertEquals(1, endpoint.readSelectionCalls)
            assertNull(phases.recoverExactSelection(account))
            assertFailsWith<IllegalStateException> { accept(phases, selected) }
            phases.cancel(selected)
        }
    }

    @Test
    fun `cached selection does not survive a rejected native liveness read`() {
        val endpoint = Endpoint()
        val phases = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint(
            "/durable/revoked-selection", endpoint,
        ).initialEnrollment()
        phases.begin(account)
        endpoint.rejectSelectionRead = true
        assertFailsWith<IllegalStateException> { phases.recoverExactSelection(account) }
        assertEquals(1, endpoint.readSelectionCalls)
    }

    @Test
    fun `lost cancellation response permits only the original ticket retry`() {
        val endpoint = Endpoint().apply { loseCancel = true }
        val native = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/cancel", endpoint)
        val phases = native.initialEnrollment()
        val selected = phases.begin(account)
        assertFailsWith<IllegalStateException> { phases.cancel(selected) }
        assertEquals(1, endpoint.cancelCalls)
        assertNull(phases.recoverExactSelection(account))
        assertFailsWith<IllegalStateException> { accept(phases, selected) }
        assertFailsWith<IllegalStateException> { phases.begin(account) }
        val foreign = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint(
            "/durable/foreign-cancel", Endpoint(),
        ).initialEnrollment().begin(account)
        assertFailsWith<IllegalStateException> { phases.cancel(foreign) }
        assertEquals(1, endpoint.cancelCalls)

        endpoint.loseCancel = false
        phases.cancel(selected)
        assertEquals(2, endpoint.cancelCalls)
        native.close()
        assertFailsWith<IllegalStateException> { phases.cancel(selected) }
        assertEquals(2, endpoint.cancelCalls)
    }

    @Test
    fun `poisoned proof reply still permits original ticket cancellation and exact retry`() {
        val endpoint = Endpoint().apply {
            wrongProofChallenge = true
            loseCancel = true
        }
        val native = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/poison-cancel", endpoint)
        val phases = native.initialEnrollment()
        val selected = phases.begin(account)
        val accepted = accept(phases, selected)
        assertFailsWith<IllegalStateException> {
            phases.prepareProof(accepted, ByteArray(64) { 11 }, byteArrayOf(12))
        }
        assertNull(phases.recoverExactSelection(account))
        assertFailsWith<IllegalStateException> { phases.cancel(selected) }
        assertEquals(1, endpoint.cancelCalls)
        endpoint.loseCancel = false
        phases.cancel(selected)
        assertEquals(2, endpoint.cancelCalls)
        assertFailsWith<IllegalStateException> { phases.prepareProof(accepted, ByteArray(64) { 11 }, byteArrayOf(12)) }
        native.close()
        assertFailsWith<IllegalStateException> { phases.cancel(selected) }
        assertEquals(2, endpoint.cancelCalls)
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
            byteArrayOf(6), 121_000)

    private fun preparation(selection: KagemushaNativeEnrollmentPhasesV1.Selection): ByteArray =
        ByteArray(273).also { bytes ->
            bytes[0] = 1
            putU64(bytes, 1, 1_000)
            putU64(bytes, 9, 121_000)
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
        var loseCancel = false
        var wrongProofChallenge = false
        var cancelCalls = 0
        var selectionCalls = 0
        var readSelectionCalls = 0
        var changedSelectionField: Int? = null
        var rejectSelectionRead = false
        private var challengeId = ByteArray(32) { 7 }
        override fun contract() = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 14)
        override fun open(storagePath: String) = 31L
        override fun close(handle: Long) = 0
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            calls++
            assertEquals(31L, handle)
            assertEquals(12, method)
            val ticket = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(17).array()
            return when (ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int) {
                1 -> {
                    selectionCalls++
                    if (loseBegin) null else selection(ticket)
                }
                7 -> {
                    readSelectionCalls++
                    if (rejectSelectionRead) null else selection(ticket).also { response ->
                        changedSelectionField?.let { response[it][0] = (response[it][0].toInt() xor 1).toByte() }
                    }
                }
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
                6 -> {
                    cancelCalls++
                    if (loseCancel) null else emptyArray()
                }
                else -> error("Unexpected enrollment phase")
            }
        }

        private fun proof(ticket: ByteArray) = arrayOf(ticket,
            if (wrongProofChallenge) ByteArray(32) { 22 } else challengeId, byteArrayOf(99))

        private fun selection(ticket: ByteArray) = arrayOf(ticket, ByteArray(32) { 1 },
            ByteArray(32) { 3 }, ByteArray(32) { 4 }, ByteArray(32) { 5 }, ByteArray(32) { 6 },
            ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(120_007).array())
    }

    private fun putU64(bytes: ByteArray, offset: Int, value: Long) {
        ByteBuffer.wrap(bytes, offset, 8).order(ByteOrder.LITTLE_ENDIAN).putLong(value)
    }

    private fun hex(value: String): ByteArray = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
