package org.hyperledger.iroha.sdk.core.model

import java.lang.reflect.InvocationTargetException
import java.lang.reflect.Modifier
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class TransactionPayloadTest {

    private fun defaultPayload() = testPayload(
        creationTimeMs = 1000L,
    )

    @Test
    fun `constructor applies defaults`() {
        val payload = defaultPayload()
        assertEquals(TEST_NETWORK_ID, payload.networkId)
        assertEquals(sampleAuthority(0x00), payload.authority)
        assertEquals(1000L, payload.creationTimeMs)
        assertEquals(100_000L, payload.timeToLiveMs)
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
                TEST_NETWORK_ID,
                null,
                0L,
                null,
                null,
                null,
                FeePaymentIntent.authority(emptyList()),
                null,
                null, // attachments
                0x1bf,
                null,
            )
        }
        assertTrue(error.cause is NullPointerException)
        assertTrue(error.cause?.message?.contains("authority") == true)
    }

    @Test
    fun `networkId cannot be synthesized by the Kotlin default constructor`() {
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
                null, // attachments
                0x1bf,
                null,
            )
        }
        assertTrue(error.cause is NullPointerException)
        assertTrue(error.cause?.message?.contains("networkId") == true)
    }

    @Test
    fun `blank networkId throws`() {
        assertFailsWith<IllegalArgumentException> {
            NetworkId.parse("  ")
        }
    }

    @Test
    fun `blank authority throws`() {
        assertFailsWith<IllegalArgumentException> {
            testPayload(authority = "", creationTimeMs = 1000L)
        }
    }

    @Test
    fun networkIdRejectsNonCanonicalText() {
        val error = assertFailsWith<IllegalArgumentException> {
            NetworkId.parse(TEST_NETWORK_ID.literal.lowercase())
        }
        assertTrue(error.message?.contains("exact canonical") == true)
    }

    @Test
    fun `networkId requires a checked 32 byte genesis hash`() {
        for (invalid in listOf(
            TEST_NETWORK_ID.literal.dropLast(1) + "1",
            TEST_NETWORK_ID.literal.removeSuffix("#A2F0"),
            "32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149",
            "network-label",
        )) {
            assertFailsWith<IllegalArgumentException> {
                NetworkId.parse(invalid)
            }
        }
        assertEquals(
            TEST_NETWORK_ID,
            NetworkId.fromBytes(TEST_NETWORK_ID.bytes()),
        )
    }

    @Test
    fun `networkId defensively copies raw bytes`() {
        val source = TEST_NETWORK_ID.bytes()
        val networkId = NetworkId.fromBytes(source)
        source[0] = (source[0].toInt() xor 0x7f).toByte()
        val exposed = networkId.bytes()
        exposed[1] = (exposed[1].toInt() xor 0x7f).toByte()

        assertEquals(TEST_NETWORK_ID, networkId)
        assertNotEquals(source[0], networkId.bytes()[0])
        assertNotEquals(exposed[1], networkId.bytes()[1])
    }

    @Test
    fun `networkId raw bytes require exact width and genesis marker`() {
        listOf(0, NetworkId.BYTE_LENGTH - 1, NetworkId.BYTE_LENGTH + 1).forEach { size ->
            assertFailsWith<IllegalArgumentException> {
                NetworkId.fromBytes(ByteArray(size))
            }
        }

        val missingMarker = TEST_NETWORK_ID.bytes().also { bytes ->
            bytes[bytes.lastIndex] = (bytes.last().toInt() and 0xfe).toByte()
        }
        val failure = assertFailsWith<IllegalArgumentException> {
            NetworkId.fromBytes(missingMarker)
        }
        assertTrue(failure.message.orEmpty().contains("marker bit"))
    }

    @Test
    fun publicTransactionApiDoesNotExposeLegacyChainNames() {
        val publicTypes = listOf(
            TransactionPayload::class.java,
            NetworkId::class.java,
            LocalSigningContext::class.java,
            NativeSignerBridge::class.java,
        )
        publicTypes.forEach { type ->
            val exposedNames = buildList {
                type.fields.mapTo(this) { it.name }
                type.methods.mapTo(this) { it.name }
                type.constructors.flatMapTo(this) { constructor ->
                    constructor.parameters.map { it.name }
                }
            }
            assertFalse(
                exposedNames.any(::isLegacyChainIdentityName),
                "${type.name} exposes a retired ChainId surface: $exposedNames",
            )
        }

        val payloadConstructor = TransactionPayload::class.java.constructors.single {
            Modifier.isPublic(it.modifiers) && !it.isSynthetic
        }
        assertEquals(NetworkId::class.java, payloadConstructor.parameterTypes.first())
        assertEquals(
            NetworkId::class.java,
            LocalSigningContext::class.java.constructors.single().parameterTypes.single(),
        )
        val signer = NativeSignerBridge::class.java.declaredMethods.single {
            it.name == "encodeRegisterZkAssetSignedTransaction" &&
                Modifier.isPublic(it.modifiers) &&
                Modifier.isStatic(it.modifiers)
        }
        assertEquals(NetworkId::class.java, signer.parameterTypes[1])
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
            networkId = TEST_NETWORK_ID,
            authority = sampleAuthority(0x21),
            creationTimeMs = 2000L,
            nonce = 7,
        )
        val copied = original.copy(networkId = OTHER_NETWORK_ID, nonce = 10)
        assertEquals(OTHER_NETWORK_ID, copied.networkId)
        assertEquals(sampleAuthority(0x21), copied.authority)
        assertEquals(2000L, copied.creationTimeMs)
        assertEquals(10L, copied.nonce)
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
            networkId = TEST_NETWORK_ID,
            authority = sampleAuthority(0x31),
            creationTimeMs = 100,
            executable = executable,
            feePayment = FeePaymentIntent.authority(emptyList(), 1L),
            timeToLiveMs = 500,
            nonce = 1,
            metadata = mapOf("k" to JsonValue.string("v")),
        )
        val b = testPayload(
            networkId = TEST_NETWORK_ID,
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
        val a = testPayload(networkId = TEST_NETWORK_ID, creationTimeMs = 100)
        val b = testPayload(networkId = OTHER_NETWORK_ID, creationTimeMs = 100)
        assertNotEquals(a, b)
    }

    private fun testPayload(
        networkId: NetworkId = TEST_NETWORK_ID,
        authority: String = sampleAuthority(0x00),
        creationTimeMs: Long = System.currentTimeMillis(),
        executable: Executable = Executable.instructions(emptyList()),
        timeToLiveMs: Long? = 100_000L,
        nonce: Long? = null,
        feePayment: FeePaymentIntent = FeePaymentIntent.authority(emptyList()),
        metadata: Map<String, JsonValue> = emptyMap(),
    ): TransactionPayload = TransactionPayload(
        networkId = networkId,
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

    private fun isLegacyChainIdentityName(name: String): Boolean {
        val compact = name.replace("_", "").lowercase()
        return compact == "chain" || compact.contains("chainid")
    }

    companion object {
        private const val CONTRACT_ADDRESS =
            "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
        private val TEST_NETWORK_ID = NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
        )
        private val OTHER_NETWORK_ID = NetworkId.parse(
            "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22",
        )
    }
}
