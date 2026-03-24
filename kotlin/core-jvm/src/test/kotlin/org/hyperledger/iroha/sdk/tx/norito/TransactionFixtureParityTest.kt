package org.hyperledger.iroha.sdk.tx.norito

import java.nio.file.Files
import java.util.Base64
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotNull

class TransactionFixtureParityTest {
    private val adapter = NoritoJavaCodecAdapter()

    @Test
    fun `transaction payload fixtures round-trip with kotlin codec`() {
        for (fixture in AndroidFixtureSupport.loadPayloadFixtures()) {
            val payload = fixture.materializePayload(adapter)

            assertEquals(fixture.chain, payload.chainId, "${fixture.name}: chain mismatch")
            assertEquals(fixture.authority, payload.authority, "${fixture.name}: authority mismatch")
            assertEquals(
                fixture.creationTimeMs,
                payload.creationTimeMs,
                "${fixture.name}: creation_time_ms mismatch",
            )
            assertEquals(
                fixture.timeToLiveMs,
                payload.timeToLiveMs,
                "${fixture.name}: TTL mismatch",
            )
            assertEquals(fixture.nonce, payload.nonce, "${fixture.name}: nonce mismatch")

            val encoded = adapter.encodeTransaction(payload)
            fixture.encodedBase64?.let { expected ->
                assertEquals(
                    expected,
                    Base64.getEncoder().encodeToString(encoded),
                    "${fixture.name}: encoded payload mismatch",
                )
            }

            val decoded = adapter.decodeTransaction(encoded)
            assertEquals(payload, decoded, "${fixture.name}: Kotlin payload round-trip mismatch")
        }
    }

    @Test
    fun `transaction fixture manifest remains compatible with kotlin codec`() {
        val payloadFixturesByName = AndroidFixtureSupport.loadPayloadFixtures().associateBy { it.name }

        for (fixture in AndroidFixtureSupport.loadManifestFixtures()) {
            val encodedPath = AndroidFixtureSupport.resolveSharedResource(fixture.encodedFile)
            val encodedBytes = Files.readAllBytes(encodedPath)
            val payloadBytes = Base64.getDecoder().decode(fixture.payloadBase64)
            val signedBytes = Base64.getDecoder().decode(fixture.signedBase64)

            assertEquals(
                fixture.encodedLen,
                encodedBytes.size.toLong(),
                "${fixture.name}: encoded_len mismatch",
            )
            assertEquals(
                fixture.payloadBase64,
                Base64.getEncoder().encodeToString(encodedBytes),
                "${fixture.name}: payload_base64 mismatch vs encoded file",
            )
            assertEquals(
                fixture.signedLen,
                signedBytes.size.toLong(),
                "${fixture.name}: signed_len mismatch",
            )
            assertEquals(
                hex(IrohaHash.prehash(payloadBytes)),
                fixture.payloadHash,
                "${fixture.name}: payload_hash mismatch",
            )
            assertEquals(
                SignedTransactionHasher.hashCanonicalHex(signedBytes),
                fixture.signedHash,
                "${fixture.name}: signed_hash mismatch",
            )

            val payload = adapter.decodeTransaction(payloadBytes)
            assertEquals(fixture.chain, payload.chainId, "${fixture.name}: chain mismatch")
            assertEquals(
                normalizeAuthority(fixture.authority),
                normalizeAuthority(payload.authority),
                "${fixture.name}: authority mismatch",
            )
            assertEquals(
                fixture.creationTimeMs,
                payload.creationTimeMs,
                "${fixture.name}: creation_time_ms mismatch",
            )
            assertEquals(
                fixture.timeToLiveMs,
                payload.timeToLiveMs,
                "${fixture.name}: TTL mismatch",
            )
            assertEquals(
                fixture.nonce?.toInt(),
                payload.nonce,
                "${fixture.name}: nonce mismatch",
            )
            assertContentEquals(
                payloadBytes,
                adapter.encodeTransaction(payload),
                "${fixture.name}: Kotlin payload re-encoding drift",
            )

            payloadFixturesByName[fixture.name]?.let { sourceFixture ->
                sourceFixture.encodedBase64?.let { expected ->
                    assertEquals(expected, fixture.payloadBase64, "${fixture.name}: manifest payload mismatch")
                }
            }

            val signedParts = decodeSignedParts(fixture.name, signedBytes)
            assertContentEquals(
                payloadBytes,
                signedParts.payloadBytes,
                "${fixture.name}: signed payload mismatch",
            )

            val signed = SignedTransaction(
                payloadBytes,
                signedParts.signature,
                byteArrayOf(),
                SIGNED_SCHEMA,
            )
            assertContentEquals(
                signedBytes,
                SignedTransactionEncoder.encode(signed),
                "${fixture.name}: Kotlin signed transaction re-encoding drift",
            )

            val versioned = SignedTransactionEncoder.encodeVersioned(signed)
            assertEquals(
                signedBytes.size + 1,
                versioned.size,
                "${fixture.name}: versioned signed length mismatch",
            )
            assertEquals(
                VERSION_BYTE,
                versioned.first(),
                "${fixture.name}: versioned prefix mismatch",
            )
            assertContentEquals(
                signedBytes,
                versioned.copyOfRange(1, versioned.size),
                "${fixture.name}: versioned signed payload mismatch",
            )
        }
    }

