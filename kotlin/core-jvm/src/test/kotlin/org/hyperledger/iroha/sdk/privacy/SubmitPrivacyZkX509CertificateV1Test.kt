package org.hyperledger.iroha.sdk.privacy

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.client.TairaTestnetProfile
import org.hyperledger.iroha.sdk.core.model.*
import org.hyperledger.iroha.sdk.norito.*
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.TransactionBuilder
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder
import org.hyperledger.iroha.sdk.tx.norito.TransactionPayloadAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import kotlin.test.*

class SubmitPrivacyZkX509CertificateV1Test {
    @Test
    fun inspectionCopiesNativeArraysAndChecksEveryActualContextBinding() {
        val original = byteArrayOf(1, 2, 3)
        val network = NetworkId.fromBytes(ByteArray(32) { 1 })
        fun result(): Array<ByteArray?> = arrayOf<ByteArray?>(original.copyOf(), byteArrayOf(7, 0, 0, 0),
            ByteArray(32) { 2 }, network.bytes(), *Array(5) { i -> ByteArray(32) { (i + 3).toByte() } })
        val protocol = PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1
        val fields = (3..7).map { byte -> PrivacyFixed32V1(ByteArray(32) { byte.toByte() }) }
        val profile = PrivacyCompiledProfileV1(protocol, protocol.expectedProofSystem, protocol.expectedEngine,
            fields[0], fields[1], fields[2], fields[3], fields[4], PrivacyProtocolLimitsV1(protocol, null))
        val inputs = result()
        val inspected = PrivacyZkX509RequestInspectionV1.fromNative(original, inputs)
        inspected.requireCompiledContextV1(network, profile)
        assertEquals(7L, inspected.proofActionIndex)
        inputs.forEach { it!!.fill(0) }
        val output = inspected.canonicalRequestBytes().also { it.fill(0) }
        assertFalse(output.contentEquals(inspected.canonicalRequestBytes()))
        inspected.requireCompiledContextV1(network, profile)
        for (index in 3..8) {
            val changed = result().also { it[index]!![0] = (it[index]!![0].toInt() xor 0x40).toByte() }
            val mutated = PrivacyZkX509RequestInspectionV1.fromNative(original, changed)
            assertFailsWith<IllegalArgumentException> { mutated.requireCompiledContextV1(network, profile) }
        }
        for (bad in listOf(null, emptyArray<ByteArray?>(), result().copyOf(8), result().also { it[1] = ByteArray(8) },
            result().also { it[0] = byteArrayOf(9) }, result().also { it[2] = ByteArray(32) },
            result().also { it[8] = null })) {
            assertFailsWith<IllegalArgumentException> { PrivacyZkX509RequestInspectionV1.fromNative(original, bad) }
        }
    }

    @Test
    fun unadmittedNewWireCannotEncodeOrHideBehindAGenericBox() {
        val box = InstructionBox.fromWirePayload(SubmitPrivacyZkX509CertificateV1.WIRE_ID, byteArrayOf(1))
        assertFailsWith<IllegalArgumentException> { box.requirePrivacyExact12ConstructionAdmission() }
        assertFailsWith<IllegalArgumentException> { box.requirePrivacyExact12Network(NetworkId.fromBytes(ByteArray(32) { 1 })) }
        assertFailsWith<IllegalArgumentException> { SubmitPrivacyZkX509CertificateV1.plannedProofOccurrenceV1(-1) }
        assertFailsWith<IllegalArgumentException> { SubmitPrivacyZkX509CertificateV1.plannedProofOccurrenceV1(0x1_0000_0000L) }
    }

