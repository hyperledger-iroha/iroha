package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.core.model.NetworkId
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

/** Metadata fixtures authenticate no proof. Genuine page positives are in the native C suite. */
class NativeFinalityCheckpointAdvanceBridgeTest {
    private val network = NetworkId.fromBytes(ByteArray(32) { 0x11 })
    private val checkpoint = byteArrayOf(1, 2, 3)
    private val sha = MessageDigest.getInstance("SHA-256").digest(checkpoint)
    private fun hex(bytes: ByteArray) = bytes.joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun metadata(): Map<String, Any?> = mapOf(
        "schema" to "iroha.fi-checkpoint-advance.verified.v1", "networkId" to network.literal,
        "chainId" to "chain", "anchorHeight" to "9007199254740993", "height" to "9007199254740995",
        "proofCount" to "3", "checkpointSha256" to hex(sha), "contextId" to "33".repeat(32), "blockHash" to "55".repeat(32))
    private fun decode(fields: Map<String, Any?> = metadata(), bytes: ByteArray = checkpoint) =
        VerifiedFiCheckpointAdvance.fromNativeResult(bytes, JsonEncoder.encode(fields).toByteArray(),
            network, "chain", BigInteger("9007199254740993"), 3)

    @Test fun typedAdvanceRetainsExactCheckpointAndLosslessMetadataWithDefensiveCopies() {
        val bytes = checkpoint.copyOf(); val value = decode(bytes = bytes); bytes.fill(0)
        value.checkpointBytes.fill(0); value.contextId.fill(0); value.blockHash.fill(0); value.checkpointSha256.fill(0)
        assertContentEquals(checkpoint, value.checkpointBytes); assertContentEquals(sha, value.checkpointSha256)
        assertEquals(BigInteger("9007199254740995"), value.height)
        assertEquals(BigInteger("9007199254740993"), value.anchorHeight)
        assertEquals(3, value.proofCount); assertEquals(network, value.networkId)
    }

    @Test fun typedAdvanceRejectsSubstitutedCheckpointUnknownFieldsAndMismatchedLocalPins() {
        for (fields in listOf(metadata() + ("unknown" to null), metadata() - "contextId",
            metadata() + ("height" to "9007199254740994"), metadata() + ("proofCount" to 3),
            metadata() + ("chainId" to "other"), metadata() + ("anchorHeight" to "2"),
            metadata() + ("contextId" to "22".repeat(32)), metadata() + ("checkpointSha256" to "AA".repeat(32)))) {
            assertFails { decode(fields) }
        }
        assertFails { decode(bytes = byteArrayOf(4, 5, 6)) }
        assertFails { VerifiedFiCheckpointAdvance.fromNativeResult(checkpoint, ByteArray(4097), network, "chain", BigInteger.TEN, 3) }
    }

    @Test fun inputBoundsSingleFrameAndWrongOriginalDigestFailBeforeNativeCall() {
        fun verify(frames: List<ByteArray> = listOf(byteArrayOf(1), byteArrayOf(2)),
            bytes: ByteArray = checkpoint, digest: ByteArray = sha) =
            NativeFinalityCheckpointAdvanceBridge.advanceFiCheckpointV1(bytes, frames, network, "chain",
                digest, BigInteger.valueOf(2), ByteArray(32) { 0x33 })
        assertFailsWith<IllegalArgumentException> { verify(frames = emptyList()) }
        assertFailsWith<IllegalArgumentException> { verify(frames = listOf(byteArrayOf(1))) }
        assertFailsWith<IllegalArgumentException> { verify(frames = List(4097) { byteArrayOf(1) }) }
        assertFailsWith<IllegalArgumentException> { verify(frames = listOf(ByteArray(16 * 1024 * 1024), byteArrayOf(1))) }
        assertFailsWith<IllegalArgumentException> { verify(frames = listOf(byteArrayOf(), byteArrayOf(1))) }
        assertFailsWith<IllegalArgumentException> { verify(bytes = byteArrayOf()) }
        assertFailsWith<IllegalArgumentException> { verify(digest = ByteArray(32) { 0x55 }) }
    }

