package org.hyperledger.iroha.sdk.offline

import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class IrohaPeerIsoDepContactV1Test {
    /** A fake tag whose connect blocks until the platform close (or the test) releases it. */
    private class BlockingTag {
        val connected = AtomicBoolean(false)
        val connects = AtomicInteger()
        val closes = AtomicInteger()
        val connectEntered = CountDownLatch(1)
        val release = CountDownLatch(1)
        var connectSucceedsAfterRelease = false
        val contact = IrohaPeerIsoDepContactV1(
            isConnected = { connected.get() },
            connectTag = {
                connects.incrementAndGet()
                connectEntered.countDown()
                check(release.await(5, TimeUnit.SECONDS)) { "connect was never interrupted" }
                if (!connectSucceedsAfterRelease) throw IOException("Tag connection interrupted")
                connected.set(true)
            },
            closeTag = {
                closes.incrementAndGet()
                connected.set(false)
                release.countDown()
            },
        )
    }

    @Test
    fun `cancel interrupts a connect that has not reported a connection yet`() {
        val tag = BlockingTag()
        val failure = AtomicBoolean(false)
        val worker = Thread { try { tag.contact.ensureConnected() } catch (_: IOException) { failure.set(true) } }
        worker.start()
        assertTrue(tag.connectEntered.await(5, TimeUnit.SECONDS))
        tag.contact.close()
        worker.join(5_000)
        assertEquals(1, tag.closes.get(), "Close must reach the tag while connect is still blocked")
        assertTrue(failure.get(), "The interrupted connect reports the contact as lost")
        assertFailsWith<IOException> { tag.contact.ensureConnected() }
        assertEquals(1, tag.connects.get(), "A closed contact never reconnects")
    }

    @Test
    fun `a connect that completes after cancel is closed again and never used`() {
        val tag = BlockingTag().apply { connectSucceedsAfterRelease = true }
        val failure = AtomicBoolean(false)
        val worker = Thread { try { tag.contact.ensureConnected() } catch (_: IOException) { failure.set(true) } }
        worker.start()
        assertTrue(tag.connectEntered.await(5, TimeUnit.SECONDS))
        tag.contact.close()
        worker.join(5_000)
        assertTrue(failure.get(), "Cancel wins over a platform connect that still completed")
        assertEquals(2, tag.closes.get(), "The late connection is closed again")
        assertTrue(!tag.connected.get())
    }

    @Test
    fun `cancel before the exchange starts prevents a silent reconnect`() {
        var connected = true
        var connects = 0
        val contact = IrohaPeerIsoDepContactV1({ connected }, { connects++; connected = true }, { connected = false })
        contact.close()
        assertFailsWith<IOException> { contact.requireOpen() }
        assertFailsWith<IOException> { contact.ensureConnected() }
        assertEquals(0, connects, "SELECT after Cancel cannot reopen the radio")
        val successor = IrohaPeerIsoDepContactV1({ connected }, { connects++; connected = true }, { connected = false })
        successor.ensureConnected()
        assertEquals(1, connects, "The next contact is unaffected")
    }

    @Test
    fun `a tag that already left the field cannot turn close into a crash`() {
        val contact = IrohaPeerIsoDepContactV1({ false }, {}, { throw SecurityException("Tag is out of date") })
        contact.close()
        assertFailsWith<IOException> { contact.ensureConnected() }
    }
}

class IrohaPeerIsoDepTimeoutsV1Test {
    @Test
    fun `default durable waits cover receiver persistence without extending ordinary commands`() {
        val timeouts = IrohaPeerIsoDepTimeoutsV1()
        assertEquals(30_000L, IrohaPeerNfcCardApduBridgeV1.DEFAULT_DURABLE_DEADLINE_MILLIS)
        assertEquals(10_000, timeouts.ordinaryMillis)
        assertEquals(10_000, timeouts.forOperation(IrohaPeerNfcOperationV1.GET_INFO))
        assertEquals(35_000, timeouts.forOperation(IrohaPeerNfcOperationV1.BEGIN_PAYMENT))
        assertEquals(35_000, timeouts.forOperation(IrohaPeerNfcOperationV1.COMMIT))
        assertEquals(40_000, IrohaPeerIsoDepTimeoutsV1(40_000).forOperation(IrohaPeerNfcOperationV1.COMMIT))
    }

    @Test
    fun `only durable commands receive the longer wallet wait`() {
        val timeouts = IrohaPeerIsoDepTimeoutsV1(8_000, 35_000)
        for (operation in IrohaPeerNfcOperationV1.values()) {
            val expected = if (operation == IrohaPeerNfcOperationV1.BEGIN_PAYMENT ||
                operation == IrohaPeerNfcOperationV1.COMMIT) 35_000 else 8_000
            assertEquals(expected, timeouts.forOperation(operation), operation.name)
        }
        assertEquals(8_000, timeouts.ordinaryMillis)
    }

    @Test
    fun `invalid or unbounded timeout pairs are refused`() {
        for ((ordinary, durable) in listOf(0 to 35_000, 8_000 to 7_999,
            8_000 to 120_001, 120_001 to 120_001)) {
            assertFailsWith<IllegalArgumentException> { IrohaPeerIsoDepTimeoutsV1(ordinary, durable) }
        }
        assertEquals(1, IrohaPeerIsoDepTimeoutsV1(1, 1).forOperation(IrohaPeerNfcOperationV1.COMMIT))
        assertEquals(120_000, IrohaPeerIsoDepTimeoutsV1(120_000, 120_000)
            .forOperation(IrohaPeerNfcOperationV1.GET_INFO))
    }
}
