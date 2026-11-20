package org.hyperledger.iroha.sdk.tx.norito

import java.security.KeyFactory
import java.security.spec.PKCS8EncodedKeySpec
import java.security.spec.X509EncodedKeySpec
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.ActionAuthorityBindingV1
import org.hyperledger.iroha.sdk.core.model.ActionIntentV1
import org.hyperledger.iroha.sdk.core.model.ActionInvocationBindingV1
import org.hyperledger.iroha.sdk.core.model.ActionOperationV1
import org.hyperledger.iroha.sdk.core.model.ActionPublicInputsDigestV1
import org.hyperledger.iroha.sdk.core.model.ActionRoleV1
import org.hyperledger.iroha.sdk.core.model.ActionStatementDigestV1
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.ExecutableBatchItem
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.SignedAttachmentsV1
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.core.model.instructions.UpdatePlainConvictionWirePayloadEncoder
import org.hyperledger.iroha.sdk.crypto.Ed25519Signer
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.WireLayout
import org.hyperledger.iroha.sdk.privacy.SubmitPrivacyZkX509CertificateV1
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.tx.MultisigSignature
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.TransactionBuilder
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The typed transaction codec in the dense Norito layout, against
 * `fixtures/norito_rpc/transaction_fixtures.dense.json`.
 *
 * The same `TransactionPayload` codec that round-trips the current-layout fixtures
 * ([TransactionFixtureParityTest]) runs here with [WireLayout.DENSE]. Like that suite, it builds
 * account addresses, and its instruction bodies are transcoded by the native bridge, so it needs
 * the same-source bridge; [DenseTransactionWireTest] covers the same fixtures without it.
 */
class TransactionDenseFixtureParityTest {
    private val compactAdapter = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
    private val denseAdapter = NoritoJavaCodecAdapter(
        AccountAddress.DEFAULT_I105_DISCRIMINANT,
        layout = WireLayout.DENSE,
    )
    private val manifest = AndroidFixtureSupport.loadManifestFixtures().associateBy { it.name }
    private val sources = AndroidFixtureSupport.loadPayloadFixtures().associateBy { it.name }
    private val dense = AndroidFixtureSupport.loadDenseFixtures()

    @Test
    fun `dense payload fixtures decode and re-encode byte-identically with the typed codec`() {
        var envelopes = 0
        for (fixture in dense) {
            val name = fixture.name
            val payloadBytes = payloadBytes(fixture)
            val payload = denseAdapter.decodeTransaction(payloadBytes)
            assertContentEquals(payloadBytes, denseAdapter.encodeTransaction(payload), "$name: dense re-encoding")
            NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(payloadBytes, WireLayout.DENSE)
            assertEquals(payload, denseAdapter.decodeTransaction(denseAdapter.encodeTransaction(payload)), name)

            // The decoded value is the transaction of the current-layout fixture.
            val source = sources.getValue(name).materializePayload()
            assertEquals(source.networkId, payload.networkId, "$name: network_id")
            assertEquals(source.authority, payload.authority, "$name: authority")
            assertEquals(source.creationTimeMs, payload.creationTimeMs, "$name: creation_time_ms")
            assertEquals(source.timeToLiveMs, payload.timeToLiveMs, "$name: time_to_live_ms")
            assertEquals(source.nonce, payload.nonce, "$name: nonce")
            assertEquals(source.feePayment, payload.feePayment, "$name: fee_payment")
            assertEquals(source.metadata, payload.metadata, "$name: metadata")
            assertEquals(null, payload.attachments, "$name: attachments")
            envelopes += assertSameExecutable(name, source.executable, payload.executable)

            // The hash a dense transaction is signed over is the hash of its dense payload.
            assertEquals(fixture.signingHash, hex(IrohaHash.prehash(denseAdapter.encodeTransaction(payload))), name)
        }
        assertTrue(envelopes > 0, "fixtures must exercise instruction envelopes")
    }

    @Test
    fun `dense signed transaction fixtures decode and re-encode byte-identically`() {
        for (fixture in dense) {
            val name = fixture.name
            val signedBytes = signedBytes(fixture)
            val signed = SignedTransactionEncoder.decode(signedBytes, WireLayout.DENSE)
            assertContentEquals(payloadBytes(fixture), signed.encodedPayload(), "$name: signed payload")
            assertEquals(64, signed.signature().size, "$name: signature length")
            assertFalse(signed.multisigSignatures().isPresent, "$name: multisig signatures")
            assertContentEquals(
                signedBytes,
                SignedTransactionEncoder.encode(signed, WireLayout.DENSE),
                "$name: signed re-encoding",
            )
            assertEquals(fixture.signingHash, hex(IrohaHash.prehash(signed.encodedPayload())), "$name: signing_hash")
        }
    }

