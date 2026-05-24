package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Base64
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Solana SCCP proof request helpers for local-first UI proof generation. */
object SccpSolana {
    const val DOMAIN_SORA: Int = 0
    const val DOMAIN_SOLANA: Int = 3
    const val RECURSIVE_PROOF_BACKEND_V1: String = "sccp-solana-recursive-mainnet-v1"
    const val MAINNET_GENESIS_HASH: String = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"

    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    @JvmStatic
    fun normalizeWitness(input: SolanaSccpWitnessInput): SolanaSccpWitness {
        return SolanaSccpWitness(
            version = 1,
            sourceDomain = DOMAIN_SOLANA,
            targetDomain = input.targetDomain,
            mainnetGenesisHash = normalizeNonEmpty(input.mainnetGenesisHash, "mainnetGenesisHash"),
            finalizedSlot = normalizeU64(input.finalizedSlot, "finalizedSlot").toString(),
            blockhash = normalizeNonEmpty(input.blockhash, "blockhash"),
            bankHash = normalizeHex32(input.bankHash, "bankHash"),
            transactionStatusRoot = normalizeHex32(input.transactionStatusRoot, "transactionStatusRoot"),
            messageProofHash = normalizeHex32(input.messageProofHash, "messageProofHash"),
            transactionSignature = normalizeNonEmpty(input.transactionSignature, "transactionSignature"),
            emitterProgramId = normalizeNonEmpty(input.emitterProgramId, "emitterProgramId"),
            messageId = normalizeHex32(input.messageId, "messageId"),
            payloadHash = normalizeHex32(input.payloadHash, "payloadHash"),
            commitmentRoot = normalizeHex32(input.commitmentRoot, "commitmentRoot"),
            sourceEventDigest = normalizeHex32(input.sourceEventDigest, "sourceEventDigest"),
        )
    }

    @JvmStatic
    fun canonicalWitnessBytes(input: SolanaSccpWitnessInput): ByteArray =
        canonicalWitnessBytes(normalizeWitness(input))

