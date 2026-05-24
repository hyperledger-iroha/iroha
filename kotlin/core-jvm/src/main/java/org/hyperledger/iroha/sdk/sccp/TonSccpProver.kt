package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.util.Base64
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** TON SCCP proof request and internal-message helpers for local-first UI proof generation. */
object SccpTon {
    const val DOMAIN_TON: Int = 4
    const val CONTRACT_PROOF_BACKEND_V1: String = "ton-contract-v1"
    const val MESSAGE_BODY_BOC_V1: String = "ton_message_body_boc_v1"

    private const val SUBMIT_OP_V1: Long = 0x53434350L
    private const val MESSAGE_SCHEMA_VERSION_V1: Int = 1
    private const val MAX_CELL_DATA_BYTES: Int = 127
    private const val MAX_REFS: Int = 4
    private val BOC_MAGIC = byteArrayOf(0xb5.toByte(), 0xee.toByte(), 0x9c.toByte(), 0x72.toByte())
    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    @JvmStatic
    fun canonicalPublicInputsBytes(input: TonSccpPublicInputsInput): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(input.version)
        out.write(hex32Bytes(input.messageId, "messageId"))
        out.write(hex32Bytes(input.payloadHash, "payloadHash"))
        writeU32Le(out, input.targetDomain)
        out.write(hex32Bytes(input.commitmentRoot, "commitmentRoot"))
        writeU64Le(out, normalizeU64(input.finalityHeight, "finalityHeight"))
        out.write(hex32Bytes(input.finalityBlockHash, "finalityBlockHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun submissionQueryId(publicInputs: TonSccpPublicInputsInput): String {
        val messageId = hex32Bytes(publicInputs.messageId, "messageId")
        var value = BigInteger.ZERO
        for (i in 0 until 8) {
            value = value.shiftLeft(8).or(BigInteger.valueOf((messageId[i].toInt() and 0xff).toLong()))
        }
        return value.toString()
    }

    @JvmStatic
    fun buildMessageBodyBoc(input: TonSccpMessageBodyInput): ByteArray {
        val publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs)
        val statementHash = hex32Bytes(input.statementHash, "statementHash")
        val destinationBindingHash = hex32Bytes(input.destinationBindingHash, "destinationBindingHash")
        val rootData = ByteArrayOutputStream()
        writeU32Be(rootData, SUBMIT_OP_V1)
        writeU64Be(rootData, normalizeU64(input.queryId ?: submissionQueryId(input.publicInputs), "queryId"))
        writeU16Be(rootData, MESSAGE_SCHEMA_VERSION_V1)
        rootData.write(statementHash)
        rootData.write(destinationBindingHash)

        val cells = ArrayList<TonCell>()
        cells.add(TonCell(rootData.toByteArray(), mutableListOf()))
        val publicInputsRoot = pushSnakeCells(cells, publicInputsBytes)
        val proofRoot = pushSnakeCells(cells, input.proofBytes)
        val bundleRoot = pushSnakeCells(cells, input.bundleBytes)
        val metadataRoot = pushSnakeCells(cells, input.metadataBytes)
        cells[0].refs.addAll(listOf(publicInputsRoot, proofRoot, bundleRoot, metadataRoot))
        return encodeBocSingleRoot(cells, 0)
    }

    @JvmStatic
    fun buildSubmission(input: TonSccpMessageBodyInput): TonSccpSubmission {
        val messageBodyBoc = buildMessageBodyBoc(input)
        return TonSccpSubmission(
            envelopeEncoding = MESSAGE_BODY_BOC_V1,
            messageBodyBoc = messageBodyBoc,
            messageBodyBocHex = "0x" + hexLower(messageBodyBoc),
        )
    }