    @Test
    fun `a dense build of the fixture inputs reproduces the fixture bytes`() {
        val signer = fixtureSigner()
        var builtFromDescriptorAlone = 0
        for (fixture in dense) {
            val name = fixture.name
            val source = sources.getValue(name).materializePayload()
            val decoded = denseAdapter.decodeTransaction(payloadBytes(fixture))
            // Every field, including each typed instruction frame, comes from the fixture
            // descriptor; the bridge transcodes the frames into their bare dense payloads.
            val payload = TransactionPayload(
                networkId = source.networkId,
                authority = source.authority,
                creationTimeMs = source.creationTimeMs,
                executable = source.executable,
                timeToLiveMs = source.timeToLiveMs,
                nonce = source.nonce,
                feePayment = source.feePayment,
                metadata = source.metadata,
            )
            assertEquals(decoded, payload, "$name: decoded dense payload")
            builtFromDescriptorAlone++

            val signed = TransactionBuilder(denseAdapter).encodeAndSign(payload, signer)
            assertContentEquals(payloadBytes(fixture), signed.encodedPayload(), "$name: built dense payload")
            assertContentEquals(
                signedBytes(fixture),
                SignedTransactionEncoder.encode(signed, WireLayout.DENSE),
                "$name: built dense signed transaction",
            )
            // The same key signs a different hash in the current layout.
            val compactSignature = SignedTransactionEncoder
                .decode(bare(manifest.getValue(name).signedBase64, SIGNED_TRANSACTION_TYPE))
                .signature()
            assertFalse(compactSignature.contentEquals(signed.signature()), "$name: layout-bound signature")
        }
        assertEquals(dense.size, builtFromDescriptorAlone, "every fixture is built from its descriptor alone")
    }

    @Test
    fun `current layout is unchanged by an explicit compact selection`() {
        val explicit = NoritoJavaCodecAdapter(
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            layout = WireLayout.COMPACT,
        )
        assertEquals(WireLayout.COMPACT, compactAdapter.layout)
        assertEquals(WireLayout.DENSE, denseAdapter.layout)
        for (fixture in dense) {
            val name = fixture.name
            val twin = manifest.getValue(name)
            val compactPayload = bare(twin.payloadBase64, TRANSACTION_PAYLOAD_TYPE)
            val payload = compactAdapter.decodeTransaction(compactPayload)
            assertContentEquals(compactPayload, explicit.encodeTransaction(payload), "$name: compact payload")
            assertEquals(payload, explicit.decodeTransaction(compactPayload), "$name: compact decode")
            NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(compactPayload, WireLayout.COMPACT)

            val compactSigned = bare(twin.signedBase64, SIGNED_TRANSACTION_TYPE)
            val signed = SignedTransactionEncoder.decode(compactSigned, WireLayout.COMPACT)
            assertContentEquals(compactSigned, SignedTransactionEncoder.encode(signed), "$name: compact signed")
            assertContentEquals(
                compactSigned,
                SignedTransactionEncoder.encode(signed, WireLayout.COMPACT),
                "$name: explicit compact signed",
            )
        }
    }

    @Test
    fun `payload bytes of one layout are refused by the other`() {
        for (fixture in dense) {
            val name = fixture.name
            val densePayload = payloadBytes(fixture)
            val compactPayload = bare(manifest.getValue(name).payloadBase64, TRANSACTION_PAYLOAD_TYPE)

            assertFailsWith<NoritoException>("$name: dense read as compact") {
                compactAdapter.decodeTransaction(densePayload)
            }
            assertFailsWith<NoritoException>("$name: compact read as dense") {
                denseAdapter.decodeTransaction(compactPayload)
            }
            assertFailsWith<NoritoException>("$name: dense validated as compact") {
                NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(densePayload)
            }
            assertFailsWith<NoritoException>("$name: compact validated as dense") {
                NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(compactPayload, WireLayout.DENSE)
            }
            assertFailsWith<NoritoException>("$name: truncated") {
                denseAdapter.decodeTransaction(densePayload.copyOf(densePayload.size - 1))
            }
            assertFailsWith<NoritoException>("$name: trailing byte") {
                denseAdapter.decodeTransaction(densePayload + byteArrayOf(0))
            }
        }
    }