    @JvmStatic
    fun canonicalWitnessBytes(witness: SolanaSccpWitness): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(witness.version)
        writeU32Le(out, witness.sourceDomain)
        writeU32Le(out, witness.targetDomain)
        writeString(out, witness.mainnetGenesisHash, "mainnetGenesisHash")
        writeU64Le(out, normalizeU64(witness.finalizedSlot, "finalizedSlot"))
        writeString(out, witness.blockhash, "blockhash")
        writeString(out, witness.transactionSignature, "transactionSignature")
        writeString(out, witness.emitterProgramId, "emitterProgramId")
        out.write(hex32Bytes(witness.bankHash, "bankHash"))
        out.write(hex32Bytes(witness.transactionStatusRoot, "transactionStatusRoot"))
        out.write(hex32Bytes(witness.messageProofHash, "messageProofHash"))
        out.write(hex32Bytes(witness.messageId, "messageId"))
        out.write(hex32Bytes(witness.payloadHash, "payloadHash"))
        out.write(hex32Bytes(witness.commitmentRoot, "commitmentRoot"))
        out.write(hex32Bytes(witness.sourceEventDigest, "sourceEventDigest"))
        return out.toByteArray()
    }

    @JvmStatic
    fun canonicalMessageProofBytes(
        sourceEventDigest: String,
        transactionStatusRoot: String,
        inclusionBranch: List<ByteArray>,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(hex32Bytes(sourceEventDigest, "sourceEventDigest"))
        out.write(hex32Bytes(transactionStatusRoot, "transactionStatusRoot"))
        writeU32Le(out, inclusionBranch.size)
        inclusionBranch.forEachIndexed { index, sibling ->
            require(sibling.size == 32) { "inclusionBranch[$index] must be 32 bytes" }
            out.write(sibling)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun messageProofHash(
        sourceEventDigest: String,
        transactionStatusRoot: String,
        inclusionBranch: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:solana:message-proof:v1",
            canonicalMessageProofBytes(sourceEventDigest, transactionStatusRoot, inclusionBranch),
        )

    @JvmStatic
    fun buildProofRequest(input: SolanaSccpWitnessInput): SolanaSccpProofRequest {
        val witness = normalizeWitness(input)
        val witnessHash = hashHex("sccp:solana:witness:v1", canonicalWitnessBytes(witness))
        return SolanaSccpProofRequest(
            version = 1,
            backend = RECURSIVE_PROOF_BACKEND_V1,
            sourceDomain = DOMAIN_SOLANA,
            targetDomain = witness.targetDomain,
            mainnetGenesisHash = witness.mainnetGenesisHash,
            witnessHash = witnessHash,
            publicInputs = SolanaSccpPublicInputs(
                messageId = witness.messageId,
                payloadHash = witness.payloadHash,
                commitmentRoot = witness.commitmentRoot,
                finalizedSlot = witness.finalizedSlot,
                blockhash = witness.blockhash,
                sourceEventDigest = witness.sourceEventDigest,
            ),
            witness = witness,
        )
    }

    @JvmStatic
    fun wrapProofResult(
        proofBytes: ByteArray,
        request: SolanaSccpProofRequest,
    ): SolanaSccpProofResult {
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(request.witnessHash, "witnessHash"))
        envelopePayload.write(proofBytes)
        return SolanaSccpProofResult(
            version = 1,
            backend = request.backend,
            proofBytes = proofBytes.copyOf(),
            proofBase64 = Base64.getEncoder().encodeToString(proofBytes),
            publicInputs = request.publicInputs,
            witnessHash = request.witnessHash,
            envelopeHash = hashHex("sccp:solana:proof-envelope:v1", envelopePayload.toByteArray()),
        )
    }

    private fun hashHex(prefix: String, payload: ByteArray): String {
        val prefixBytes = prefix.toByteArray(StandardCharsets.UTF_8)
        val preimage = ByteArray(prefixBytes.size + payload.size)
        System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.size)
        System.arraycopy(payload, 0, preimage, prefixBytes.size, payload.size)
        return "0x" + hexLower(Blake2b.digest256(preimage))
    }

    private fun normalizeNonEmpty(value: String, field: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$field must be non-empty" }
        return trimmed
    }

    private fun normalizeHex32(value: String, field: String): String = "0x" + hexLower(hex32Bytes(value, field))

    private fun hex32Bytes(value: String, field: String): ByteArray {
        var body = value.trim()
        if (body.startsWith("0x", ignoreCase = true)) {
            body = body.substring(2)
        }
        require(body.length == 64) { "$field must be 32 bytes" }
        val out = ByteArray(32)
        for (i in out.indices) {
            val byteText = body.substring(i * 2, i * 2 + 2)
            out[i] = byteText.toIntOrNull(16)?.toByte()
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

    private fun writeString(out: ByteArrayOutputStream, value: String, field: String) {
        val bytes = normalizeNonEmpty(value, field).toByteArray(StandardCharsets.UTF_8)
        writeU32Le(out, bytes.size)
        out.write(bytes)
    }

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0) { "u32 value must not be negative" }
        out.write(value and 0xFF)
        out.write((value ushr 8) and 0xFF)
        out.write((value ushr 16) and 0xFF)
        out.write((value ushr 24) and 0xFF)
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        for (i in 0 until 8) {
            out.write(working.and(BigInteger.valueOf(0xFFL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) {
            builder.append(String.format("%02x", byte.toInt() and 0xFF))
        }
        return builder.toString()
    }
}

/** Raw Solana SCCP witness data collected by portal or mobile UI code. */
data class SolanaSccpWitnessInput(
    val targetDomain: Int = SccpSolana.DOMAIN_SORA,
    val mainnetGenesisHash: String = SccpSolana.MAINNET_GENESIS_HASH,
    val finalizedSlot: String,
    val blockhash: String,
    val bankHash: String,
    val transactionStatusRoot: String,
    val messageProofHash: String,
    val transactionSignature: String,
    val emitterProgramId: String,
    val messageId: String,
    val payloadHash: String,
    val commitmentRoot: String,
    val sourceEventDigest: String,
)

/** Canonical Solana SCCP witness passed into local proof generation. */
data class SolanaSccpWitness(
    val version: Int,
    val sourceDomain: Int,
    val targetDomain: Int,
    val mainnetGenesisHash: String,
    val finalizedSlot: String,
    val blockhash: String,
    val bankHash: String,
    val transactionStatusRoot: String,
    val messageProofHash: String,
    val transactionSignature: String,
    val emitterProgramId: String,
    val messageId: String,
    val payloadHash: String,
    val commitmentRoot: String,
    val sourceEventDigest: String,
)

/** Public inputs exposed by the Solana SCCP proof request. */
data class SolanaSccpPublicInputs(
    val messageId: String,
    val payloadHash: String,
    val commitmentRoot: String,
    val finalizedSlot: String,
    val blockhash: String,
    val sourceEventDigest: String,
)

/** Request passed to a linked local Solana SCCP prover. */
data class SolanaSccpProofRequest(
    val version: Int,
    val backend: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val mainnetGenesisHash: String,
    val witnessHash: String,
    val publicInputs: SolanaSccpPublicInputs,
    val witness: SolanaSccpWitness,
)

/** Proof envelope returned after local Solana SCCP proof generation. */
data class SolanaSccpProofResult(
    val version: Int,
    val backend: String,
    val proofBytes: ByteArray,
    val proofBase64: String,
    val publicInputs: SolanaSccpPublicInputs,
    val witnessHash: String,
    val envelopeHash: String,
)

/** Optional witness resolver backed by app-controlled Solana RPC calls. */
fun interface SolanaSccpWitnessProvider {
    fun resolveWitness(input: SolanaSccpWitnessInput): SolanaSccpWitnessInput
}

/** Local proof engine linked by the application bundle. */
fun interface SolanaSccpProofEngine {
    fun prove(request: SolanaSccpProofRequest): ByteArray
}

/** Local-first Solana SCCP proof wrapper for UI SDKs. */
class SolanaSccpProver(
    private val witnessProvider: SolanaSccpWitnessProvider? = null,
    private val proofEngine: SolanaSccpProofEngine? = null,
) {
    fun buildRequest(input: SolanaSccpWitnessInput): SolanaSccpProofRequest {
        val resolved = witnessProvider?.resolveWitness(input) ?: input
        return SccpSolana.buildProofRequest(resolved)
    }

    fun prove(input: SolanaSccpWitnessInput): SolanaSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine
            ?: throw IllegalStateException("Solana SCCP local prover is not linked")
        return SccpSolana.wrapProofResult(engine.prove(request), request)
    }
}