    @Test fun realJniRejectsMalformedFramesUnderMatchingRetainedCheckpointDigest() {
        assertTrue(NativeFinalityCheckpointAdvanceBridge.isNativeAvailable(), "Requires qualified checkpoint advance JNI producer")
        assertFailsWith<IllegalArgumentException> {
            NativeFinalityCheckpointAdvanceBridge.advanceFiCheckpointV1(checkpoint,
                listOf(byteArrayOf(1), byteArrayOf(2)), network, "chain", sha,
                BigInteger.valueOf(2), ByteArray(32) { 0x33 })
        }
    }
}

/** Pinned public inputs only. Source/native-artifact matching is an independent run gate.
 * Receipt assertions are not proved by parsing JSON. World file hashes do not qualify its decoder.
 * Ordinary suite discovery skips only when all four input keys are absent. Any selection requires
 * all four valid inputs; missing native support is a failure. Dedicated runner requires one pass, zero skips.
 * Immediately before this run, pinned native_c_driver.py --admit-only must admit these same
 * four inputs, including duplicate-key and owner-controlled-root checks.
 */
private object TimeIronwoodPublicCorpusV1 {
    data class Original(val name: String, val size: Long, val sha256: String)
    data class Corpus(val checkpoint: ByteArray, val promoted: ByteArray, val proofs: List<ByteArray>,
        val network: NetworkId, val chain: String, val anchor: BigInteger, val finalHeight: BigInteger,
        val context: ByteArray, val finalContext: ByteArray)
    fun digest(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256").digest(bytes)
        .joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun text(value: Any?): String = value as? String ?: error("text required")
    private fun hex(value: Any?, width: Int = 64): String = text(value).also {
        require(Regex("[0-9a-f]{$width}").matches(it) && it.any { char -> char != '0' }) { "nonzero lowercase hex required" }
    }
    fun unhex(value: String): ByteArray = hex(value).chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun record(value: Any?, fields: Set<String>): Map<*, *> = (value as? Map<*, *> ?: error("object required")).also {
        require(it.keys == fields) { "closed JSON fields required" }
    }
    private fun integer(value: Any?): Long = value as? Long ?: error("exact bounded JSON integer required")
    private fun decimal(value: Any?): BigInteger {
        val raw = text(value); require(Regex("[1-9][0-9]*").matches(raw))
        return BigInteger(raw).also { require(it <= BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)) }
    }
    private fun path(raw: String): java.nio.file.Path {
        val result = java.nio.file.Paths.get(raw)
        require(result.isAbsolute && result.normalize() == result && result.toRealPath() == result) { "absolute canonical path without symlinks required" }
        return result
    }
    private fun read(file: java.nio.file.Path, maximum: Long): ByteArray {
        path(file.toString())
        require(java.nio.file.Files.isRegularFile(file, java.nio.file.LinkOption.NOFOLLOW_LINKS))
        val size = java.nio.file.Files.size(file); require(size in 1..maximum)
        return java.nio.file.Files.readAllBytes(file).also { require(it.size.toLong() == size) }
    }
    private fun json(bytes: ByteArray): Any? = JsonParser.parse(java.nio.charset.StandardCharsets.UTF_8.newDecoder()
        .onMalformedInput(java.nio.charset.CodingErrorAction.REPORT).onUnmappableCharacter(java.nio.charset.CodingErrorAction.REPORT)
        .decode(java.nio.ByteBuffer.wrap(bytes)).toString())
    val inputKeys = listOf("BOI_ACTIVATION_FIXTURE_DIR", "BOI_ACTIVATION_MANIFEST_SHA256",
        "BOI_ACTIVATION_PRODUCER_RECEIPT", "BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256")
    // Selection is pure: it does not read files or touch the native bridge.
    fun selection(environment: Map<String, String>): Map<String, String> {
        val present = inputKeys.filter { environment.containsKey(it) }
        require(present.size == inputKeys.size) { "all four BOI_ACTIVATION inputs are required" }
        val selected = inputKeys.associateWith { key -> environment.getValue(key).also { require(it.isNotEmpty()) { "empty $key" } } }
        for (key in listOf("BOI_ACTIVATION_MANIFEST_SHA256", "BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256")) hex(selected.getValue(key))
        for (key in listOf("BOI_ACTIVATION_FIXTURE_DIR", "BOI_ACTIVATION_PRODUCER_RECEIPT")) {
            val raw = selected.getValue(key)
            val path = java.nio.file.Paths.get(raw)
            require(path.isAbsolute && path.normalize() == path && path.toString() == raw) { "absolute normalized activation path required" }
        }
        return selected
    }
    fun load(environment: Map<String, String>): Corpus {
        fun required(name: String) = environment[name]?.takeIf { it.isNotEmpty() } ?: error("missing $name")
        val root = path(required("BOI_ACTIVATION_FIXTURE_DIR"))
        val receiptPath = path(required("BOI_ACTIVATION_PRODUCER_RECEIPT"))
        val manifestHash = hex(required("BOI_ACTIVATION_MANIFEST_SHA256"))
        val receiptHash = hex(required("BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256"))
        val repo = generateSequence(java.io.File(System.getProperty("user.dir")).canonicalFile) { it.parentFile }
            .firstOrNull { java.io.File(it, "IrohaSwift/Package.swift").isFile }?.toPath() ?: error("cannot establish consumer source root")
        require(!root.startsWith(repo) && !receiptPath.startsWith(repo) && !receiptPath.startsWith(root)) { "external inputs required; receipt outside corpus" }
        val manifestBytes = read(root.resolve("manifest.json"), 65536)
        val receiptBytes = read(receiptPath, 65536)
        require(digest(manifestBytes) == manifestHash && digest(receiptBytes) == receiptHash) { "independent public pin differs" }
        val manifest = record(json(manifestBytes), setOf("schema", "fixtureKind", "patchBaseCommit", "patchBaseCommitIsProducerSource", "consumerAdmission",
            "networkId", "chainId", "anchorHeight", "finalHeight", "scheduleHeight", "activationHeight", "bootstrapHeight",
            "anchorContextId", "finalContextId", "predecessorSelection", "successorSelection", "checkpoint", "promotedCheckpoint", "world", "proofs"))
        require(manifest["schema"] == "iroha.time-ironwood-native-consumer-fixture.v1" && manifest["fixtureKind"] == "certified-synthetic-activation-history"
            && manifest["consumerAdmission"] == "requires-external-producer-receipt" && manifest["patchBaseCommitIsProducerSource"] == false)
        hex(manifest["patchBaseCommit"], 40) // This is never treated as the producer source.
        hex(manifest["predecessorSelection"]); hex(manifest["successorSelection"])
        val anchor = decimal(manifest["anchorHeight"]); val finalHeight = decimal(manifest["finalHeight"])
        val schedule = decimal(manifest["scheduleHeight"]); val activation = decimal(manifest["activationHeight"])
        val bootstrap = decimal(manifest["bootstrapHeight"])
        require(anchor < schedule && schedule < activation && activation < bootstrap && bootstrap == finalHeight
            && finalHeight < BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE) && finalHeight - activation == BigInteger.ONE)
        val context = unhex(text(manifest["anchorContextId"])); val finalContext = unhex(text(manifest["finalContextId"]))
        fun original(value: Any?, name: String): Original {
            val ref = record(value, setOf("name", "size", "sha256")); val size = decimal(ref["size"])
            require(ref["name"] == name && !name.contains('/') && !name.contains('\\') && size <= BigInteger.valueOf(68L * 1024 * 1024))
            return Original(name, size.toLong(), hex(ref["sha256"]))
        }
        val originals = mutableListOf(original(manifest["checkpoint"], "anchor-checkpoint.norito"),
            original(manifest["promotedCheckpoint"], "promoted-checkpoint.norito"), original(manifest["world"], "world.norito"))
        val proofs = manifest["proofs"] as? List<*> ?: error("proof list required")
        require(proofs.size in 2..4096 && finalHeight - anchor == BigInteger.valueOf(proofs.size.toLong() - 1))
        proofs.forEachIndexed { index, ref -> originals.add(original(ref, "proof-%04d.norito".format(java.util.Locale.ROOT, index))) }
        require(originals[2].size <= 32L * 1024 * 1024 && originals.drop(3).sumOf { it.size } <= 16L * 1024 * 1024)
        val receipt = record(json(receiptBytes), setOf("schema", "source", "testExecutable", "buildReceiptSha256", "run", "fixtureManifestSha256", "files"))
        require(receipt["schema"] == "boi.time-ironwood-fixture-producer.v1" && hex(receipt["fixtureManifestSha256"]) == manifestHash)
        val source = record(receipt["source"], setOf("commit", "tree", "signedHeadVerified", "cleanBefore", "cleanAfter"))
        hex(source["commit"], 40); hex(source["tree"], 40)
        for (flag in listOf("signedHeadVerified", "cleanBefore", "cleanAfter")) require(source[flag] == true)
        val executable = record(receipt["testExecutable"], setOf("sha256", "size"))
        hex(executable["sha256"]); require(integer(executable["size"]) > 0); hex(receipt["buildReceiptSha256"])
        val run = record(receipt["run"], setOf("testName", "exitCode", "passed", "failed", "ignored"))
        require(run["testName"] == "runtime::taira_activation::tests::export_time_ironwood_activation_originals_for_native_consumers")
        for ((name, expected) in mapOf("exitCode" to 0L, "passed" to 1L, "failed" to 0L, "ignored" to 0L)) require(integer(run[name]) == expected)
        val admitted = mutableMapOf<String, Original>()
        for (value in receipt["files"] as? List<*> ?: error("producer files required")) {
            val ref = record(value, setOf("name", "size", "sha256")); val name = text(ref["name"]); val size = integer(ref["size"])
            require(size > 0 && name !in admitted); admitted[name] = Original(name, size, hex(ref["sha256"]))
        }
        require(admitted == originals.associateBy { it.name }) { "producer original set differs" }
        val contents = java.nio.file.Files.newDirectoryStream(root).use { stream -> stream.map { it.fileName.toString() }.toSet() }
        require(contents == originals.map { it.name }.toSet() + "manifest.json") { "unexpected corpus contents" }
        val held = mutableMapOf<String, ByteArray>()
        for (ref in originals) {
            val bytes = read(root.resolve(ref.name), 68L * 1024 * 1024)
            require(bytes.size.toLong() == ref.size && digest(bytes) == ref.sha256) { "original bytes differ" }
            if (ref.name != "world.norito") held[ref.name] = bytes
        }
        val chain = text(manifest["chainId"]); require(chain.isNotEmpty() && chain.toByteArray(Charsets.UTF_8).size <= 1024)
        return Corpus(held.getValue(originals[0].name), held.getValue(originals[1].name), originals.drop(3).map { held.getValue(it.name) },
            NetworkId.parse(text(manifest["networkId"])), chain, anchor, finalHeight, context, finalContext)
    }
}

