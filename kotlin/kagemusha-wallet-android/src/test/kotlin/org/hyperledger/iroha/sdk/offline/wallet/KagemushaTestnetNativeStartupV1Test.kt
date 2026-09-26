// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.junit.jupiter.api.Test

class KagemushaTestnetNativeStartupV1Test {
    @Test
    fun `invalid lengths never reach native activation`() {
        val endpoint = Endpoint()
        val startup = KagemushaTestnetNativeStartupV1.openEndpoint(endpoint)
        for (bytes in listOf(byteArrayOf(), ByteArray(1_048_577))) {
            assertFailsWith<IllegalArgumentException> { startup.activate(bytes) }
        }
        assertEquals(0, endpoint.received.size)
    }

    @Test
    fun `missing and mismatched contracts cannot activate`() {
        for (words in listOf(null, intArrayOf(), intArrayOf(1), intArrayOf(2, 1_048_576),
            intArrayOf(1, 1_048_575), intArrayOf(1, 1_048_576, 0))) {
            val endpoint = Endpoint(words = words)
            assertFailsWith<IllegalStateException> {
                KagemushaTestnetNativeStartupV1.openEndpoint(endpoint)
            }
            assertEquals(0, endpoint.received.size)
        }
    }

    @Test
    fun `opaque package is bounded and defensively copied`() {
        val endpoint = Endpoint(mutate = true)
        val startup = KagemushaTestnetNativeStartupV1.openEndpoint(endpoint)
        for (bytes in listOf(byteArrayOf(0, 1, -1), ByteArray(1_048_576) { 9 })) {
            val original = bytes.copyOf()
            startup.activate(bytes)
            assertContentEquals(original, bytes)
            assertContentEquals(original, endpoint.received.last())
        }
    }

    @Test
    fun `only zero means activation success`() {
        for (status in listOf(-312, -311, -1, 1, Int.MAX_VALUE)) {
            val endpoint = Endpoint(status = status)
            val startup = KagemushaTestnetNativeStartupV1.openEndpoint(endpoint)
            val error = assertFailsWith<KagemushaTestnetNativeStartupExceptionV1> {
                startup.activate(byteArrayOf(1))
            }
            assertEquals(status, error.nativeStatus)
            assertEquals(1, endpoint.received.size)
        }
    }

    @Test
    fun `each retry reaches native authentication`() {
        val endpoint = Endpoint()
        val startup = KagemushaTestnetNativeStartupV1.openEndpoint(endpoint)
        val bytes = byteArrayOf(1, 2, 3)
        startup.activate(bytes)
        endpoint.status = -311
        assertFailsWith<KagemushaTestnetNativeStartupExceptionV1> { startup.activate(bytes) }
        assertEquals(2, endpoint.received.size)
        endpoint.received.forEach { assertContentEquals(bytes, it) }
    }

    @Test
    fun `missing JNI symbols cannot imply activation success`() {
        val endpoint = object : KagemushaTestnetNativeStartupEndpointV1 {
            override fun contract(): IntArray = intArrayOf(1, 1_048_576)
            override fun activate(signedBootstrap: ByteArray): Int = throw UnsatisfiedLinkError()
        }
        val startup = KagemushaTestnetNativeStartupV1.openEndpoint(endpoint)
        assertFailsWith<IllegalStateException> { startup.activate(byteArrayOf(1)) }
        val missingContract = object : KagemushaTestnetNativeStartupEndpointV1 {
            override fun contract(): IntArray = throw UnsatisfiedLinkError()
            override fun activate(signedBootstrap: ByteArray): Int = error("must not activate")
        }
        assertFailsWith<IllegalStateException> {
            KagemushaTestnetNativeStartupV1.openEndpoint(missingContract)
        }
    }

    private class Endpoint(
        private val words: IntArray? = intArrayOf(1, 1_048_576),
        var status: Int = 0,
        private val mutate: Boolean = false,
    ) : KagemushaTestnetNativeStartupEndpointV1 {
        val received = mutableListOf<ByteArray>()
        override fun contract(): IntArray? = words?.copyOf()
        override fun activate(signedBootstrap: ByteArray): Int {
            received.add(signedBootstrap.copyOf())
            if (mutate) signedBootstrap[0] = (signedBootstrap[0].toInt() xor 1).toByte()
            return status
        }
    }
}
