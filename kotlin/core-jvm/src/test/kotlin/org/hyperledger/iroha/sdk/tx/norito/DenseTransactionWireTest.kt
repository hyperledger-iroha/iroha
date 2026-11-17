package org.hyperledger.iroha.sdk.tx.norito

import java.util.Optional
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer as BcEd25519Signer
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.DenseWirePayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.norito.WireLayout
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

/**
 * The dense transaction fixtures (`fixtures/norito_rpc/transaction_fixtures.dense.json`) at the
 * wire level, through the SDK's layout-aware Norito primitives only.
 *
 * Nothing here builds an account address, so the suite runs without the native bridge. The
 * typed `TransactionPayload` path is covered by [TransactionDenseFixtureParityTest], which needs
 * native account admission like every other typed transaction test.
 */
class DenseTransactionWireTest {
    private val manifest = AndroidFixtureSupport.loadManifestFixtures()
    private val sources = AndroidFixtureSupport.loadPayloadFixtures().associateBy { it.name }
    private val dense = AndroidFixtureSupport.loadDenseFixtures()

    @Test
    fun `dense manifest carries every fixture transaction in manifest order`() {
        assertTrue(dense.isNotEmpty())
        assertEquals(manifest.map { it.name }, dense.map { it.name })
        assertEquals(sources.keys, dense.map { it.name }.toSet())
    }

    @Test
    fun `dense fixture loader requires the exact dense schema`() {
        val descriptor = mapOf(
            "name" to "fixture",
            "payload_base64" to "AA==",
            "signing_hash" to "ab".repeat(32),
            "signed_base64" to "AQ==",
        )
        assertEquals("fixture", AndroidFixtureSupport.denseFixtureFromValue(descriptor).name)
        for (field in descriptor.keys) {
            assertFailsWith<IllegalArgumentException>(field) {
                AndroidFixtureSupport.denseFixtureFromValue(descriptor - field)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.denseFixtureFromValue(descriptor + ("payload_hash" to "00"))
        }
        assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.denseFixtureFromValue(descriptor + ("signing_hash" to "AB".repeat(32)))
        }
    }

    @Test
    fun `every dense signed transaction decodes and re-encodes byte-identically`() {
        for ((fixture, twin) in dense.zip(manifest)) {
            val name = fixture.name
            val signedBytes = AndroidFixtureSupport.decodeCanonicalBase64(fixture.signedBase64, "$name.signed_base64")
            val payloadBytes = AndroidFixtureSupport.decodeCanonicalBase64(fixture.payloadBase64, "$name.payload_base64")

            val signed = NoritoCodec.decodeBare(signedBytes, SIGNED_WIRE, WireLayout.DENSE)
            assertContentEquals(payloadBytes, signed.payload, "$name: signed payload")
            assertEquals(64, signed.signature.size, "$name: signature length")
            assertFalse(signed.multisigSignatures.isPresent, "$name: multisig signatures")
            assertContentEquals(
                signedBytes,
                NoritoCodec.encodeBare(signed, SIGNED_WIRE, WireLayout.DENSE),
                "$name: signed transaction re-encoding",
            )

            // No nested instruction frame survives in the dense layout.
            assertEquals(-1, indexOf(signedBytes, NoritoHeader.MAGIC), "$name: nested Norito frame")
            val compactSigned = bareFrame(twin.signedBase64, "$name.signed")
            assertTrue(signedBytes.size < compactSigned.size, "$name: dense is smaller")

            // One layout's bytes are refused under the other, truncated or extended.
            assertRefused("$name: dense read as compact") {
                NoritoCodec.decodeBare(signedBytes, SIGNED_WIRE, WireLayout.COMPACT)
            }
            assertRefused("$name: compact read as dense") {
                NoritoCodec.decodeBare(compactSigned, SIGNED_WIRE, WireLayout.DENSE)
            }
            assertRefused("$name: truncated") {
                NoritoCodec.decodeBare(signedBytes.copyOf(signedBytes.size - 1), SIGNED_WIRE, WireLayout.DENSE)
            }
            assertRefused("$name: trailing byte") {
                NoritoCodec.decodeBare(signedBytes + byteArrayOf(0), SIGNED_WIRE, WireLayout.DENSE)
            }
        }
    }