class NativeTimeIronwoodActivationFixtureTest {
    @Test fun pinnedExternalActivationCallsRealJniAndRefusesSubstitutions() {
        val fixture = TimeIronwoodPublicCorpusV1.load(TimeIronwoodPublicCorpusV1.selection(System.getenv()))
        val digest = MessageDigest.getInstance("SHA-256").digest(fixture.checkpoint)
        fun advance(frames: List<ByteArray>, checkpoint: ByteArray = fixture.checkpoint, network: NetworkId = fixture.network,
            chain: String = fixture.chain, sha: ByteArray = digest, height: BigInteger = fixture.anchor, context: ByteArray = fixture.context) =
            NativeFinalityCheckpointAdvanceBridge.advanceFiCheckpointV1(checkpoint, frames, network, chain, sha, height, context)
        assertTrue(NativeFinalityCheckpointAdvanceBridge.isNativeAvailable(), "Requires matching admitted native artifact; no fallback")
        val result = advance(fixture.proofs)
        assertContentEquals(fixture.promoted, result.checkpointBytes)
        assertContentEquals(MessageDigest.getInstance("SHA-256").digest(fixture.promoted), result.checkpointSha256)
        assertEquals(fixture.network, result.networkId); assertEquals(fixture.chain, result.chainId)
        assertEquals(fixture.anchor, result.anchorHeight); assertEquals(fixture.finalHeight, result.height)
        assertContentEquals(fixture.finalContext, result.contextId); assertEquals(fixture.proofs.size, result.proofCount)
        val mutated = fixture.proofs.map { it.copyOf() }; mutated.last()[0] = (mutated.last()[0].toInt() xor 1).toByte()
        assertFails { advance(mutated) }
        val truncated = fixture.proofs.toMutableList(); truncated[truncated.lastIndex] = truncated.last().dropLast(1).toByteArray()
        assertFails { advance(truncated) }; assertFails { advance(fixture.proofs.drop(1)) }
        val gap = fixture.proofs.toMutableList(); gap.removeAt(1); assertFails { advance(gap) }
        val foreign = NetworkId.fromBytes(ByteArray(32) { 0x11 })
        assertTrue(foreign != fixture.network, "Foreign network must be a distinct valid SDK identity")
        assertFails { advance(fixture.proofs, network = foreign) }
        assertFails { advance(fixture.proofs, chain = fixture.chain + "-foreign") }
        val wrongDigest = digest.copyOf(); wrongDigest[0] = (wrongDigest[0].toInt() xor 1).toByte()
        assertFails { advance(fixture.proofs, sha = wrongDigest) }
        assertFails { advance(fixture.proofs, height = fixture.anchor + BigInteger.ONE) }
        val wrongContext = fixture.context.copyOf(); wrongContext[0] = (wrongContext[0].toInt() xor 1).toByte()
        assertFails { advance(fixture.proofs, context = wrongContext) }
        assertFails { advance(emptyList()) }; assertFails { advance(listOf(fixture.proofs.first())) }
        assertFails { advance(fixture.proofs, checkpoint = byteArrayOf()) }; assertFails { advance(fixture.proofs, height = BigInteger.ZERO) }
        assertFails { advance(fixture.proofs, sha = digest.dropLast(1).toByteArray()) }
        assertFails { advance(fixture.proofs, context = fixture.context.dropLast(1).toByteArray()) }
    }
}