    @Test
    fun nativeRequestMaterializesTheFixtureActionStatementAndRejectsBindingMutations() {
        val row = X509AttachedRequestTestInputs.row()
        val request = SubmitPrivacyZkX509CertificateV1.fromCanonicalRequestV1(row.submitProofInstructionNorito)
        val codec = X509AttachedRequestTestInputs.codec()
        val original = codec.decodeTransaction(row.unsignedTransactionPayloadNorito)
        assertContentEquals(row.statementNorito, request.materializeStatementV1(original, codec.chainDiscriminant))
        assertContentEquals(row.transactionIntentProjectionNorito,
            codec.actionStatementProjectionV1(original, request.proofActionIndex))
        assertContentEquals(row.transactionIntentDigest, original.attachments!!.actions.single().statementDigest.bytes)
        val requestCopy = request.canonicalRequestBytes().also { it.fill(0) }
        assertFalse(requestCopy.contentEquals(request.canonicalRequestBytes()))
        assertFailsWith<IllegalArgumentException> {
            SubmitPrivacyZkX509CertificateV1.fromStatementV1(row.statementNorito, request.proofActionIndex)
        } // Supplied materialized intent is forbidden.
        assertFailsWith<IllegalArgumentException> { request.materializeStatementV1(original.copy(nonce = 99L), codec.chainDiscriminant) }
        val action = original.attachments!!.actions.single()
        fun changed(action: ActionIntentV1) = original.copy(attachments = SignedAttachmentsV1(listOf(action), original.attachments!!.occurrences))
        fun replace(role: ActionRoleV1 = action.role, operation: ActionOperationV1 = action.operation,
            publicInputs: ActionPublicInputsDigestV1 = action.publicInputsDigest,
            invocation: ActionInvocationBindingV1 = action.invocation,
            refs: List<AttachmentReferenceV1> = action.attachments) = ActionIntentV1(action.actionIndex,
                operation, role, action.authority, invocation, publicInputs, action.statementDigest, refs)
        for (bad in listOf(replace(role = ActionRoleV1.ProofVerification),
            replace(operation = ActionOperationV1.NativeInstruction("iroha.privacy.submit_proof.v1")),
            replace(publicInputs = ActionPublicInputsDigestV1(ByteArray(32) { 9 })),
            replace(invocation = ActionInvocationBindingV1.Direct(1)), replace(refs = emptyList()))) {
            assertFailsWith<IllegalArgumentException> { request.materializeStatementV1(changed(bad), codec.chainDiscriminant) }
        }
        for (executable in listOf(original.executable, Executable.Batch(
            (original.executable as Executable.Instructions).instructions.map { ExecutableBatchItem.Instruction(it) }))) {
            val candidate = original.copy(executable = executable)
            fun refuses(payload: TransactionPayload, expected: String) {
                val error = assertFailsWith<NoritoException> { codec.encodeTransaction(payload) }
                assertTrue(generateSequence<Throwable>(error) { it.cause }.any { it.message?.contains(expected) == true })
            }
            refuses(candidate.copy(attachments = null), "X509 request requires signed attachments")
            refuses(candidate.copy(networkId = NetworkId.fromBytes(ByteArray(32) { 3 })), "X509 request network mismatch")
            val wrong = replace(invocation = ActionInvocationBindingV1.Direct(1))
            refuses(candidate.copy(attachments = SignedAttachmentsV1(listOf(wrong), original.attachments!!.occurrences)), "direct invocation mismatch")
        }
    }

    @Test
    fun authenticSignedFixturesRoundtripWithoutGrantingConstructionAdmission() {
        assertTrue(PrivacyNativeBridge.isNativeAvailable())
        val rows = PrivacyExact12FixtureCodecV1.decodeCanonical(PrivacyNativeBridge.exact12FixtureBundleV1()).rows
        val codec = X509AttachedRequestTestInputs.codec()
        var signerCalls = 0
        val signer = object : Signer {
            override fun algorithm(): String = "Ed25519"
            override fun publicKey(): ByteArray = error("unadmitted payload must not request a signer key")
            override fun sign(message: ByteArray): ByteArray { signerCalls++; error("unadmitted payload must not be signed") }
        }
        for (row in rows) {
            val exact = row.signedTransactionVersionedNorito
            val signed = SignedTransactionEncoder.decodeVersioned(exact)
            assertContentEquals(exact, SignedTransactionEncoder.encodeVersioned(signed))
            assertContentEquals(row.unsignedTransactionPayloadNorito, signed.encodedPayload())
            val decoded = codec.decodeTransaction(signed.encodedPayload())
            assertFailsWith<NoritoException> { codec.encodeTransaction(decoded) }
            assertFailsWith<NoritoException> { TransactionBuilder(codec).encodeAndSign(decoded, signer) }
        }
        assertEquals(0, signerCalls)
    }

