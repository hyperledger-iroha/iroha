package org.hyperledger.iroha.sdk.core.model.instructions

import java.io.File
import java.math.BigInteger
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.*
import org.hyperledger.iroha.sdk.norito.*
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import kotlin.test.*

/** Real Kotlin wire and transaction checks; fixture proof bytes confer no authorization. */
class KaigiWirePayloadV1Test {
    private val fixture = loadFixture()
    private val accounts = (fixture["accounts"] as List<*>).map { it as String }
    private val call = KaigiInstructionUtils.CallId("wonderland.sora", "weekly-sync")
    private val p = BigInteger("40000000000000000000000000000000224698fc094cf91b992d30ed00000001", 16)

    @Test
    fun `all five typed instructions match complete reference instruction boxes`() {
        val vectors = (fixture["vectors"] as List<*>).map { it as Map<*, *> }.associateBy { it["name"] }
        val instructions = listOf(
            CreateKaigiInstruction.create(call, accounts[0]), JoinKaigiInstruction(call, accounts[0]),
            LeaveKaigiInstruction(call, accounts[0]), EndKaigiInstruction(call),
            RecordKaigiUsageInstruction(call, 1, 2),
        )
        for (typed in instructions) {
            val expected = Base64.getDecoder().decode(vectors.getValue(typed.arguments.getValue("action"))["instruction_box_base64"] as String)
            val box = typed.toInstructionBox()
            assertIs<WirePayload>(box.payload)
            val actual = NoritoJavaCodecAdapter.encodeInstructionBox(box)
            assertContentEquals(expected, actual)
            assertContentEquals(actual, NoritoJavaCodecAdapter.encodeInstructionBox(NoritoJavaCodecAdapter.decodeInstructionBox(actual)))
            assertEquals(typed, KaigiWirePayloadEncoderV1.decode(typed.wireName, typed.payloadBytes))
        }
    }

    @Test
    fun `complex private Create matches full width reference with one-field scalar wrappers`() {
        val manifest = KaigiInstructionUtils.RelayManifest(Long.MIN_VALUE, listOf(
            KaigiInstructionUtils.RelayManifestHop(accounts[0], "ECA=", 1),
            KaigiInstructionUtils.RelayManifestHop(accounts[1], "MA==", 2),
            KaigiInstructionUtils.RelayManifestHop(accounts[2], "QFBg", 255),
        ))
        val create = CreateKaigiInstruction.create(
            call, accounts[0], title = "Roadmap 🛰", description = "exact", maxParticipants = 7,
            gasRatePerMinute = 9_007_199_254_740_993, metadata = mapOf(
                "z" to JsonValue.parse("[true,null,7]"), "a" to JsonValue.parse("{\"nested\":\"value\"}"),
            ), scheduledStartMs = 1_234_567_890_123, billingAccount = accounts[0],
            privacyMode = KaigiInstructionUtils.PrivacyMode("ZkRosterV1", null),
            roomPolicy = KaigiInstructionUtils.RoomPolicy("Public", null), relayManifest = manifest,
            commitment = KaigiAuthorizationScalarV1.fromLeBytes(ByteArray(32) { if (it == 31) 4 else 0x44 }),
            nullifierDigest = KaigiAuthorizationScalarV1.fromLeBytes(ByteArray(32) { if (it == 31) 5 else 0x55 }),
            rosterRoot = "66".repeat(31) + "67", proofBase64 = "AQID",
        )
        val expected = Base64.getDecoder().decode((fixture["complex_create"] as Map<*, *>)["instruction_box_base64"] as String)
        val actual = NoritoJavaCodecAdapter.encodeInstructionBox(create.toInstructionBox())
        assertEquals(872, actual.size)
        assertContentEquals(expected, actual)
        val decoded = KaigiWirePayloadEncoderV1.decode(create.wireName, create.payloadBytes) as CreateKaigiInstruction
        assertContentEquals(create.commitment!!.toLeBytes(), decoded.commitment!!.toLeBytes())
        assertContentEquals(create.nullifierDigest!!.toLeBytes(), decoded.nullifierDigest!!.toLeBytes())
        assertEquals(create.metadata, decoded.metadata)
        assertEquals(create.gasRatePerMinute, decoded.gasRatePerMinute)
    }