    @Test
    fun `fixture loader accepts wire instruction entries`() {
        val wirePayload = NoritoCodec.encode(
            "wire-fixture",
            "iroha.test.WirePayload",
            NoritoAdapters.stringAdapter(),
        )
        val fixture = AndroidFixtureSupport.payloadFixtureFromValue(
            mapOf(
                "name" to "wire-instruction-fixture",
                "chain" to "00000001",
                "authority" to "alice@wonderland",
                "creation_time_ms" to 0L,
                "payload" to mapOf(
                    "chain" to "00000001",
                    "authority" to "alice@wonderland",
                    "creation_time_ms" to 0L,
                    "metadata" to emptyMap<String, String>(),
                    "executable" to mapOf(
                        "Instructions" to listOf(
                            mapOf(
                                "wire_name" to "iroha.custom",
                                "payload_base64" to Base64.getEncoder().encodeToString(wirePayload),
                            ),
                        ),
                    ),
                ),
            ),
        )

        val payload = fixture.materializePayload(adapter)
        val executable = assertIs<Executable.Instructions>(payload.executable)
        assertEquals(1, executable.instructions.size)
        val wireInstruction = assertIs<WirePayload>(executable.instructions.single().payload)
        assertEquals("iroha.custom", wireInstruction.wireName)
        assertContentEquals(wirePayload, wireInstruction.payloadBytes)
    }

