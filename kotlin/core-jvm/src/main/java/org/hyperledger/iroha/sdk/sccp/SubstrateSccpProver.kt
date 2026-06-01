package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.util.Base64
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Substrate-family SCCP runtime proof request helpers for local-first UI proof generation. */
object SccpSubstrate {
    const val DOMAIN_SORA: Int = SccpSourceProofs.DOMAIN_SORA
    const val DOMAIN_SORA_KUSAMA: Int = SccpSourceProofs.DOMAIN_SORA_KUSAMA
    const val DOMAIN_SORA_POLKADOT: Int = SccpSourceProofs.DOMAIN_SORA_POLKADOT
    const val DOMAIN_SORA2: Int = SccpSourceProofs.DOMAIN_SORA2
    const val RUNTIME_PROOF_BACKEND_V1: String = "substrate-runtime-v1"
    const val RUNTIME_CALL_SCALE_V1: String = "scale_call_v1"
    const val SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1: String = "SccpBridge.submit_message_proof"
    const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"
    const val NATIVE_RECURSIVE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val SOURCE_STATE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024

    private const val PROOF_REQUEST_PREFIX_V1: String = "sccp:substrate:runtime-proof-request:v1"
    private const val PROOF_ENVELOPE_PREFIX_V1: String = "sccp:substrate:runtime-proof-envelope:v1"
    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    @JvmStatic
    fun canonicalPublicInputsBytes(input: SubstrateSccpPublicInputsInput): ByteArray {
        require(input.version == 1) { "publicInputs.version must be 1" }
        require(targetDomainIsSupported(input.targetDomain)) {
            "publicInputs.targetDomain must be a Substrate-family SCCP domain"
        }
        val out = ByteArrayOutputStream()
        out.write(input.version)
        out.write(nonZeroHex32Bytes(input.messageId, "messageId"))
        out.write(nonZeroHex32Bytes(input.payloadHash, "payloadHash"))
        writeU32Le(out, input.targetDomain)
        out.write(nonZeroHex32Bytes(input.commitmentRoot, "commitmentRoot"))
        writeU64Le(out, normalizeU64(input.finalityHeight, "finalityHeight"))
        out.write(nonZeroHex32Bytes(input.finalityBlockHash, "finalityBlockHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun buildProofRequest(input: SubstrateSccpProofRequestInput): SubstrateSccpProofRequest {
        require(input.backend == RUNTIME_PROOF_BACKEND_V1) { "backend must be substrate-runtime-v1" }
        val bundleBytes = requireNativeRecursivePayloadBytes(input.bundleBytes, "bundleBytes")
        val sourceProofBytes = requireOptionalSourceProofBytes(input.sourceProofBytes, "sourceProofBytes")
        require(input.sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        require(input.sourceDomain != input.publicInputs.targetDomain) {
            "sourceDomain and publicInputs.targetDomain must differ"
        }
        val publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs)
        val proofContext = normalizeProofContext(input.statementHash, input.destinationBindingHash)
        val preimage = ByteArrayOutputStream()
        writeU32Le(preimage, input.sourceDomain)
        preimage.write(publicInputsBytes)
        writeU32Le(preimage, bundleBytes.size)
        preimage.write(bundleBytes)
        writeU32Le(preimage, sourceProofBytes.size)
        preimage.write(sourceProofBytes)
        preimage.write(hex32Bytes(proofContext.statementHash, "statementHash"))
        preimage.write(hex32Bytes(proofContext.destinationBindingHash, "destinationBindingHash"))
        return SubstrateSccpProofRequest(
            version = 1,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
            targetDomain = input.publicInputs.targetDomain,
            publicInputs = input.publicInputs,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            proofContext = proofContext,
            statementHash = proofContext.statementHash,
            destinationBindingHash = proofContext.destinationBindingHash,
            requestHash = hashHex(PROOF_REQUEST_PREFIX_V1, preimage.toByteArray()),
        )
    }

    @JvmStatic
    fun wrapProofResult(
        proofBytes: ByteArray,
        request: SubstrateSccpProofRequest,
    ): SubstrateSccpProofResult {
        require(request.backend == RUNTIME_PROOF_BACKEND_V1) { "backend must be substrate-runtime-v1" }
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        requireProductionProofRequest(request)
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(request.requestHash, "requestHash"))
        envelopePayload.write(proofBytes)
        return SubstrateSccpProofResult(
            version = 1,
            backend = request.backend,
            proofBytes = proofBytes.copyOf(),
            proofBase64 = Base64.getEncoder().encodeToString(proofBytes),
            publicInputs = request.publicInputs,
            bundleBytes = request.bundleBytes,
            sourceProofBytes = request.sourceProofBytes,
            proofContext = request.proofContext,
            statementHash = request.statementHash,
            destinationBindingHash = request.destinationBindingHash,
            requestHash = request.requestHash,
            envelopeHash = hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()),
        )
    }