    @Test
    fun `all private operations actually round trip inside a transaction`() {
        val scalar = KaigiAuthorizationScalarV1.fromLeBytes(le(p.subtract(BigInteger.ONE)))
        val commitment = KaigiAuthorizationScalarV1.fromLeBytes(ByteArray(32))
        val root = "55".repeat(32)
        val typed = listOf(
            CreateKaigiInstruction.create(call, accounts[0], privacyMode = KaigiInstructionUtils.PrivacyMode("ZkRosterV1", null), commitment = commitment, nullifierDigest = scalar, rosterRoot = root, proofBase64 = "AQID"),
            JoinKaigiInstruction(call, accounts[0], commitment, scalar, root, "AQID"),
            LeaveKaigiInstruction(call, accounts[0], commitment, scalar, root, "AQID"),
            EndKaigiInstruction(call, null, commitment, scalar, root, "AQID"),
            RecordKaigiUsageInstruction(call, -1L, -1L, scalar, "AQID"),
        )
        val boxes = typed.map { it.toInstructionBox() }
        val payload = TransactionPayload(
            networkId = NetworkId.fromBytes(ByteArray(32) { 1 }), authority = accounts[0], creationTimeMs = 1,
            executable = Executable.instructions(boxes), feePayment = FeePaymentIntent.authority(emptyList()),
        )
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val encoded = codec.encodeTransaction(payload)
        val decoded = codec.decodeTransaction(encoded)
        assertEquals(boxes, (decoded.executable as Executable.Instructions).instructions)
        assertContentEquals(encoded, codec.encodeTransaction(decoded))
        for (instruction in typed) {
            assertContentEquals(instruction.payloadBytes, KaigiWirePayloadEncoderV1.decode(instruction.wireName, instruction.payloadBytes).payloadBytes)
        }
    }

    @Test
    fun `Pasta outputs preserve exact bytes and reject the modulus without reduction`() {
        for (value in listOf(BigInteger.ZERO, BigInteger.ONE, p.subtract(BigInteger.ONE))) {
            val raw = le(value); val scalar = KaigiAuthorizationScalarV1.fromLeBytes(raw)
            assertContentEquals(raw, scalar.toLeBytes())
            val snapshot = scalar.toLeBytes(); raw[0] = (raw[0].toInt() xor 1).toByte()
            assertContentEquals(snapshot, scalar.toLeBytes())
            scalar.toLeBytes().fill(0x55)
            assertContentEquals(snapshot, scalar.toLeBytes())
        }
        for (value in listOf(p, p.add(BigInteger.ONE), BigInteger.ONE.shiftLeft(256).subtract(BigInteger.ONE))) {
            val raw = le(value); val snapshot = raw.copyOf()
            assertFailsWith<IllegalArgumentException> { KaigiAuthorizationScalarV1.fromLeBytes(raw) }
            assertContentEquals(snapshot, raw)
        }
        for (size in listOf(0, 31, 33)) assertFailsWith<IllegalArgumentException> { KaigiAuthorizationScalarV1.fromLeBytes(ByteArray(size)) }
        for (text in listOf("hash:" + "01".repeat(32), " 01".repeat(32), "AB".repeat(32))) {
            assertFailsWith<IllegalArgumentException> { KaigiAuthorizationScalarV1.fromArgument(text) }
        }
    }

    @Test
    fun `raw wire bypass cannot admit out-of-field scalars retired slots or noncanonical frames`() {
        val scalar = KaigiAuthorizationScalarV1.fromLeBytes(le(p.subtract(BigInteger.ONE)))
        val typed = JoinKaigiInstruction(call, accounts[0], scalar, scalar, "55".repeat(32), "AQID")
        val fields = fields(typed.payloadBytes.copyOfRange(40, typed.payloadBytes.size))
        val malformed = listOf(
            fields.toMutableList().also { it[2] = byteArrayOf(1) + field(field(le(p))) },
            fields.toMutableList().also { it[2] = byteArrayOf(1) + field(field(scalar.toLeBytes()) + field(byteArrayOf(0))) },
            fields.toMutableList().also { it[3] = byteArrayOf(1) + field(field(scalar.toLeBytes()) + field(ByteArray(8))) },
        ).map { frame(it.fold(byteArrayOf()) { out, item -> out + field(item) }, "JoinKaigi") } + listOf(
            typed.payloadBytes + byteArrayOf(0), typed.payloadBytes.copyOf().also { it[39] = 0 },
        )
        for (bad in malformed) {
            assertFailsWith<Exception> { KaigiWirePayloadEncoderV1.decode(typed.wireName, bad) }
            val untrusted = InstructionBox.fromWirePayload(typed.wireName, bad)
            assertFailsWith<NoritoException> { NoritoJavaCodecAdapter.encodeInstructionBox(untrusted) }
        }
    }