class NativeTimeIronwoodActivationSelectionTest {
    private val valid = mapOf("BOI_ACTIVATION_FIXTURE_DIR" to "/activation-selection/corpus",
        "BOI_ACTIVATION_MANIFEST_SHA256" to "1".repeat(64),
        "BOI_ACTIVATION_PRODUCER_RECEIPT" to "/activation-selection/producer.json",
        "BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256" to "2".repeat(64))
    @Test fun allAbsentRefuses() {
        assertFails { TimeIronwoodPublicCorpusV1.selection(emptyMap()) }
        assertFails { TimeIronwoodPublicCorpusV1.selection(mapOf("UNRELATED" to "value")) }
    }
    @Test fun everyPartialSelectionRefuses() {
        val keys = TimeIronwoodPublicCorpusV1.inputKeys
        for (mask in 1 until 15) {
            val partial = keys.filterIndexed { index, _ -> (mask and (1 shl index)) != 0 }.associateWith { valid.getValue(it) }
            assertFails { TimeIronwoodPublicCorpusV1.selection(partial) }
        }
    }
    @Test fun emptySelectedValuesRefuse() {
        for (key in TimeIronwoodPublicCorpusV1.inputKeys) assertFails { TimeIronwoodPublicCorpusV1.selection(valid + (key to "")) }
    }
    @Test fun malformedSelectedValuesRefuse() {
        for (key in listOf("BOI_ACTIVATION_MANIFEST_SHA256", "BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256")) {
            for (value in listOf("0".repeat(64), "A".repeat(64), "abc", " " + valid.getValue(key))) {
                assertFails { TimeIronwoodPublicCorpusV1.selection(valid + (key to value)) }
            }
        }
        for (key in listOf("BOI_ACTIVATION_FIXTURE_DIR", "BOI_ACTIVATION_PRODUCER_RECEIPT")) {
            for (value in listOf("relative/path", "/activation-selection/../corpus", "/activation-selection/\u0000corpus")) {
                assertFails { TimeIronwoodPublicCorpusV1.selection(valid + (key to value)) }
            }
        }
    }
    @Test fun completeWellFormedSelectionNeedsNoFiles() {
        assertEquals(valid, TimeIronwoodPublicCorpusV1.selection(valid))
    }
}
