package org.hyperledger.iroha.sdk.core.model

import java.lang.reflect.InvocationTargetException
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

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
    fun `authority cannot be synthesized by the Kotlin default constructor`() {
        val defaultingConstructor = TransactionPayload::class.java.declaredConstructors.single {
            it.isSynthetic && it.parameterTypes.last().name == "kotlin.jvm.internal.DefaultConstructorMarker"
        }
        val error = assertFailsWith<InvocationTargetException> {
            defaultingConstructor.newInstance(
                "00000000",
                null,
                0L,
                null,
                null,
                null,
                FeePaymentIntent.authority(emptyList()),
                null,
                0xbf,
                null,
            )
        }
        assertTrue(error.cause is NullPointerException)
        assertTrue(error.cause?.message?.contains("authority") == true)
    }

    @Test
    fun `chainId cannot be synthesized by the Kotlin default constructor`() {
        val defaultingConstructor = TransactionPayload::class.java.declaredConstructors.single {
            it.isSynthetic && it.parameterTypes.last().name == "kotlin.jvm.internal.DefaultConstructorMarker"
        }
        val error = assertFailsWith<InvocationTargetException> {
            defaultingConstructor.newInstance(
                null,
                sampleAuthority(0x00),
                0L,
                null,
                null,
                null,
                FeePaymentIntent.authority(emptyList()),
                null,
                0xbf,
                null,
            )
        }
        assertTrue(error.cause is NullPointerException)
        assertTrue(error.cause?.message?.contains("chainId") == true)
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
            testPayload(nonce = 0L, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `negative nonce throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(nonce = -1L, creationTimeMs = 1000L)
        }
    }

    @Test
    fun `full nonzero u32 nonce range is valid`() {
        val payload = testPayload(nonce = 0xffff_ffffL, creationTimeMs = 1000L)
        assertEquals(0xffff_ffffL, payload.nonce)

        val adapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload))
        assertEquals(0xffff_ffffL, decoded.nonce)
    }

    @Test
    fun `nonce above u32 range throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(nonce = 0x1_0000_0000L, creationTimeMs = 1000L)
        }
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
        assertEquals(10L, copied.nonce)
    }

    @Test
    fun `copy validates new values`() {
        val original = defaultPayload()
        assertFailsWith<IllegalArgumentException> {
            original.copy(chainId = "")
        }
    }

    @Test
    fun `VM and contract executables require gas before payload encoding`() {
        val invocation = ContractInvocation(
            CONTRACT_ADDRESS,
            ByteArray(32) { 1 },
            "run",
        )
        val executables = listOf(
            Executable.ivm(byteArrayOf(1)),
            Executable.contractCall(invocation),
            Executable.batch(listOf(ExecutableBatchItem.contractCall(invocation))),
        )

        executables.forEach { executable ->
            val gasless = testPayload(executable = executable)
            assertFailsWith<NoritoException> {
                NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).encodeTransaction(gasless)
            }
            val gasBound = testPayload(
                executable = executable,
                feePayment = FeePaymentIntent.authority(emptyList(), 1L),
            )
            NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).encodeTransaction(gasBound)
        }
    }

    @Test
    fun `native instructions do not require a transaction gas limit`() {
        testPayload(
            executable = Executable.instructions(
                listOf(InstructionBox.fromWirePayload("iroha.test", byteArrayOf(1))),
            ),
        )
    }

    @Test
    fun `equal instances are equal`() {
        val executable = Executable.ivm(byteArrayOf(1, 2, 3))
        val a = testPayload(
            chainId = "c",
            authority = sampleAuthority(0x31),
            creationTimeMs = 100,
            executable = executable,
            feePayment = FeePaymentIntent.authority(emptyList(), 1L),
            timeToLiveMs = 500,
            nonce = 1,
            metadata = mapOf("k" to JsonValue.string("v")),
        )
        val b = testPayload(
            chainId = "c",
            authority = sampleAuthority(0x31),
            creationTimeMs = 100,
            executable = executable,
            feePayment = FeePaymentIntent.authority(emptyList(), 1L),
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
        executable: Executable = Executable.instructions(emptyList()),
        timeToLiveMs: Long? = null,
        nonce: Long? = null,
        feePayment: FeePaymentIntent = FeePaymentIntent.authority(emptyList()),
        metadata: Map<String, JsonValue> = emptyMap(),
    ): TransactionPayload = TransactionPayload(
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = executable,
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        feePayment = feePayment,
        metadata = metadata,
    )

    private fun sampleAuthority(fill: Int): String = AccountAddress
        .fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    companion object {
        private const val CONTRACT_ADDRESS =
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
    }
}