    @Test
    fun `fixture loader rejects wire instruction arguments`() {
        val wirePayload = NoritoCodec.encode(
            "wire-arguments",
            "iroha.test.WirePayload",
            NoritoAdapters.stringAdapter(),
        )
        val fixture = AndroidFixtureSupport.payloadFixtureFromValue(
            mapOf(
                "name" to "wire-instruction-arguments-fixture",
                "chain" to "00000001",
                "authority" to "alice@wonderland",
                "creation_time_ms" to 0L,
                "payload" to mapOf(
                    "chain" to "00000001",
                    "authority" to "alice@wonderland",
                    "creation_time_ms" to 0L,
                    "metadata" to emptyMap<String, String>(),
                    "executable" to mapOf(
                        "Instructions" to listOf(
                            mapOf(
                                "arguments" to mapOf(
                                    "wire_name" to "iroha.custom",
                                    "payload_base64" to Base64.getEncoder().encodeToString(wirePayload),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )

        assertFailsWith<RuntimeException> {
            fixture.materializePayload(adapter)
        }
    }

    @Test
    fun `fixture loader rejects missing wire instruction fields`() {
        val fixture = AndroidFixtureSupport.payloadFixtureFromValue(
            mapOf(
                "name" to "missing-wire-fields",
                "chain" to "00000002",
                "authority" to "alice@wonderland",
                "creation_time_ms" to 1_735_000_000_000L,
                "time_to_live_ms" to null,
                "nonce" to null,
                "payload" to mapOf(
                    "chain" to "00000002",
                    "authority" to "alice@wonderland",
                    "creation_time_ms" to 1_735_000_000_000L,
                    "metadata" to emptyMap<String, String>(),
                    "executable" to mapOf(
                        "Instructions" to listOf(
                            mapOf("wire_name" to "iroha.register"),
                        ),
                    ),
                ),
            ),
        )

        assertFailsWith<RuntimeException> {
            fixture.materializePayload(adapter)
        }
    }

    private fun decodeSignedParts(name: String, signedBytes: ByteArray): SignedParts {
        val decoder = NoritoDecoder(signedBytes, NoritoHeader.MINOR_VERSION)
        val signatureField = readField(decoder, "$name.signed.signature")
        val payloadField = readField(decoder, "$name.signed.payload")
        val attachmentsField = readField(decoder, "$name.signed.attachments")
        val multisigField = readField(decoder, "$name.signed.multisig_signatures")
        require(decoder.remaining() == 0) { "$name: signed transaction has trailing bytes" }

        val signature = decodeSignature(name, signatureField)
        decodeOptionField("$name.signed.attachments", attachmentsField)
        decodeOptionField("$name.signed.multisig_signatures", multisigField)
        return SignedParts(signature = signature, payloadBytes = payloadField)
    }

    private fun decodeSignature(name: String, signatureField: ByteArray): ByteArray {
        val fieldDecoder = NoritoDecoder(signatureField, NoritoHeader.MINOR_VERSION)
        val inner = readField(fieldDecoder, "$name.signed.signature.inner")
        require(fieldDecoder.remaining() == 0) { "$name: signature field has trailing bytes" }
        val decoder = NoritoDecoder(inner, NoritoHeader.MINOR_VERSION)
        val signature = BYTE_VECTOR_ADAPTER.decode(decoder)
        require(decoder.remaining() == 0) { "$name: signature payload has trailing bytes" }
        return signature
    }

    private fun decodeOptionField(name: String, fieldBytes: ByteArray): ByteArray? {
        val decoder = NoritoDecoder(fieldBytes, NoritoHeader.MINOR_VERSION)
        val tag = decoder.readByte()
        return when (tag) {
            0 -> {
                require(decoder.remaining() == 0) { "$name: Option::None has trailing bytes" }
                null
            }

            1 -> {
                val length = decoder.readLength(decoder.compactLenActive())
                require(length <= Int.MAX_VALUE) { "$name: Option payload too large" }
                val payload = decoder.readBytes(length.toInt())
                require(decoder.remaining() == 0) { "$name: Option payload has trailing bytes" }
                payload
            }

            else -> error("$name: invalid Option tag $tag")
        }
    }

    private fun readField(decoder: NoritoDecoder, field: String): ByteArray {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$field length too large: $length" }
        return decoder.readBytes(length.toInt())
    }

    private fun normalizeAuthority(authority: String?): String? {
        val trimmed = authority?.trim() ?: return null
        if (trimmed.isEmpty()) return trimmed
        val atIndex = trimmed.lastIndexOf('@')
        return if (atIndex > 0) trimmed.substring(0, atIndex) else trimmed
    }

    private fun hex(bytes: ByteArray): String = buildString(bytes.size * 2) {
        for (byte in bytes) {
            append("%02x".format(byte.toInt() and 0xFF))
        }
    }

    private data class SignedParts(
        val signature: ByteArray,
        val payloadBytes: ByteArray,
    )

    companion object {
        private const val SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1"
        private const val VERSION_BYTE: Byte = 0x01
        private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
    }
}
