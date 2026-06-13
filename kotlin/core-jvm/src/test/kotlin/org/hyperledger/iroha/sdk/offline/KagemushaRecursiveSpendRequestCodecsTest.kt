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
        val initLineageArtifacts = sampleInitLineageArtifacts()
        assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(
                InitSpendRequest(
                    recordBundle = sampleRecordBundle(),
                    pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                    currentNote = sampleNote(),
                    lineageKeyArtifacts = initLineageArtifacts.typed,
                    blockHeight = 7L,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST,
        )

        assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(
                AppendSpendRequest(
                    previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    recordBundle = sampleRecordBundle(),
                    pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
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
        val recordBundle = sampleRecordBundle()
        val pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive()
        val lineageArtifacts = sampleInitLineageArtifacts(seed = 0x5b)
        val lineageVerifierKey = lineageArtifacts.verifierKey
        val lineageProvingKeyArchive = lineageArtifacts.provingKeyArchive
        val note = sampleNote()

        val initFields = requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(
                InitSpendRequest(
                    recordBundle = recordBundle,
                    pallasOpenEnvelopes = pallasOpenEnvelopes,
                    currentNote = note,
                    lineageKeyArtifacts = lineageArtifacts.typed,
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
        val fixture = transferProofFixture(rootBefore)
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
    fun `verified fold record bundle rejects adversarial hop continuity and public input shapes`() {
        val asset = sampleAssetDefinition()
        val otherAsset = sampleAssetDefinition(seed = 0x41)
        val first = transferProofFixture(fixedBytes(0x61))
        val secondLinked = transferProofFixture(fixedBytes(0x62))
        val secondUnlinked = transferProofFixture(fixedBytes(0x63))

        val extraColumnError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = transferProofFixture(
                            fixedBytes(0x70),
                            extraColumns = listOf(fixedBytes(0x71)),
                        ).proofOutputArchive,
                        verifierRecord = first.verifierRecordRef,
                        chainId = "kagemusha-test-chain",
                        asset = asset,
                        rootAfter = fixedBytes(0x72),
                    ),
                ),
            )
        }
        assertTrue(extraColumnError.message.orEmpty().contains("exactly 9 single-row"))

        val sameRootError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = secondLinked.proofOutputArchive,
                        verifierRecord = secondLinked.verifierRecordRef,
                        chainId = "kagemusha-test-chain",
                        asset = asset,
                        rootAfter = fixedBytes(0x62),
                    ),
                ),
            )
        }
        assertTrue(sameRootError.message.orEmpty().contains("rootAfter must differ"))

        val rootContinuityError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = first.proofOutputArchive,
                        verifierRecord = first.verifierRecordRef,
                        chainId = "kagemusha-test-chain",
                        asset = asset,
                        rootAfter = fixedBytes(0x62),
                    ),
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = secondUnlinked.proofOutputArchive,
                        verifierRecord = secondUnlinked.verifierRecordRef,
                        chainId = "kagemusha-test-chain",
                        asset = asset,
                        rootAfter = fixedBytes(0x64),
                    ),
                ),
            )
        }
        assertTrue(rootContinuityError.message.orEmpty().contains("rootBefore must equal previous hop rootAfter"))

        val chainError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = first.proofOutputArchive,
                        verifierRecord = first.verifierRecordRef,
                        chainId = "kagemusha-test-chain",
                        asset = asset,
                        rootAfter = fixedBytes(0x62),
                    ),
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = secondLinked.proofOutputArchive,
                        verifierRecord = secondLinked.verifierRecordRef,
                        chainId = "kagemusha-other-chain",
                        asset = asset,
                        rootAfter = fixedBytes(0x63),
                    ),
                ),
            )
        }
        assertTrue(chainError.message.orEmpty().contains("chainId does not match"))

        val assetError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = first.proofOutputArchive,
                        verifierRecord = first.verifierRecordRef,
                        chainId = "kagemusha-test-chain",
                        asset = asset,
                        rootAfter = fixedBytes(0x62),
                    ),
                    VerifiedFoldHopEvidence(
                        proofOutputArchive = secondLinked.proofOutputArchive,
                        verifierRecord = secondLinked.verifierRecordRef,
                        chainId = "kagemusha-test-chain",
                        asset = otherAsset,
                        rootAfter = fixedBytes(0x63),
                    ),
                ),
            )
        }
        assertTrue(assetError.message.orEmpty().contains("asset does not match"))
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
        val pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive()
        val initLineageArtifacts = sampleInitLineageArtifacts(seed = 0x5c)

        val init = KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
            hop = evidence,
            pallasOpenEnvelopes = pallasOpenEnvelopes,
            spendableNote = sampleNote(),
            lineageKeyArtifacts = initLineageArtifacts.typed,
            blockHeight = 11L,
        )
        assertArchiveSchema(init, KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST)
        val initFields = requestFields(init, KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST)
        assertContentEquals(
            compactPayload(expectedRecordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
            initFields[0],
        )
        assertContentEquals(pallasOpenEnvelopes, readBytesVecPayload(initFields[1]))

        val previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle")
        val append = KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
            previousBundle = previousBundle,
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
                previousBundle,
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
            appendFields[0],
        )
        assertContentEquals(
            compactPayload(expectedRecordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
            appendFields[1],
        )
        assertContentEquals(pallasOpenEnvelopes, readBytesVecPayload(appendFields[2]))

        val appendLineageArtifacts = sampleAppendLineageArtifacts(seed = 0x5d)
        val lineageAppend = KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
            previousBundle = previousBundle,
            hop = evidence,
            pallasOpenEnvelopes = pallasOpenEnvelopes,
            spendableNote = sampleNote(seed = 0x72),
            outputCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            previousLineageVerifierRecord = sampleVerifierRecord(),
            previousProofOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
            lineageKeyArtifacts = appendLineageArtifacts.typed,
            blockHeight = 13L,
        )
        assertArchiveSchema(lineageAppend, KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST)
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
        val initLineageArtifacts = sampleInitLineageArtifacts(seed = 0x6a)
        val appendLineageArtifacts = sampleAppendLineageArtifacts(seed = 0x6b)
        assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageVerifierKey = initLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            VerifierRecordRef(
                "halo2/ipa:wrong-schema",
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        val corruptedPallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive()
        corruptedPallasOpenEnvelopes[corruptedPallasOpenEnvelopes.lastIndex] =
            (corruptedPallasOpenEnvelopes.last().toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = corruptedPallasOpenEnvelopes,
                currentNote = sampleNote(),
                lineageVerifierKey = initLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
            )
        }
        val malformedPallasOpenArchives = listOf(
            syntheticArchive("test.WrongPallasOpenEnvelopes") to
                "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>",
            pallasOpenEnvelopeVectorArchive(count = 0) to "requires exactly 1 envelope",
            pallasOpenEnvelopeVectorArchive(count = 2) to "requires exactly 1 envelope",
            pallasOpenEnvelopeVectorArchive { it.publicCurveId = 2 } to "curve_id must be Pallas",
            pallasOpenEnvelopeVectorArchive { it.includeDomainTag = false } to "domain_tag is required",
            pallasOpenEnvelopeVectorArchiveWithPayload(byteArrayOf(0x00)) to "Unexpected end of data",
        )
        for ((archive, expectedMessage) in malformedPallasOpenArchives) {
            val archiveError = assertFailsWith<IllegalArgumentException> {
                InitSpendRequest(
                    recordBundle = sampleRecordBundle(),
                    pallasOpenEnvelopes = archive,
                    currentNote = sampleNote(),
                    lineageVerifierKey = initLineageArtifacts.verifierKey,
                    lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
                )
            }
            assertTrue(
                archiveError.message.orEmpty().contains(expectedMessage) ||
                    archiveError.cause?.message.orEmpty().contains(expectedMessage),
                "expected `$expectedMessage` in ${archiveError.message} / ${archiveError.cause?.message}",
            )
        }
        val countMismatch = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(hopCount = 2),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageVerifierKey = initLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
            )
        }
        assertTrue(countMismatch.message.orEmpty().contains("requires exactly 2 envelope"))
        val appendArtifactsOnInit = sampleAppendLineageArtifacts(seed = 0x6c)
        val wrongInitLineage = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageVerifierKey = appendArtifactsOnInit.verifierKey,
                lineageProvingKeyArchive = appendArtifactsOnInit.provingKeyArchive,
            )
        }
        assertTrue(
            wrongInitLineage.message.orEmpty().contains("lineage key artifacts") ||
                wrongInitLineage.cause?.message.orEmpty().contains("lineage_verifier_key"),
        )
        val wrongInitLineageProfile = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageKeyArtifacts = appendArtifactsOnInit.typed,
            )
        }
        assertTrue(wrongInitLineageProfile.message.orEmpty().contains("init artifacts"))
        val forgedCommitmentArchive = lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            appendArtifactsOnInit.verifierKey,
            seed = 0x6d,
        )
        val forgedCommitment = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageVerifierKey = initLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = forgedCommitmentArchive,
            )
        }
        assertTrue(
            forgedCommitment.message.orEmpty().contains("lineage key artifacts") ||
                forgedCommitment.cause?.message.orEmpty().contains("lineage_proving_key_archive"),
        )
        val malformedVerifierKey = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageVerifierKey = "not-zk1".toByteArray(),
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
            )
        }
        assertTrue(
            malformedVerifierKey.message.orEmpty().contains("lineage key artifacts") ||
                malformedVerifierKey.cause?.message.orEmpty().contains("lineage_verifier_key"),
        )
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
        assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                lineageWitness = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
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
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x41),
                outputProofCircuitId = KagemushaRecursiveSpendProver
                    .RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                lineageVerifierKey = appendLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = appendLineageArtifacts.provingKeyArchive,
            )
        }
        assertTrue(error.message.orEmpty().contains("previousProofOpenEnvelopes is required"))

        val initArtifactsOnAppend = sampleInitLineageArtifacts(seed = 0x6e)
        val wrongAppendLineage = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x43),
                outputProofCircuitId = KagemushaRecursiveSpendProver
                    .RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                previousProofOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                lineageVerifierKey = initArtifactsOnAppend.verifierKey,
                lineageProvingKeyArchive = initArtifactsOnAppend.provingKeyArchive,
            )
        }
        assertTrue(
            wrongAppendLineage.message.orEmpty().contains("lineage key artifacts") ||
                wrongAppendLineage.cause?.message.orEmpty().contains("lineage_verifier_key"),
        )
        val wrongAppendLineageProfile = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x43),
                outputProofCircuitId = KagemushaRecursiveSpendProver
                    .RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                previousProofOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                lineageKeyArtifacts = initArtifactsOnAppend.typed,
                blockHeight = null,
            )
        }
        assertTrue(wrongAppendLineageProfile.message.orEmpty().contains("append artifacts"))

        assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(
                AppendSpendRequest(
                    previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    recordBundle = sampleRecordBundle(),
                    pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                    currentNote = sampleNote(seed = 0x44),
                    outputProofCircuitId = KagemushaRecursiveSpendProver
                        .RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                    previousLineageVerifierRecord = sampleVerifierRecord(),
                    previousProofOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                    lineageKeyArtifacts = appendLineageArtifacts.typed,
                    blockHeight = null,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST,
        )

        val malformedPreviousOpenArchives = listOf(
            syntheticArchive("test.WrongPreviousProofOpenEnvelopes") to
                "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>",
            pallasOpenEnvelopeVectorArchive(count = 0) to "requires exactly 1 envelope",
            pallasOpenEnvelopeVectorArchive(count = 2) to "requires exactly 1 envelope",
            pallasOpenEnvelopeVectorArchive { it.paramsCurveId = 2 } to "curve_id must be Pallas",
            pallasOpenEnvelopeVectorArchive { it.transcriptLabel = "" } to "transcript_label must be non-empty",
            pallasOpenEnvelopeVectorArchive { it.includeVkCommitment = false } to "vk_commitment is required",
            pallasOpenEnvelopeVectorArchive { it.trailingEnvelopeBytes = byteArrayOf(0x7f) } to "Trailing bytes",
            pallasOpenEnvelopeVectorArchiveWithPayload(byteArrayOf(0x00)) to "Unexpected end of data",
        )
        for ((archive, expectedMessage) in malformedPreviousOpenArchives) {
            val archiveError = assertFailsWith<IllegalArgumentException> {
                AppendSpendRequest(
                    previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    recordBundle = sampleRecordBundle(),
                    pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                    currentNote = sampleNote(seed = 0x45),
                    outputProofCircuitId = KagemushaRecursiveSpendProver
                        .RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                    previousLineageVerifierRecord = sampleVerifierRecord(),
                    previousProofOpenEnvelopes = archive,
                    lineageVerifierKey = appendLineageArtifacts.verifierKey,
                    lineageProvingKeyArchive = appendLineageArtifacts.provingKeyArchive,
                )
            }
            assertTrue(
                archiveError.message.orEmpty().contains(expectedMessage) ||
                    archiveError.cause?.message.orEmpty().contains(expectedMessage),
                "expected `$expectedMessage` in ${archiveError.message} / ${archiveError.cause?.message}",
            )
        }

        assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x42),
                outputProofCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
            )
        }
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

    private fun transferProofFixture(
        rootBefore: ByteArray,
        extraColumns: List<ByteArray> = emptyList(),
    ): ProofFixture =
        proofFixture(
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
                ) + extraColumns,
            ),
        )

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

    private data class PallasOpenEnvelopeSpec(
        var paramsCurveId: Int = 1,
        var publicCurveId: Int = 1,
        var transcriptLabel: String = "previous-proof-open",
        var includeVkCommitment: Boolean = true,
        var includePublicInputsSchemaHash: Boolean = true,
        var includeDomainTag: Boolean = true,
        var trailingEnvelopeBytes: ByteArray = ByteArray(0),
    )

    private fun pallasOpenEnvelopeVectorArchive(
        count: Int = 1,
        mutate: (PallasOpenEnvelopeSpec) -> Unit = {},
    ): ByteArray {
        val spec = PallasOpenEnvelopeSpec()
        mutate(spec)
        val archive = NoritoCodec.encode(
            Unit,
            "test.PallasOpenEnvelopeVector",
            object : TypeAdapter<Unit> {
                override fun encode(encoder: NoritoEncoder, value: Unit) {
                    encoder.writeUInt(count.toLong(), 64)
                    repeat(count) {
                        writeTestField(encoder) { envelope ->
                            writeTestPallasOpenEnvelope(envelope, spec)
                            envelope.writeBytes(spec.trailingEnvelopeBytes)
                        }
                    }
                }

                override fun decode(decoder: NoritoDecoder): Unit =
                    throw UnsupportedOperationException("test Pallas envelope vectors are encode-only")
            },
            NoritoHeader.COMPACT_LEN,
        )
        System.arraycopy(PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH, 0, archive, 6, 16)
        return archive
    }

    private fun pallasOpenEnvelopeVectorArchiveWithPayload(payload: ByteArray): ByteArray {
        val archive = NoritoCodec.encode(
            payload,
            "test.PallasOpenEnvelopeVector",
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
        System.arraycopy(PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH, 0, archive, 6, 16)
        return archive
    }

    private fun writeTestPallasOpenEnvelope(encoder: NoritoEncoder, spec: PallasOpenEnvelopeSpec) {
        val n = 4
        writeTestField(encoder) { params ->
            writeTestField(params) { it.writeUInt(1, 16) }
            writeTestField(params) { it.writeUInt(spec.paramsCurveId.toLong(), 16) }
            writeTestField(params) { it.writeUInt(n.toLong(), 32) }
            writeTestField(params) { writeTestFixed32Sequence(it, n, 0x10) }
            writeTestField(params) { writeTestFixed32Sequence(it, n, 0x20) }
            writeTestField(params) { it.writeBytes(fixedBytes(0x30)) }
        }
        writeTestField(encoder) { public ->
            writeTestField(public) { it.writeUInt(1, 16) }
            writeTestField(public) { it.writeUInt(spec.publicCurveId.toLong(), 16) }
            writeTestField(public) { it.writeUInt(n.toLong(), 32) }
            writeTestField(public) { it.writeBytes(fixedBytes(0x31)) }
            writeTestField(public) { it.writeBytes(fixedBytes(0x32)) }
            writeTestField(public) { it.writeBytes(fixedBytes(0x33)) }
        }
        writeTestField(encoder) { proof ->
            writeTestField(proof) { it.writeUInt(1, 16) }
            writeTestField(proof) { writeTestFixed32Sequence(it, 2, 0x40) }
            writeTestField(proof) { writeTestFixed32Sequence(it, 2, 0x50) }
            writeTestField(proof) { it.writeBytes(fixedBytes(0x60)) }
            writeTestField(proof) { it.writeBytes(fixedBytes(0x61)) }
        }
        writeTestField(encoder) { writeTestString(it, spec.transcriptLabel) }
        writeTestField(encoder) {
            writeTestOptionRaw(it, if (spec.includeVkCommitment) fixedBytes(0x70) else null)
        }
        writeTestField(encoder) {
            writeTestOptionRaw(it, if (spec.includePublicInputsSchemaHash) fixedBytes(0x71) else null)
        }
        writeTestField(encoder) {
            writeTestOptionRaw(it, if (spec.includeDomainTag) fixedBytes(0x72) else null)
        }
    }

    private fun writeTestFixed32Sequence(encoder: NoritoEncoder, count: Int, seed: Int) {
        encoder.writeUInt(count.toLong(), 64)
        repeat(count) { index ->
            writeTestField(encoder) { it.writeBytes(fixedBytes(seed + index)) }
        }
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

    private fun sampleAssetDefinition(seed: Int = 0x01): String {
        val bytes = ByteArray(16) { (it + seed).toByte() }
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

    private data class SampleLineageArtifacts(
        val verifierKey: ByteArray,
        val provingKeyArchive: ByteArray,
        val typed: KagemushaRecursiveSpendProver.LineageKeyArtifacts,
    )

    private fun sampleInitLineageArtifacts(seed: Int = 0x5a): SampleLineageArtifacts =
        sampleLineageArtifacts(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            seed,
        )

    private fun sampleAppendLineageArtifacts(seed: Int = 0x6a): SampleLineageArtifacts =
        sampleLineageArtifacts(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            seed,
        )

    private fun sampleLineageArtifacts(circuitId: String, seed: Int): SampleLineageArtifacts {
        val verifierKey = lineageVerifierKey(circuitId, seed)
        val provingKeyArchive = lineageProvingKeyArchive(circuitId, verifierKey, seed + 1)
        return SampleLineageArtifacts(
            verifierKey = verifierKey,
            provingKeyArchive = provingKeyArchive,
            typed = KagemushaRecursiveSpendProver.lineageKeyArtifacts(
                circuitId,
                SAMPLE_LINEAGE_OPENING_LEN,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                verifierKey,
                provingKeyArchive,
            ),
        )
    }

    private fun lineageVerifierKey(circuitId: String, seed: Int): ByteArray {
        val out = "ZK1\u0000".toByteArray(Charsets.US_ASCII).toMutableList()
        appendTlv(out, "IPAK", byteArrayOf(8, 0, 0, 0))
        appendTlv(out, "CID1", circuitId.toByteArray(Charsets.UTF_8))
        appendTlv(out, "H2VK", ByteArray(32) { seed.toByte() })
        return out.toByteArray()
    }

    private fun lineageProvingKeyArchive(
        circuitId: String,
        verifierKey: ByteArray,
        seed: Int,
    ): ByteArray {
        val archive = NoritoCodec.encode(
            Unit,
            "test.LineageProvingKeyArchive",
            object : TypeAdapter<Unit> {
                override fun encode(encoder: NoritoEncoder, value: Unit) {
                    writeTestField(encoder) { it.writeUInt(1, 16) }
                    writeTestField(encoder) { writeTestString(it, circuitId) }
                    writeTestField(encoder) { it.writeBytes(lineageVerifierKeyCommitment(verifierKey)) }
                    writeTestField(encoder) { writeTestBytesVec(it, ByteArray(64) { seed.toByte() }) }
                }

                override fun decode(decoder: NoritoDecoder): Unit =
                    throw UnsupportedOperationException("test lineage proving key archives are encode-only")
            },
            NoritoHeader.COMPACT_LEN,
        )
        System.arraycopy(LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH, 0, archive, 6, 16)
        return archive
    }

    private fun lineageVerifierKeyCommitment(verifierKey: ByteArray): ByteArray =
        verifyingKeyCommitment(
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            verifierKey,
        )

    private fun sampleRecordBundle(hopCount: Int = 1): ByteArray {
        require(hopCount >= 1)
        val asset = sampleAssetDefinition()
        val hops = ArrayList<VerifiedFoldHopEvidence>()
        var rootBefore = fixedBytes(0x31)
        repeat(hopCount) { index ->
            val rootAfter = fixedBytes(0x32 + index)
            val fixture = transferProofFixture(rootBefore)
            hops.add(
                VerifiedFoldHopEvidence(
                    proofOutputArchive = fixture.proofOutputArchive,
                    verifierRecord = fixture.verifierRecordRef,
                    chainId = "kagemusha-test-chain",
                    asset = asset,
                    rootAfter = rootAfter,
                ),
            )
            rootBefore = rootAfter
        }
        return KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops)
    }

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
        private const val SAMPLE_LINEAGE_OPENING_LEN = 2
        private val PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH = byteArrayOf(
            0xfe.toByte(),
            0x38,
            0x26,
            0x32,
            0x8f.toByte(),
            0x08,
            0x17,
            0x71,
            0x75,
            0x0f,
            0x24,
            0xfe.toByte(),
            0x11,
            0x02,
            0x60,
            0xca.toByte(),
        )
        private val LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = byteArrayOf(
            0xc8.toByte(),
            0x84.toByte(),
            0x89.toByte(),
            0x61,
            0x8a.toByte(),
            0x01,
            0x2c,
            0x28,
            0x3f,
            0xf3.toByte(),
            0xbb.toByte(),
            0x2e,
            0xba.toByte(),
            0xbc.toByte(),
            0x77,
            0x75,
        )
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
