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
    @Suppress("UNCHECKED_CAST")
    fun `ABI 7 fixture manifest matches archive fixture`() {
        val manifest = JsonParser.parse(
            sharedRecursiveSpendFixture(FixtureAbi.ABI7, "manifest.json"),
        ) as Map<String, Any?>
        assertEquals(
            setOf(
                "schema",
                "fixture_kind",
                "archive_fixture",
                "native_bridge_abi_version",
                "operation_count",
                "generator",
                "domains",
                "operations",
            ),
            manifest.keys,
        )
        assertEquals("iroha.kagemusha.recursive_spend.abi7.fixture_manifest.v1", manifest["schema"])
        assertEquals("native_bridge_norito_archives", manifest["fixture_kind"])
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
            (manifest["native_bridge_abi_version"] as Number).toInt(),
        )

        val archiveFixtureRef = manifest["archive_fixture"] as Map<String, Any?>
        assertEquals(setOf("path", "schema"), archiveFixtureRef.keys)
        assertEquals("fixtures/kagemusha_recursive_spend_abi7/archives.json", archiveFixtureRef["path"])
        assertEquals(
            "iroha.kagemusha.recursive_spend.abi7.archive_fixtures.v1",
            archiveFixtureRef["schema"],
        )

        val generator = manifest["generator"] as Map<String, Any?>
        assertEquals(setOf("crate", "test", "print_env"), generator.keys)
        assertEquals("iroha_python_rs", generator["crate"])
        assertEquals(
            "kagemusha_recursive_spend_abi7_archive_fixture_matches_python_native_bridge",
            generator["test"],
        )
        assertEquals("KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI7_ARCHIVES", generator["print_env"])

        val domains = manifest["domains"] as Map<String, Any?>
        assertEquals(setOf("lineage_accumulator", "fixture_label"), domains.keys)
        assertEquals(
            "iroha:kagemusha:v1:recursive-spend-accumulator",
            domains["lineage_accumulator"],
        )
        assertEquals("kagemusha-recursive-spend-python-real", domains["fixture_label"])

        val expectedOperations = mapOf(
            "append_bundle" to listOf("append", "KagemushaRecursiveSpendBundleV1", "bundle"),
            "verify_request" to listOf("verify", "KagemushaRecursiveSpendVerifyRequestV1", "request"),
            "verify_result" to listOf("verify", "KagemushaRecursiveSpendVerifyResultV1", "result"),
            "redeem_request" to listOf("redeem", "KagemushaRecursiveSpendRedeemRequestV1", "request"),
            "redeem_instruction" to listOf("redeem", "RedeemKagemushaRecursive", "instruction"),
        )
        val operations = manifest["operations"] as List<Map<String, Any?>>
        assertEquals(expectedOperations.size, (manifest["operation_count"] as Number).toInt())
        assertEquals(expectedOperations.size, operations.size)
        val operationsByName = operations.associateBy { it["name"] as String }
        assertEquals(expectedOperations.keys, operationsByName.keys)
        for ((name, expected) in expectedOperations) {
            val operation = operationsByName.getValue(name)
            assertEquals(setOf("name", "operation", "norito_type", "archive_kind"), operation.keys)
            assertEquals(expected[0], operation["operation"])
            assertEquals(expected[1], operation["norito_type"])
            assertEquals(expected[2], operation["archive_kind"])
        }

        val archiveFixture = JsonParser.parse(
            sharedRecursiveSpendFixture(FixtureAbi.ABI7, "archives.json"),
        ) as Map<String, Any?>
        assertEquals(
            setOf("schema", "fixture_kind", "native_bridge_abi_version", "archives"),
            archiveFixture.keys,
        )
        assertEquals(archiveFixtureRef["schema"], archiveFixture["schema"])
        assertEquals("native_bridge_norito_archives", archiveFixture["fixture_kind"])
        assertEquals(
            (manifest["native_bridge_abi_version"] as Number).toInt(),
            (archiveFixture["native_bridge_abi_version"] as Number).toInt(),
        )
        val archives = archiveFixture["archives"] as List<Map<String, Any?>>
        assertEquals(expectedOperations.size, archives.size)
        assertEquals(expectedOperations.keys, archives.map { it["name"] as String }.toSet())
        for (archive in archives) {
            assertEquals(
                setOf("name", "operation", "norito_type", "byte_len", "sha256_hex", "bytes_base64"),
                archive.keys,
            )
            val expected = expectedOperations.getValue(archive["name"] as String)
            assertEquals(expected[0], archive["operation"])
            assertEquals(expected[1], archive["norito_type"])
            val archiveBytes = Base64.getDecoder().decode(archive["bytes_base64"] as String)
            assertEquals(archiveBytes.size, (archive["byte_len"] as Number).toInt())
            assertEquals(sha256Hex(archiveBytes), archive["sha256_hex"])
        }
    }

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
        val trailingVerifyResultField = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
                recursiveSpendVerifyResultWithTrailingField(),
            )
        }
        assertEquals("Trailing bytes after verify result", trailingVerifyResultField.message)
    }

    @Test
    fun `lineage witness rejects trailing fields`() {
        val malformedWitnesses = listOf(
            recursiveSpendLineageWitnessWithTrailingField() to "Trailing bytes after lineageWitness",
            recursiveSpendLineageWitnessWithTrailingPreviousProofsField() to
                "Trailing bytes after lineageWitness.previousRecursiveProofs",
            recursiveSpendLineageWitnessWithTrailingPreviousProofField() to
                "Trailing bytes after lineageWitness.previousRecursiveProofs",
            recursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField() to
                "Trailing bytes after verifier key id",
        )
        for ((archive, expected) in malformedWitnesses) {
            val error = assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendRequestCodecs.lineageWitnessHasReservedPreviousProof(archive)
            }
            assertEquals(expected, error.message)
        }
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
        assertEquals("686w6ABhTWPaCrWNjjXs7X1SW6w9", init.asset)
        val fallbackAssetBundle = KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            recursiveSpendBundleWithAccumulatorField(
                2,
                fixedArrayPayload(0x01, 16),
            ),
        )
        assertEquals("hex:01010101010101010101010101010101", fallbackAssetBundle.asset)
        assertTrue(init.initialRoot.any { it.toInt() != 0 })
        assertTrue(init.finalRoot.any { it.toInt() != 0 })
        assertEquals("7", init.currentNote.amount)
        assertEquals(
            "iroha:kagemusha:v1:recursive-spend-accumulator",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
        )
        assertTrue(init.topupAnchorNullifiers.size >= 2)
        val malformedTopupAnchorSets = listOf(
            emptyList<ByteArray>(),
            listOf(ByteArray(32)),
            listOf(
                init.topupAnchorNullifiers[0],
                init.topupAnchorNullifiers[1],
                ByteArray(32) { 0x34 },
            ),
            listOf(init.topupAnchorNullifiers[0], init.topupAnchorNullifiers[0]),
            listOf(init.topupAnchorNullifiers[1], init.topupAnchorNullifiers[0]),
            listOf(init.currentNote.noteCommitment),
            listOf(init.currentNote.spendNullifier),
        )
        malformedTopupAnchorSets.forEach { nullifiers ->
            val malformedTopupAnchors = assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                    recursiveSpendBundleWithTopupAnchorNullifiers(nullifiers),
                )
            }
            assertEquals(
                "bundle.accumulator.topup_anchor_nullifiers",
                malformedTopupAnchors.message,
            )
        }

        val append = KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
        )
        assertTrue(append.hopCount >= init.hopCount)
        assertTrue(KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(append.proofCircuitId))
        assertEquals("7Y5nGzchCJcxcv98NUoBfwBR1nTk", append.asset)
        assertTrue(append.currentNote.noteCommitment.any { it.toInt() != 0 })
        assertTrue(append.currentNote.spendNullifier.any { it.toInt() != 0 })
        assertTrue(append.currentNote.amount != "0")

        val malformedProofCircuit = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithProofCircuitId(
                    UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID,
                ),
            )
        }
        assertEquals(
            "bundle.proof_circuit_id unsupported recursive proof circuit id",
            malformedProofCircuit.message,
        )

        val malformedProofBackend = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithProofBackend(UNSUPPORTED_RECURSIVE_SPEND_PROOF_BACKEND),
            )
        }
        assertEquals("bundle.proof_backend unsupported recursive proof backend", malformedProofBackend.message)
        val malformedProofBoxBackend = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithProofBoxBackend(UNSUPPORTED_RECURSIVE_SPEND_PROOF_BACKEND),
            )
        }
        assertEquals("bundle.proof_backend unsupported recursive proof backend", malformedProofBoxBackend.message)

        val trailingRecursiveProofField = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithTrailingRecursiveProofField(),
            )
        }
        assertEquals("Trailing bytes after recursive proof", trailingRecursiveProofField.message)

        val trailingVerifierKeyIdField = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithTrailingVerifierKeyIdField(),
            )
        }
        assertEquals("Trailing bytes after verifier key id", trailingVerifierKeyIdField.message)

        val trailingProofBoxField = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithTrailingProofBoxField(),
            )
        }
        assertEquals("Trailing bytes after proof", trailingProofBoxField.message)

        val malformedProofBytes = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithEmptyProofBytes(),
            )
        }
        assertEquals("bundle.proof_bytes empty recursive proof", malformedProofBytes.message)

        val malformedProofPublicInputs = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithEmptyProofPublicInputs(),
            )
        }
        assertEquals("bundle.proof_public_inputs empty recursive proof inputs", malformedProofPublicInputs.message)

        val malformedProofPublicInputsHash = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithZeroProofPublicInputsHash(),
            )
        }
        assertEquals("bundle.proof_public_inputs_hash must be non-zero", malformedProofPublicInputsHash.message)

        val mismatchedProofPublicInputsHash = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithMismatchedProofPublicInputsHash(),
            )
        }
        assertEquals("bundle.proof_public_inputs_hash mismatch", mismatchedProofPublicInputsHash.message)

        val malformedCurrentNotes = listOf(
            Pair(
                recursiveSpendBundleWithCurrentNoteField(0, ByteArray(32)),
                "noteCommitment must be non-zero",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(1, ByteArray(32)),
                "spendNullifier must be non-zero",
            ),
            Pair(
                recursiveSpendBundleWithEqualCurrentNoteNullifier(),
                "spendNullifier must differ from noteCommitment",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(2, zeroNumericPayload()),
                "numeric amount must be greater than zero",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(0, fixedArrayPayload(0x04, 31)),
                "note_commitment must be exactly 32 bytes",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(0, fixedArrayPayload(0x04, 33)),
                "note_commitment must be exactly 32 bytes",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(1, fixedArrayPayload(0x05, 31)),
                "spend_nullifier must be exactly 32 bytes",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(1, fixedArrayPayload(0x05, 33)),
                "spend_nullifier must be exactly 32 bytes",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(
                    2,
                    numericPayload(byteArrayOf(1), scale = 1),
                ),
                "numeric scale must be zero",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(
                    2,
                    numericPayload(ByteArray(16) + byteArrayOf(1)),
                ),
                "numeric amount must fit in u128",
            ),
            Pair(
                recursiveSpendBundleWithCurrentNoteField(
                    2,
                    numericPayloadWithTrailingField(),
                ),
                "Trailing bytes after field decode",
            ),
        )
        malformedCurrentNotes.forEach { (archive, expectedMessage) ->
            val malformedCurrentNote = assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendRequestCodecs.decodeBundle(archive)
            }
            assertEquals(expectedMessage, malformedCurrentNote.message)
        }
        val trailingBundleField = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithTrailingBundleField(),
            )
        }
        assertEquals("Trailing bytes after bundle", trailingBundleField.message)
        val trailingCurrentNoteField = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithTrailingCurrentNoteField(),
            )
        }
        assertEquals("Trailing bytes after field decode", trailingCurrentNoteField.message)

        val malformedDomain = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithAccumulatorField(
                    0,
                    testStringPayload("iroha:kagemusha:v1:recursive-spend-accumulator-digest"),
                ),
            )
        }
        assertEquals(
            "bundle.accumulator.domain must be " +
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
            malformedDomain.message,
        )
        val malformedAccumulatorFields = listOf(
            Triple(1, testStringPayload("kagemusha-recursive-spend-abi-chain"), "bundle.accumulator.chain_id"),
            Triple(3, ByteArray(32), "bundle.accumulator.initial_root"),
            Triple(4, ByteArray(32), "bundle.accumulator.final_root"),
            Triple(4, init.initialRoot, "bundle.accumulator.final_root"),
            Triple(2, fixedArrayPayload(0x01, 15), "asset must be exactly 16 bytes"),
            Triple(2, fixedArrayPayload(0x01, 17), "asset must be exactly 16 bytes"),
            Triple(3, fixedArrayPayload(0x02, 31), "initial_root must be exactly 32 bytes"),
            Triple(3, fixedArrayPayload(0x02, 33), "initial_root must be exactly 32 bytes"),
            Triple(4, fixedArrayPayload(0x03, 31), "final_root must be exactly 32 bytes"),
            Triple(4, fixedArrayPayload(0x03, 33), "final_root must be exactly 32 bytes"),
            Triple(
                6,
                byteArrayOf(0, 0, 0, 0),
                "bundle.accumulator.hop_count must be in 1.." +
                    KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
            ),
            Triple(
                6,
                byteArrayOf(
                    (KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 + 1).toByte(),
                    0,
                    0,
                    0,
                ),
                "bundle.accumulator.hop_count must be in 1.." +
                    KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
            ),
            Triple(7, ByteArray(32), "bundle.accumulator.lineage_digest"),
            Triple(8, ByteArray(32) { 0x7d.toByte() }, "bundle.accumulator.aggregation_transcript_digest"),
            Triple(14, ByteArray(32) { 0x7e.toByte() }, "bundle.accumulator.append_opening_preflight_digest"),
            Triple(15, ByteArray(32) { 0x7f.toByte() }, "bundle.accumulator.append_boundary_digest"),
            Triple(21, byteArrayOf(3, 0, 0, 0), "bundle.accumulator.verifier_opening_len"),
        )
        malformedAccumulatorFields.forEach { (fieldIndex, replacement, expectedMessage) ->
            val malformedField = assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                    recursiveSpendBundleWithAccumulatorField(fieldIndex, replacement),
                )
            }
            assertEquals(expectedMessage, malformedField.message)
        }
        val trailingAccumulatorField = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                recursiveSpendBundleWithTrailingAccumulatorField(),
            )
        }
        assertEquals("Trailing bytes after accumulator", trailingAccumulatorField.message)
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
                    bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
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

        val redeemBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle")
        val redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT)
        val lineageWitness = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result")
        val lineageVerifierRecord = sampleVerifierRecord()
        val changeOutput = ByteArray(32) { (0x80 + it).toByte() }
        val redeemFields = requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(
                RedeemSpendRequest(
                    bundle = redeemBundle,
                    recipient = sampleRecipient(),
                    publicAmount = "6",
                    redeemProof = redeemProof,
                    lineageWitness = lineageWitness,
                    changeOutput = changeOutput,
                    lineageVerifierRecord = lineageVerifierRecord,
                    blockHeight = 10L,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_REDEEM_REQUEST,
        )
        assertEquals(8, redeemFields.size)
        assertContentEquals(
            compactPayload(redeemBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE),
            redeemFields[0],
        )
        assertContentEquals(
            compactPayload(redeemProof, KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            redeemFields[3],
        )
        assertContentEquals(
            compactPayload(lineageWitness, KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS),
            optionSomePayload(redeemFields[4]),
        )
        assertContentEquals(changeOutput, readFixedArrayPayload(optionSomePayload(redeemFields[5]), 32))
        assertContentEquals(
            compactPayload(
                lineageVerifierRecord.recordBytes,
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD,
            ),
            optionSomePayload(redeemFields[6]),
        )
        assertEquals(10L, readU64Payload(optionSomePayload(redeemFields[7])))

        val exactRedeemFields = requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(
                RedeemSpendRequest(
                    bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    recipient = sampleRecipient(),
                    publicAmount = "7",
                    redeemProof = redeemProof,
                    lineageVerifierRecord = lineageVerifierRecord,
                ),
            ),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_REDEEM_REQUEST,
        )
        assertEquals(8, exactRedeemFields.size)
        assertOptionNone(exactRedeemFields[4])
        assertOptionNone(exactRedeemFields[5])
        assertContentEquals(
            compactPayload(
                lineageVerifierRecord.recordBytes,
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD,
            ),
            optionSomePayload(exactRedeemFields[6]),
        )
        assertOptionNone(exactRedeemFields[7])

        val verifyFields = requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                VerifySpendRequest(
                    sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    lineageVerifierRecord,
                ),
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
        assertContentEquals(
            compactPayload(
                lineageVerifierRecord.recordBytes,
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD,
            ),
            optionSomePayload(verifyFields[1]),
        )
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
        assertEquals(
            "hop 0 transfer proof must expose exactly 9 single-row instance columns",
            extraColumnError.message,
        )

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
        assertEquals("hop 0 rootAfter must differ from rootBefore", sameRootError.message)

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
        assertEquals(
            "hop 1 rootBefore must equal previous hop rootAfter",
            rootContinuityError.message,
        )

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
        assertEquals("hop 1 chainId does not match first hop", chainError.message)

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
        assertEquals("hop 1 asset does not match first hop", assetError.message)
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
        val autoInitPallasMissingLineageKey = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
                hop = evidence,
                spendableNote = sampleNote(seed = 0x70),
                lineageVerifierKey = null,
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
                blockHeight = 12L,
            )
        }
        assertEquals(
            "lineageVerifierKey is required for recursive spend init",
            autoInitPallasMissingLineageKey.message,
        )
        val autoInitPallasWrongProfile = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
                hop = evidence,
                spendableNote = sampleNote(seed = 0x71),
                lineageKeyArtifacts = sampleAppendLineageArtifacts(seed = 0x5e).typed,
                blockHeight = 12L,
            )
        }
        assertEquals("lineageKeyArtifacts must be init artifacts", autoInitPallasWrongProfile.message)

        val previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle")
        val append = KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
            previousBundle = previousBundle,
            hop = evidence,
            pallasOpenEnvelopes = pallasOpenEnvelopes,
            spendableNote = sampleNote(seed = 0x72),
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
            spendableNote = sampleNote(seed = 0x73),
            outputCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            previousLineageVerifierRecord = sampleVerifierRecord(),
            previousProofOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
            lineageKeyArtifacts = appendLineageArtifacts.typed,
            blockHeight = 13L,
        )
        assertArchiveSchema(lineageAppend, KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST)

        val autoPreviousOpeningsWithoutLineageRecord = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
                previousBundle = previousBundle,
                hop = evidence,
                spendableNote = sampleNote(seed = 0x74),
                outputCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = null,
                previousProofOpenEnvelopes = null,
                lineageKeyArtifacts = appendLineageArtifacts.typed,
                blockHeight = 14L,
            )
        }
        assertEquals(
            "previousLineageVerifierRecord is required for lineage previous bundles",
            autoPreviousOpeningsWithoutLineageRecord.message,
        )
        val autoAppendLineageArtifactsOnAggregation = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
                previousBundle = previousBundle,
                hop = evidence,
                spendableNote = sampleNote(seed = 0x75),
                outputCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                previousProofOpenEnvelopes = null,
                lineageVerifierKey = appendLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = appendLineageArtifacts.provingKeyArchive,
                blockHeight = 15L,
            )
        }
        assertEquals(
            "lineageKeyArtifacts are only valid for lineage append output",
            autoAppendLineageArtifactsOnAggregation.message,
        )
        val autoAppendWrongProfile = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
                previousBundle = previousBundle,
                hop = evidence,
                spendableNote = sampleNote(seed = 0x76),
                outputCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                previousProofOpenEnvelopes = null,
                lineageKeyArtifacts = initLineageArtifacts.typed,
                blockHeight = 16L,
            )
        }
        assertEquals("lineageKeyArtifacts must be append artifacts", autoAppendWrongProfile.message)
    }

    @Test
    fun `proof output only evidence builders fail closed`() {
        val verifierRecord = sampleVerifierRecord()

        val pallasError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildPallasOpenEnvelopesArchive(emptyList())
        }
        assertEquals("hops must not be empty", pallasError.message)

        val bundleError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                listOf(byteArrayOf(1)),
                listOf(verifierRecord),
            )
        }
        assertEquals(
            "chainId, asset, and rootAfter are required to build KagemushaVerifiedFoldRecordBundle; " +
                "use VerifiedFoldHopEvidence inputs instead",
            bundleError.message,
        )

        val initError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
                proofOutputArchive = byteArrayOf(1),
                verifierRecord = verifierRecord,
                spendableNote = sampleNote(),
                lineageVerifierKey = ByteArray(64) { 0x5a.toByte() },
                lineageProvingKeyArchive = syntheticArchive("test.LineageProvingKeyArchive"),
            )
        }
        assertEquals(
            "recursive spend requests require explicit VerifiedFoldHopEvidence and a bridge-generated or explicit " +
                "Pallas open-envelopes archive; privacy proof outputs alone do not carry " +
                "Pallas IPA opening envelopes, chainId, asset, or rootAfter",
            initError.message,
        )

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
        assertEquals(
            "recursive spend requests require explicit VerifiedFoldHopEvidence and a bridge-generated or explicit " +
                "Pallas open-envelopes archive; privacy proof outputs alone do not carry " +
                "Pallas IPA opening envelopes, chainId, asset, or rootAfter",
            appendError.message,
        )
    }

    @Test
    fun `evidence builders reject malformed privacy and verifier records`() {
        val fixture = proofFixture(
            circuitId = KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            schema = CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
            algorithmId = "unshield",
            entrypoint = "buildConfidentialUnshieldProofV3",
        )

        val rejectedProofResult = assertFailsWith<IllegalArgumentException> {
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
        assertEquals(
            "unshieldProofOutputArchive must be a successful privacy proof result: status=1 error_code=5",
            rejectedProofResult.message,
        )

        val inactiveUnshieldVerifierRecord = assertFailsWith<IllegalArgumentException> {
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
        assertEquals(
            "unshieldVerifierRecord status must be Active",
            inactiveUnshieldVerifierRecord.message,
        )

        val unshieldProofAsFoldHop = assertFailsWith<IllegalArgumentException> {
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
        assertEquals(
            "hop 0 proofOutputArchive algorithm_id must be confidential-transfer-v2",
            unshieldProofAsFoldHop.message,
        )
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
        val shortNoteCommitment = assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(31) { 1 }, ByteArray(32) { 2 }, "1")
        }
        assertEquals("noteCommitment must be exactly 32 bytes", shortNoteCommitment.message)
        val zeroNoteCommitment = assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(32), ByteArray(32) { 2 }, "1")
        }
        assertEquals("noteCommitment must be non-zero", zeroNoteCommitment.message)
        val shortSpendNullifier = assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(32) { 1 }, ByteArray(31) { 2 }, "1")
        }
        assertEquals("spendNullifier must be exactly 32 bytes", shortSpendNullifier.message)
        val zeroSpendNullifier = assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(32) { 1 }, ByteArray(32), "1")
        }
        assertEquals("spendNullifier must be non-zero", zeroSpendNullifier.message)
        val repeatedDigest = assertFailsWith<IllegalArgumentException> {
            SpendableNoteDescriptor(ByteArray(32) { 3 }, ByteArray(32) { 3 }, "1")
        }
        assertEquals("spendNullifier must differ from noteCommitment", repeatedDigest.message)
        for ((amount, amountMessage, publicAmountMessage) in listOf(
            Triple("", "amount must be a decimal integer", "publicAmount must be a decimal integer"),
            Triple("0", "amount must be greater than zero", "publicAmount must be greater than zero"),
            Triple("00", "amount must be canonical", "publicAmount must be canonical"),
            Triple("01", "amount must be canonical", "publicAmount must be canonical"),
            Triple("0007", "amount must be canonical", "publicAmount must be canonical"),
            Triple("-1", "amount must be a decimal integer", "publicAmount must be a decimal integer"),
            Triple("+1", "amount must be a decimal integer", "publicAmount must be a decimal integer"),
            Triple("1.0", "amount must be a decimal integer", "publicAmount must be a decimal integer"),
            Triple("1e3", "amount must be a decimal integer", "publicAmount must be a decimal integer"),
            Triple("7 ", "amount must be a decimal integer", "publicAmount must be a decimal integer"),
            Triple(" 7", "amount must be a decimal integer", "publicAmount must be a decimal integer"),
            Triple(U128_MAX_PLUS_ONE, "amount must fit in u128", "publicAmount must fit in u128"),
        )) {
            val invalidAmount = assertFailsWith<IllegalArgumentException> {
                SpendableNoteDescriptor(ByteArray(32) { 4 }, ByteArray(32) { 5 }, amount)
            }
            assertEquals(amountMessage, invalidAmount.message)
            val invalidPublicAmount = assertFailsWith<IllegalArgumentException> {
                RedeemSpendRequest(
                    bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    recipient = sampleRecipient(),
                    publicAmount = amount,
                    redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                )
            }
            assertEquals(publicAmountMessage, invalidPublicAmount.message)
        }
        for ((changeOutput, expectedMessage) in listOf(
            ByteArray(31) { 1 } to "changeOutput must be exactly 32 bytes",
            ByteArray(32) to "changeOutput must be non-zero",
        )) {
            val invalidChangeOutput = assertFailsWith<IllegalArgumentException> {
                RedeemSpendRequest(
                    bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                    recipient = sampleRecipient(),
                    publicAmount = "7",
                    redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                    changeOutput = changeOutput,
                )
            }
            assertEquals(expectedMessage, invalidChangeOutput.message)
        }
        val partialBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle")
        val partialSummary = KagemushaRecursiveSpendRequestCodecs.decodeBundle(partialBundle)
        assertTrue(partialSummary.topupAnchorNullifiers.isNotEmpty())
        for (changeOutput in listOf(
            partialSummary.currentNote.noteCommitment,
            partialSummary.currentNote.spendNullifier,
            partialSummary.topupAnchorNullifiers[0],
        )) {
            val invalidReservedChangeOutput = assertFailsWith<IllegalArgumentException> {
                RedeemSpendRequest(
                    bundle = partialBundle,
                    recipient = sampleRecipient(),
                    publicAmount = "6",
                    redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                    changeOutput = changeOutput,
                )
            }
            assertEquals(
                "changeOutput must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier",
                invalidReservedChangeOutput.message,
            )
        }
        val missingChangeOutput = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "6",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertEquals(
            "changeOutput is required when publicAmount is less than current note amount",
            missingChangeOutput.message,
        )
        val overAmountWithoutChange = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "8",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertEquals(
            "publicAmount must not exceed current note amount",
            overAmountWithoutChange.message,
        )
        val fullAmountWithChange = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                changeOutput = ByteArray(32) { 0x42 },
            )
        }
        assertEquals(
            "publicAmount must be less than current note amount when changeOutput is present",
            fullAmountWithChange.message,
        )
        val overAmountWithChange = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "8",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                changeOutput = ByteArray(32) { 0x43 },
            )
        }
        assertEquals(
            "publicAmount must be less than current note amount when changeOutput is present",
            overAmountWithChange.message,
        )
        val missingLineageWitness = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertEquals(
            "lineageWitness is required for this bundle",
            missingLineageWitness.message,
        )
        val missingLineageVerifierRecord = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertEquals(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            missingLineageVerifierRecord.message,
        )
        assertFailsWith<IllegalArgumentException>("reserved lineage witness malformed") {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                lineageWitness = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS),
                lineageVerifierRecord = sampleVerifierRecord(),
            )
        }
        val semanticLineageRecordWithoutWitness = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                lineageVerifierRecord = sampleVerifierRecord(),
            )
        }
        assertEquals(
            "lineageVerifierRecord is only valid for reserved-lineage bundles or lineage witnesses",
            semanticLineageRecordWithoutWitness.message,
        )
        val semanticReservedWitnessMissingRecord = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                lineageWitness = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result"),
            )
        }
        assertEquals(
            "lineageVerifierRecord is required for lineage witnesses with reserved-lineage previous proofs",
            semanticReservedWitnessMissingRecord.message,
        )
        val semanticInitWitnessWithRecord = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                lineageWitness = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_from_init_result"),
                lineageVerifierRecord = sampleVerifierRecord(),
            )
        }
        assertEquals(
            "lineageVerifierRecord is only valid for reserved-lineage bundles or lineage witnesses",
            semanticInitWitnessWithRecord.message,
        )
    }

    @Test
    fun `typed requests reject malformed archives heights and lineage gaps before native dispatch`() {
        val initLineageArtifacts = sampleInitLineageArtifacts(seed = 0x6a)
        val appendLineageArtifacts = sampleAppendLineageArtifacts(seed = 0x6b)
        val missingInitLineageVerifierKey = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
            )
        }
        assertEquals(
            "lineageVerifierKey is required for recursive spend init",
            missingInitLineageVerifierKey.message,
        )
        val initWrongRecordBundle = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageVerifierKey = initLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
            )
        }
        assertEquals(
            "recordBundle must be a valid ${KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE} Norito archive",
            initWrongRecordBundle.message,
        )
        val verifierRecordWrongArchive = assertFailsWith<IllegalArgumentException> {
            VerifierRecordRef(
                "halo2/ipa:wrong-schema",
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertEquals(
            "recordBytes must be a valid ${KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD} Norito archive",
            verifierRecordWrongArchive.message,
        )
        val missingVerifyLineageRecord = assertFailsWith<IllegalArgumentException> {
            VerifySpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
            )
        }
        assertEquals(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            missingVerifyLineageRecord.message,
        )
        val semanticVerifyLineageRecord = assertFailsWith<IllegalArgumentException> {
            VerifySpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                lineageVerifierRecord = sampleVerifierRecord(),
            )
        }
        assertEquals(
            "lineageVerifierRecord is only valid for reserved-lineage bundles",
            semanticVerifyLineageRecord.message,
        )
        val appendLineageArtifactsOnAggregation = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x44),
                outputProofCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                lineageVerifierKey = appendLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = appendLineageArtifacts.provingKeyArchive,
            )
        }
        assertEquals(
            "lineageKeyArtifacts are only valid for lineage append output",
            appendLineageArtifactsOnAggregation.message,
        )
        val malformedLineageProvingKeyOnAggregation = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x45),
                outputProofCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                lineageVerifierKey = appendLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = byteArrayOf(0),
            )
        }
        assertEquals(
            "lineageKeyArtifacts are only valid for lineage append output",
            malformedLineageProvingKeyOnAggregation.message,
        )
        val invalidOutputWithLineageKeyMaterial = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x46),
                outputProofCircuitId = "kagemusha-recursive-spend-invalid-output-v1",
                previousLineageVerifierRecord = sampleVerifierRecord(),
                lineageVerifierKey = appendLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = appendLineageArtifacts.provingKeyArchive,
            )
        }
        assertEquals(
            "outputProofCircuitId is not valid for the previous bundle",
            invalidOutputWithLineageKeyMaterial.message,
        )
        val missingPreviousLineageRecordWithPreviousOpenings = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x47),
                outputProofCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousProofOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
            )
        }
        assertEquals(
            "previousLineageVerifierRecord is required for lineage previous bundles",
            missingPreviousLineageRecordWithPreviousOpenings.message,
        )
        val previousOpeningsOnAggregation = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x44),
                outputProofCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
                previousProofOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
            )
        }
        assertEquals(
            "previousProofOpenEnvelopes are only valid for lineage append output",
            previousOpeningsOnAggregation.message,
        )
        val previousLineageRecordOnAggregation = assertFailsWith<IllegalArgumentException> {
            AppendSpendRequest(
                previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(seed = 0x44),
                outputProofCircuitId = KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                previousLineageVerifierRecord = sampleVerifierRecord(),
            )
        }
        assertEquals(
            "previousLineageVerifierRecord is only valid for lineage previous bundles",
            previousLineageRecordOnAggregation.message,
        )
        val corruptedPallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive()
        corruptedPallasOpenEnvelopes[corruptedPallasOpenEnvelopes.lastIndex] =
            (corruptedPallasOpenEnvelopes.last().toInt() xor 0x01).toByte()
        val corruptedPallasOpenEnvelopeArchive = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = corruptedPallasOpenEnvelopes,
                currentNote = sampleNote(),
                lineageVerifierKey = initLineageArtifacts.verifierKey,
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
            )
        }
        assertTrue(corruptedPallasOpenEnvelopeArchive.message.orEmpty().startsWith("Checksum mismatch: expected 0x"))
        assertTrue(corruptedPallasOpenEnvelopeArchive.message.orEmpty().contains(" got 0x"))
        val malformedPallasOpenArchives = listOf(
            syntheticArchive("test.WrongPallasOpenEnvelopes") to
                "pallasOpenEnvelopes must be a valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope> Norito archive",
            pallasOpenEnvelopeVectorArchive(count = 0) to "pallasOpenEnvelopes requires exactly 1 envelope(s)",
            pallasOpenEnvelopeVectorArchive(count = 2) to "pallasOpenEnvelopes requires exactly 1 envelope(s)",
            pallasOpenEnvelopeVectorArchive { it.publicCurveId = 2 } to
                "pallasOpenEnvelopes[0].public.curve_id must be Pallas",
            pallasOpenEnvelopeVectorArchive { it.transcriptLabel = "" } to
                "pallasOpenEnvelopes[0] transcript_label must be non-empty",
            pallasOpenEnvelopeVectorArchive { it.transcriptLabel = "\u00e9".repeat(65) } to
                "pallasOpenEnvelopes[0] transcript_label exceeds 128 bytes",
            pallasOpenEnvelopeVectorArchive { it.includeDomainTag = false } to
                "pallasOpenEnvelopes[0].domain_tag is required",
            pallasOpenEnvelopeVectorArchive { it.vkCommitmentPayload = fixedArrayPayload(0x70, 32) } to
                "pallasOpenEnvelopes[0].vk_commitment must be exactly 32 bytes",
            pallasOpenEnvelopeVectorArchive { it.publicInputsSchemaHashPayload = fixedArrayPayload(0x71, 32) } to
                "pallasOpenEnvelopes[0].public_inputs_schema_hash must be exactly 32 bytes",
            pallasOpenEnvelopeVectorArchive { it.domainTagPayload = fixedArrayPayload(0x72, 32) } to
                "pallasOpenEnvelopes[0].domain_tag must be exactly 32 bytes",
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
            assertEquals(expectedMessage, archiveError.message)
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
        assertEquals("pallasOpenEnvelopes requires exactly 2 envelope(s)", countMismatch.message)
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
        assertEquals("lineage key artifacts are invalid for recursive spend init", wrongInitLineage.message)
        val wrongInitLineageProfile = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageKeyArtifacts = appendArtifactsOnInit.typed,
            )
        }
        assertEquals("lineageKeyArtifacts must be init artifacts", wrongInitLineageProfile.message)
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
        assertEquals("lineage key artifacts are invalid for recursive spend init", forgedCommitment.message)
        val malformedVerifierKey = assertFailsWith<IllegalArgumentException> {
            InitSpendRequest(
                recordBundle = sampleRecordBundle(),
                pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive(),
                currentNote = sampleNote(),
                lineageVerifierKey = "not-zk1".toByteArray(),
                lineageProvingKeyArchive = initLineageArtifacts.provingKeyArchive,
            )
        }
        assertEquals("lineage key artifacts are invalid for recursive spend init", malformedVerifierKey.message)
        val negativeVerifyHeight = assertFailsWith<IllegalArgumentException> {
            VerifySpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                blockHeight = -1L,
            )
        }
        assertEquals("blockHeight must be non-negative", negativeVerifyHeight.message)
        val verifyWrongBundle = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                VerifySpendRequest(sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result")),
            )
        }
        assertEquals(
            "bundle must be a valid ${KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE} Norito archive",
            verifyWrongBundle.message,
        )
        val redeemWrongBundle = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertEquals(
            "bundle must be a valid ${KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE} Norito archive",
            redeemWrongBundle.message,
        )
        val redeemWrongProof = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
                lineageWitness = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_from_init_result"),
            )
        }
        assertEquals(
            "redeemProof must be a valid ${KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT} Norito archive",
            redeemWrongProof.message,
        )
        val redeemWrongLineageWitness = assertFailsWith<IllegalArgumentException> {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                lineageWitness = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
            )
        }
        assertEquals(
            "lineageWitness must be a valid ${KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS} Norito archive",
            redeemWrongLineageWitness.message,
        )
        assertFailsWith<IllegalArgumentException>("reserved lineage witness malformed") {
            RedeemSpendRequest(
                bundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                recipient = sampleRecipient(),
                publicAmount = "7",
                redeemProof = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                lineageWitness = syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS),
                lineageVerifierRecord = sampleVerifierRecord(),
            )
        }

        val tampered = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle")
        tampered[tampered.lastIndex] = (tampered[tampered.lastIndex].toInt() xor 0x01).toByte()
        val tamperedError = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendRequestCodecs.decodeBundle(tampered)
        }
        assertTrue(
            tamperedError.message?.contains("Checksum mismatch") == true,
            "expected tampered bundle to fail checksum validation, actual: ${tamperedError.message}",
        )

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
        assertEquals("previousProofOpenEnvelopes is required for lineage append output", error.message)

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
        assertEquals("lineage key artifacts are invalid for lineage append output", wrongAppendLineage.message)
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
        assertEquals("lineageKeyArtifacts must be append artifacts", wrongAppendLineageProfile.message)

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
                "previousProofOpenEnvelopes must be a valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope> Norito archive",
            pallasOpenEnvelopeVectorArchive(count = 0) to "previousProofOpenEnvelopes requires exactly 1 envelope(s)",
            pallasOpenEnvelopeVectorArchive(count = 2) to "previousProofOpenEnvelopes requires exactly 1 envelope(s)",
            pallasOpenEnvelopeVectorArchive { it.paramsCurveId = 2 } to
                "previousProofOpenEnvelopes[0].params.curve_id must be Pallas",
            pallasOpenEnvelopeVectorArchive { it.transcriptLabel = "" } to
                "previousProofOpenEnvelopes[0] transcript_label must be non-empty",
            pallasOpenEnvelopeVectorArchive { it.transcriptLabel = "\u00e9".repeat(65) } to
                "previousProofOpenEnvelopes[0] transcript_label exceeds 128 bytes",
            pallasOpenEnvelopeVectorArchive { it.includeVkCommitment = false } to
                "previousProofOpenEnvelopes[0].vk_commitment is required",
            pallasOpenEnvelopeVectorArchive { it.vkCommitmentPayload = fixedArrayPayload(0x70, 32) } to
                "previousProofOpenEnvelopes[0].vk_commitment must be exactly 32 bytes",
            pallasOpenEnvelopeVectorArchive { it.publicInputsSchemaHashPayload = fixedArrayPayload(0x71, 32) } to
                "previousProofOpenEnvelopes[0].public_inputs_schema_hash must be exactly 32 bytes",
            pallasOpenEnvelopeVectorArchive { it.domainTagPayload = fixedArrayPayload(0x72, 32) } to
                "previousProofOpenEnvelopes[0].domain_tag must be exactly 32 bytes",
            pallasOpenEnvelopeVectorArchive { it.trailingEnvelopeBytes = byteArrayOf(0x7f) } to
                "Trailing bytes after previousProofOpenEnvelopes[0]",
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
            assertEquals(expectedMessage, archiveError.message)
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
        var vkCommitmentPayload: ByteArray? = null,
        var publicInputsSchemaHashPayload: ByteArray? = null,
        var domainTagPayload: ByteArray? = null,
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
            writeTestOptionRaw(
                it,
                if (spec.includeVkCommitment) spec.vkCommitmentPayload ?: fixedBytes(0x70) else null,
            )
        }
        writeTestField(encoder) {
            writeTestOptionRaw(
                it,
                if (spec.includePublicInputsSchemaHash) {
                    spec.publicInputsSchemaHashPayload ?: fixedBytes(0x71)
                } else {
                    null
                },
            )
        }
        writeTestField(encoder) {
            writeTestOptionRaw(
                it,
                if (spec.includeDomainTag) spec.domainTagPayload ?: fixedBytes(0x72) else null,
            )
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

    private fun sha256Hex(bytes: ByteArray): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(bytes)
        val out = CharArray(digest.size * 2)
        val hex = "0123456789abcdef"
        for (index in digest.indices) {
            val value = digest[index].toInt() and 0xff
            out[index * 2] = hex[value ushr 4]
            out[index * 2 + 1] = hex[value and 0x0f]
        }
        return String(out)
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

    private fun recursiveSpendBundleWithAccumulatorField(
        fieldIndex: Int,
        replacement: ByteArray,
    ): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val accumulatorFields = fieldPayloads(bundleFields[0]).toMutableList()
        accumulatorFields[fieldIndex] = replacement
        bundleFields[0] = encodeFields(accumulatorFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithTopupAnchorNullifiers(nullifiers: List<ByteArray>): ByteArray =
        recursiveSpendBundleWithAccumulatorField(5, encodeSequence(nullifiers.map { it.copyOf() }))

    private fun recursiveSpendBundleWithTrailingBundleField(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        bundleFields.add(testStringPayload("ignored-extra-bundle-field"))
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendVerifyResultWithTrailingField(): ByteArray {
        val fields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI7, "verify_result"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT,
            ),
        ).toMutableList()
        fields.add(byteArrayOf(0x01))
        return NoritoCodec.encode(
            encodeFields(fields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendLineageWitnessWithTrailingField(): ByteArray {
        val fields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            ),
        ).toMutableList()
        fields.add(testStringPayload("ignored-extra-lineage-witness-field"))
        return NoritoCodec.encode(
            encodeFields(fields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendLineageWitnessWithTrailingPreviousProofsField(): ByteArray {
        val fields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            ),
        ).toMutableList()
        fields[3] = fields[3] + encodeFields(listOf(testStringPayload("ignored-extra-previous-proofs-field")))
        return NoritoCodec.encode(
            encodeFields(fields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendLineageWitnessWithTrailingPreviousProofField(): ByteArray {
        val fields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            ),
        ).toMutableList()
        val previousProofs = sequencePayloads(fields[3]).toMutableList()
        assertTrue(previousProofs.isNotEmpty())
        val previousProofFields = fieldPayloads(previousProofs[0]).toMutableList()
        previousProofFields.add(testStringPayload("ignored-extra-previous-proof-field"))
        previousProofs[0] = encodeFields(previousProofFields)
        fields[3] = encodeSequence(previousProofs)
        return NoritoCodec.encode(
            encodeFields(fields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField(): ByteArray {
        val fields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            ),
        ).toMutableList()
        val previousProofs = sequencePayloads(fields[3]).toMutableList()
        assertTrue(previousProofs.isNotEmpty())
        val previousProofFields = fieldPayloads(previousProofs[0]).toMutableList()
        val verifierKeyIdFields = fieldPayloads(previousProofFields[0]).toMutableList()
        verifierKeyIdFields.add(testStringPayload("ignored-extra-previous-verifier-key-field"))
        previousProofFields[0] = encodeFields(verifierKeyIdFields)
        previousProofs[0] = encodeFields(previousProofFields)
        fields[3] = encodeSequence(previousProofs)
        return NoritoCodec.encode(
            encodeFields(fields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithTrailingAccumulatorField(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val accumulatorFields = fieldPayloads(bundleFields[0]).toMutableList()
        accumulatorFields.add(testStringPayload("ignored-extra-accumulator-field"))
        bundleFields[0] = encodeFields(accumulatorFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithCurrentNoteField(
        fieldIndex: Int,
        replacement: ByteArray,
    ): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val accumulatorFields = fieldPayloads(bundleFields[0]).toMutableList()
        val currentNoteFields = fieldPayloads(accumulatorFields[22]).toMutableList()
        currentNoteFields[fieldIndex] = replacement
        accumulatorFields[22] = encodeFields(currentNoteFields)
        bundleFields[0] = encodeFields(accumulatorFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithTrailingCurrentNoteField(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val accumulatorFields = fieldPayloads(bundleFields[0]).toMutableList()
        val currentNoteFields = fieldPayloads(accumulatorFields[22]).toMutableList()
        currentNoteFields.add(testStringPayload("ignored-extra-current-note-field"))
        accumulatorFields[22] = encodeFields(currentNoteFields)
        bundleFields[0] = encodeFields(accumulatorFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithEqualCurrentNoteNullifier(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val accumulatorFields = fieldPayloads(bundleFields[0]).toMutableList()
        val currentNoteFields = fieldPayloads(accumulatorFields[22]).toMutableList()
        currentNoteFields[1] = currentNoteFields[0]
        accumulatorFields[22] = encodeFields(currentNoteFields)
        bundleFields[0] = encodeFields(accumulatorFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun zeroNumericPayload(): ByteArray =
        numericPayload(ByteArray(0))

    private fun numericPayload(mantissa: ByteArray, scale: Int = 0): ByteArray =
        encodeFields(
            listOf(
                littleEndianU32(mantissa.size.toLong()) + mantissa,
                littleEndianU32(scale.toLong()),
            ),
        )

    private fun numericPayloadWithTrailingField(): ByteArray =
        numericPayload(byteArrayOf(1)) + encodeFields(listOf(littleEndianU32(0x42)))

    private fun littleEndianU32(value: Long): ByteArray =
        byteArrayOf(
            (value and 0xff).toByte(),
            ((value ushr 8) and 0xff).toByte(),
            ((value ushr 16) and 0xff).toByte(),
            ((value ushr 24) and 0xff).toByte(),
        )

    private fun recursiveSpendBundleWithProofCircuitId(proofCircuitId: String): ByteArray {
        val payload = compactPayload(
            sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
        )
        val expected =
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                .toByteArray(Charsets.UTF_8)
        val replacement = proofCircuitId.toByteArray(Charsets.UTF_8)
        require(replacement.size == expected.size) { "test proof circuit id must be same length" }
        val (mutatedPayload, replacements) = replaceAllSameLength(payload, expected, replacement)
        require(replacements == 2) { "test proof circuit id fixture replacements must be exhaustive" }
        return NoritoCodec.encode(
            mutatedPayload,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithProofBackend(proofBackend: String): ByteArray {
        val payload = compactPayload(
            sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
        )
        val expected =
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND
                .toByteArray(Charsets.UTF_8)
        val replacement = proofBackend.toByteArray(Charsets.UTF_8)
        require(replacement.size == expected.size) { "test proof backend must be same length" }
        val (mutatedPayload, replacements) = replaceAllSameLength(payload, expected, replacement)
        require(replacements == 2) { "test proof backend fixture replacements must be exhaustive" }
        return NoritoCodec.encode(
            mutatedPayload,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithProofBoxBackend(proofBackend: String): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        val proofBoxFields = fieldPayloads(proofFields[3]).toMutableList()
        proofBoxFields[0] = testStringPayload(proofBackend)
        proofFields[3] = encodeFields(proofBoxFields)
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithTrailingVerifierKeyIdField(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        val verifierKeyIdFields = fieldPayloads(proofFields[0]).toMutableList()
        verifierKeyIdFields.add(testStringPayload("ignored-extra-verifier-key-field"))
        proofFields[0] = encodeFields(verifierKeyIdFields)
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithTrailingRecursiveProofField(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        proofFields.add(testStringPayload("ignored-extra-recursive-proof-field"))
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithTrailingProofBoxField(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        val proofBoxFields = fieldPayloads(proofFields[3]).toMutableList()
        proofBoxFields.add(testStringPayload("ignored-extra-proof-box-field"))
        proofFields[3] = encodeFields(proofBoxFields)
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithEmptyProofBytes(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        val proofBoxFields = fieldPayloads(proofFields[3]).toMutableList()
        proofBoxFields[1] = testPayload { writeTestBytesVec(this, ByteArray(0)) }
        proofFields[3] = encodeFields(proofBoxFields)
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithEmptyProofPublicInputs(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        proofFields[1] = ByteArray(0)
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithZeroProofPublicInputsHash(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        proofFields[2] = ByteArray(32)
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun recursiveSpendBundleWithMismatchedProofPublicInputsHash(): ByteArray {
        val bundleFields = fieldPayloads(
            compactPayload(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            ),
        ).toMutableList()
        val proofFields = fieldPayloads(bundleFields[1]).toMutableList()
        val mismatchedHash = proofFields[2].copyOf()
        mismatchedHash[0] = (mismatchedHash[0].toInt() xor 0x01).toByte()
        proofFields[2] = mismatchedHash
        bundleFields[1] = encodeFields(proofFields)
        return NoritoCodec.encode(
            encodeFields(bundleFields),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun replaceAllSameLength(
        source: ByteArray,
        expected: ByteArray,
        replacement: ByteArray,
    ): Pair<ByteArray, Int> {
        require(expected.isNotEmpty()) { "test expected bytes must not be empty" }
        require(expected.size == replacement.size) { "test replacement must be same length" }
        val output = source.copyOf()
        var replacements = 0
        var index = 0
        while (index <= output.size - expected.size) {
            var matched = true
            for (fieldIndex in expected.indices) {
                if (output[index + fieldIndex] != expected[fieldIndex]) {
                    matched = false
                    break
                }
            }
            if (matched) {
                replacement.copyInto(output, index)
                replacements += 1
                index += replacement.size
            } else {
                index += 1
            }
        }
        return output to replacements
    }

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

    private fun encodeFields(fields: List<ByteArray>): ByteArray =
        testPayload {
            for (field in fields) {
                writeLength(field.size.toLong(), true)
                writeBytes(field)
            }
        }

    private fun encodeSequence(fields: List<ByteArray>): ByteArray =
        testPayload {
            writeUInt(fields.size.toLong(), 64)
            for (field in fields) {
                writeLength(field.size.toLong(), true)
                writeBytes(field)
            }
        }

    private fun fixedArrayPayload(value: Int, count: Int): ByteArray =
        encodeFields(List(count) { byteArrayOf(value.toByte()) })

    private fun testStringPayload(value: String): ByteArray =
        testPayload { writeTestString(this, value) }

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
        private const val UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID =
            "kagemusha-recursive-spend-lineage-badhop-v1"
        private const val UNSUPPORTED_RECURSIVE_SPEND_PROOF_BACKEND = "halo2/kzg"
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