    @JvmStatic
    fun buildSubmission(input: SubstrateSccpSubmissionInput): SubstrateSccpSubmission {
        require(input.sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        val proofBytes = input.proofBytes.copyOf()
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        val bundleBytes = requireNativeRecursivePayloadBytes(input.bundleBytes, "bundleBytes")
        val sourceProofBytes = requireOptionalSourceProofBytes(input.sourceProofBytes, "sourceProofBytes")
        require(input.proofResult != null || sourceProofBytes.isEmpty()) {
            "sourceProofBytes requires proofResult for request-bound submission"
        }
        val request = buildProofRequest(
            SubstrateSccpProofRequestInput(
                publicInputs = input.publicInputs,
                bundleBytes = bundleBytes,
                sourceProofBytes = sourceProofBytes,
                statementHash = input.statementHash,
                destinationBindingHash = input.destinationBindingHash,
                backend = RUNTIME_PROOF_BACKEND_V1,
                sourceDomain = input.sourceDomain,
            ),
        )
        input.proofResult?.let { proofResult ->
            require(proofResult.backend == RUNTIME_PROOF_BACKEND_V1) {
                "proofResult.backend must be substrate-runtime-v1"
            }
            require(proofResult.publicInputs == input.publicInputs) {
                "proofResult.publicInputs must match publicInputs"
            }
            require(proofResult.bundleBytes.contentEquals(bundleBytes)) {
                "bundleBytes must match proofResult.bundleBytes"
            }
            require(proofResult.sourceProofBytes.contentEquals(sourceProofBytes)) {
                "sourceProofBytes must match proofResult.sourceProofBytes"
            }
            val expectedResult = wrapProofResult(proofResult.proofBytes, request)
            require(expectedResult == proofResult) { "proofResult must match request" }
            require(proofResult.proofBytes.contentEquals(proofBytes)) {
                "proofBytes must match proofResult.proofBytes"
            }
        }
        val argumentPairs = listOf(
            "proof_bytes" to proofBytes,
            "public_inputs" to request.publicInputsBytes,
            "bundle_bytes" to bundleBytes,
        )
        val runtimeCall = encodeSubstrateRuntimeCall(argumentPairs.map { it.second })
        val arguments = argumentPairs.map { (key, bytes) ->
            SubstrateSccpSubmissionArgument(key = key, encoding = "raw_bytes", bytesHex = "0x" + hexLower(bytes))
        }
        return SubstrateSccpSubmission(
            version = 1,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
            verifierBackend = RUNTIME_PROOF_BACKEND_V1,
            platformPayload = "substrate_runtime_call",
            envelopeEncoding = RUNTIME_CALL_SCALE_V1,
            submissionKind = "runtime_call",
            verifierEntrypoint = SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
            sourceDomain = input.sourceDomain,
            targetDomain = input.publicInputs.targetDomain,
            publicInputs = input.publicInputs,
            proofContext = request.proofContext,
            statementHash = request.statementHash,
            destinationBindingHash = request.destinationBindingHash,
            requestHash = request.requestHash,
            proofBytes = proofBytes,
            publicInputsBytes = request.publicInputsBytes,
            bundleBytes = bundleBytes,
            arguments = arguments,
            runtimeCall = runtimeCall,
            runtimeCallHex = "0x" + hexLower(runtimeCall),
            envelopeBytes = runtimeCall,
            envelopeHex = "0x" + hexLower(runtimeCall),
        )
    }

    private fun requireCanonicalProofRequest(request: SubstrateSccpProofRequest) {
        val expected = buildProofRequest(
            SubstrateSccpProofRequestInput(
                publicInputs = request.publicInputs,
                bundleBytes = request.bundleBytes,
                sourceProofBytes = request.sourceProofBytes,
                statementHash = request.statementHash,
                destinationBindingHash = request.destinationBindingHash,
                backend = request.backend,
                sourceDomain = request.sourceDomain,
            ),
        )
        require(
            request.version == expected.version &&
                request.backend == expected.backend &&
                request.sourceDomain == expected.sourceDomain &&
                request.targetDomain == expected.targetDomain &&
                request.publicInputs == expected.publicInputs &&
                request.publicInputsBytes.contentEquals(expected.publicInputsBytes) &&
                request.bundleBytes.contentEquals(expected.bundleBytes) &&
                request.sourceProofBytes.contentEquals(expected.sourceProofBytes) &&
                request.proofContext == expected.proofContext &&
                request.statementHash == expected.statementHash &&
                request.destinationBindingHash == expected.destinationBindingHash &&
                request.requestHash == expected.requestHash,
        ) { "proof request must be canonical" }
    }

    internal fun requireProductionProofRequest(request: SubstrateSccpProofRequest) {
        requireCanonicalProofRequest(request)
        require(request.version == 1) { "proof request version must be 1" }
        require(request.backend == RUNTIME_PROOF_BACKEND_V1) {
            "Substrate SCCP proof request backend must be substrate-runtime-v1"
        }
        require(request.sourceDomain == DOMAIN_SORA) {
            "Substrate SCCP production proofs must start from SORA"
        }
        require(
            request.targetDomain == request.publicInputs.targetDomain &&
                targetDomainIsSupported(request.targetDomain),
        ) {
            "Substrate SCCP production proofs must target a Substrate-family domain"
        }
        requireNativeRecursivePayloadBytes(request.bundleBytes, "Substrate SCCP proof request bundleBytes")
        requireOptionalSourceProofBytes(
            request.sourceProofBytes,
            "Substrate SCCP proof request sourceProofBytes",
        )
    }

    internal fun callbackRequestSnapshot(request: SubstrateSccpProofRequest): SubstrateSccpProofRequest =
        request.copy()

    private fun targetDomainIsSupported(value: Int): Boolean =
        value == DOMAIN_SORA_KUSAMA || value == DOMAIN_SORA_POLKADOT || value == DOMAIN_SORA2

    private fun requireNativeRecursivePayloadBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.isNotEmpty()) { "$label must not be empty" }
        require(copy.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "$label must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        return copy
    }

