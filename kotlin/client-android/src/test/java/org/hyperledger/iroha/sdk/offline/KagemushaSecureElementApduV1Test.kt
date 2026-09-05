package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class KagemushaSecureElementApduV1Test {
    @Test
    fun `capability probe preserves the foundation diagnostic allocation`() {
        val channel = ScriptedChannel(
            response = ByteArray(116) { it.toByte() },
            capabilities = ByteArray(96) { (it + 1).toByte() },
        )
        val endpoint = KagemushaSecureElementApduEndpointV1(channel)
        assertContentEquals(channel.capabilities, endpoint.capabilities())
        assertContentEquals(
            byteArrayOf(0x80.toByte(), 0x11, 0, 0, 96),
            channel.commands.single(),
        )
        assertTrue(channel.commands.none { it[1] == 0x10.toByte() })
        assertTrue(channel.commands.none { (it[1].toInt() and 0xff) in 0x20..0x27 })
    }

    @Test
    fun `command and response are chunked deterministically and digest bound`() {
        val command = ByteArray(80 + 500) { (it * 17).toByte() }
        val response = ByteArray(116 + 509) { (it * 29).toByte() }
        val channel = ScriptedChannel(response = response)
        val endpoint = KagemushaSecureElementApduEndpointV1(channel)

        assertContentEquals(response, endpoint.execute(command))
        assertContentEquals(command, channel.receivedCommand)
        assertEquals(listOf(224, 224, 132), channel.writeSizes)
        assertEquals(listOf(224, 224, 177), channel.readSizes)
        assertEquals(listOf(0, 1, 2), channel.writeIndexes)
        assertEquals(listOf(0, 1, 2), channel.readIndexes)
        assertEquals(0, channel.abortCount)
    }

    @Test
    fun `bad response digest fails closed and aborts transport`() {
        val channel = ScriptedChannel(response = ByteArray(116) { 7 })
        channel.corruptResponseDigest = true
        val endpoint = KagemushaSecureElementApduEndpointV1(channel)
        assertFailsWith<IllegalArgumentException> {
            endpoint.execute(ByteArray(80) { 3 })
        }
        assertEquals(1, channel.abortCount)
    }

    @Test
    fun `foundation ODJ0 diagnostic cannot become an available provider`() {
        val channel = object : KagemushaSecureElementApduEndpointV1.Channel {
            override fun transmit(command: ByteArray): ByteArray =
                byteArrayOf(0x4f, 0x44, 0x4a, 0x30, 1, 0, 0, 0, 0x90.toByte(), 0)
        }
        val endpoint = KagemushaSecureElementApduEndpointV1(channel)
        assertFailsWith<IllegalArgumentException> {
            KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
        }
    }

    private class ScriptedChannel(
        private val response: ByteArray,
        val capabilities: ByteArray = ByteArray(96) { (it + 1).toByte() },
    ) : KagemushaSecureElementApduEndpointV1.Channel {
        val commands = mutableListOf<ByteArray>()
        val writeSizes = mutableListOf<Int>()
        val readSizes = mutableListOf<Int>()
        val writeIndexes = mutableListOf<Int>()
        val readIndexes = mutableListOf<Int>()
        var receivedCommand = ByteArray(0)
        var corruptResponseDigest = false
        var abortCount = 0
        private var declaredLength = 0
        private var expectedCommandDigest = ByteArray(0)

        override fun transmit(command: ByteArray): ByteArray {
            commands += command.copyOf()
            val instruction = command[1].toInt() and 0xff
            return when (instruction) {
                0x11 -> success(capabilities)
                0x12 -> {
                    val data = command.copyOfRange(5, command.size)
                    declaredLength = readU32Le(data)
                    expectedCommandDigest = data.copyOfRange(4, 36)
                    receivedCommand = ByteArray(0)
                    success()
                }
                0x13 -> {
                    writeIndexes += index(command)
                    val data = command.copyOfRange(5, command.size)
                    writeSizes += data.size
                    receivedCommand += data
                    success()
                }
                0x14 -> {
                    assertEquals(declaredLength, receivedCommand.size)
                    assertContentEquals(expectedCommandDigest, sha256(receivedCommand))
                    val digest = sha256(response)
                    if (corruptResponseDigest) digest[0] = (digest[0].toInt() xor 1).toByte()
                    success(u32Le(response.size) + digest)
                }
                0x15 -> {
                    val chunkIndex = index(command)
                    readIndexes += chunkIndex
                    val expected = command[4].toInt() and 0xff
                    val count = if (expected == 0) 256 else expected
                    val offset = chunkIndex * 224
                    val chunk = response.copyOfRange(offset, minOf(offset + count, response.size))
                    readSizes += chunk.size
                    success(chunk)
                }
                0x16 -> {
                    abortCount += 1
                    success()
                }
                else -> error("unexpected instruction %02x".format(instruction))
            }
        }

        private fun index(command: ByteArray): Int =
            ((command[2].toInt() and 0xff) shl 8) or (command[3].toInt() and 0xff)

        private fun success(data: ByteArray = ByteArray(0)): ByteArray =
            data + byteArrayOf(0x90.toByte(), 0)
    }

    companion object {
        private fun sha256(bytes: ByteArray): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(bytes)

        private fun u32Le(value: Int): ByteArray = ByteArray(4) { index ->
            (value ushr (index * 8)).toByte()
        }

        private fun readU32Le(bytes: ByteArray): Int =
            (0 until 4).fold(0) { value, index ->
                value or ((bytes[index].toInt() and 0xff) shl (index * 8))
            }
    }
}
