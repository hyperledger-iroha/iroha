package org.hyperledger.iroha.sdk.tx.norito

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.PublicKeyPayload
import org.hyperledger.iroha.sdk.address.algorithmForCurveId
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.MultisigSignature
import org.hyperledger.iroha.sdk.tx.MultisigSignatures
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NoritoJavaCodecAdapterParityTest {
    private val adapter = NoritoJavaCodecAdapter()

    @Test
    fun `codec round-trips payload as bare payload`() {
        val instructions = "android-instructions".toByteArray()
        val payload = TransactionPayload(
            chainId = "00000001",
            authority = sampleAuthority(0x11),
            creationTimeMs = 1_735_000_000_123L,
            executable = Executable.ivm(instructions),
            timeToLiveMs = 5_000L,
            nonce = 42,
            metadata = mapOf("purpose" to "unit-test"),
        )

        val encoded = adapter.encodeTransaction(payload)
        val decoded = adapter.decodeTransaction(encoded)

        assertEquals(payload.chainId, decoded.chainId)
        assertEquals(payload.authority, decoded.authority)
        assertEquals(payload.creationTimeMs, decoded.creationTimeMs)
        assertContentEquals(instructions, (decoded.executable as Executable.Ivm).ivmBytes)
        assertEquals(payload.timeToLiveMs, decoded.timeToLiveMs)
        assertEquals(payload.nonce, decoded.nonce)
        assertEquals(payload.metadata, decoded.metadata)
        assertBarePayload(encoded)
    }

    @Test
    fun `codec encodes account id authority as struct`() {
        val publicKey = ByteArray(32) { 0x3A.toByte() }
        val authority = AccountAddress
            .fromAccount(publicKey, "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val payload = TransactionPayload(
            chainId = "00000002",
            authority = authority,
            creationTimeMs = 1_735_000_000_456L,
            executable = Executable.ivm(byteArrayOf(0x01, 0x02, 0x03)),
        )

        val encoded = adapter.encodeTransaction(payload)
        val decoded = adapter.decodeTransaction(encoded)
        assertEquals(authority, decoded.authority)

        val decoder = canonicalDecoder(encoded)
        readField(decoder, "payload.chain_id")
        val authorityField = readField(decoder, "payload.authority")
        val expectedStringPayloadLen = 8 + authority.toByteArray(StandardCharsets.UTF_8).size
        assertFalse(authorityField.size == expectedStringPayloadLen, "authority must not use string layout")

        val authorityDecoder = canonicalDecoder(authorityField)
        val controllerTag = NoritoAdapters.uint(32).decode(authorityDecoder)
        assertEquals(0L, controllerTag)
        val publicKeyField = readField(authorityDecoder, "authority.controller.public_key")
        val publicKeyLiteral = decodeFieldPayload(publicKeyField, NoritoAdapters.stringAdapter(), "authority.controller.public_key")
        assertEquals(encodePublicKeyMultihash(0x01, publicKey), publicKeyLiteral)
        assertEquals(0, authorityDecoder.remaining())
    }

    @Test
    fun `codec encodes multisig authority and signatures`() {
        val memberKeyA = ByteArray(32) { 0x11.toByte() }
        val memberKeyB = ByteArray(32) { 0x22.toByte() }
        val memberA = MultisigMemberPayload(0x01, 1, memberKeyA)
        val memberB = MultisigMemberPayload(0x01, 2, memberKeyB)
        val policy = MultisigPolicyPayload.of(1, 2, listOf(memberA, memberB))
        val authority = AccountAddress
            .fromMultisigPolicy(policy)
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

        val payload = TransactionPayload(
            chainId = "00000003",
            authority = authority,
            creationTimeMs = 1_735_000_000_789L,
            executable = Executable.ivm(byteArrayOf(0x0A, 0x0B)),
        )
        val encodedPayload = adapter.encodeTransaction(payload)
        val sigA = MultisigSignature.fromCurveId(0x01, fill(0x11, 32), fill(0x22, 64))
        val sigBKeyLiteral = encodePublicKeyMultihash(0x01, fill(0x33, 32))
        val sigB = MultisigSignature.fromPublicKeyLiteral(sigBKeyLiteral, fill(0x44, 64))
        val signed = SignedTransaction(encodedPayload, fill(0x44, 64), fill(0x55, 32), adapter.schemaName())
            .toBuilder()
            .setMultisigSignatures(MultisigSignatures.of(listOf(sigA, sigB)))
            .build()

        val encodedAuthorityPayload = adapter.encodeTransaction(payload)
        val decodedAuthorityPayload = adapter.decodeTransaction(encodedAuthorityPayload)
        assertEquals(authority, decodedAuthorityPayload.authority)

        val authorityDecoder = canonicalDecoder(encodedAuthorityPayload)
        readField(authorityDecoder, "payload.chain_id")
        val authorityField = readField(authorityDecoder, "payload.authority")
        val controllerDecoder = canonicalDecoder(authorityField)
        val controllerTag = NoritoAdapters.uint(32).decode(controllerDecoder)
        assertEquals(1L, controllerTag)
        val policyField = readField(controllerDecoder, "authority.controller.policy")
        assertEquals(0, controllerDecoder.remaining())

        val policyDecoder = canonicalDecoder(policyField)
        val version = NoritoAdapters.uint(8).decode(policyDecoder).toInt()
        val threshold = NoritoAdapters.uint(16).decode(policyDecoder).toInt()
        val memberCount = policyDecoder.readLength(false)
        assertEquals(1, version)
        assertEquals(2, threshold)
        assertEquals(2L, memberCount)

        assertMultisigMember(
            policyDecoder,
            encodePublicKeyMultihash(0x01, memberKeyA),
            1,
            "member[0]",
        )
        assertMultisigMember(
            policyDecoder,
            encodePublicKeyMultihash(0x01, memberKeyB),
            2,
            "member[1]",
        )
        assertEquals(0, policyDecoder.remaining())

        val encodedSigned = SignedTransactionEncoder.encode(signed)
        val signedDecoder = canonicalDecoder(encodedSigned)
        readField(signedDecoder, "signed.signature")
        readField(signedDecoder, "signed.payload")
        val attachmentsField = readField(signedDecoder, "signed.attachments")
        val multisigField = readField(signedDecoder, "signed.multisig_signatures")
        assertNull(decodeOptionPayload(attachmentsField, "signed.attachments"))
        val multisigPayload = assertNotNull(decodeOptionPayload(multisigField, "signed.multisig_signatures"))
        assertEquals(0, signedDecoder.remaining())

        val multisigDecoder = canonicalDecoder(multisigPayload)
        val count = multisigDecoder.readLength(false)
        assertEquals(2L, count)
        val compact = multisigDecoder.compactLenActive()
        assertMultisigSignaturePayload(
            canonicalDecoder(readSequenceElement(multisigDecoder, compact, "multisig[0]")),
            sigA,
            "multisig[0]",
        )
        assertMultisigSignaturePayload(
            canonicalDecoder(readSequenceElement(multisigDecoder, compact, "multisig[1]")),
            sigB,
            "multisig[1]",
        )
        assertEquals(0, multisigDecoder.remaining())
    }

    @Test
    fun `codec supports instructions and wire payload variants`() {
        val wirePayloadA = NoritoCodec.encode(
            "wire-A",
            "iroha.test.WirePayload",
            NoritoAdapters.stringAdapter(),
        )
        val wirePayloadB = NoritoCodec.encode(
            "wire-B",
            "iroha.test.WirePayload",
            NoritoAdapters.stringAdapter(),
        )
        val payload = TransactionPayload(
            chainId = "00000009",
            authority = sampleAuthority(0x41),
            creationTimeMs = 1_735_111_111_000L,
            executable = Executable.instructions(
                listOf(
                    InstructionBox.fromWirePayload("iroha.custom.a", wirePayloadA),
                    InstructionBox.fromWirePayload("iroha.custom.b", wirePayloadB),
                ),
            ),
        )

        val encoded = adapter.encodeTransaction(payload)
        val decoded = adapter.decodeTransaction(encoded)
        val instructions = (decoded.executable as Executable.Instructions).instructions
        assertEquals(2, instructions.size)

        val first = assertIs<WirePayload>(instructions[0].payload)
        assertEquals("iroha.custom.a", first.wireName)
        assertContentEquals(wirePayloadA, first.payloadBytes)

        val second = assertIs<WirePayload>(instructions[1].payload)
        assertEquals("iroha.custom.b", second.wireName)
        assertContentEquals(wirePayloadB, second.payloadBytes)
    }

    @Test
    fun `codec encodes chain id ivm and instruction layouts`() {
        val chainId = "00000003"
        val chainPayload = TransactionPayload(
            chainId = chainId,
            authority = sampleAuthority(0x42),
            creationTimeMs = 1_735_000_000_789L,
            executable = Executable.ivm(byteArrayOf(0x01)),
        )
        val chainEncoded = adapter.encodeTransaction(chainPayload)
        val chainDecoder = canonicalDecoder(chainEncoded)
        val chainField = readField(chainDecoder, "payload.chain_id")
        val chainPayloadDecoder = canonicalDecoder(chainField)
        val stringField = readField(chainPayloadDecoder, "payload.chain_id.string")
        assertEquals(0, chainPayloadDecoder.remaining())
        val decodedChain = decodeFieldPayload(stringField, NoritoAdapters.stringAdapter(), "payload.chain_id.string")
        assertEquals(chainId, decodedChain)

        val ivmBytes = byteArrayOf(0x01, 0x02, 0x03, 0x04)
        val ivmPayload = TransactionPayload(
            chainId = "00000012",
            authority = sampleAuthority(0x43),
            creationTimeMs = 1_735_222_222_123L,
            executable = Executable.ivm(ivmBytes),
        )
        val ivmEncoded = adapter.encodeTransaction(ivmPayload)
        val ivmDecoder = canonicalDecoder(ivmEncoded)
        readField(ivmDecoder, "payload.chain_id")
        readField(ivmDecoder, "payload.authority")
        readField(ivmDecoder, "payload.creation_time_ms")
        val ivmExecutableField = readField(ivmDecoder, "payload.executable")
        readField(ivmDecoder, "payload.time_to_live_ms")
        readField(ivmDecoder, "payload.nonce")
        readField(ivmDecoder, "payload.metadata")
        assertEquals(0, ivmDecoder.remaining())

        val executableDecoder = canonicalDecoder(ivmExecutableField)
        assertEquals(2L, NoritoAdapters.uint(32).decode(executableDecoder))
        val ivmField = readField(executableDecoder, "payload.executable.ivm")
        assertEquals(0, executableDecoder.remaining())
        val ivmFieldDecoder = canonicalDecoder(ivmField)
        val ivmPayloadBytes = readField(ivmFieldDecoder, "payload.executable.ivm.bytes")
        assertEquals(0, ivmFieldDecoder.remaining())
        val decodedIvm = decodeFieldPayload(
            ivmPayloadBytes,
            RAW_BYTE_VECTOR_ADAPTER,
            "payload.executable.ivm.bytes",
        )
        assertContentEquals(ivmBytes, decodedIvm)

        val wirePayload = NoritoCodec.encode(
            "layout",
            "iroha.test.Layout",
            NoritoAdapters.stringAdapter(),
        )
        val instructionPayload = TransactionPayload(
            chainId = "00000013",
            authority = sampleAuthority(0x44),
            creationTimeMs = 1_735_222_333_123L,
            executable = Executable.instructions(
                listOf(InstructionBox.fromWirePayload("iroha.custom.layout", wirePayload)),
            ),
        )
        val instructionEncoded = adapter.encodeTransaction(instructionPayload)
        val instructionDecoder = canonicalDecoder(instructionEncoded)
        readField(instructionDecoder, "payload.chain_id")
        readField(instructionDecoder, "payload.authority")
        readField(instructionDecoder, "payload.creation_time_ms")
        val instructionExecutableField = readField(instructionDecoder, "payload.executable")
        readField(instructionDecoder, "payload.time_to_live_ms")
        readField(instructionDecoder, "payload.nonce")
        readField(instructionDecoder, "payload.metadata")
        assertEquals(0, instructionDecoder.remaining())

        val listFieldDecoder = canonicalDecoder(instructionExecutableField)
        assertEquals(0L, NoritoAdapters.uint(32).decode(listFieldDecoder))
        val instructionsField = readField(listFieldDecoder, "payload.executable.instructions")
        assertEquals(0, listFieldDecoder.remaining())

        val elementDecoder = canonicalDecoder(instructionsField)
        assertEquals(1L, elementDecoder.readLength(false))
        val elementLength = elementDecoder.readLength(elementDecoder.compactLenActive())
        val elementPayload = elementDecoder.readBytes(elementLength.toInt())
        assertEquals(0, elementDecoder.remaining())

        val payloadDecoder = canonicalDecoder(elementPayload)
        val nameField = readField(payloadDecoder, "instruction.name")
        val payloadField = readField(payloadDecoder, "instruction.payload")
        assertEquals(0, payloadDecoder.remaining())
        val decodedName = decodeFieldPayload(nameField, NoritoAdapters.stringAdapter(), "instruction.name")
        val decodedPayload = decodeFieldPayload(payloadField, RAW_BYTE_VECTOR_ADAPTER, "instruction.payload")
        assertEquals("iroha.custom.layout", decodedName)
        assertContentEquals(wirePayload, decodedPayload)
    }

    private fun readField(decoder: NoritoDecoder, field: String): ByteArray {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$field length too large: $length" }
        return decoder.readBytes(length.toInt())
    }

    private fun readSequenceElement(decoder: NoritoDecoder, compact: Boolean, field: String): ByteArray {
        val length = decoder.readLength(compact)
        require(length <= Int.MAX_VALUE) { "$field length too large: $length" }
        return decoder.readBytes(length.toInt())
    }

    private fun canonicalDecoder(payload: ByteArray): NoritoDecoder =
        NoritoDecoder(payload, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)

    private fun readU64(payload: ByteArray, offset: Int, field: String): Long {
        require(offset >= 0 && payload.size - offset >= 8) { "$field missing u64 payload" }
        var value = 0L
        for (i in 0 until 8) {
            value = value or ((payload[offset + i].toLong() and 0xFFL) shl (8 * i))
        }
        return value
    }

    private fun readU32(payload: ByteArray, offset: Int, field: String): Long {
        require(offset >= 0 && payload.size - offset >= 4) { "$field missing u32 payload" }
        var value = 0L
        for (i in 0 until 4) {
            value = value or ((payload[offset + i].toLong() and 0xFFL) shl (8 * i))
        }
        return value
    }

    private fun readU16(payload: ByteArray, offset: Int, field: String): Int {
        require(offset >= 0 && payload.size - offset >= 2) { "$field missing u16 payload" }
        return (payload[offset].toInt() and 0xFF) or ((payload[offset + 1].toInt() and 0xFF) shl 8)
    }

    private fun assertMultisigMember(
        decoder: NoritoDecoder,
        expectedPublicKey: String,
        expectedWeight: Int,
        label: String,
    ) {
        val memberPayload = readSequenceElement(decoder, decoder.compactLenActive(), label)
        val memberDecoder = canonicalDecoder(memberPayload)
        val publicKey = NoritoAdapters.stringAdapter().decode(memberDecoder)
        val weight = NoritoAdapters.uint(16).decode(memberDecoder).toInt()
        assertEquals(0, memberDecoder.remaining(), "$label payload should not have trailing bytes")
        assertEquals(expectedPublicKey, publicKey)
        assertEquals(expectedWeight, weight)
    }

    private fun <T> decodeFieldPayload(payload: ByteArray, adapter: TypeAdapter<T>, field: String): T {
        val decoder = canonicalDecoder(payload)
        val value = adapter.decode(decoder)
        require(decoder.remaining() == 0) { "$field has trailing bytes" }
        return value
    }

    private fun assertBarePayload(encoded: ByteArray) {
        if (encoded.size < 4) return
        val hasMagic = encoded[0] == 'N'.code.toByte() &&
            encoded[1] == 'R'.code.toByte() &&
            encoded[2] == 'T'.code.toByte() &&
            encoded[3] == '0'.code.toByte()
        assertFalse(hasMagic, "encoded payload should be bare")
    }

    private fun decodeOptionPayload(payload: ByteArray, field: String): ByteArray? {
        val decoder = canonicalDecoder(payload)
        val tag = decoder.readByte()
        return when (tag) {
            0 -> {
                require(decoder.remaining() == 0) { "$field Option::None has trailing bytes" }
                null
            }

            1 -> {
                val length = decoder.readLength(decoder.compactLenActive())
                require(length <= Int.MAX_VALUE) { "$field Option payload too large" }
                val inner = decoder.readBytes(length.toInt())
                require(decoder.remaining() == 0) { "$field Option payload has trailing bytes" }
                inner
            }

            else -> error("$field invalid Option tag: $tag")
        }
    }

    private fun assertMultisigSignaturePayload(
        decoder: NoritoDecoder,
        signature: MultisigSignature,
        field: String,
    ) {
        val publicKeyPayload = BYTE_VECTOR_ADAPTER.decode(decoder)
        val signaturePayload = BYTE_VECTOR_ADAPTER.decode(decoder)
        assertEquals(signature.publicKey().size + 1, publicKeyPayload.size, "$field public key payload length mismatch")
        assertEquals(signature.algorithmTag, publicKeyPayload[0].toInt() and 0xFF, "$field algorithm tag mismatch")
        assertContentEquals(signature.publicKey(), publicKeyPayload.copyOfRange(1, publicKeyPayload.size))
        assertContentEquals(signature.signature(), signaturePayload, "$field signature payload mismatch")
        assertEquals(0, decoder.remaining(), "$field payload should not have trailing bytes")
    }

    private fun fill(value: Int, length: Int): ByteArray = ByteArray(length) { value.toByte() }

    private fun sampleAuthority(fill: Int): String = AccountAddress
        .fromAccount(ByteArray(32) { fill.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    companion object {
        private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
        private val RAW_BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.rawByteVecAdapter()
    }
}