    private fun requireOptionalSourceProofBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.size <= SOURCE_STATE_MAX_PROOF_BYTES) {
            "$label must be at most $SOURCE_STATE_MAX_PROOF_BYTES bytes"
        }
        require(copy.isEmpty() || copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        return copy
    }

    private fun hashHex(prefix: String, payload: ByteArray): String {
        val prefixBytes = prefix.toByteArray(Charsets.UTF_8)
        val preimage = ByteArray(prefixBytes.size + payload.size)
        System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.size)
        System.arraycopy(payload, 0, preimage, prefixBytes.size, payload.size)
        return "0x" + hexLower(Blake2b.digest256(preimage))
    }

    private fun hex32Bytes(value: String, field: String): ByteArray {
        require(value.trim() == value) { "$field must be canonical hex" }
        var body = value
        if (body.startsWith("0x", ignoreCase = true)) body = body.substring(2)
        require(body.length == 64) { "$field must be 32 bytes" }
        val out = ByteArray(32)
        for (i in out.indices) {
            out[i] = body.substring(i * 2, i * 2 + 2).toIntOrNull(16)?.toByte()
                ?: throw IllegalArgumentException("$field must be canonical hex")
        }
        return out
    }

    private fun nonZeroHex32Bytes(value: String, field: String): ByteArray {
        val out = hex32Bytes(value, field)
        require(out.any { it.toInt() != 0 }) { "$field must not be zero" }
        return out
    }

    private fun normalizeNonZeroHex32(value: String, field: String): String =
        "0x" + hexLower(nonZeroHex32Bytes(value, field))

    private fun normalizeProofContext(
        statementHash: String,
        destinationBindingHash: String,
    ): SubstrateSccpProofContext =
        SubstrateSccpProofContext(
            version = 1,
            statementHash = normalizeNonZeroHex32(statementHash, "statementHash"),
            destinationBindingHash = normalizeNonZeroHex32(destinationBindingHash, "destinationBindingHash"),
        )

    private fun normalizeU64(value: String, field: String): BigInteger {
        require(isCanonicalDecimalText(value)) { "$field must be an unsigned integer" }
        val numeric = BigInteger(value)
        require(numeric <= MAX_U64) { "$field must fit u64" }
        require(numeric != BigInteger.ZERO) { "$field must not be zero" }
        return numeric
    }

    private fun isCanonicalDecimalText(value: String): Boolean =
        value == "0" || (value.isNotEmpty() && value[0] in '1'..'9' && value.all { it in '0'..'9' })

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0) { "u32 value must not be negative" }
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        for (i in 0 until 8) {
            out.write(working.and(BigInteger.valueOf(0xffL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun scaleCompactU32(value: Int, label: String): ByteArray {
        require(value >= 0) { "$label length must fit u32" }
        val out = ByteArrayOutputStream()
        when {
            value < (1 shl 6) -> out.write(value shl 2)
            value < (1 shl 14) -> {
                val encoded = (value shl 2) or 0b01
                out.write(encoded and 0xff)
                out.write((encoded ushr 8) and 0xff)
            }
            value < (1 shl 30) -> {
                val encoded = (value shl 2) or 0b10
                out.write(encoded and 0xff)
                out.write((encoded ushr 8) and 0xff)
                out.write((encoded ushr 16) and 0xff)
                out.write((encoded ushr 24) and 0xff)
            }
            else -> {
                out.write(0b11)
                writeU32Le(out, value)
            }
        }
        return out.toByteArray()
    }

    private fun scaleVec(bytes: ByteArray, label: String): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(scaleCompactU32(bytes.size, label))
        out.write(bytes)
        return out.toByteArray()
    }

    private fun encodeSubstrateRuntimeCall(argumentBytes: List<ByteArray>): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(scaleVec(SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.toByteArray(Charsets.UTF_8), "verifierEntrypoint"))
        argumentBytes.forEachIndexed { index, bytes ->
            out.write(scaleVec(bytes, "arguments[$index]"))
        }
        return out.toByteArray()
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) builder.append(String.format("%02x", byte.toInt() and 0xff))
        return builder.toString()
    }
}