    @JvmStatic
    fun buildProofRequest(input: TonSccpProofRequestInput): TonSccpProofRequest {
        val publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs)
        val preimage = ByteArrayOutputStream()
        preimage.write(publicInputsBytes)
        preimage.write(input.bundleBytes)
        preimage.write(input.sourceProofBytes)
        return TonSccpProofRequest(
            version = 1,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
            targetDomain = input.publicInputs.targetDomain,
            publicInputs = input.publicInputs,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
            requestHash = hashHex("sccp:ton:proof-request:v1", preimage.toByteArray()),
        )
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: TonSccpProofRequest): TonSccpProofResult {
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        return TonSccpProofResult(
            version = 1,
            backend = request.backend,
            proofBytes = proofBytes.copyOf(),
            proofBase64 = Base64.getEncoder().encodeToString(proofBytes),
            publicInputs = request.publicInputs,
            requestHash = request.requestHash,
        )
    }

    private fun pushSnakeCells(cells: MutableList<TonCell>, bytes: ByteArray): Int {
        val start = cells.size
        if (bytes.isEmpty()) {
            cells.add(TonCell(ByteArray(0), mutableListOf()))
            return start
        }
        val chunkCount = (bytes.size + MAX_CELL_DATA_BYTES - 1) / MAX_CELL_DATA_BYTES
        for (index in 0 until chunkCount) {
            val chunkStart = index * MAX_CELL_DATA_BYTES
            val chunkEnd = Math.min(chunkStart + MAX_CELL_DATA_BYTES, bytes.size)
            val chunk = bytes.copyOfRange(chunkStart, chunkEnd)
            val refs = if (index + 1 == chunkCount) mutableListOf() else mutableListOf(start + index + 1)
            cells.add(TonCell(chunk, refs))
        }
        return start
    }

    private fun encodeBocSingleRoot(cells: List<TonCell>, rootIndex: Int): ByteArray {
        require(cells.isNotEmpty()) { "cells must not be empty" }
        require(rootIndex >= 0 && rootIndex < cells.size) { "rootIndex is invalid" }
        val sizeBytes = minSizeBytes(Math.max(cells.size, rootIndex))
        val cellsBytes = serializeCells(cells, sizeBytes)
        val offsetBytes = minSizeBytes(cellsBytes.size)
        val out = ByteArrayOutputStream()
        out.write(BOC_MAGIC)
        out.write(sizeBytes)
        out.write(offsetBytes)
        out.write(sizedUInt(cells.size, sizeBytes))
        out.write(sizedUInt(1, sizeBytes))
        out.write(sizedUInt(0, sizeBytes))
        out.write(sizedUInt(cellsBytes.size, offsetBytes))
        out.write(sizedUInt(rootIndex, sizeBytes))
        out.write(cellsBytes)
        return out.toByteArray()
    }

    private fun serializeCells(cells: List<TonCell>, sizeBytes: Int): ByteArray {
        val out = ByteArrayOutputStream()
        for ((index, cell) in cells.withIndex()) {
            require(cell.data.size <= MAX_CELL_DATA_BYTES) { "cell[$index] data exceeds one TON cell" }
            require(cell.refs.size <= MAX_REFS) { "cell[$index] refs exceed TON ref count" }
            out.write(cell.refs.size)
            out.write(cell.data.size * 2)
            out.write(cell.data)
            for (ref in cell.refs) {
                require(ref >= 0 && ref < cells.size) { "cell[$index] has invalid ref" }
                out.write(sizedUInt(ref, sizeBytes))
            }
        }
        return out.toByteArray()
    }

    private fun minSizeBytes(value: Int): Int {
        val numeric = BigInteger.valueOf(value.toLong())
        for (size in 1..7) {
            if (numeric <= BigInteger.ONE.shiftLeft(size * 8).subtract(BigInteger.ONE)) return size
        }
        throw IllegalArgumentException("TON sized integer is too large")
    }

    private fun sizedUInt(value: Int, size: Int): ByteArray {
        require(size in 1..7) { "TON size must be 1..7 bytes" }
        var working = BigInteger.valueOf(value.toLong())
        val out = ByteArray(size)
        for (index in size - 1 downTo 0) {
            out[index] = working.and(BigInteger.valueOf(0xffL)).toByte()
            working = working.shiftRight(8)
        }
        require(working == BigInteger.ZERO) { "TON sized integer overflows" }
        return out
    }

    private fun hashHex(prefix: String, payload: ByteArray): String {
        val prefixBytes = prefix.toByteArray(Charsets.UTF_8)
        val preimage = ByteArray(prefixBytes.size + payload.size)
        System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.size)
        System.arraycopy(payload, 0, preimage, prefixBytes.size, payload.size)
        return "0x" + hexLower(Blake2b.digest256(preimage))
    }

    private fun hex32Bytes(value: String, field: String): ByteArray {
        var body = value.trim()
        if (body.startsWith("0x", ignoreCase = true)) body = body.substring(2)
        require(body.length == 64) { "$field must be 32 bytes" }
        val out = ByteArray(32)
        for (i in out.indices) {
            out[i] = body.substring(i * 2, i * 2 + 2).toIntOrNull(16)?.toByte()
                ?: throw IllegalArgumentException("$field must be canonical hex")
        }
        return out
    }

    private fun normalizeU64(value: String, field: String): BigInteger {
        val trimmed = value.trim()
        require(trimmed.matches(Regex("[0-9]+"))) { "$field must be an unsigned integer" }
        val numeric = BigInteger(trimmed)
        require(numeric <= MAX_U64) { "$field must fit u64" }
        return numeric
    }

    private fun writeU16Be(out: ByteArrayOutputStream, value: Int) {
        out.write((value ushr 8) and 0xff)
        out.write(value and 0xff)
    }

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0) { "u32 value must not be negative" }
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun writeU32Be(out: ByteArrayOutputStream, value: Long) {
        out.write(((value ushr 24) and 0xff).toInt())
        out.write(((value ushr 16) and 0xff).toInt())
        out.write(((value ushr 8) and 0xff).toInt())
        out.write((value and 0xff).toInt())
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        for (i in 0 until 8) {
            out.write(working.and(BigInteger.valueOf(0xffL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun writeU64Be(out: ByteArrayOutputStream, value: BigInteger) {
        val bytes = ByteArray(8)
        var working = value
        for (index in 7 downTo 0) {
            bytes[index] = working.and(BigInteger.valueOf(0xffL)).toByte()
            working = working.shiftRight(8)
        }
        out.write(bytes)
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) builder.append(String.format("%02x", byte.toInt() and 0xff))
        return builder.toString()
    }

    private data class TonCell(val data: ByteArray, val refs: MutableList<Int>)
}

/** SCCP public inputs shared by TON message-body and proof request builders. */
data class TonSccpPublicInputsInput(
    val version: Int = 1,
    val messageId: String,
    val payloadHash: String,
    val targetDomain: Int = SccpTon.DOMAIN_TON,
    val commitmentRoot: String,
    val finalityHeight: String,
    val finalityBlockHash: String,
)

/** Inputs for a TON internal message body carrying an SCCP proof submission. */
data class TonSccpMessageBodyInput(
    val publicInputs: TonSccpPublicInputsInput,
    val proofBytes: ByteArray,
    val bundleBytes: ByteArray,
    val statementHash: String,
    val destinationBindingHash: String,
    val metadataBytes: ByteArray = ByteArray(0),
    val queryId: String? = null,
)

/** Prebuilt TON SCCP submission envelope for wallet or liteserver broadcasting. */
data class TonSccpSubmission(
    val envelopeEncoding: String,
    val messageBodyBoc: ByteArray,
    val messageBodyBocHex: String,
)

/** Inputs used to build a local TON SCCP proof request. */
data class TonSccpProofRequestInput(
    val publicInputs: TonSccpPublicInputsInput,
    val bundleBytes: ByteArray,
    val sourceProofBytes: ByteArray = ByteArray(0),
    val backend: String = SccpTon.CONTRACT_PROOF_BACKEND_V1,
    val sourceDomain: Int = SccpTon.DOMAIN_TON,
)

/** Request passed to a linked local TON SCCP prover. */
data class TonSccpProofRequest(
    val version: Int,
    val backend: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: TonSccpPublicInputsInput,
    val publicInputsBytes: ByteArray,
    val bundleBytes: ByteArray,
    val sourceProofBytes: ByteArray,
    val requestHash: String,
)

/** Proof bytes returned by a linked local TON SCCP prover. */
data class TonSccpProofResult(
    val version: Int,
    val backend: String,
    val proofBytes: ByteArray,
    val proofBase64: String,
    val publicInputs: TonSccpPublicInputsInput,
    val requestHash: String,
)

/** Optional witness resolver backed by app-controlled TON liteserver calls. */
fun interface TonSccpWitnessProvider {
    fun resolveWitness(input: TonSccpProofRequestInput): TonSccpProofRequestInput
}

/** Local TON proof engine linked by the application bundle. */
fun interface TonSccpProofEngine {
    fun prove(request: TonSccpProofRequest): ByteArray
}

/** Local-first TON SCCP proof wrapper for UI SDKs. */
class TonSccpProver(
    private val witnessProvider: TonSccpWitnessProvider? = null,
    private val proofEngine: TonSccpProofEngine? = null,
) {
    fun buildRequest(input: TonSccpProofRequestInput): TonSccpProofRequest =
        SccpTon.buildProofRequest(witnessProvider?.resolveWitness(input) ?: input)

    fun prove(input: TonSccpProofRequestInput): TonSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine ?: throw IllegalStateException("TON SCCP local prover is not linked")
        return SccpTon.wrapProofResult(engine.prove(request), request)
    }
}
