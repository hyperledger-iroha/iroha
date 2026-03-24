package org.hyperledger.iroha.sdk.core.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals

class TransactionPayloadTest {

    private fun defaultPayload() = TransactionPayload(
        creationTimeMs = 1000L,
    )

    @Test
    fun `constructor applies defaults`() {
        val payload = defaultPayload()
        assertEquals("00000000", payload.chainId)
        assertEquals("anonymous@wonderland", payload.authority)
        assertEquals(1000L, payload.creationTimeMs)
        assertEquals(null, payload.timeToLiveMs)
        assertEquals(null, payload.nonce)
        assertEquals(emptyMap(), payload.metadata)
    }

    @Test
    fun `blank chainId throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(chainId = "  ", creationTimeMs = 1000L)
        }
    }

    @Test
    fun `blank authority throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(authority = "", creationTimeMs = 1000L)
        }
    }

    @Test
    fun `negative creationTimeMs throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(creationTimeMs = -1)
        }
    }

    @Test
    fun `zero creationTimeMs is valid`() {
        val payload = TransactionPayload(creationTimeMs = 0)
        assertEquals(0L, payload.creationTimeMs)
    }

    @Test
    fun `zero timeToLiveMs throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(timeToLiveMs = 0L, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `negative timeToLiveMs throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(timeToLiveMs = -5L, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `positive timeToLiveMs is valid`() {
        val payload = TransactionPayload(timeToLiveMs = 500L, creationTimeMs = 1000L)
        assertEquals(500L, payload.timeToLiveMs)
    }

    @Test
    fun `zero nonce throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(nonce = 0, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `negative nonce throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(nonce = -1, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `positive nonce is valid`() {
        val payload = TransactionPayload(nonce = 42, creationTimeMs = 1000L)
        assertEquals(42, payload.nonce)
    }

    @Test
    fun `blank metadata key throws`() {
        assertFailsWith<IllegalArgumentException> {
            TransactionPayload(metadata = mapOf("  " to "value"), creationTimeMs = 1000L)
        }
    }

    @Test
    fun `defensive copy on metadata input`() {
        val original = mutableMapOf("key" to "value")
        val payload = TransactionPayload(metadata = original, creationTimeMs = 1000L)
        original["injected"] = "bad"
        assertEquals(1, payload.metadata.size)
        assertEquals("value", payload.metadata["key"])
    }

    @Test
    fun `metadata getter returns immutable snapshot`() {
        val payload = TransactionPayload(
            metadata = mapOf("a" to "1"),
            creationTimeMs = 1000L,
        )
        val meta = payload.metadata
        assertFailsWith<UnsupportedOperationException> {
            (meta as MutableMap)["b"] = "2"
        }
    }

    @Test
    fun `copy preserves values and allows overrides`() {
        val original = TransactionPayload(
            chainId = "chain1",
            authority = "user@domain",
            creationTimeMs = 2000L,
            nonce = 7,
        )
        val copied = original.copy(chainId = "chain2", nonce = 10)
        assertEquals("chain2", copied.chainId)
        assertEquals("user@domain", copied.authority)
        assertEquals(2000L, copied.creationTimeMs)
        assertEquals(10, copied.nonce)
    }

    @Test
    fun `copy validates new values`() {
        val original = defaultPayload()
        assertFailsWith<IllegalArgumentException> {
            original.copy(chainId = "")
        }
    }

    @Test
    fun `equal instances are equal`() {
        val executable = Executable.ivm(byteArrayOf(1, 2, 3))
        val a = TransactionPayload(
            chainId = "c",
            authority = "a@b",
            creationTimeMs = 100,
            executable = executable,
            timeToLiveMs = 500,
            nonce = 1,
            metadata = mapOf("k" to "v"),
        )
        val b = TransactionPayload(
            chainId = "c",
            authority = "a@b",
            creationTimeMs = 100,
            executable = executable,
            timeToLiveMs = 500,
            nonce = 1,
            metadata = mapOf("k" to "v"),
        )
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `different instances are not equal`() {
        val a = TransactionPayload(chainId = "c1", creationTimeMs = 100)
        val b = TransactionPayload(chainId = "c2", creationTimeMs = 100)
        assertNotEquals(a, b)
    }
}