/** SCCP public inputs shared by Substrate runtime proof requests. */
data class SubstrateSccpPublicInputsInput(
    val version: Int = 1,
    val messageId: String,
    val payloadHash: String,
    val targetDomain: Int = SccpSubstrate.DOMAIN_SORA2,
    val commitmentRoot: String,
    val finalityHeight: String,
    val finalityBlockHash: String,
)

/** Inputs used to build a local Substrate SCCP runtime proof request. */
data class SubstrateSccpProofRequestInput(
    val publicInputs: SubstrateSccpPublicInputsInput,
    val bundleBytes: ByteArray,
    val sourceProofBytes: ByteArray = ByteArray(0),
    val statementHash: String,
    val destinationBindingHash: String,
    val backend: String = SccpSubstrate.RUNTIME_PROOF_BACKEND_V1,
    val sourceDomain: Int = SccpSubstrate.DOMAIN_SORA,
)

/** Statement and destination context proved by the local Substrate SCCP prover. */
data class SubstrateSccpProofContext(
    val version: Int,
    val statementHash: String,
    val destinationBindingHash: String,
)

/** Request passed to a linked local Substrate runtime prover. */
class SubstrateSccpProofRequest(
    val version: Int,
    val backend: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: SubstrateSccpPublicInputsInput,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    sourceProofBytes: ByteArray,
    val proofContext: SubstrateSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val requestHash: String,
) {
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val sourceProofBytesStorage: ByteArray = sourceProofBytes.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val sourceProofBytes: ByteArray
        get() = sourceProofBytesStorage.copyOf()

    fun copy(
        version: Int = this.version,
        backend: String = this.backend,
        sourceDomain: Int = this.sourceDomain,
        targetDomain: Int = this.targetDomain,
        publicInputs: SubstrateSccpPublicInputsInput = this.publicInputs,
        publicInputsBytes: ByteArray = this.publicInputsBytes,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: SubstrateSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        requestHash: String = this.requestHash,
    ): SubstrateSccpProofRequest =
        SubstrateSccpProofRequest(
            version,
            backend,
            sourceDomain,
            targetDomain,
            publicInputs,
            publicInputsBytes,
            bundleBytes,
            sourceProofBytes,
            proofContext,
            statementHash,
            destinationBindingHash,
            requestHash,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): Int = sourceDomain
    operator fun component4(): Int = targetDomain
    operator fun component5(): SubstrateSccpPublicInputsInput = publicInputs
    operator fun component6(): ByteArray = publicInputsBytes
    operator fun component7(): ByteArray = bundleBytes
    operator fun component8(): ByteArray = sourceProofBytes
    operator fun component9(): SubstrateSccpProofContext = proofContext
    operator fun component10(): String = statementHash
    operator fun component11(): String = destinationBindingHash
    operator fun component12(): String = requestHash

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is SubstrateSccpProofRequest &&
            version == other.version &&
            backend == other.backend &&
            sourceDomain == other.sourceDomain &&
            targetDomain == other.targetDomain &&
            publicInputs == other.publicInputs &&
            publicInputsBytesStorage.contentEquals(other.publicInputsBytesStorage) &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            sourceProofBytesStorage.contentEquals(other.sourceProofBytesStorage) &&
            proofContext == other.proofContext &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            requestHash == other.requestHash

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + backend.hashCode()
        result = 31 * result + sourceDomain
        result = 31 * result + targetDomain
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + publicInputsBytesStorage.contentHashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + sourceProofBytesStorage.contentHashCode()
        result = 31 * result + proofContext.hashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + requestHash.hashCode()
        return result
    }

    override fun toString(): String =
        "SubstrateSccpProofRequest(version=$version, backend=$backend, sourceDomain=$sourceDomain, " +
            "targetDomain=$targetDomain, publicInputs=$publicInputs, " +
            "publicInputsBytes=${publicInputsBytesStorage.size} bytes, " +
            "bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, proofContext=$proofContext, " +
            "statementHash=$statementHash, destinationBindingHash=$destinationBindingHash, " +
            "requestHash=$requestHash)"
}