    @Test
    fun `Kaigi preserves the full multisig controller through transaction encoding`() {
        val policy = MultisigPolicyPayload.of(1, 2, listOf(
            MultisigMemberPayload(1, 1, TestEd25519Keys.publicKey(0x11)),
            MultisigMemberPayload(1, 2, TestEd25519Keys.publicKey(0x22)),
        ))
        val account = AccountAddress.fromMultisigPolicy(policy).toI105Default()
        val instructions = listOf(
            CreateKaigiInstruction.create(call, account),
            JoinKaigiInstruction(call, account),
            LeaveKaigiInstruction(call, account),
        )
        for (instruction in instructions) {
            assertEquals(instruction, KaigiWirePayloadEncoderV1.decode(instruction.wireName, instruction.payloadBytes))
        }
        val boxes = instructions.map { it.toInstructionBox() }
        val transaction = TransactionPayload(
            networkId = NetworkId.fromBytes(ByteArray(32) { 1 }), authority = account,
            creationTimeMs = 1, executable = Executable.instructions(boxes),
            feePayment = FeePaymentIntent.authority(emptyList()),
        )
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val encoded = codec.encodeTransaction(transaction)
        val decoded = codec.decodeTransaction(encoded)
        assertEquals(account, decoded.authority)
        assertEquals(boxes, (decoded.executable as Executable.Instructions).instructions)
        assertContentEquals(encoded, codec.encodeTransaction(decoded))
    }

    @Test
    fun `Kaigi rejects external noncanonical multisig member order and duplicate keys`() {
        val policy = MultisigPolicyPayload.of(1, 2, listOf(
            MultisigMemberPayload(1, 1, TestEd25519Keys.publicKey(0x11)),
            MultisigMemberPayload(1, 2, TestEd25519Keys.publicKey(0x22)),
        ))
        val typed = JoinKaigiInstruction(call, AccountAddress.fromMultisigPolicy(policy).toI105Default())
        val instructionFields = fields(typed.payloadBytes.copyOfRange(40, typed.payloadBytes.size))
        val controller = instructionFields[1]
        val policyFields = fields(fields(controller.copyOfRange(4, controller.size)).single())
        val members = fields(policyFields[2].copyOfRange(8, policyFields[2].size))
        for (changedMembers in listOf(members.reversed(), listOf(members[0], members[0]))) {
            val count = NoritoEncoder(2).also { it.writeUInt(2, 64) }.toByteArray()
            val changedPolicy = policyFields.take(2).fold(byteArrayOf()) { out, item -> out + field(item) } +
                field(count + changedMembers.fold(byteArrayOf()) { out, item -> out + field(item) })
            val malformedAccount = byteArrayOf(1, 0, 0, 0) + field(changedPolicy)
            assertFailsWith<IllegalArgumentException> {
                TransferWirePayloadEncoder.decodeAccountIdPayload(malformedAccount, AccountAddress.DEFAULT_I105_DISCRIMINANT)
            }
            val changed = instructionFields.toMutableList().also { it[1] = malformedAccount }
            val malformed = frame(changed.fold(byteArrayOf()) { out, item -> out + field(item) }, "JoinKaigi")
            assertFailsWith<IllegalArgumentException> { KaigiWirePayloadEncoderV1.decode(typed.wireName, malformed) }
            assertFailsWith<NoritoException> {
                NoritoJavaCodecAdapter.encodeInstructionBox(InstructionBox.fromWirePayload(typed.wireName, malformed))
            }
        }
    }

