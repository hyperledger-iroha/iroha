package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class KagemushaRecursiveSpendRequestCodecsTest {
    @Test
    fun `decode verify result reads ABI 6 and ABI 7 fields`() {
        val abi6 = KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
            sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result"),
        )
        assertFalse(abi6.valid)
        assertEquals(2, abi6.hopCount)
        assertEquals(4011, abi6.encodedBytes)
        assertEquals("fixture recursive proof is not a production proof", abi6.reason)
        assertFalse(abi6.chainAdmissible)
        assertEquals("offline verification failed", abi6.chainAdmissionReason)
        assertFalse(abi6.witnesslessRedeemSupported)
        assertTrue(abi6.lineageWitnessRequired)

        val abi7 = KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
            sharedRecursiveSpendArchive(FixtureAbi.ABI7, "verify_result"),
        )
        assertTrue(abi7.hopCount >= 1)
        assertTrue(abi7.encodedBytes > 0)
        assertEquals(abi7.chainAdmissionReason.isEmpty(), abi7.chainAdmissible)
        assertEquals(!abi7.lineageWitnessRequired, abi7.witnesslessRedeemSupported)
    }

    @Test
    fun `decode bundle extracts lineage summaries from fixture archives`() {
        val init = KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
        )
        assertEquals(1, init.hopCount)
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            init.proofCircuitId,
        )
        assertEquals("kagemusha-recursive-spend-abi-chain", init.chainId)
        assertTrue(init.asset.isNotBlank())
        assertTrue(init.initialRoot.any { it.toInt() != 0 })
        assertTrue(init.finalRoot.any { it.toInt() != 0 })
        assertEquals("7", init.currentNote.amount)

        val append = KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
        )
        assertTrue(append.hopCount >= init.hopCount)
        assertTrue(KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(append.proofCircuitId))
        assertTrue(append.currentNote.noteCommitment.any { it.toInt() != 0 })
        assertTrue(append.currentNote.spendNullifier.any { it.toInt() != 0 })
        assertTrue(append.currentNote.amount != "0")
    }

    @Test
    fun `typed encoders write expected request schemas`() {
        assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(
                InitSpendRequest(
                    recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
                    pallasOpenEnvelopes = syntheticArchive("test.PallasOpenEnvelopes"),
                    currentNote = sampleNote(),
                    lineageVerifierKey = ByteArray(64) { 0x5a.toByte() },
                    lineageProvingKeyArchive = syntheticArchive("test.LineageProvingKeyArchive"),
                    blockHeight = 7L,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST,
        )

        assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(
                AppendSpendRequest(
                    previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
                    pallasOpenEnvelopes = syntheticArchive("test.PallasOpenEnvelopes"),
                    currentNote = sampleNote(seed = 0x31),
                    outputProofCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                    previousLineageVerifierRecord = sampleVerifierRecord(),
                    blockHeight = 8L,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST,
        )

        assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                VerifySpendRequest(
                    bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                    lineageVerifierRecord = sampleVerifierRecord(),
                    blockHeight = 9L,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_REQUEST,
        )

        assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(
                RedeemSpendRequest(
                    bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                    recipient = sampleRecipient(),
                    publicAmount = "7",
                    redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                    lineageWitness = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result"),
                    lineageVerifierRecord = sampleVerifierRecord(),
                    blockHeight = 10L,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_REDEEM_REQUEST,
        )
    }

    @Test
    fun `typed encoders use Rust compatible compact field layouts`() {
        val recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE)
        val pallasOpenEnvelopes = syntheticArchive("test.PallasOpenEnvelopes")
        val lineageVerifierKey = ByteArray(64) { (it + 1).toByte() }
        val lineageProvingKeyArchive = syntheticArchive("test.LineageProvingKeyArchive")
        val note = sampleNote()

        val initFields = requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(
                InitSpendRequest(
                    recordBundle = recordBundle,
                    pallasOpenEnvelopes = pallasOpenEnvelopes,
                    currentNote = note,
                    lineageVerifierKey = lineageVerifierKey,
                    lineageProvingKeyArchive = lineageProvingKeyArchive,
                    blockHeight = 7L,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST,
        )

        assertEquals(6, initFields.size)
        assertContentEquals(
            compactPayload(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
            initFields[0],
        )
        assertContentEquals(pallasOpenEnvelopes, readBytesVecPayload(initFields[1]))

        val noteFields = fieldPayloads(initFields[2])
        assertEquals(3, noteFields.size)
        assertContentEquals(note.noteCommitment, readFixedArrayPayload(noteFields[0], 32))
        assertContentEquals(note.spendNullifier, readFixedArrayPayload(noteFields[1], 32))
        assertEquals(64, noteFields[0].size)
        assertEquals(64, noteFields[1].size)

        val lineageKeyFields = fieldPayloads(optionSomePayload(initFields[3]))
        assertEquals(2, lineageKeyFields.size)
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            readStringPayload(lineageKeyFields[0]),
        )
        assertContentEquals(lineageVerifierKey, readBytesVecPayload(lineageKeyFields[1]))
        assertContentEquals(lineageProvingKeyArchive, readBytesVecPayload(optionSomePayload(initFields[4])))
        assertEquals(7L, readU64Payload(optionSomePayload(initFields[5])))

        val verifyFields = requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                VerifySpendRequest(sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle")),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_REQUEST,
        )
        assertEquals(3, verifyFields.size)
        assertContentEquals(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
            verifyFields[0],
        )
        assertOptionNone(verifyFields[1])
        assertOptionNone(verifyFields[2])
    }

    @Test
    fun `spendable note constructors defensively copy arrays`() {
        val commitment = ByteArray(32) { 0x11 }
        val nullifier = ByteArray(32) { 0x22 }
        val note = SpendableNoteDescriptor(commitment, nullifier, "13")
        commitment[0] = 0x7f
        nullifier[0] = 0x7e

        assertEquals(0x11, note.noteCommitment[0].toInt())
        assertEquals(0x22, note.spendNullifier[0].toInt())

        val exposedCommitment = note.noteCommitment
        val exposedNullifier = note.spendNullifier
        exposedCommitment[1] = 0x55
        exposedNullifier[1] = 0x66
        assertEquals(0x11, note.noteCommitment[1].toInt())
        assertEquals(0x22, note.spendNullifier[1].toInt())
        assertContentEquals(note.noteCommitment, note.noteCommitmentBytes())
        assertContentEquals(note.spendNullifier, note.spendNullifierBytes())
    }

    @Test
    fun `spendable note rejects malformed digests and noncanonical amounts`() {
        assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(31) { 1 }, ByteArray(32) { 2 }, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(32), ByteArray(32) { 2 }, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(32) { 3 }, ByteArray(32) { 3 }, "1")
        }
        for (amount in listOf("", "0", "01", "-1", "+1", "1.0", "1e3", U128_MAX_PLUS_ONE)) {
            assertFailsWith<IllegalArgumentException> {
                SpendableNoteDescriptor(ByteArray(32) { 4 }, ByteArray(32) { 5 }, amount)
            }
        }
    }

    @Test
    fun `typed requests reject malformed archives heights and lineage gaps before native dispatch`() {
        assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
                pallasOpenEnvelopes = syntheticArchive("test.PallasOpenEnvelopes"),
                currentNote = sampleNote(),
            )
        }
        val corruptedPallasOpenEnvelopes = syntheticArchive("test.PallasOpenEnvelopes")
        corruptedPallasOpenEnvelopes[corruptedPallasOpenEnvelopes.lastIndex] =
            (corruptedPallasOpenEnvelopes.last().toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
                pallasOpenEnvelopes = corruptedPallasOpenEnvelopes,
                currentNote = sampleNote(),
                lineageVerifierKey = ByteArray(64) { 0x5a.toByte() },
                lineageProvingKeyArchive = syntheticArchive("test.LineageProvingKeyArchive"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            VerifySpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                blockHeight = -1L,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                VerifySpendRequest(sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result")),
            )
        }

        val tampered = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle")
        tampered[tampered.lastIndex] = (tampered[tampered.lastIndex].toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(tampered)
        }

        val error = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
                pallasOpenEnvelopes = syntheticArchive("test.PallasOpenEnvelopes"),
                currentNote = sampleNote(seed = 0x41),
                outputProofCircuitId = KagemushaRecursiveSpendProver
                    .RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                lineageVerifierKey = ByteArray(64) { 0x6b.toByte() },
                lineageProvingKeyArchive = syntheticArchive("test.LineageProvingKeyArchive"),
            )
        }
        assertTrue(error.message.orEmpty().contains("previousProofOpenEnvelopes is required"))
    }

    private fun sampleNote(seed: Int = 0x21): SpendableNoteDescriptor =
        SpendableNoteDescriptor(
            noteCommitment = ByteArray(32) { seed.toByte() },
            spendNullifier = ByteArray(32) { (seed + 1).toByte() },
            amount = "17",
        )

    private fun sampleVerifierRecord(): VerifierRecordRef =
        VerifierRecordRef(
            verifierKeyId = "halo2/ipa:kagemusha-recursive-spend-lineage-test",
            recordBytes = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD),
        )

    private fun sampleRecipient(): String = AccountAddress
        .fromAccount(ByteArray(32) { 0x2a.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    private fun assertArchiveSchema(archive: ByteArray, schema: String) {
        val decoded = NoritoHeader.decode(archive, SchemaHash.hash16(schema))
        decoded.header.validateChecksum(decoded.payload)
        assertEquals(NoritoHeader.COMPACT_LEN, decoded.header.flags)
        assertTrue(decoded.payload.isNotEmpty())
    }

    private fun compactPayload(archive: ByteArray, schema: String): ByteArray {
        val decoded = NoritoHeader.decode(archive, SchemaHash.hash16(schema))
        decoded.header.validateChecksum(decoded.payload)
        assertEquals(NoritoHeader.COMPACT_LEN, decoded.header.flags)
        return decoded.payload
    }

    private fun requestFields(archive: ByteArray, schema: String): List<ByteArray> =
        fieldPayloads(compactPayload(archive, schema))

    private fun fieldPayloads(payload: ByteArray): List<ByteArray> {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val fields = ArrayList<ByteArray>()
        while (decoder.remaining() > 0) {
            val length = decoder.readLength(true)
            require(length <= Int.MAX_VALUE) { "test field too large" }
            fields.add(decoder.readBytes(length.toInt()))
        }
        return fields
    }

    private fun readBytesVecPayload(payload: ByteArray): ByteArray {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val length = decoder.readUInt(64)
        require(length <= Int.MAX_VALUE) { "test Vec<u8> too large" }
        val bytes = decoder.readBytes(length.toInt())
        assertEquals(0, decoder.remaining())
        return bytes
    }

    private fun readFixedArrayPayload(payload: ByteArray, expectedSize: Int): ByteArray {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val bytes = ByteArray(expectedSize)
        for (index in 0 until expectedSize) {
            assertEquals(1L, decoder.readLength(true))
            bytes[index] = decoder.readByte().toByte()
        }
        assertEquals(0, decoder.remaining())
        return bytes
    }

    private fun readStringPayload(payload: ByteArray): String {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "test string too large" }
        val value = String(decoder.readBytes(length.toInt()), Charsets.UTF_8)
        assertEquals(0, decoder.remaining())
        return value
    }

    private fun readU64Payload(payload: ByteArray): Long {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val value = decoder.readUInt(64)
        assertEquals(0, decoder.remaining())
        return value
    }

    private fun optionSomePayload(payload: ByteArray): ByteArray {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        assertEquals(1, decoder.readByte())
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "test Option payload too large" }
        val value = decoder.readBytes(length.toInt())
        assertEquals(0, decoder.remaining())
        return value
    }

    private fun assertOptionNone(payload: ByteArray) {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        assertEquals(0, decoder.readByte())
        assertEquals(0, decoder.remaining())
    }

    private fun syntheticArchive(schema: String): ByteArray =
        NoritoCodec.encode(byteArrayOf(0x01, 0x02, 0x03), schema, RawPayloadAdapter, NoritoHeader.COMPACT_LEN)

    private object RawPayloadAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encoder.writeBytes(value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            throw UnsupportedOperationException("synthetic archives are encode-only")
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun sharedRecursiveSpendArchive(abi: FixtureAbi, name: String): ByteArray {
        val root = JsonParser.parse(sharedRecursiveSpendFixture(abi, "archives.json")) as Map<String, Any?>
        val archives = root["archives"] as List<Map<String, Any?>>
        val archive = archives.first { it["name"] == name }
        return Base64.getDecoder().decode(archive["bytes_base64"] as String)
    }

    private fun sharedRecursiveSpendFixture(abi: FixtureAbi, fileName: String): String {
        var directory: Path? = Paths.get("").toAbsolutePath()
        while (directory != null) {
            val candidate = directory.resolve(abi.directory).resolve(fileName)
            if (Files.isRegularFile(candidate)) {
                return String(Files.readAllBytes(candidate), Charsets.UTF_8)
            }
            directory = directory.parent
        }
        error("missing shared recursive spend ${abi.name} fixture $fileName")
    }

    private enum class FixtureAbi(val directory: String) {
        ABI6("fixtures/kagemusha_recursive_spend_abi6"),
        ABI7("fixtures/kagemusha_recursive_spend_abi7"),
    }

    private companion object {
        private const val U128_MAX_PLUS_ONE = "340282366920938463463374607431768211456"
    }
}
