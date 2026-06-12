package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.crypto.Blake2b
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
    fun `redeem proof attachment builder emits canonical proof attachment archive`() {
        val fixture = proofFixture(
            circuitId = KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            schema = CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
            algorithmId = "unshield",
            entrypoint = "buildConfidentialUnshieldProofV3",
        )

        val attachment = KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
            fixture.proofOutputArchive,
            fixture.verifierRecordRef,
        )

        assertArchiveSchema(attachment, KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT)
        val fields = requestFields(attachment, KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT)
        assertEquals(6, fields.size)
        assertEquals("halo2/ipa", readStringPayload(fields[0]))

        val proofBoxFields = fieldPayloads(fields[1])
        assertEquals("halo2/ipa", readStringPayload(proofBoxFields[0]))
        assertContentEquals(fixture.envelopeArchive, readBytesVecPayload(proofBoxFields[1]))

        val vkRefFields = fieldPayloads(fields[2])
        assertEquals("halo2/ipa", readStringPayload(vkRefFields[0]))
        assertEquals(fixture.verifierKeyName, readStringPayload(vkRefFields[1]))
        assertContentEquals(fixture.commitment, readFixedArrayPayload(optionSomePayload(fields[3]), 32))
        assertContentEquals(
            Blake2b.digest256(fixture.envelopeArchive),
            readFixedArrayPayload(optionSomePayload(fields[4]), 32),
        )
        assertOptionNone(fields[5])
    }

    @Test
    fun `verified fold record bundle builder assembles explicit hop evidence`() {
        val rootBefore = fixedBytes(0x31)
        val rootAfter = fixedBytes(0x32)
        val fixture = proofFixture(
            circuitId = KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            schema = CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA,
            algorithmId = "confidential-transfer-v2",
            entrypoint = "buildConfidentialTransferProofV2",
            proofBytes = zk1Proof(
                listOf(
                    fixedBytes(0x41),
                    fixedBytes(0x42),
                    fixedBytes(0x43),
                    ByteArray(32),
                    fixedBytes(0x44),
                    ByteArray(32),
                    rootBefore,
                    fixedBytes(0x45),
                    fixedBytes(0x46),
                ),
            ),
        )
        val asset = sampleAssetDefinition()

        val recordBundle = KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
            listOf(
                VerifiedFoldHopEvidence(
                    proofOutputArchive = fixture.proofOutputArchive,
                    verifierRecord = fixture.verifierRecordRef,
                    chainId = "kagemusha-test-chain",
                    asset = asset,
                    rootAfter = rootAfter,
                ),
            ),
        )

        assertArchiveSchema(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE)
        val fields = requestFields(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE)
        assertEquals(2, fields.size)
        val bundleFields = fieldPayloads(fields[0])
        assertEquals("kagemusha-test-chain", readStringPayload(fieldPayloads(bundleFields[0]).single()))
        assertContentEquals(AssetDefinitionIdEncoder.parseAddressBytes(asset), bundleFields[1])

        val steps = sequencePayloads(bundleFields[2])
        assertEquals(1, steps.size)
        val stepFields = fieldPayloads(steps[0])
        assertContentEquals(rootBefore, readFixedArrayPayload(stepFields[0], 32))
        assertContentEquals(fixedBytes(0x43), readFixed32VecPayload(stepFields[1]).single())
        assertContentEquals(fixedBytes(0x44), readFixed32VecPayload(stepFields[2]).single())
        assertContentEquals(rootAfter, readFixedArrayPayload(stepFields[3], 32))
        assertEquals("halo2/ipa", readStringPayload(fieldPayloads(stepFields[4])[0]))
        assertEquals("halo2/ipa", readStringPayload(fieldPayloads(stepFields[5])[0]))

        val records = sequencePayloads(fields[1])
        assertEquals(1, records.size)
        val recordFields = fieldPayloads(records[0])
        val idFields = fieldPayloads(recordFields[0])
        assertEquals("halo2/ipa", readStringPayload(idFields[0]))
        assertEquals(fixture.verifierKeyName, readStringPayload(idFields[1]))
        assertContentEquals(
            compactPayload(fixture.verifierRecordRef.recordBytes, KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD),
            recordFields[1],
        )
    }

    @Test
    fun `recursive spend request helpers assemble explicit hop evidence`() {
        val rootBefore = fixedBytes(0x51)
        val rootAfter = fixedBytes(0x52)
        val fixture = proofFixture(
            circuitId = KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            schema = CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA,
            algorithmId = "confidential-transfer-v2",
            entrypoint = "buildConfidentialTransferProofV2",
            proofBytes = zk1Proof(
                listOf(
                    fixedBytes(0x61),
                    fixedBytes(0x62),
                    fixedBytes(0x63),
                    ByteArray(32),
                    fixedBytes(0x64),
                    ByteArray(32),
                    rootBefore,
                    fixedBytes(0x65),
                    fixedBytes(0x66),
                ),
            ),
        )
        val evidence = VerifiedFoldHopEvidence(
            proofOutputArchive = fixture.proofOutputArchive,
            verifierRecord = fixture.verifierRecordRef,
            chainId = "kagemusha-test-chain",
            asset = sampleAssetDefinition(),
            rootAfter = rootAfter,
        )
        val expectedRecordBundle = KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(listOf(evidence))
        val pallasOpenEnvelopes = syntheticArchive("test.PallasOpenEnvelopes")

        val init = KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
            hop = evidence,
            pallasOpenEnvelopes = pallasOpenEnvelopes,
            spendableNote = sampleNote(),
            lineageVerifierKey = ByteArray(64) { 0x5a.toByte() },
            lineageProvingKeyArchive = syntheticArchive("test.LineageProvingKeyArchive"),
            blockHeight = 11L,
        )
        assertArchiveSchema(init, KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST)
        val initFields = requestFields(init, KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST)
        assertContentEquals(
            compactPayload(expectedRecordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
            initFields[0],
        )
        assertContentEquals(pallasOpenEnvelopes, readBytesVecPayload(initFields[1]))

        val append = KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
            previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
            hop = evidence,
            pallasOpenEnvelopes = pallasOpenEnvelopes,
            spendableNote = sampleNote(seed = 0x71),
            outputCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            previousLineageVerifierRecord = sampleVerifierRecord(),
            blockHeight = 12L,
        )
        assertArchiveSchema(append, KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST)
        val appendFields = requestFields(append, KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST)
        assertContentEquals(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
            appendFields[0],
        )
        assertContentEquals(
            compactPayload(expectedRecordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
            appendFields[1],
        )
        assertContentEquals(pallasOpenEnvelopes, readBytesVecPayload(appendFields[2]))
    }

    @Test
    fun `proof output only evidence builders fail closed`() {
        val verifierRecord = sampleVerifierRecord()

        val pallasError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildPallasOpenEnvelopesArchive(listOf(byteArrayOf(1)))
        }
        assertTrue(pallasError.message.orEmpty().contains("cannot be derived"))

        val bundleError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(byteArrayOf(1)),
                listOf(verifierRecord),
            )
        }
        assertTrue(bundleError.message.orEmpty().contains("chainId, asset, and rootAfter"))

        val initError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
                proofOutputArchive = byteArrayOf(1),
                verifierRecord = verifierRecord,
                spendableNote = sampleNote(),
                lineageVerifierKey = ByteArray(64) { 0x5a.toByte() },
                lineageProvingKeyArchive = syntheticArchive("test.LineageProvingKeyArchive"),
            )
        }
        assertTrue(initError.message.orEmpty().contains("VerifiedFoldHopEvidence"))

        val appendError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                proofOutputArchive = byteArrayOf(1),
                verifierRecord = verifierRecord,
                spendableNote = sampleNote(),
                outputCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = verifierRecord,
            )
        }
        assertTrue(appendError.message.orEmpty().contains("Pallas open-envelopes archive"))
    }

    @Test
    fun `evidence builders reject malformed privacy and verifier records`() {
        val fixture = proofFixture(
            circuitId = KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            schema = CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
            algorithmId = "unshield",
            entrypoint = "buildConfidentialUnshieldProofV3",
        )

        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                privacyBuildResultArchive(
                    algorithmId = "unshield",
                    entrypoint = "buildConfidentialUnshieldProofV3",
                    proof = fixture.envelopeArchive,
                    status = 1,
                    errorCode = 5,
                    message = "rejected",
                ),
                fixture.verifierRecordRef,
            )
        }

        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                fixture.proofOutputArchive,
                VerifierRecordRef(
                    "halo2/ipa:${fixture.verifierKeyName}",
                    verifierRecordArchive(
                        circuitId = KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
                        schema = CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
                        verifierKey = fixture.verifierKey,
                        status = 2,
                    ),
                ),
            )
        }

        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = fixture.proofOutputArchive,
                        verifierRecord = fixture.verifierRecordRef,
                        chainId = "chain",
                        asset = sampleAssetDefinition(),
                        rootAfter = fixedBytes(0x77),
                    ),
                ),
            )
        }
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

    private data class ProofFixture(
        val proofOutputArchive: ByteArray,
        val envelopeArchive: ByteArray,
        val verifierRecordRef: VerifierRecordRef,
        val verifierKeyName: String,
        val verifierKey: ByteArray,
        val commitment: ByteArray,
    )

    private fun proofFixture(
        circuitId: String,
        schema: ByteArray,
        algorithmId: String,
        entrypoint: String,
        proofBytes: ByteArray = zk1Proof(
            listOf(
                fixedBytes(0x11),
                fixedBytes(0x12),
                fixedBytes(0x13),
                fixedBytes(0x14),
                fixedBytes(0x15),
                fixedBytes(0x16),
                fixedBytes(0x17),
                fixedBytes(0x18),
                fixedBytes(0x19),
            ),
        ),
    ): ProofFixture {
        val verifierKeyName = "kagemusha-test-${circuitId.substringAfterLast('/').replace('_', '-')}"
        val verifierKey = zk1VerifierKey(circuitId)
        val commitment = verifyingKeyCommitment("halo2/ipa", verifierKey)
        val envelope = openVerifyEnvelopeArchive(
            circuitId = circuitId,
            schema = schema,
            vkHash = commitment,
            proofBytes = proofBytes,
        )
        val proofOutput = privacyBuildResultArchive(
            algorithmId = algorithmId,
            entrypoint = entrypoint,
            proof = envelope,
        )
        val record = verifierRecordArchive(
            circuitId = circuitId,
            schema = schema,
            verifierKey = verifierKey,
        )
        return ProofFixture(
            proofOutputArchive = proofOutput,
            envelopeArchive = envelope,
            verifierRecordRef = VerifierRecordRef("halo2/ipa:$verifierKeyName", record),
            verifierKeyName = verifierKeyName,
            verifierKey = verifierKey,
            commitment = commitment,
        )
    }

    private fun privacyBuildResultArchive(
        algorithmId: String,
        entrypoint: String,
        proof: ByteArray,
        status: Int = 0,
        errorCode: Int = 0,
        message: String = "",
    ): ByteArray {
        val archive = NoritoCodec.encode(
            Unit,
            "privacy.BuildProofResultV1",
            object : TypeAdapter<Unit> {
                override fun encode(encoder: NoritoEncoder, value: Unit) {
                    writeTestField(encoder) { it.writeUInt(1, 32) }
                    writeTestField(encoder) { it.writeUInt(status.toLong(), 32) }
                    writeTestField(encoder) { it.writeUInt(errorCode.toLong(), 32) }
                    writeTestField(encoder) { writeTestString(it, message) }
                    writeTestField(encoder) { writeTestString(it, algorithmId) }
                    writeTestField(encoder) { writeTestString(it, entrypoint) }
                    writeTestField(encoder) { writeTestString(it, "halo2/ipa:kagemusha-test") }
                    writeTestField(encoder) { writeTestBytesVec(it, ByteArray(0)) }
                    writeTestField(encoder) { writeTestBytesVec(it, proof) }
                    writeTestField(encoder) { it.writeByte(0) }
                }

                override fun decode(decoder: NoritoDecoder): Unit =
                    throw UnsupportedOperationException("test privacy results are encode-only")
            },
            NoritoHeader.COMPACT_LEN,
        )
        for (index in 6 until 22) {
            archive[index] = 0x42
        }
        return archive
    }

    private fun openVerifyEnvelopeArchive(
        circuitId: String,
        schema: ByteArray,
        vkHash: ByteArray,
        proofBytes: ByteArray,
        aux: ByteArray = ByteArray(0),
    ): ByteArray =
        NoritoCodec.encode(
            Unit,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_OPEN_VERIFY_ENVELOPE,
            object : TypeAdapter<Unit> {
                override fun encode(encoder: NoritoEncoder, value: Unit) {
                    writeTestField(encoder) { it.writeUInt(0, 32) }
                    writeTestField(encoder) { writeTestString(it, circuitId) }
                    writeTestField(encoder) { it.writeBytes(vkHash) }
                    writeTestField(encoder) { writeTestBytesVec(it, schema) }
                    writeTestField(encoder) { writeTestBytesVec(it, proofBytes) }
                    writeTestField(encoder) { writeTestBytesVec(it, aux) }
                }

                override fun decode(decoder: NoritoDecoder): Unit =
                    throw UnsupportedOperationException("test OpenVerifyEnvelope archives are encode-only")
            },
            NoritoHeader.COMPACT_LEN,
        )

    private fun verifierRecordArchive(
        circuitId: String,
        schema: ByteArray,
        verifierKey: ByteArray,
        status: Int = 1,
    ): ByteArray {
        val commitment = verifyingKeyCommitment("halo2/ipa", verifierKey)
        return NoritoCodec.encode(
            Unit,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD,
            object : TypeAdapter<Unit> {
                override fun encode(encoder: NoritoEncoder, value: Unit) {
                    writeTestField(encoder) { it.writeUInt(1, 32) }
                    writeTestField(encoder) { writeTestString(it, circuitId) }
                    writeTestField(encoder) { writeTestOptionRaw(it, null) }
                    writeTestField(encoder) { writeTestString(it, "offline_kagemusha") }
                    writeTestField(encoder) { it.writeUInt(0, 32) }
                    writeTestField(encoder) { writeTestString(it, "pallas") }
                    writeTestField(encoder) { it.writeBytes(Blake2b.digest256(schema)) }
                    writeTestField(encoder) { it.writeBytes(commitment) }
                    writeTestField(encoder) { it.writeUInt(verifierKey.size.toLong(), 32) }
                    writeTestField(encoder) { it.writeUInt((192 * 1024).toLong(), 32) }
                    writeTestField(encoder) { writeTestOptionRaw(it, null) }
                    writeTestField(encoder) { writeTestOptionRaw(it, null) }
                    writeTestField(encoder) { writeTestOptionRaw(it, null) }
                    writeTestField(encoder) { writeTestOptionRaw(it, null) }
                    writeTestField(encoder) { writeTestOptionRaw(it, null) }
                    writeTestField(encoder) {
                        writeTestOptionRaw(
                            it,
                            testPayload {
                                writeTestField(this) { field -> writeTestString(field, "halo2/ipa") }
                                writeTestField(this) { field -> writeTestBytesVec(field, verifierKey) }
                            },
                        )
                    }
                    writeTestField(encoder) { it.writeUInt(status.toLong(), 8) }
                }

                override fun decode(decoder: NoritoDecoder): Unit =
                    throw UnsupportedOperationException("test verifier records are encode-only")
            },
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun zk1VerifierKey(circuitId: String): ByteArray {
        val out = "ZK1\u0000".toByteArray(Charsets.US_ASCII).toMutableList()
        appendTlv(out, "CID1", circuitId.toByteArray(Charsets.UTF_8))
        appendTlv(out, "IPAK", byteArrayOf(7, 0, 0, 0))
        appendTlv(out, "H2VK", ByteArray(32) { (it + 1).toByte() })
        return out.toByteArray()
    }

    private fun zk1Proof(columns: List<ByteArray>): ByteArray {
        val out = "ZK1\u0000".toByteArray(Charsets.US_ASCII).toMutableList()
        appendTlv(out, "PROF", byteArrayOf(0x55))
        val payload = ArrayList<Byte>()
        appendU32Le(payload, columns.size)
        appendU32Le(payload, 1)
        for (column in columns) {
            require(column.size == 32)
            for (byte in column) payload.add(byte)
        }
        appendTlv(out, "I10P", payload.toByteArray())
        return out.toByteArray()
    }

    private fun appendTlv(out: MutableList<Byte>, tag: String, payload: ByteArray) {
        for (byte in tag.toByteArray(Charsets.US_ASCII)) out.add(byte)
        appendU32Le(out, payload.size)
        for (byte in payload) out.add(byte)
    }

    private fun appendU32Le(out: MutableList<Byte>, value: Int) {
        out.add((value and 0xff).toByte())
        out.add(((value ushr 8) and 0xff).toByte())
        out.add(((value ushr 16) and 0xff).toByte())
        out.add(((value ushr 24) and 0xff).toByte())
    }

    private fun writeTestField(parent: NoritoEncoder, writePayload: (NoritoEncoder) -> Unit) {
        val child = parent.childEncoder()
        writePayload(child)
        val payload = child.toByteArray()
        parent.writeLength(payload.size.toLong(), true)
        parent.writeBytes(payload)
    }

    private fun testPayload(writePayload: NoritoEncoder.() -> Unit): ByteArray {
        val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        encoder.writePayload()
        return encoder.toByteArray()
    }

    private fun writeTestString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(Charsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), true)
        encoder.writeBytes(bytes)
    }

    private fun writeTestBytesVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun writeTestOptionRaw(encoder: NoritoEncoder, payload: ByteArray?) {
        if (payload == null) {
            encoder.writeByte(0)
            return
        }
        encoder.writeByte(1)
        encoder.writeLength(payload.size.toLong(), true)
        encoder.writeBytes(payload)
    }

    private fun verifyingKeyCommitment(backend: String, verifierKey: ByteArray): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        val backendBytes = backend.toByteArray(Charsets.UTF_8)
        digest.update("iroha:zk:v1:vk".toByteArray(Charsets.US_ASCII))
        digest.update(longBigEndian(backendBytes.size.toLong()))
        digest.update(backendBytes)
        digest.update(longBigEndian(verifierKey.size.toLong()))
        digest.update(verifierKey)
        return digest.digest()
    }

    private fun longBigEndian(value: Long): ByteArray {
        val out = ByteArray(8)
        for (index in out.indices) {
            out[index] = ((value ushr ((7 - index) * 8)) and 0xff).toByte()
        }
        return out
    }

    private fun fixedBytes(seed: Int): ByteArray = ByteArray(32) { seed.toByte() }

    private fun sampleAssetDefinition(): String {
        val bytes = ByteArray(16) { (it + 1).toByte() }
        bytes[6] = ((bytes[6].toInt() and 0x0f) or 0x40).toByte()
        bytes[8] = ((bytes[8].toInt() and 0x3f) or 0x80).toByte()
        return AssetDefinitionIdEncoder.encodeFromBytes(bytes)
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

    private fun sequencePayloads(payload: ByteArray): List<ByteArray> {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val count = decoder.readUInt(64)
        require(count <= Int.MAX_VALUE) { "test sequence too large" }
        val fields = ArrayList<ByteArray>(count.toInt())
        repeat(count.toInt()) {
            val length = decoder.readLength(true)
            require(length <= Int.MAX_VALUE) { "test sequence item too large" }
            fields.add(decoder.readBytes(length.toInt()))
        }
        assertEquals(0, decoder.remaining())
        return fields
    }

    private fun readFixed32VecPayload(payload: ByteArray): List<ByteArray> =
        sequencePayloads(payload).map { readFixedArrayPayload(it, 32) }

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
        private val CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA: ByteArray =
            (
                "{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\"," +
                    "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\"," +
                    "\"output_commitment_1\",\"root\",\"asset_tag\",\"chain_tag\"]}"
                ).toByteArray(Charsets.UTF_8)
        private val CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA: ByteArray =
            (
                "{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\"," +
                    "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\"," +
                    "\"root\",\"public_amount\",\"asset_tag\",\"chain_tag\"]}"
                ).toByteArray(Charsets.UTF_8)
    }
}
