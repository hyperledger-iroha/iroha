package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicReference
import kotlin.concurrent.thread
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class IrohaPeerNfcAidSelectionV1Test {
    @Test
    fun `reader selects the exact registered AID before protocol APDUs`() {
        val command = IrohaPeerNfcAidSelectionV1.command()
        assertContentEquals(
            byteArrayOf(0x00, 0xa4.toByte(), 0x04, 0x00, 0x0b) +
                IrohaPeerNfcV1.applicationIdentifier() + byteArrayOf(0x00),
            command,
        )
        assertTrue(IrohaPeerNfcAidSelectionV1.isSelect(command))
        assertTrue(IrohaPeerNfcAidSelectionV1.accepts(command))
        assertTrue(IrohaPeerNfcAidSelectionV1.accepts(command.copyOf(command.size - 1)))
        assertFalse(IrohaPeerNfcAidSelectionV1.isSelect(IrohaPeerNfcAPDUCodecV1.encode(IrohaPeerNfcCommandV1.GET_INFO)))
    }

    @Test
    fun `wrong or malformed AID never selects the receiver`() {
        val command = IrohaPeerNfcAidSelectionV1.command()
        assertFalse(IrohaPeerNfcAidSelectionV1.accepts(command.copyOf(command.size - 2)))
        assertFalse(IrohaPeerNfcAidSelectionV1.accepts(command + byteArrayOf(0x00)))
        assertFalse(IrohaPeerNfcAidSelectionV1.accepts(command.copyOf().apply { this[5] = 0x01 }))
        assertFalse(IrohaPeerNfcAidSelectionV1.accepts(command.copyOf().apply { this[2] = 0x00 }))
        assertFalse(IrohaPeerNfcAidSelectionV1.accepts(command.copyOf().apply { this[4] = 0x0a }))
    }

    @Test
    fun `selection success requires an exact status response`() {
        assertTrue(IrohaPeerNfcAidSelectionV1.succeeded(byteArrayOf(0x90.toByte(), 0x00)))
        assertFalse(IrohaPeerNfcAidSelectionV1.succeeded(byteArrayOf(0x6a, 0x82.toByte())))
        assertFalse(IrohaPeerNfcAidSelectionV1.succeeded(byteArrayOf(0x01, 0x90.toByte(), 0x00)))
    }

    @Test
    fun `reselect and deactivation invalidate an old asynchronous reply`() {
        val state = IrohaPeerNfcApduSelectionStateV1()
        val select = IrohaPeerNfcAidSelectionV1.command()
        assertNull(state.currentEpoch())
        assertTrue(state.select(select))
        val first = requireNotNull(state.currentEpoch())
        assertTrue(state.isCurrent(first))
        assertTrue(state.select(select))
        val second = requireNotNull(state.currentEpoch())
        assertFalse(state.isCurrent(first))
        assertTrue(state.isCurrent(second))
        state.deactivate()
        assertFalse(state.isCurrent(second))
        assertNull(state.currentEpoch())
        assertTrue(state.select(select))
        val third = requireNotNull(state.currentEpoch())
        assertFalse(state.select(select.copyOf().apply { this[5] = 0x01 }))
        assertFalse(state.isCurrent(third))
    }

    @Test
    fun `reply before return is direct and never posted twice`() {
        val posted = mutableListOf<ByteArray>()
        val gate = IrohaPeerNfcApduReplyGateV1 { posted.add(it) }
        val answer = byteArrayOf(0x90.toByte(), 0x00)
        gate.deliver(answer)
        gate.deliver(byteArrayOf(0x65, 0x81.toByte()))
        assertContentEquals(answer, gate.finish())
        assertTrue(posted.isEmpty())
    }

    @Test
    fun `reply after return is posted exactly once`() {
        val posted = mutableListOf<ByteArray>()
        val gate = IrohaPeerNfcApduReplyGateV1 { posted.add(it) }
        assertNull(gate.finish())
        val answer = byteArrayOf(0x90.toByte(), 0x00)
        gate.deliver(answer)
        gate.deliver(byteArrayOf(0x65, 0x81.toByte()))
        assertEquals(1, posted.size)
        assertContentEquals(answer, posted.single())
    }

    @Test
    fun `cross thread reply and method return have exactly one delivery`() {
        val answer = byteArrayOf(0x90.toByte(), 0x00)
        repeat(128) {
            val posted = CopyOnWriteArrayList<ByteArray>()
            val direct = AtomicReference<ByteArray?>()
            val gate = IrohaPeerNfcApduReplyGateV1 { posted.add(it) }
            val start = CountDownLatch(1)
            val responder = thread {
                start.await()
                gate.deliver(answer)
            }
            val returning = thread {
                start.await()
                direct.set(gate.finish())
            }
            start.countDown()
            responder.join()
            returning.join()
            assertEquals(1, posted.size + if (direct.get() == null) 0 else 1)
            assertContentEquals(answer, direct.get() ?: posted.single())
        }
    }
}