    @Test
    fun `every dense payload decodes and re-encodes byte-identically and matches its current twin`() {
        var envelopes = 0
        for ((fixture, twin) in dense.zip(manifest)) {
            val name = fixture.name
            val payloadBytes = AndroidFixtureSupport.decodeCanonicalBase64(fixture.payloadBase64, "$name.payload_base64")
            val payload = NoritoCodec.decodeBare(payloadBytes, PAYLOAD_WIRE, WireLayout.DENSE)
            assertContentEquals(
                payloadBytes,
                NoritoCodec.encodeBare(payload, PAYLOAD_WIRE, WireLayout.DENSE),
                "$name: payload re-encoding",
            )

            // The dense payload is the same transaction as the current-layout fixture.
            assertContentEquals(HashLiteral.decode(twin.networkId), payload.networkId, "$name: network_id")
            assertEquals(twin.creationTimeMs, payload.creationTimeMs, "$name: creation_time_ms")
            assertEquals(Optional.of(twin.timeToLiveMs), payload.timeToLiveMs, "$name: time_to_live_ms")
            assertEquals(Optional.ofNullable(twin.nonce), payload.nonce, "$name: nonce")
            assertFalse(payload.attachments.isPresent, "$name: attachments")
            val compact = NoritoCodec.decodeBare(bareFrame(twin.payloadBase64, "$name.payload"), PAYLOAD_WIRE, WireLayout.COMPACT)
            assertContentEquals(compact.networkId, payload.networkId, "$name: twin network_id")
            assertContentEquals(compact.authorityKey, payload.authorityKey, "$name: twin authority")
            assertEquals(compact.creationTimeMs, payload.creationTimeMs)
            assertEquals(compact.timeToLiveMs, payload.timeToLiveMs)
            assertEquals(compact.nonce, payload.nonce)
            assertTrue(payloadBytes.size < bareFrame(twin.payloadBase64, "$name.payload").size, "$name: dense is smaller")

            envelopes += assertSameExecutable(name, compact.executable, payload.executable)
        }
        assertTrue(envelopes > 0, "fixtures must exercise instruction envelopes")
    }

    @Test
    fun `every dense transaction is signed over the hash of its dense payload`() {
        val key = Ed25519PrivateKeyParameters(FIXTURE_SIGNING_SEED, 0)
        val publicKey = key.generatePublicKey()
        for ((fixture, twin) in dense.zip(manifest)) {
            val name = fixture.name
            val signedBytes = AndroidFixtureSupport.decodeCanonicalBase64(fixture.signedBase64, "$name.signed_base64")
            val payloadBytes = AndroidFixtureSupport.decodeCanonicalBase64(fixture.payloadBase64, "$name.payload_base64")
            val signed = NoritoCodec.decodeBare(signedBytes, SIGNED_WIRE, WireLayout.DENSE)
            val payload = NoritoCodec.decodeBare(payloadBytes, PAYLOAD_WIRE, WireLayout.DENSE)

            // A payload's layout is part of its identity: the signing hash is the dense hash.
            val signingHash = IrohaHash.prehash(payloadBytes)
            assertEquals(fixture.signingHash, hex(signingHash), "$name: signing_hash")
            val compactHash = IrohaHash.prehash(bareFrame(twin.payloadBase64, "$name.payload"))
            assertNotEquals(fixture.signingHash, hex(compactHash), "$name: compact hash")

            assertContentEquals(publicKey.encoded, payload.authorityKey, "$name: fixture authority key")
            assertTrue(verify(publicKey, signingHash, signed.signature), "$name: dense signature")
            assertFalse(verify(publicKey, compactHash, signed.signature), "$name: dense signature over compact hash")

            // Ed25519 is deterministic: signing the dense hash with the fixture key reproduces
            // the fixture signature, and with it the exact signed transaction bytes.
            val signature = sign(key, signingHash)
            assertContentEquals(signed.signature, signature, "$name: reproduced signature")
            assertContentEquals(
                signedBytes,
                NoritoCodec.encodeBare(SignedWire(signature, payloadBytes, Optional.empty()), SIGNED_WIRE, WireLayout.DENSE),
                "$name: reproduced signed transaction",
            )
        }
    }