    @Test
    fun canonicalInspectionRefusesMissingActionStaleBindingAndNoncanonicalBytes() {
        assertTrue(PrivacyNativeBridge.isNativeAvailable())
        val rows = PrivacyExact12FixtureCodecV1.decodeCanonical(PrivacyNativeBridge.exact12FixtureBundleV1()).rows
        val row = rows.single { it.protocolId == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1 }
        val exact = row.unsignedTransactionPayloadNorito
        val decoded = X509AttachedRequestTestInputs.codec().decodeTransaction(exact)
        val fields = readRawFields(exact)
        assertEquals(9, fields.size)
        fun refuses(bytes: ByteArray, reason: String) {
            val error = assertFailsWith<IllegalArgumentException> {
                TransactionPayloadAdapter.validateCanonicalPayloadBytes(bytes)
            }
            assertTrue(generateSequence<Throwable>(error) { it.cause }.any { it.message?.contains(reason) == true })
            val signed = SignedTransactionEncoder.decodeVersioned(row.signedTransactionVersionedNorito)
            assertFailsWith<NoritoException> {
                SignedTransactionEncoder.encode(signed.toBuilder().setEncodedPayload(bytes).build())
            }
            val signedFields = readRawFields(row.signedTransactionVersionedNorito.copyOfRange(1, row.signedTransactionVersionedNorito.size))
            signedFields[1] = bytes
            assertFailsWith<NoritoException> { SignedTransactionEncoder.decode(writeRawFields(signedFields)) }
        }
        refuses(writeRawFields(fields.toMutableList().also { it[8] = byteArrayOf(0) }), "X509 request requires signed attachments")
        val emptySequence = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS).also { it.writeLength(0, false) }.toByteArray()
        val emptyAttachments = byteArrayOf(1) + rawField(writeRawFields(listOf(emptySequence, emptySequence)))
        refuses(writeRawFields(fields.toMutableList().also { it[8] = emptyAttachments }), "X509 request action is absent")
        val changedNetwork = fields[0].copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 0x40).toByte() }
        refuses(writeRawFields(fields.toMutableList().also { it[0] = changedNetwork }), "X509 request network mismatch")
        val digest = decoded.attachments!!.actions.single().statementDigest.bytes
        val stale = fields[8].copyOf()
        val matches = (0..stale.size - digest.size).filter { at -> digest.indices.all { stale[at + it] == digest[it] } }
        assertEquals(1, matches.size)
        stale[matches.single()] = (stale[matches.single()].toInt() xor 0x40).toByte()
        refuses(writeRawFields(fields.toMutableList().also { it[8] = stale }), "action statement binding mismatch")
        assertFailsWith<IllegalArgumentException> { TransactionPayloadAdapter.validateCanonicalPayloadBytes(exact + byteArrayOf(0)) }
        // Generic privacy bytes cannot skip native canonical inspection when attachments are absent.
        val executable = fields[3]
        val genericName = "iroha.privacy.submit_proof.v1"
        val originalName = SubmitPrivacyZkX509CertificateV1.WIRE_ID.toByteArray(Charsets.UTF_8)
        val at = (0..executable.size - originalName.size).single { start ->
            originalName.indices.all { executable[start + it] == originalName[it] }
        }
        assertTrue(at > 0)
        fun withoutAttachmentsAsGeneric(frame: ByteArray): ByteArray {
            val instruction = encodeRawInstruction(genericName, frame)
            val list = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS).also {
                it.writeLength(1, false); it.writeBytes(rawField(instruction))
            }.toByteArray()
            val genericExecutable = executable.copyOfRange(0, 4) + rawField(list)
            return writeRawFields(fields.toMutableList().also { it[3] = genericExecutable; it[8] = byteArrayOf(0) })
        }
        refuses(withoutAttachmentsAsGeneric(row.submitProofInstructionNorito), "submit-proof instruction is not canonical typed Norito")
        // Reconstruct the retired single-envelope request only as a negative mutation. The
        // schema and padding come from an authentic generic fixture; no golden is invented.
        val reference = rows.first { it.protocolId != row.protocolId }.submitProofInstructionNorito
        val referenceFrame = NoritoHeader.decode(reference, reference.copyOfRange(6, 22))
        referenceFrame.header.validateChecksum(referenceFrame.payload)
        val envelope = NoritoHeader.decode(row.envelopeNorito, row.envelopeNorito.copyOfRange(6, 22))
        envelope.header.validateChecksum(envelope.payload)
        assertEquals(NoritoCodec.DEFAULT_FLAGS, envelope.header.flags)
        val retiredBody = rawField(envelope.payload)
        val paddingLength = reference.size - NoritoHeader.HEADER_LENGTH - referenceFrame.header.payloadLength
        val padding = reference.copyOfRange(NoritoHeader.HEADER_LENGTH, NoritoHeader.HEADER_LENGTH + paddingLength)
        assertTrue(padding.all { it == 0.toByte() })
        val retired = NoritoHeader(referenceFrame.header.schemaHash, retiredBody.size,
            CRC64.compute(retiredBody), referenceFrame.header.flags, referenceFrame.header.compression).encode() + padding + retiredBody
        refuses(withoutAttachmentsAsGeneric(retired), "submit-proof instruction is not canonical typed Norito")
    }

    @Test
    fun privateInspectionPreservesBatchOrderAndEveryProvedOverlayField() {
        val codec = X509AttachedRequestTestInputs.codec()
        val original = codec.decodeTransaction(X509AttachedRequestTestInputs.row().unsignedTransactionPayloadNorito)
        val instructions = listOf(NoritoJavaCodecAdapter.logInstruction(2, "first"), NoritoJavaCodecAdapter.logInstruction(3, "second"))
        val invocation = ContractInvocation("irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
            ByteArray(32) { 9 }, "call")
        val batch = Executable.Batch(listOf(ExecutableBatchItem.Instruction(instructions[0]),
            ExecutableBatchItem.ContractCall(invocation), ExecutableBatchItem.Instruction(instructions[1])))
        val proved = Executable.IvmProved(byteArrayOf(1, 2, 3), instructions,
            ByteArray(32) { 5 }, ByteArray(32) { 7 })
        for (executable in listOf(batch, proved)) {
            val payload = original.copy(executable = executable, attachments = null,
                feePayment = FeePaymentIntent.authority(emptyList(), 100L))
            val exact = codec.encodeTransaction(payload)
            val decoded = TransactionPayloadAdapter.validateCanonicalPayloadBytes(exact)
            assertContentEquals(exact, codec.encodeTransaction(codec.decodeTransaction(exact)))
            when (val inspected = decoded.executable) {
                is Executable.Batch -> assertEquals(batch.entries, inspected.entries)
                is Executable.IvmProved -> {
                    assertContentEquals(proved.ivmBytes, inspected.ivmBytes)
                    assertEquals(proved.overlay, inspected.overlay)
                    assertContentEquals(proved.eventsCommitment, inspected.eventsCommitment)
                    assertContentEquals(proved.gasPolicyCommitment, inspected.gasPolicyCommitment)
                }
                else -> fail("inspection changed executable kind")
            }
        }
    }

    private fun rawField(value: ByteArray): ByteArray = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS).also {
        it.writeLength(value.size.toLong(), true); it.writeBytes(value)
    }.toByteArray()

    private fun readRawFields(value: ByteArray): MutableList<ByteArray> {
        val decoder = NoritoDecoder(value, NoritoCodec.DEFAULT_FLAGS)
        val fields = mutableListOf<ByteArray>()
        while (decoder.remaining() > 0) fields.add(decoder.readBytes(decoder.readLength(true).toInt()))
        return fields
    }

    private fun writeRawFields(fields: List<ByteArray>): ByteArray =
        fields.fold(ByteArray(0)) { result, field -> result + rawField(field) }

    private fun encodeRawInstruction(wireId: String, payload: ByteArray): ByteArray = writeRawFields(listOf(
        NoritoCodec.encodeAdaptive(wireId, NoritoAdapters.stringAdapter()).payload(),
        NoritoCodec.encodeAdaptive(payload, NoritoAdapters.rawByteVecAdapter()).payload(),
    ))

    @Test
    fun explicitPlanPreservesPrivacyOrdinalAndIntentWhenSignedProofArrives() {
        val row = X509AttachedRequestTestInputs.row()
        val codec = X509AttachedRequestTestInputs.codec()
        val original = codec.decodeTransaction(row.unsignedTransactionPayloadNorito)
        val independent = X509AttachedRequestTestInputs.independentStatement(row.statementNorito)
        val request = SubmitPrivacyZkX509CertificateV1.fromStatementV1(independent, 1L)
        assertEquals(0L, X509AttachedRequestTestInputs.privacyOrdinal(independent))
        assertEquals(1L, request.proofActionIndex)
        assertEquals(0L, request.proofSlotIndex)
        val prefix = ActionIntentV1(0, ActionOperationV1.NativeInstruction("iroha.log"), ActionRoleV1.Effect,
            ActionAuthorityBindingV1.TransactionAuthority, ActionInvocationBindingV1.Direct(0),
            ActionPublicInputsDigestV1(ByteArray(32) { 4 }), ActionStatementDigestV1(ByteArray(32)), emptyList())
        val action = request.actionIntentV1(ActionAuthorityBindingV1.TransactionAuthority, ActionInvocationBindingV1.Direct(1), 0)
        val placeholder = SubmitPrivacyZkX509CertificateV1.plannedProofOccurrenceV1(0)
        val plan = original.copy(executable = Executable.Instructions(listOf(
            NoritoJavaCodecAdapter.logInstruction(2, "before X509"),
            InstructionBox.fromWirePayload(SubmitPrivacyZkX509CertificateV1.WIRE_ID, request.canonicalRequestBytes()))),
            attachments = SignedAttachmentsV1(listOf(prefix, action), listOf(placeholder)))
        val statement = request.materializeStatementV1(plan, codec.chainDiscriminant)
        assertEquals(0L, X509AttachedRequestTestInputs.privacyOrdinal(statement))
        assertContentEquals(independent, X509AttachedRequestTestInputs.independentStatement(statement))
        assertFailsWith<IllegalArgumentException> {
            request.materializeStatementV1(plan.copy(attachments = SignedAttachmentsV1(listOf(prefix, action), emptyList())), codec.chainDiscriminant)
        }
        val signedProof = SignedAttachmentOccurrenceV1.of(0, AttachmentKindV1.Proof, byteArrayOf(7, 8, 9))
        val completed = plan.copy(attachments = SignedAttachmentsV1(listOf(prefix, request.withSignedProofV1(action, signedProof)), listOf(signedProof)))
        assertEquals(codec.orderedIntentDigest(plan), codec.orderedIntentDigest(completed))
        val bound = codec.bindAttachmentStatements(completed)
        assertContentEquals(statement, request.materializeStatementV1(bound, codec.chainDiscriminant))
        assertContentEquals(codec.actionStatementProjectionV1(plan, 1), codec.actionStatementProjectionV1(bound, 1))
        assertFailsWith<IllegalArgumentException> {
            request.withSignedProofV1(action, SignedAttachmentOccurrenceV1.of(1, AttachmentKindV1.Proof, byteArrayOf(1)))
        }
        assertFailsWith<IllegalArgumentException> {
            request.withSignedProofV1(action, SignedAttachmentOccurrenceV1.of(0, AttachmentKindV1.KeyMaterial, byteArrayOf(1)))
        }
    }
}