/** Proof bytes returned by a linked local Substrate runtime prover. */
class SubstrateSccpProofResult(
    val version: Int,
    val backend: String,
    proofBytes: ByteArray,
    val proofBase64: String,
    val publicInputs: SubstrateSccpPublicInputsInput,
    bundleBytes: ByteArray = ByteArray(0),
    sourceProofBytes: ByteArray = ByteArray(0),
    val proofContext: SubstrateSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val requestHash: String,
    val envelopeHash: String,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val sourceProofBytesStorage: ByteArray = sourceProofBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val sourceProofBytes: ByteArray
        get() = sourceProofBytesStorage.copyOf()

    fun copy(
        version: Int = this.version,
        backend: String = this.backend,
        proofBytes: ByteArray = this.proofBytes,
        proofBase64: String = this.proofBase64,
        publicInputs: SubstrateSccpPublicInputsInput = this.publicInputs,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: SubstrateSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        requestHash: String = this.requestHash,
        envelopeHash: String = this.envelopeHash,
    ): SubstrateSccpProofResult =
        SubstrateSccpProofResult(
            version,
            backend,
            proofBytes,
            proofBase64,
            publicInputs,
            bundleBytes,
            sourceProofBytes,
            proofContext,
            statementHash,
            destinationBindingHash,
            requestHash,
            envelopeHash,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): ByteArray = proofBytes
    operator fun component4(): String = proofBase64
    operator fun component5(): SubstrateSccpPublicInputsInput = publicInputs
    operator fun component6(): ByteArray = bundleBytes
    operator fun component7(): ByteArray = sourceProofBytes
    operator fun component8(): SubstrateSccpProofContext = proofContext
    operator fun component9(): String = statementHash
    operator fun component10(): String = destinationBindingHash
    operator fun component11(): String = requestHash
    operator fun component12(): String = envelopeHash

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is SubstrateSccpProofResult &&
            version == other.version &&
            backend == other.backend &&
            proofBytesStorage.contentEquals(other.proofBytesStorage) &&
            proofBase64 == other.proofBase64 &&
            publicInputs == other.publicInputs &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            sourceProofBytesStorage.contentEquals(other.sourceProofBytesStorage) &&
            proofContext == other.proofContext &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            requestHash == other.requestHash &&
            envelopeHash == other.envelopeHash

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + backend.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        result = 31 * result + proofBase64.hashCode()
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + sourceProofBytesStorage.contentHashCode()
        result = 31 * result + proofContext.hashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + requestHash.hashCode()
        result = 31 * result + envelopeHash.hashCode()
        return result
    }

    override fun toString(): String =
        "SubstrateSccpProofResult(version=$version, backend=$backend, " +
            "proofBytes=${proofBytesStorage.size} bytes, proofBase64=$proofBase64, " +
            "publicInputs=$publicInputs, bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, proofContext=$proofContext, " +
            "statementHash=$statementHash, destinationBindingHash=$destinationBindingHash, " +
            "requestHash=$requestHash, envelopeHash=$envelopeHash)"
}