    @Test
    fun `signed transaction bytes of one layout are refused by the other`() {
        for (fixture in dense) {
            val name = fixture.name
            val denseSigned = signedBytes(fixture)
            val compactSigned = bare(manifest.getValue(name).signedBase64, SIGNED_TRANSACTION_TYPE)

            assertFailsWith<NoritoException>("$name: dense read as compact") {
                SignedTransactionEncoder.decode(denseSigned)
            }
            assertFailsWith<NoritoException>("$name: compact read as dense") {
                SignedTransactionEncoder.decode(compactSigned, WireLayout.DENSE)
            }
            // The version byte names the current layout; there is no versioned dense form.
            assertFailsWith<NoritoException>("$name: versioned dense") {
                SignedTransactionEncoder.decodeVersioned(byteArrayOf(0x01) + denseSigned)
            }
            assertFailsWith<NoritoException>("$name: truncated") {
                SignedTransactionEncoder.decode(denseSigned.copyOf(denseSigned.size - 1), WireLayout.DENSE)
            }
            assertFailsWith<NoritoException>("$name: trailing byte") {
                SignedTransactionEncoder.decode(denseSigned + byteArrayOf(0), WireLayout.DENSE)
            }

            // A signed transaction encodes only in the layout of its payload bytes.
            val denseTransaction = SignedTransactionEncoder.decode(denseSigned, WireLayout.DENSE)
            assertFailsWith<NoritoException>("$name: dense payload encoded as compact") {
                SignedTransactionEncoder.encode(denseTransaction)
            }
            assertFailsWith<NoritoException>("$name: dense payload in the versioned form") {
                SignedTransactionEncoder.encodeVersioned(denseTransaction)
            }
            val compactTransaction = SignedTransactionEncoder.decode(compactSigned)
            assertFailsWith<NoritoException>("$name: compact payload encoded as dense") {
                SignedTransactionEncoder.encode(compactTransaction, WireLayout.DENSE)
            }
            assertFailsWith<NoritoException>("$name: fixed-width transaction") {
                SignedTransactionEncoder.encode(compactTransaction, WireLayout.FIXED)
            }
            assertFailsWith<NoritoException>("$name: fixed-width transaction decode") {
                SignedTransactionEncoder.decode(compactSigned, WireLayout.FIXED)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT, layout = WireLayout.FIXED)
        }
    }

    @Test
    fun `framed dense payloads are selected by the header minor`() {
        val fixture = dense.first()
        val densePayload = payloadBytes(fixture)
        val schema = denseAdapter.schemaName()
        fun framed(minor: Int, flags: Int, payload: ByteArray): ByteArray = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            flags,
            NoritoHeader.COMPRESSION_NONE,
            minor,
        ).encode() + payload