/** Shared Kotlin/Java test inputs use maintained Rust fixtures, never invented golden archives. */
object X509AttachedRequestTestInputs {
    @JvmStatic fun codec(): NoritoJavaCodecAdapter = NoritoJavaCodecAdapter(TairaTestnetProfile.I105_DISCRIMINANT)
    @JvmStatic fun row(): PrivacyExact12TypedFixtureRowV1 {
        assertTrue(PrivacyNativeBridge.isNativeAvailable(), "matching X509 native exports and generated fixture are required")
        return PrivacyExact12FixtureCodecV1.decodeCanonical(PrivacyNativeBridge.exact12FixtureBundleV1()).rows
            .single { it.protocolId == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1 }
    }
    private fun field(value: ByteArray): ByteArray = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS).also {
        it.writeLength(value.size.toLong(), true); it.writeBytes(value)
    }.toByteArray()
    private fun fields(bytes: ByteArray): List<ByteArray> {
        val decoder = NoritoDecoder(bytes, NoritoCodec.DEFAULT_FLAGS)
        val result = mutableListOf<ByteArray>()
        while (decoder.remaining() != 0) result.add(decoder.readBytes(decoder.readLength(true).toInt()))
        return result
    }
    private fun statementFields(tagged: ByteArray): Pair<NoritoHeader, List<ByteArray>> {
        val decoded = NoritoHeader.decode(tagged, tagged.copyOfRange(6, 22))
        decoded.header.validateChecksum(decoded.payload)
        val decoder = NoritoDecoder(decoded.payload, decoded.header.flags)
        check(decoder.readUInt(32) == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1.ordinal.toLong())
        val statement = decoder.readBytes(decoder.readLength(true).toInt())
        check(decoder.remaining() == 0)
        return decoded.header to fields(statement)
    }
    @JvmStatic fun privacyOrdinal(tagged: ByteArray): Long {
        val context = fields(statementFields(tagged).second.first())
        check(context[1].size == 4)
        return ByteBuffer.wrap(context[1]).order(ByteOrder.LITTLE_ENDIAN).int.toLong() and 0xffff_ffffL
    }
    @JvmStatic fun independentStatement(tagged: ByteArray): ByteArray {
        val (header, original) = statementFields(tagged)
        val statement = original.toMutableList()
        val context = fields(statement.first()).toMutableList()
        check(fields(context[2]).single().size == 32)
        context[2] = field(ByteArray(32))
        statement[0] = context.fold(ByteArray(0)) { bytes, value -> bytes + field(value) }
        val body = statement.fold(ByteArray(0)) { bytes, value -> bytes + field(value) }
        val payload = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(5).array() + field(body)
        // Mutation preserves the native frame's authenticated alignment padding exactly.
        check(payload.size == header.payloadLength)
        val paddingLength = tagged.size - NoritoHeader.HEADER_LENGTH - header.payloadLength
        check(paddingLength >= 0)
        val padding = tagged.copyOfRange(NoritoHeader.HEADER_LENGTH, NoritoHeader.HEADER_LENGTH + paddingLength)
        check(padding.all { it == 0.toByte() })
        return NoritoHeader(header.schemaHash, payload.size, CRC64.compute(payload), header.flags, header.compression).encode() + padding + payload
    }
}
