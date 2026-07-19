package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.address.AccountAddress
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals

class TransactionPayloadTest {

    private fun defaultPayload() = testPayload(
        creationTimeMs = 1000L,
    )

    @Test
    fun `constructor applies defaults`() {
        val payload = defaultPayload()
        assertEquals("00000000", payload.chainId)
        assertEquals(sampleAuthority(0x00), payload.authority)
        assertEquals(1000L, payload.creationTimeMs)
        assertEquals(null, payload.timeToLiveMs)
        assertEquals(null, payload.nonce)
        assertEquals(emptyMap(), payload.metadata)
    }

    @Test
    fun `blank chainId throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(chainId = "  ", creationTimeMs = 1000L)
        }
    }

    @Test
    fun `blank authority throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(authority = "", creationTimeMs = 1000L)
        }
    }

    @Test
    fun `padded chainId throws before payload can be signed`() {
        val error = assertFailsWith<IllegalArgumentException> {
            testPayload(chainId = " chain", creationTimeMs = 1000L)
        }
        assertEquals("chainId must not contain surrounding whitespace", error.message)
    }

    @Test
    fun `authority must be exact canonical I105 before payload can be signed`() {
        val authority = sampleAuthority(0x01)
        val padded = assertFailsWith<IllegalArgumentException> {
            testPayload(authority = " $authority", creationTimeMs = 1000L)
        }
        assertEquals("authority must not contain surrounding whitespace", padded.message)

        val alias = assertFailsWith<IllegalArgumentException> {
            testPayload(authority = "alice@wonderland", creationTimeMs = 1000L)
        }
        assertEquals(
            "authority must use canonical I105 encoded account without @domain",
            alias.message,
        )
    }

    @Test
    fun `negative creationTimeMs throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(creationTimeMs = -1)
        }
    }

    @Test
    fun `zero creationTimeMs is valid`() {
        val payload = testPayload(creationTimeMs = 0)
        assertEquals(0L, payload.creationTimeMs)
    }

    @Test
    fun `zero timeToLiveMs throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(timeToLiveMs = 0L, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `negative timeToLiveMs throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(timeToLiveMs = -5L, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `positive timeToLiveMs is valid`() {
        val payload = testPayload(timeToLiveMs = 500L, creationTimeMs = 1000L)
        assertEquals(500L, payload.timeToLiveMs)
    }

    @Test
    fun `zero nonce throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(nonce = 0, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `negative nonce throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(nonce = -1, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `positive nonce is valid`() {
        val payload = testPayload(nonce = 42, creationTimeMs = 1000L)
        assertEquals(42, payload.nonce)
    }

    @Test
    fun `blank metadata key throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(metadata = mapOf("  " to JsonValue.string("value")), creationTimeMs = 1000L)
        }
    }

    @Test
    fun `defensive copy on metadata input`() {
        val original = mutableMapOf("key" to JsonValue.string("value"))
        val payload = testPayload(metadata = original, creationTimeMs = 1000L)
        original["injected"] = JsonValue.string("bad")
        assertEquals(1, payload.metadata.size)
        assertEquals(JsonValue.string("value"), payload.metadata["key"])
    }

    @Test
    fun `metadata getter returns immutable snapshot`() {
        val payload = testPayload(
            metadata = mapOf("a" to JsonValue.string("1")),
            creationTimeMs = 1000L,
        )
        val meta = payload.metadata
        assertFailsWith<UnsupportedOperationException> {
            (meta as MutableMap)["b"] = JsonValue.string("2")
        }
    }

    @Test
    fun `copy preserves values and allows overrides`() {
        val original = testPayload(
            chainId = "chain1",
            authority = sampleAuthority(0x21),
            creationTimeMs = 2000L,
            nonce = 7,
        )
        val copied = original.copy(chainId = "chain2", nonce = 10)
        assertEquals("chain2", copied.chainId)
        assertEquals(sampleAuthority(0x21), copied.authority)
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
        val a = testPayload(
            chainId = "c",
            authority = sampleAuthority(0x31),
            creationTimeMs = 100,
            executable = executable,
            timeToLiveMs = 500,
            nonce = 1,
            metadata = mapOf("k" to JsonValue.string("v")),
        )
        val b = testPayload(
            chainId = "c",
            authority = sampleAuthority(0x31),
            creationTimeMs = 100,
            executable = executable,
            timeToLiveMs = 500,
            nonce = 1,
            metadata = mapOf("k" to JsonValue.string("v")),
        )
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `different instances are not equal`() {
        val a = testPayload(chainId = "c1", creationTimeMs = 100)
        val b = testPayload(chainId = "c2", creationTimeMs = 100)
        assertNotEquals(a, b)
    }

    private fun testPayload(
        chainId: String = "00000000",
        authority: String = sampleAuthority(0x00),
        creationTimeMs: Long = System.currentTimeMillis(),
        executable: Executable = Executable.ivm(byteArrayOf()),
        timeToLiveMs: Long? = null,
        nonce: Int? = null,
        metadata: Map<String, JsonValue> = emptyMap(),
    ): TransactionPayload = TransactionPayload(
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = executable,
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        feePayment = FeePaymentIntent.authority(emptyList()),
        metadata = metadata,
    )

    private fun sampleAuthority(fill: Int): String = AccountAddress
        .fromAccount(ByteArray(32) { fill.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
}