/** Inputs used to package a completed Substrate proof for runtime submission. */
data class SubstrateSccpSubmissionInput(
    val publicInputs: SubstrateSccpPublicInputsInput,
    val proofBytes: ByteArray,
    val bundleBytes: ByteArray,
    val sourceProofBytes: ByteArray = ByteArray(0),
    val statementHash: String,
    val destinationBindingHash: String,
    val sourceDomain: Int = SccpSubstrate.DOMAIN_SORA,
    val proofResult: SubstrateSccpProofResult? = null,
) {
    constructor(
        proofResult: SubstrateSccpProofResult,
        sourceDomain: Int = SccpSubstrate.DOMAIN_SORA,
    ) : this(
        publicInputs = proofResult.publicInputs,
        proofBytes = proofResult.proofBytes,
        bundleBytes = proofResult.bundleBytes,
        sourceProofBytes = proofResult.sourceProofBytes,
        statementHash = proofResult.statementHash,
        destinationBindingHash = proofResult.destinationBindingHash,
        sourceDomain = sourceDomain,
        proofResult = proofResult,
    )
}

/** One Substrate SCCP runtime-call argument in verifier order. */
data class SubstrateSccpSubmissionArgument(
    val key: String,
    val encoding: String,
    val bytesHex: String,
)

/** Substrate-family SCCP runtime call ready for wallet or relayer submission. */
class SubstrateSccpSubmission(
    val version: Int,
    val proofFamily: String,
    val verifierBackend: String,
    val platformPayload: String,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: SubstrateSccpPublicInputsInput,
    val proofContext: SubstrateSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val requestHash: String,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    val arguments: List<SubstrateSccpSubmissionArgument>,
    runtimeCall: ByteArray,
    val runtimeCallHex: String,
    envelopeBytes: ByteArray,
    val envelopeHex: String,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val runtimeCallStorage: ByteArray = runtimeCall.copyOf()
    private val envelopeBytesStorage: ByteArray = envelopeBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val runtimeCall: ByteArray
        get() = runtimeCallStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = envelopeBytesStorage.copyOf()
}

/** Optional witness resolver backed by app-controlled Substrate RPC calls. */
fun interface SubstrateSccpWitnessProvider {
    fun resolveWitness(input: SubstrateSccpProofRequestInput): SubstrateSccpProofRequestInput
}

/** Local Substrate runtime proof engine linked by the application bundle. */
fun interface SubstrateSccpProofEngine {
    fun prove(request: SubstrateSccpProofRequest): ByteArray
}

/** Local-first Substrate SCCP runtime proof wrapper for UI SDKs. */
class SubstrateSccpProver(
    private val witnessProvider: SubstrateSccpWitnessProvider? = null,
    private val proofEngine: SubstrateSccpProofEngine? = null,
) {
    fun buildRequest(input: SubstrateSccpProofRequestInput): SubstrateSccpProofRequest =
        SccpSubstrate.buildProofRequest(witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input)

    fun prove(input: SubstrateSccpProofRequestInput): SubstrateSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine ?: throw IllegalStateException("Substrate SCCP runtime prover is not linked")
        SccpSubstrate.requireProductionProofRequest(request)
        return SccpSubstrate.wrapProofResult(engine.prove(SccpSubstrate.callbackRequestSnapshot(request)), request)
    }

    private fun witnessProviderInputSnapshot(input: SubstrateSccpProofRequestInput): SubstrateSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )
}