        val denseFrame = framed(NoritoHeader.MINOR_VERSION_DENSE, 0, densePayload)
        assertEquals(denseAdapter.decodeTransaction(densePayload), denseAdapter.decodeTransaction(denseFrame))
        // A dense frame is never read by the current-layout codec, and the reverse.
        assertFailsWith<NoritoException> { compactAdapter.decodeTransaction(denseFrame) }
        val compactPayload = bare(manifest.getValue(fixture.name).payloadBase64, TRANSACTION_PAYLOAD_TYPE)
        val compactFrame = framed(NoritoHeader.MINOR_VERSION, NoritoHeader.COMPACT_LEN, compactPayload)
        assertEquals(compactAdapter.decodeTransaction(compactPayload), compactAdapter.decodeTransaction(compactFrame))
        assertFailsWith<NoritoException> { denseAdapter.decodeTransaction(compactFrame) }
        // The header decides: a compact payload under a dense header is refused, not guessed.
        assertFailsWith<NoritoException> {
            denseAdapter.decodeTransaction(framed(NoritoHeader.MINOR_VERSION_DENSE, 0, compactPayload))
        }
        assertFailsWith<NoritoException> {
            compactAdapter.decodeTransaction(framed(NoritoHeader.MINOR_VERSION, NoritoHeader.COMPACT_LEN, densePayload))
        }
        // An unknown minor version is refused by both.
        val future = denseFrame.copyOf().also { it[5] = 2 }
        assertFailsWith<NoritoException> { denseAdapter.decodeTransaction(future) }
        assertFailsWith<NoritoException> { compactAdapter.decodeTransaction(future) }
    }

    @Test
    fun `typed instruction frames encode in both layouts`() {
        for (fixture in dense) {
            val name = fixture.name
            val source = sources.getValue(name).materializePayload()
            val decoded = denseAdapter.decodeTransaction(payloadBytes(fixture))
            assertContentEquals(payloadBytes(fixture), denseAdapter.encodeTransaction(source), name)
            assertContentEquals(
                bare(manifest.getValue(name).payloadBase64, TRANSACTION_PAYLOAD_TYPE),
                compactAdapter.encodeTransaction(decoded),
                "$name: a dense decode re-encodes in the current layout",
            )
        }
    }

    @Test
    fun `only an unregistered wire ID stays an opaque dense envelope`() {
        val fixture = dense.single { it.name == "set_parameter_next_mode" }
        val decoded = denseAdapter.decodeTransaction(payloadBytes(fixture))
        val typed = assertIs<WirePayload>(instructions(decoded.executable).single().payload)
        assertEquals("iroha.set_parameter", typed.wireName)
        val compact = compactAdapter.encodeTransaction(decoded)
        val densePayload = assertNotNull(NativeInstructionLayoutBridge.denseFromFrame(typed.wireName, typed.payloadBytes))

        // An unregistered wire ID keeps its bytes and its layout.
        val unregistered = "test.unreleased.instruction.v1"
        val opaque = decoded.copy(
            executable = Executable.instructions(
                listOf(InstructionBox.fromDenseWirePayload(unregistered, densePayload)),
            ),
        )
        val opaqueBytes = denseAdapter.encodeTransaction(opaque)
        assertEquals(opaque, denseAdapter.decodeTransaction(opaqueBytes))
        val inCurrent = assertFailsWith<NoritoException> { compactAdapter.encodeTransaction(opaque) }
        assertTrue(inCurrent.cause?.message.orEmpty().contains("cannot be encoded in the current layout"), inCurrent.cause?.message)
        val framedUnregistered = decoded.copy(
            executable = Executable.instructions(
                listOf(InstructionBox.fromWirePayload(unregistered, typed.payloadBytes)),
            ),
        )
        compactAdapter.encodeTransaction(framedUnregistered)
        val inDense = assertFailsWith<NoritoException> { denseAdapter.encodeTransaction(framedUnregistered) }
        assertTrue(inDense.cause?.message.orEmpty().contains("no registered dense encoding"), inDense.cause?.message)

        // A registered instruction is built from its typed frame, never as an opaque envelope.
        val registered = decoded.copy(
            executable = Executable.instructions(
                listOf(InstructionBox.fromDenseWirePayload(typed.wireName, densePayload)),
            ),
        )
        val refused = assertFailsWith<NoritoException> { denseAdapter.encodeTransaction(registered) }
        assertTrue(refused.cause?.message.orEmpty().contains("is registered"), refused.cause?.message)
        assertContentEquals(compact, compactAdapter.encodeTransaction(denseAdapter.decodeTransaction(payloadBytes(fixture))))
    }

    @Test
    fun `a dense envelope is decoded as exactly the instruction its wire ID registers`() {
        val fixture = dense.single { it.name == "set_parameter_next_mode" }
        val decoded = denseAdapter.decodeTransaction(payloadBytes(fixture))
        val typed = assertIs<WirePayload>(instructions(decoded.executable).single().payload)
        val densePayload = assertNotNull(NativeInstructionLayoutBridge.denseFromFrame(typed.wireName, typed.payloadBytes))

        for (wireName in listOf(
            "iroha.privacy.submit_proof.v1",
            SubmitPrivacyZkX509CertificateV1.WIRE_ID,
            "iroha.instruction.v1::kaigi::CreateKaigi",
            UpdatePlainConvictionWirePayloadEncoder.WIRE_NAME,
        )) {
            // Another instruction's payload under a typed-check wire ID is never accepted.
            assertFailsWith<NoritoException>(wireName) {
                denseAdapter.encodeTransaction(
                    decoded.copy(
                        executable = Executable.instructions(
                            listOf(InstructionBox.fromDenseWirePayload(wireName, densePayload)),
                        ),
                    ),
                )
            }

            // The same envelope on the wire is refused by the decoder: a placeholder of equal
            // length keeps every length prefix exact, then the wire ID is patched in place.
            val placeholder = "x".repeat(wireName.length)
            val encoded = denseAdapter.encodeTransaction(
                decoded.copy(
                    executable = Executable.instructions(
                        listOf(InstructionBox.fromDenseWirePayload(placeholder, densePayload)),
                    ),
                ),
            )
            val offset = indexOf(encoded, placeholder.toByteArray(Charsets.US_ASCII))
            assertTrue(offset > 0, "placeholder wire ID must be on the wire")
            val patched = encoded.copyOf()
            wireName.toByteArray(Charsets.US_ASCII).copyInto(patched, offset)
            assertFailsWith<NoritoException>("$wireName on the wire") { denseAdapter.decodeTransaction(patched) }
        }

        // The registered instruction's own body with a trailing byte is refused as well: the
        // opaque placeholder carries it onto the wire, then the wire ID is patched in place.
        val extended = densePayload + byteArrayOf(0)
        assertFailsWith<NoritoException> {
            denseAdapter.encodeTransaction(
                decoded.copy(
                    executable = Executable.instructions(
                        listOf(InstructionBox.fromDenseWirePayload(typed.wireName, extended)),
                    ),
                ),
            )
        }
        val placeholder = "x".repeat(typed.wireName.length)
        val encoded = denseAdapter.encodeTransaction(
            decoded.copy(
                executable = Executable.instructions(
                    listOf(InstructionBox.fromDenseWirePayload(placeholder, extended)),
                ),
            ),
        )
        val offset = indexOf(encoded, placeholder.toByteArray(Charsets.US_ASCII))
        assertTrue(offset > 0, "placeholder wire ID must be on the wire")
        val patched = encoded.copyOf()
        typed.wireName.toByteArray(Charsets.US_ASCII).copyInto(patched, offset)
        assertFailsWith<NoritoException> { denseAdapter.decodeTransaction(patched) }
    }

    @Test
    fun `signed attachments are refused in the dense layout`() {
        val fixture = dense.first()
        val payloadBytes = payloadBytes(fixture)
        val decoded = denseAdapter.decodeTransaction(payloadBytes)
        val attachments = SignedAttachmentsV1(
            listOf(
                ActionIntentV1(
                    0,
                    ActionOperationV1.TransactionProof,
                    ActionRoleV1.ProofVerification,
                    ActionAuthorityBindingV1.TransactionAuthority,
                    ActionInvocationBindingV1.Transaction,
                    ActionPublicInputsDigestV1(ByteArray(32) { 1 }),
                    ActionStatementDigestV1(ByteArray(32) { 2 }),
                    emptyList(),
                ),
            ),
            emptyList(),
        )
        val encodeError = assertFailsWith<NoritoException> {
            denseAdapter.encodeTransaction(decoded.copy(attachments = attachments))
        }
        assertTrue(encodeError.cause?.message.orEmpty().contains("Signed attachments are not supported"), encodeError.cause?.message)

        // On the wire: the trailing `Option::None` field (`01 00`) becomes `Some` of an empty frame.
        assertContentEquals(byteArrayOf(1, 0), payloadBytes.copyOfRange(payloadBytes.size - 2, payloadBytes.size))
        val withAttachments = payloadBytes.copyOf(payloadBytes.size - 2) + byteArrayOf(2, 1, 0)
        val decodeError = assertFailsWith<NoritoException> { denseAdapter.decodeTransaction(withAttachments) }
        assertTrue(generateSequence<Throwable>(decodeError) { it.cause }
            .any { it.message.orEmpty().contains("Signed attachments are not supported") })
    }

    @Test
    fun `dense signed transactions round-trip multisig signatures with raw keys`() {
        val payloadBytes = payloadBytes(dense.single { it.name == "transfer_asset" })
        val memberPublicKey = TestEd25519Keys.publicKey(0x41)
        val memberSignature = ByteArray(64) { ((0x80 + it) and 0xFF).toByte() }
        val signed = SignedTransaction.builder()
            .setEncodedPayload(payloadBytes)
            .setSignature(ByteArray(64) { (it + 1).toByte() })
            .setPublicKey(ByteArray(0))
            .setSchemaName(SIGNED_SCHEMA)
            .setMultisigSignatures(listOf(MultisigSignature.fromCurveId(0x01, memberPublicKey, memberSignature)))
            .build()

        val encoded = SignedTransactionEncoder.encode(signed, WireLayout.DENSE)
        val decoded = SignedTransactionEncoder.decode(encoded, WireLayout.DENSE)
        assertContentEquals(payloadBytes, decoded.encodedPayload())
        assertContentEquals(signed.signature(), decoded.signature())
        val member = assertNotNull(decoded.multisigSignatures().orElse(null)).signatures.single()
        assertEquals(0x01, member.curveId)
        assertContentEquals(memberPublicKey, member.publicKey())
        assertContentEquals(memberSignature, member.signature())
        assertContentEquals(encoded, SignedTransactionEncoder.encode(decoded, WireLayout.DENSE))

        // Dense byte sequences are raw: the key and both signatures appear as contiguous bytes.
        assertTrue(indexOf(encoded, memberPublicKey) > 0)
        assertTrue(indexOf(encoded, memberSignature) > 0)
        assertTrue(indexOf(encoded, signed.signature()) > 0)
        assertFailsWith<NoritoException> { SignedTransactionEncoder.decode(encoded) }
    }

    /**
     * The dense executable decodes into exactly the descriptor's typed executable: each instruction
     * is the same framed `WirePayload` the current layout carries. Returns the instruction count.
     */
    private fun assertSameExecutable(name: String, source: Executable, decoded: Executable): Int {
        assertEquals(source, decoded, "$name: executable")
        val typed = instructions(decoded)
        for ((index, instruction) in typed.withIndex()) {
            val frame = assertIs<WirePayload>(instruction.payload, "$name[$index]: typed instruction")
            assertEquals(0, indexOf(frame.payloadBytes, NoritoHeader.MAGIC), "$name[$index]: framed")
        }
        return typed.size
    }

    private fun instructions(executable: Executable): List<InstructionBox> = when (executable) {
        is Executable.Instructions -> executable.instructions
        is Executable.Batch -> executable.entries.filterIsInstance<ExecutableBatchItem.Instruction>().map { it.instruction }
        else -> emptyList()
    }

    private fun payloadBytes(fixture: TransactionDenseFixture): ByteArray =
        AndroidFixtureSupport.decodeCanonicalBase64(fixture.payloadBase64, "${fixture.name}.payload_base64")

    private fun signedBytes(fixture: TransactionDenseFixture): ByteArray =
        AndroidFixtureSupport.decodeCanonicalBase64(fixture.signedBase64, "${fixture.name}.signed_base64")

    /** The bare payload of a current-layout fixture frame. */
    private fun bare(frameBase64: String, typeName: String): ByteArray {
        val frame = AndroidFixtureSupport.decodeCanonicalBase64(frameBase64, typeName)
        val decoded = NoritoHeader.decode(frame, SchemaHash.hash16(typeName))
        decoded.header.validateChecksum(decoded.payload)
        return decoded.payload
    }

    /** The SDK's Ed25519 signer over the fixture signing key. */
    private fun fixtureSigner(): Signer {
        val key = Ed25519PrivateKeyParameters(FIXTURE_SIGNING_SEED, 0)
        val factory = KeyFactory.getInstance("Ed25519")
        return Ed25519Signer(
            factory.generatePrivate(PKCS8EncodedKeySpec(unhex("302e020100300506032b657004220420") + FIXTURE_SIGNING_SEED)),
            factory.generatePublic(X509EncodedKeySpec(unhex("302a300506032b6570032100") + key.generatePublicKey().encoded)),
        )
    }

    private fun indexOf(haystack: ByteArray, needle: ByteArray): Int {
        for (start in 0..haystack.size - needle.size) {
            if (needle.indices.all { haystack[start + it] == needle[it] }) return start
        }
        return -1
    }

    private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }

    private companion object {
        private const val SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1"
        private const val TRANSACTION_PAYLOAD_TYPE =
            "iroha_data_model::transaction::signed::model::TransactionPayload"
        private const val SIGNED_TRANSACTION_TYPE =
            "iroha_data_model::transaction::signed::model::SignedTransaction"

        private fun unhex(text: String): ByteArray = text.chunked(2).map { it.toInt(16).toByte() }.toByteArray()

        /** `android-fixture-signing-key-0102`, the seed of the Norito RPC fixture signing key. */
        private val FIXTURE_SIGNING_SEED =
            unhex("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032")
    }
}