    @Test
    fun `typed transaction entry points refuse the other layout before any account work`() {
        val compactAdapter = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val denseAdapter = NoritoJavaCodecAdapter(
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            layout = WireLayout.DENSE,
        )
        assertEquals(WireLayout.COMPACT, compactAdapter.layout)
        assertEquals(WireLayout.DENSE, denseAdapter.layout)
        // No transaction wire uses fixed-width lengths.
        assertFailsWith<IllegalArgumentException> {
            NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT, layout = WireLayout.FIXED)
        }

        // The refusal is structural: it never reaches the native account admission.
        fun refusedByLayout(label: String, block: () -> Any?) {
            val error = assertFailsWith<NoritoException>(label) { block() }
            assertTrue(
                generateSequence<Throwable>(error) { it.cause }.none { it is AccountAddressException },
                "$label: refused by account admission, not by the layout",
            )
        }
        for ((fixture, twin) in dense.zip(manifest)) {
            val name = fixture.name
            val densePayload = AndroidFixtureSupport.decodeCanonicalBase64(fixture.payloadBase64, "$name.payload_base64")
            val denseSigned = AndroidFixtureSupport.decodeCanonicalBase64(fixture.signedBase64, "$name.signed_base64")
            val compactPayload = bareFrame(twin.payloadBase64, "$name.payload")
            val compactSigned = bareFrame(twin.signedBase64, "$name.signed")

            refusedByLayout("$name: dense payload read as compact") { compactAdapter.decodeTransaction(densePayload) }
            refusedByLayout("$name: compact payload read as dense") { denseAdapter.decodeTransaction(compactPayload) }
            refusedByLayout("$name: dense payload validated as compact") {
                NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(densePayload)
            }
            refusedByLayout("$name: compact payload validated as dense") {
                NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(compactPayload, WireLayout.DENSE)
            }
            refusedByLayout("$name: dense signed read as compact") { SignedTransactionEncoder.decode(denseSigned) }
            refusedByLayout("$name: compact signed read as dense") {
                SignedTransactionEncoder.decode(compactSigned, WireLayout.DENSE)
            }
            // The version byte names the current layout; there is no versioned dense form.
            refusedByLayout("$name: versioned dense") {
                SignedTransactionEncoder.decodeVersioned(byteArrayOf(0x01) + denseSigned)
            }
            refusedByLayout("$name: fixed-width signed transaction") {
                SignedTransactionEncoder.decode(compactSigned, WireLayout.FIXED)
            }
        }
    }

    @Test
    fun `dense instruction envelopes are opaque and distinct from framed payloads`() {
        val payload = byteArrayOf(0x0c, 0x00, 0x0a)
        val dense = InstructionBox.fromDenseWirePayload("iroha.set_parameter", payload)
        val envelope = assertIs<DenseWirePayload>(dense.payload)
        assertEquals("iroha.set_parameter", envelope.wireName)
        assertEquals("iroha.set_parameter", dense.name)
        assertContentEquals(payload, envelope.payloadBytes)
        // The envelope is no `WirePayload`: framed-payload consumers never see dense bytes.
        assertFalse(dense.payload is WirePayload)
        assertEquals(dense, InstructionBox.fromDenseWirePayload("iroha.set_parameter", payload))
        assertEquals(dense.hashCode(), InstructionBox.fromDenseWirePayload("iroha.set_parameter", payload).hashCode())
        assertNotEquals(dense, InstructionBox.fromWirePayload("iroha.set_parameter", payload))
        assertNotEquals(dense, InstructionBox.fromDenseWirePayload("iroha.set_parameter", byteArrayOf(0x0c)))
        assertNotEquals(dense, InstructionBox.fromDenseWirePayload("iroha.log", payload))

        // Bytes are copied in and out.
        payload[0] = 0x7f
        assertEquals(0x0c, envelope.payloadBytes[0].toInt())
        envelope.payloadBytes[0] = 0x7f
        assertEquals(0x0c, envelope.payloadBytes[0].toInt())
        // An instruction without fields has an empty dense payload.
        assertEquals(0, assertIs<DenseWirePayload>(InstructionBox.fromDenseWirePayload("iroha.unit", ByteArray(0)).payload).payloadBytes.size)

        // A caller-supplied envelope is snapshotted once.
        val supplied = object : DenseWirePayload {
            var calls = 0
            override val wireName = "iroha.custom"
            override val payloadBytes: ByteArray get() = byteArrayOf((++calls).toByte())
            override val kind get() = org.hyperledger.iroha.sdk.core.model.instructions.InstructionKind.CUSTOM
            override val arguments: Map<String, String> get() = emptyMap()
        }
        val snapshot = assertIs<DenseWirePayload>(InstructionBox.of(supplied).payload)
        assertContentEquals(snapshot.payloadBytes, snapshot.payloadBytes)

        // Wire IDs follow the envelope rule: 1 to 192 bytes of ASCII letters, digits and `._:-@/`.
        InstructionBox.fromDenseWirePayload("iroha.instruction.v1::a_b-c@d/e", byteArrayOf(1))
        InstructionBox.fromDenseWirePayload("a".repeat(192), byteArrayOf(1))
        for (malformed in listOf("", " ", "iroha.log ", "iroha log", "iroha.lög", "a".repeat(193), "iroha\u0000")) {
            assertFailsWith<IllegalArgumentException>(malformed) {
                InstructionBox.fromDenseWirePayload(malformed, byteArrayOf(1))
            }
        }
    }

    /** Compare executables structurally; returns the number of instruction envelopes checked. */
    private fun assertSameExecutable(name: String, compact: ExecutableWire, dense: ExecutableWire): Int {
        assertEquals(compact.tag, dense.tag, "$name: executable variant")
        assertContentEquals(compact.ivm, dense.ivm, "$name: IVM bytecode")
        assertEquals(compact.items.size, dense.items.size, "$name: executable item count")
        var envelopes = 0
        for ((index, pair) in compact.items.zip(dense.items).withIndex()) {
            val (framed, bare) = pair
            assertEquals(framed.tag, bare.tag, "$name[$index]: item variant")
            assertEquals(framed.call?.fields(), bare.call?.fields(), "$name[$index]: contract call")
            assertEquals(framed.envelope == null, bare.envelope == null, "$name[$index]: item kind")
            val compactEnvelope = framed.envelope ?: continue
            val denseEnvelope = bare.envelope!!
            assertEquals(compactEnvelope.wireName, denseEnvelope.wireName, "$name[$index]: wire ID")
            // The current layout nests a complete frame; the dense envelope carries the bare
            // dense payload, with no header, checksum or padding.
            val frame = NoritoHeader.decode(compactEnvelope.payload, null)
            frame.header.validateChecksum(frame.payload)
            assertEquals(-1, indexOf(denseEnvelope.payload, NoritoHeader.MAGIC), "$name[$index]: nested frame")
            assertTrue(denseEnvelope.payload.size <= frame.payload.size, "$name[$index]: dense payload size")
            envelopes++
        }
        return envelopes
    }

    private fun bareFrame(frameBase64: String, field: String): ByteArray {
        val frame = AndroidFixtureSupport.decodeCanonicalBase64(frameBase64, field)
        val decoded = NoritoHeader.decode(frame, null)
        decoded.header.validateChecksum(decoded.payload)
        return decoded.payload
    }

    private class SignedWire(
        val signature: ByteArray,
        val payload: ByteArray,
        val multisigSignatures: Optional<ByteArray>,
    )

    private class EnvelopeWire(val wireName: String, val payload: ByteArray)

    private class CallWire(
        val contractAddress: String,
        val expectedCodeHash: ByteArray,
        val entrypoint: String,
        val arguments: Optional<ByteArray>,
    ) {
        fun fields(): List<Any?> =
            listOf(contractAddress, hex(expectedCodeHash), entrypoint, arguments.map { hex(it) }.orElse(null))
    }

    /** One executable or batch item: an instruction envelope or a contract call. */
    private class ItemWire(val tag: Long, val envelope: EnvelopeWire?, val call: CallWire?)

    private class ExecutableWire(val tag: Long, val items: List<ItemWire>, val ivm: ByteArray?)

    private class PayloadWire(
        val networkId: ByteArray,
        val authorityKey: ByteArray,
        val creationTimeMs: Long,
        val executable: ExecutableWire,
        val timeToLiveMs: Optional<Long>,
        val nonce: Optional<Long>,
        val feePayment: ByteArray,
        val metadata: ByteArray,
        val attachments: Optional<ByteArray>,
    )

    private companion object {
        /** `android-fixture-signing-key-0102`, the seed of the Norito RPC fixture signing key. */
        private val FIXTURE_SIGNING_SEED =
            "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032".chunked(2)
                .map { it.toInt(16).toByte() }.toByteArray()

        private const val EXECUTABLE_INSTRUCTIONS = 0L
        private const val EXECUTABLE_IVM = 2L
        private const val EXECUTABLE_BATCH = 4L
        private const val BATCH_ITEM_INSTRUCTION = 0L
        private const val BATCH_ITEM_CONTRACT_CALL = 1L
        private const val ED25519_COMPACT_TAG = 0

        private val TAG = NoritoAdapters.enumTag()
        private val HASH = NoritoAdapters.fixedBytes(32)

        /** The uninterpreted rest of the enclosing field. */
        private val REST = object : TypeAdapter<ByteArray> {
            override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
            override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
        }

        /** A length-prefixed value: a record field, an enum variant field or a tuple member. */
        private fun <T> sized(inner: TypeAdapter<T>): TypeAdapter<T> = object : TypeAdapter<T> {
            override fun encode(encoder: NoritoEncoder, value: T) {
                val child = encoder.childEncoder()
                inner.encode(child, value)
                val payload = child.toByteArray()
                encoder.writeLength(payload.size.toLong(), (encoder.flags and NoritoHeader.COMPACT_LEN) != 0)
                encoder.writeBytes(payload)
            }

            override fun decode(decoder: NoritoDecoder): T {
                val length = decoder.readLength(decoder.compactLenActive())
                val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags)
                val value = inner.decode(child)
                require(child.remaining() == 0) { "Trailing bytes after field payload" }
                return value
            }
        }

        private val SIGNATURE = sized(sized(NoritoAdapters.byteVecAdapter()))
        private val RAW_FIELD = sized(REST)
        private val OPTIONAL_REST = sized(NoritoAdapters.option(REST))

        private val SIGNED_WIRE = object : TypeAdapter<SignedWire> {
            override fun encode(encoder: NoritoEncoder, value: SignedWire) {
                SIGNATURE.encode(encoder, value.signature)
                RAW_FIELD.encode(encoder, value.payload)
                OPTIONAL_REST.encode(encoder, value.multisigSignatures)
            }

            override fun decode(decoder: NoritoDecoder): SignedWire =
                SignedWire(SIGNATURE.decode(decoder), RAW_FIELD.decode(decoder), OPTIONAL_REST.decode(decoder))
        }

        /** The instruction envelope: `(wire_id, payload)`, a framed payload or a bare dense one. */
        private val ENVELOPE = object : TypeAdapter<EnvelopeWire> {
            private val wireName = sized(NoritoAdapters.stringAdapter())
            private val payload = sized(NoritoAdapters.rawByteVecAdapter())

            override fun encode(encoder: NoritoEncoder, value: EnvelopeWire) {
                wireName.encode(encoder, value.wireName)
                payload.encode(encoder, value.payload)
            }

            override fun decode(decoder: NoritoDecoder): EnvelopeWire =
                EnvelopeWire(wireName.decode(decoder), payload.decode(decoder))
        }

        private val INSTRUCTION_ITEM = object : TypeAdapter<ItemWire> {
            override fun encode(encoder: NoritoEncoder, value: ItemWire) = ENVELOPE.encode(encoder, value.envelope!!)
            override fun decode(decoder: NoritoDecoder): ItemWire =
                ItemWire(BATCH_ITEM_INSTRUCTION, ENVELOPE.decode(decoder), null)
        }

        private val CONTRACT_CALL = object : TypeAdapter<CallWire> {
            private val text = sized(NoritoAdapters.stringAdapter())
            private val hash = sized(HASH)
            private val arguments = sized(NoritoAdapters.option(NoritoAdapters.rawByteVecAdapter()))

            override fun encode(encoder: NoritoEncoder, value: CallWire) {
                text.encode(encoder, value.contractAddress)
                hash.encode(encoder, value.expectedCodeHash)
                text.encode(encoder, value.entrypoint)
                arguments.encode(encoder, value.arguments)
            }

            override fun decode(decoder: NoritoDecoder): CallWire =
                CallWire(text.decode(decoder), hash.decode(decoder), text.decode(decoder), arguments.decode(decoder))
        }

        private val BATCH_ITEM = object : TypeAdapter<ItemWire> {
            private val instruction = sized(ENVELOPE)
            private val call = sized(CONTRACT_CALL)

            override fun encode(encoder: NoritoEncoder, value: ItemWire) {
                TAG.encode(encoder, value.tag)
                if (value.envelope != null) instruction.encode(encoder, value.envelope) else call.encode(encoder, value.call!!)
            }

            override fun decode(decoder: NoritoDecoder): ItemWire = when (val tag = TAG.decode(decoder)) {
                BATCH_ITEM_INSTRUCTION -> ItemWire(tag, instruction.decode(decoder), null)
                BATCH_ITEM_CONTRACT_CALL -> ItemWire(tag, null, call.decode(decoder))
                else -> throw IllegalArgumentException("unknown batch item tag $tag")
            }
        }

        /** The executable shapes the fixtures use: instructions, IVM bytecode and mixed batches. */
        private val EXECUTABLE = object : TypeAdapter<ExecutableWire> {
            private val instructions = sized(NoritoAdapters.sequence(INSTRUCTION_ITEM))
            private val batch = sized(NoritoAdapters.sequence(BATCH_ITEM))
            private val ivm = sized(sized(NoritoAdapters.rawByteVecAdapter()))

            override fun encode(encoder: NoritoEncoder, value: ExecutableWire) {
                TAG.encode(encoder, value.tag)
                when (value.tag) {
                    EXECUTABLE_INSTRUCTIONS -> instructions.encode(encoder, value.items)
                    EXECUTABLE_BATCH -> batch.encode(encoder, value.items)
                    EXECUTABLE_IVM -> ivm.encode(encoder, value.ivm!!)
                    else -> throw IllegalArgumentException("fixture executable tag ${value.tag} is not covered")
                }
            }

            override fun decode(decoder: NoritoDecoder): ExecutableWire = when (val tag = TAG.decode(decoder)) {
                EXECUTABLE_INSTRUCTIONS -> ExecutableWire(tag, instructions.decode(decoder), null)
                EXECUTABLE_BATCH -> ExecutableWire(tag, batch.decode(decoder), null)
                EXECUTABLE_IVM -> ExecutableWire(tag, emptyList(), ivm.decode(decoder))
                else -> throw IllegalArgumentException("fixture executable tag $tag is not covered")
            }
        }

        private val PAYLOAD_WIRE = object : TypeAdapter<PayloadWire> {
            private val networkId = sized(object : TypeAdapter<ByteArray> {
                private val hash = sized(HASH)
                override fun encode(encoder: NoritoEncoder, value: ByteArray) {
                    TAG.encode(encoder, 0)
                    hash.encode(encoder, value)
                }
                override fun decode(decoder: NoritoDecoder): ByteArray {
                    require(TAG.decode(decoder) == 0L) { "transaction domain must be a network" }
                    return hash.decode(decoder)
                }
            })
            private val authority = sized(object : TypeAdapter<ByteArray> {
                private val key = sized(NoritoAdapters.byteVecAdapter())
                override fun encode(encoder: NoritoEncoder, value: ByteArray) {
                    TAG.encode(encoder, 0)
                    key.encode(encoder, byteArrayOf(ED25519_COMPACT_TAG.toByte()) + value)
                }
                override fun decode(decoder: NoritoDecoder): ByteArray {
                    require(TAG.decode(decoder) == 0L) { "fixture authority must be a single key" }
                    val compactKey = key.decode(decoder)
                    require(compactKey.size == 33 && compactKey[0].toInt() == ED25519_COMPACT_TAG) {
                        "fixture authority must be an Ed25519 key"
                    }
                    return compactKey.copyOfRange(1, compactKey.size)
                }
            })
            private val creationTimeMs = sized(NoritoAdapters.uint(64))
            private val executable = sized(EXECUTABLE)
            private val timeToLiveMs = sized(NoritoAdapters.option(NoritoAdapters.uint(64)))
            private val nonce = sized(NoritoAdapters.option(NoritoAdapters.uint(32)))

            override fun encode(encoder: NoritoEncoder, value: PayloadWire) {
                networkId.encode(encoder, value.networkId)
                authority.encode(encoder, value.authorityKey)
                creationTimeMs.encode(encoder, value.creationTimeMs)
                executable.encode(encoder, value.executable)
                timeToLiveMs.encode(encoder, value.timeToLiveMs)
                nonce.encode(encoder, value.nonce)
                RAW_FIELD.encode(encoder, value.feePayment)
                RAW_FIELD.encode(encoder, value.metadata)
                OPTIONAL_REST.encode(encoder, value.attachments)
            }

            override fun decode(decoder: NoritoDecoder): PayloadWire = PayloadWire(
                networkId.decode(decoder),
                authority.decode(decoder),
                creationTimeMs.decode(decoder),
                executable.decode(decoder),
                timeToLiveMs.decode(decoder),
                nonce.decode(decoder),
                RAW_FIELD.decode(decoder),
                RAW_FIELD.decode(decoder),
                OPTIONAL_REST.decode(decoder),
            )
        }

        private fun sign(key: Ed25519PrivateKeyParameters, message: ByteArray): ByteArray = BcEd25519Signer().run {
            init(true, key)
            update(message, 0, message.size)
            generateSignature()
        }

        private fun verify(key: Ed25519PublicKeyParameters, message: ByteArray, signature: ByteArray): Boolean =
            BcEd25519Signer().run {
                init(false, key)
                update(message, 0, message.size)
                verifySignature(signature)
            }

        private fun assertRefused(label: String, block: () -> Any?) {
            assertFailsWith<RuntimeException>(label) { block() }
        }

        private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }

        private fun indexOf(haystack: ByteArray, needle: ByteArray): Int {
            for (start in 0..haystack.size - needle.size) {
                if (needle.indices.all { haystack[start + it] == needle[it] }) return start
            }
            return -1
        }

    }
}