    @Test
    fun `multisig member capacity is bounded before account decoding allocates`() {
        val typed = JoinKaigiInstruction(call, accounts[0])
        for (count in listOf(1L, Int.MAX_VALUE.toLong())) {
            val members = NoritoEncoder(2).also { it.writeUInt(count, 64) }.toByteArray()
            val policy = field(byteArrayOf(1)) + field(byteArrayOf(1, 0)) + field(members)
            val account = byteArrayOf(1, 0, 0, 0) + field(policy)
            val error = assertFailsWith<IllegalArgumentException> {
                TransferWirePayloadEncoder.decodeAccountIdPayload(account, AccountAddress.DEFAULT_I105_DISCRIMINANT)
            }
            assertTrue(error.message!!.contains("member count exceeds encoded bounds"))
            val changed = fields(typed.payloadBytes.copyOfRange(40, typed.payloadBytes.size)).toMutableList()
            changed[1] = account
            val malformed = frame(changed.fold(byteArrayOf()) { out, item -> out + field(item) }, "JoinKaigi")
            assertFailsWith<IllegalArgumentException> { KaigiWirePayloadEncoderV1.decode(typed.wireName, malformed) }
            assertFailsWith<NoritoException> {
                NoritoJavaCodecAdapter.encodeInstructionBox(InstructionBox.fromWirePayload(typed.wireName, malformed))
            }
            val originalBox = NoritoJavaCodecAdapter.encodeInstructionBox(typed.toInstructionBox())
            val outer = fields(originalBox.copyOfRange(40, originalBox.size)).toMutableList()
            outer[1] = NoritoEncoder(2).also {
                it.writeUInt(malformed.size.toLong(), 64); it.writeBytes(malformed)
            }.toByteArray()
            val malformedBox = rawFrame(outer.fold(byteArrayOf()) { out, item -> out + field(item) }, fixture["outer_type_name"] as String)
            assertFailsWith<NoritoException> { NoritoJavaCodecAdapter.decodeInstructionBox(malformedBox) }
        }
    }

    @Test
    fun `typed metadata and boxed payload bytes are defensive snapshots`() {
        val metadata = linkedMapOf("original" to JsonValue.number(1))
        val typed = CreateKaigiInstruction.create(call, accounts[0], metadata = metadata)
        metadata["injected"] = JsonValue.bool(true)
        assertEquals(setOf("original"), typed.metadata.keys)
        val box = typed.toInstructionBox(); val wire = assertIs<WirePayload>(box.payload)
        val first = NoritoJavaCodecAdapter.encodeInstructionBox(box)
        wire.payloadBytes.fill(0)
        assertContentEquals(first, NoritoJavaCodecAdapter.encodeInstructionBox(box))
        assertFailsWith<IllegalArgumentException> { CreateKaigiInstruction.create(call, "alias@wonderland").toInstructionBox() }
        assertFailsWith<IllegalArgumentException> { CreateKaigiInstruction.create(KaigiInstructionUtils.CallId("wonderland", "call"), accounts[0]).toInstructionBox() }
    }

    private fun le(value: BigInteger): ByteArray = ByteArray(32) { value.shiftRight(8 * it).and(BigInteger.valueOf(255)).toByte() }
    private fun field(bytes: ByteArray): ByteArray = NoritoEncoder(2).also { it.writeLength(bytes.size.toLong(), true); it.writeBytes(bytes) }.toByteArray()
    private fun fields(bytes: ByteArray): List<ByteArray> {
        val reader = NoritoDecoder(bytes, 2); val result = ArrayList<ByteArray>()
        while (reader.remaining() != 0) result.add(reader.readBytes(reader.readLength(true).toInt()))
        return result
    }
    private fun frame(bytes: ByteArray, action: String): ByteArray = rawFrame(bytes, "iroha_data_model::isi::kaigi::$action")
    private fun rawFrame(bytes: ByteArray, schema: String): ByteArray = NoritoCodec.encode(bytes, schema, object : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) { encoder.writeBytes(value) }
        override fun decode(decoder: NoritoDecoder): ByteArray = error("encode-only mutation helper")
    })
    private fun loadFixture(): Map<*, *> {
        val path = "python/iroha_python/tests/fixtures/kaigi_instruction_wire_v1.json"
        val file = generateSequence(File(".").canonicalFile) { it.parentFile }.map { File(it, path) }.first(File::isFile)
        return JsonParser.parse(file.readText(Charsets.UTF_8)) as Map<*, *>
    }
}
