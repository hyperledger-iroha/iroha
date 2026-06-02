package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.util.Base64
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** EVM-family SCCP Groth16 proof request helpers for local-first UI proof generation. */
object SccpEvm {
    const val DOMAIN_SORA: Int = SccpSourceProofs.DOMAIN_SORA
    const val DOMAIN_ETH: Int = SccpSourceProofs.DOMAIN_ETH
    const val DOMAIN_BSC: Int = SccpSourceProofs.DOMAIN_BSC
    const val GROTH16_BN254_PROOF_BACKEND_V1: String = "evm-groth16-bn254-v1"
    const val GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1: Int = 384
    const val SOURCE_STATE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val NATIVE_RECURSIVE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val CONTRACT_CALL_ABI_TUPLE_V1: String = "abi_tuple_v1"
    const val SUBMIT_MESSAGE_PROOF_ABI_V1: String =
        "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
    const val SUBMIT_MESSAGE_PROOF_SELECTOR_V1: String = "0xbd57826c"

    private const val PROOF_REQUEST_PREFIX_V1: String = "sccp:evm:groth16-proof-request:v1"
    private const val PROOF_ENVELOPE_PREFIX_V1: String = "sccp:evm:groth16-proof-envelope:v1"
    private const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"
    private const val SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1: String =
        "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
    private val SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1 =
        byteArrayOf(0xbd.toByte(), 0x57, 0x82.toByte(), 0x6c)
    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val BN254_BASE_FIELD_MODULUS =
        BigInteger("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16)
    private val BN254_SCALAR_FIELD_MODULUS =
        BigInteger("30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001", 16)
    private val BN254_G2_B_C0 =
        BigInteger("2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5", 16)
    private val BN254_G2_B_C1 =
        BigInteger("009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2", 16)
    private val SIGNAL_LABELS = listOf(
        "sccp:groth16-bn254:signal:message-id:v1",
        "sccp:groth16-bn254:signal:payload-hash:v1",
        "sccp:groth16-bn254:signal:target-domain:v1",
        "sccp:groth16-bn254:signal:commitment-root:v1",
        "sccp:groth16-bn254:signal:finality-height:v1",
        "sccp:groth16-bn254:signal:finality-block-hash:v1",
        "sccp:groth16-bn254:signal:source-domain:v1",
        "sccp:groth16-bn254:signal:statement-hash:v1",
        "sccp:groth16-bn254:signal:destination-binding-hash:v1",
    )

    @JvmStatic
    fun canonicalPublicInputsBytes(input: EvmSccpPublicInputsInput): ByteArray {
        require(input.version == 1) { "publicInputs.version must be 1" }
        require(input.targetDomain != 0) { "publicInputs.targetDomain must not be zero" }
        require(input.targetDomain == DOMAIN_ETH || input.targetDomain == DOMAIN_BSC) {
            "publicInputs.targetDomain must be ETH or BSC"
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
    fun groth16Bn254PublicSignalWords(
        publicInputs: EvmSccpPublicInputsInput,
        sourceDomain: Int,
        statementHash: String,
        destinationBindingHash: String,
    ): List<String> {
        require(publicInputs.version == 1) { "publicInputs.version must be 1" }
        require(publicInputs.targetDomain != 0) { "publicInputs.targetDomain must not be zero" }
        require(publicInputs.targetDomain == DOMAIN_ETH || publicInputs.targetDomain == DOMAIN_BSC) {
            "publicInputs.targetDomain must be ETH or BSC"
        }
        require(sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        require(sourceDomain != publicInputs.targetDomain) {
            "sourceDomain and publicInputs.targetDomain must differ"
        }
        val values = listOf(
            nonZeroHex32Bytes(publicInputs.messageId, "messageId"),
            nonZeroHex32Bytes(publicInputs.payloadHash, "payloadHash"),
            abiWordU32(publicInputs.targetDomain),
            nonZeroHex32Bytes(publicInputs.commitmentRoot, "commitmentRoot"),
            abiWordU64(normalizeU64(publicInputs.finalityHeight, "finalityHeight")),
            nonZeroHex32Bytes(publicInputs.finalityBlockHash, "finalityBlockHash"),
            abiWordU32(sourceDomain),
            nonZeroHex32Bytes(statementHash, "statementHash"),
            nonZeroHex32Bytes(destinationBindingHash, "destinationBindingHash"),
        )
        return values.indices.map { index -> groth16SignalWord(SIGNAL_LABELS[index], values[index]) }
    }

    @JvmStatic
    fun buildProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest {
        require(input.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "backend must be evm-groth16-bn254-v1"
        }
        val bundleBytes = input.bundleBytes.copyOf()
        val sourceProofBytes = requireOptionalSourceProofBytes(input.sourceProofBytes, "sourceProofBytes")
        require(bundleBytes.isNotEmpty()) { "bundleBytes must not be empty" }
        val publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs)
        val proofContext = normalizeProofContext(input.statementHash, input.destinationBindingHash)
        val publicSignalWords = groth16Bn254PublicSignalWords(
            publicInputs = input.publicInputs,
            sourceDomain = input.sourceDomain,
            statementHash = proofContext.statementHash,
            destinationBindingHash = proofContext.destinationBindingHash,
        )
        val preimage = ByteArrayOutputStream()
        preimage.write(publicInputsBytes)
        writeU32Le(preimage, bundleBytes.size)
        preimage.write(bundleBytes)
        writeU32Le(preimage, sourceProofBytes.size)
        preimage.write(sourceProofBytes)
        preimage.write(hex32Bytes(proofContext.statementHash, "statementHash"))
        preimage.write(hex32Bytes(proofContext.destinationBindingHash, "destinationBindingHash"))
        publicSignalWords.forEach { preimage.write(hex32Bytes(it, "publicSignalWords")) }
        return EvmSccpProofRequest(
            version = 1,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
            targetDomain = input.publicInputs.targetDomain,
            publicInputs = input.publicInputs,
            publicInputsBytes = publicInputsBytes,
            publicSignalWords = publicSignalWords,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            proofContext = proofContext,
            statementHash = proofContext.statementHash,
            destinationBindingHash = proofContext.destinationBindingHash,
            requestHash = hashHex(PROOF_REQUEST_PREFIX_V1, preimage.toByteArray()),
            destinationBinding = input.destinationBinding,
        )
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: EvmSccpProofRequest): EvmSccpProofResult {
        require(request.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "backend must be evm-groth16-bn254-v1"
        }
        requireProductionProofRequest(request)
        requireProofBytesForContext(proofBytes, request.publicInputs, request.sourceDomain)
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(request.requestHash, "requestHash"))
        envelopePayload.write(proofBytes)
        return EvmSccpProofResult(
            version = 1,
            backend = request.backend,
            proofBytes = proofBytes.copyOf(),
            proofBase64 = Base64.getEncoder().encodeToString(proofBytes),
            publicInputs = request.publicInputs,
            publicSignalWords = request.publicSignalWords,
            bundleBytes = request.bundleBytes,
            sourceProofBytes = request.sourceProofBytes,
            proofContext = request.proofContext,
            statementHash = request.statementHash,
            destinationBindingHash = request.destinationBindingHash,
            requestHash = request.requestHash,
            envelopeHash = hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()),
            destinationBinding = request.destinationBinding,
        )
    }

    internal fun requireWrappedProofResultForSubmission(
        proofResult: EvmSccpProofResult,
    ): EvmSccpProofResult {
        require(proofResult.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "proofResult.backend must be evm-groth16-bn254-v1"
        }
        val expectedProofContext = normalizeProofContext(
            proofResult.statementHash,
            proofResult.destinationBindingHash,
        )
        require(proofResult.proofContext == expectedProofContext) {
            "proofResult.proofContext must match statementHash and destinationBindingHash"
        }
        val proofBytes = proofResult.proofBytes
        requireProofBytesForPublicInputs(proofBytes, proofResult.publicInputs)
        require(proofResult.proofBase64 == Base64.getEncoder().encodeToString(proofBytes)) {
            "proofResult.proofBase64 must match proofResult.proofBytes"
        }
        val requestHash = normalizeNonZeroHex32(proofResult.requestHash, "proofResult.requestHash")
        val envelopeHash = normalizeNonZeroHex32(proofResult.envelopeHash, "proofResult.envelopeHash")
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(requestHash, "proofResult.requestHash"))
        envelopePayload.write(proofBytes)
        require(envelopeHash == hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray())) {
            "proofResult.envelopeHash must match wrapped proof bytes"
        }
        val sourceProofBytes = requireOptionalSourceProofBytes(
            proofResult.sourceProofBytes,
            "proofResult.sourceProofBytes",
        )
        val expectedRequest = buildProofRequest(
            EvmSccpProofRequestInput(
                publicInputs = proofResult.publicInputs,
                bundleBytes = proofResult.bundleBytes,
                sourceProofBytes = sourceProofBytes,
                statementHash = proofResult.statementHash,
                destinationBindingHash = proofResult.destinationBindingHash,
                backend = proofResult.backend,
                sourceDomain = DOMAIN_SORA,
                destinationBinding = proofResult.destinationBinding,
            ),
        )
        require(expectedRequest.requestHash == requestHash) {
            "proofResult.requestHash must match bundleBytes and sourceProofBytes"
        }
        return proofResult
    }

    @JvmStatic
    fun messageTransparentPublicInputAbiWords(publicInputs: EvmSccpPublicInputsInput): List<String> =
        messageTransparentPublicInputAbiWordBytes(publicInputs).map { "0x" + hexLower(it) }

    @JvmStatic
    fun submitMessageProofCallData(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
        statementHash: String,
    ): ByteArray = submitMessageProofCallData(proofBytes, publicInputs, statementHash, DOMAIN_SORA)

    @JvmStatic
    fun submitMessageProofCallData(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
        statementHash: String,
        sourceDomain: Int,
    ): ByteArray {
        require(sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        requireProofBytesForContext(proofBytes, publicInputs, sourceDomain)
        val publicInputWords = messageTransparentPublicInputAbiWordBytes(publicInputs)
        val out = ByteArrayOutputStream()
        out.write(SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1)
        out.write(abiWordU256(32 * 8))
        publicInputWords.forEach { out.write(it) }
        out.write(nonZeroHex32Bytes(statementHash, "statementHash"))
        out.write(abiWordU256(proofBytes.size))
        out.write(proofBytes)
        val padding = (32 - (proofBytes.size % 32)) % 32
        if (padding > 0) out.write(ByteArray(padding))
        return out.toByteArray()
    }

    @JvmStatic
    fun buildSubmission(input: EvmSccpSubmissionInput): EvmSccpSubmission {
        require(input.sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        val publicInputs = input.publicInputs
        canonicalPublicInputsBytes(publicInputs)
        val proofBytes = input.proofBytes
        requireProofBytesForContext(proofBytes, publicInputs, input.sourceDomain)
        val statementHash = normalizeNonZeroHex32(input.statementHash, "statementHash")
        val destinationBindingHash =
            normalizeNonZeroHex32(input.destinationBindingHash, "destinationBindingHash")
        val proofResult = input.proofResult?.let { requireWrappedProofResultForSubmission(it) }
        if (proofResult != null) {
            require(proofResult.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
                "proofResult.backend must be evm-groth16-bn254-v1"
            }
            require(proofResult.publicInputs == publicInputs) {
                "publicInputs must match proofResult.publicInputs"
            }
            require(proofResult.proofBytes.contentEquals(proofBytes)) {
                "proofBytes must match proofResult.proofBytes"
            }
            require(proofResult.statementHash == statementHash) {
                "statementHash must match proofResult.statementHash"
            }
            require(proofResult.destinationBindingHash == destinationBindingHash) {
                "destinationBindingHash must match proofResult.destinationBindingHash"
            }
        }
        val publicSignalWords = groth16Bn254PublicSignalWords(
            publicInputs = publicInputs,
            sourceDomain = input.sourceDomain,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
        )
        val suppliedPublicSignalWords = input.publicSignalWords ?: proofResult?.publicSignalWords
        if (suppliedPublicSignalWords != null) {
            require(suppliedPublicSignalWords.size == 9) { "publicSignalWords must contain 9 words" }
            val normalized = suppliedPublicSignalWords.mapIndexed { index, word ->
                normalizeHex32(word, "publicSignalWords[$index]")
            }
            require(normalized == publicSignalWords) {
                "publicSignalWords must match publicInputs and proof context"
            }
        }
        val publicInputWords = messageTransparentPublicInputAbiWords(publicInputs)
        val publicInputWordsBytes = messageTransparentPublicInputAbiWordBytes(publicInputs).fold(
            ByteArrayOutputStream(),
        ) { out, word ->
            out.write(word)
            out
        }.toByteArray()
        val callData = submitMessageProofCallData(
            proofBytes,
            publicInputs,
            statementHash,
            input.sourceDomain,
        )
        val arguments = listOf(
            EvmSccpSubmissionArgument("proof_bytes", "raw_bytes", "0x" + hexLower(proofBytes)),
            EvmSccpSubmissionArgument(
                "public_inputs",
                "abi_bytes32x6",
                "0x" + hexLower(publicInputWordsBytes),
            ),
            EvmSccpSubmissionArgument("statement_hash", "abi_bytes32", statementHash),
        )
        return EvmSccpSubmission(
            version = 1,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
            verifierBackend = GROTH16_BN254_PROOF_BACKEND_V1,
            platformPayload = "evm_groth16_contract_call",
            envelopeEncoding = CONTRACT_CALL_ABI_TUPLE_V1,
            submissionKind = "contract_call",
            verifierEntrypoint = SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
            contractMethod = SUBMIT_MESSAGE_PROOF_ABI_V1,
            functionSelector = SUBMIT_MESSAGE_PROOF_SELECTOR_V1,
            sourceDomain = input.sourceDomain,
            targetDomain = publicInputs.targetDomain,
            publicInputs = publicInputs,
            publicInputWords = publicInputWords,
            publicSignalWords = publicSignalWords,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
            arguments = arguments,
            callDataHex = "0x" + hexLower(callData),
            envelopeHex = "0x" + hexLower(callData),
            proofBytes = proofBytes,
            publicInputWordsBytes = publicInputWordsBytes,
            callData = callData,
        )
    }

    private fun requireNonZeroProofBytes(proofBytes: ByteArray) {
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        require(proofBytes.size == GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1) {
            "proofBytes must be $GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 bytes"
        }
        requireGroth16ProofTuple(proofBytes, "proofBytes")
    }

    private fun requireProofBytesForPublicInputs(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
    ) {
        requireNonZeroProofBytes(proofBytes)
        require(
            proofWord(proofBytes, 1)
                .contentEquals(nonZeroHex32Bytes(publicInputs.messageId, "publicInputs.messageId")),
        ) {
            "proofBytes.messageId must match publicInputs.messageId"
        }
        require(
            proofWord(proofBytes, 3)
                .contentEquals(nonZeroHex32Bytes(publicInputs.commitmentRoot, "publicInputs.commitmentRoot")),
        ) {
            "proofBytes.commitmentRoot must match publicInputs.commitmentRoot"
        }
    }

    private fun requireProofBytesForContext(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
        sourceDomain: Int,
    ) {
        requireProofBytesForPublicInputs(proofBytes, publicInputs)
        require(proofWordValue(proofBytes, 2) == BigInteger.valueOf(sourceDomain.toLong())) {
            "proofBytes.sourceDomain must match sourceDomain"
        }
    }

    private fun proofWord(proofBytes: ByteArray, index: Int): ByteArray =
        proofBytes.copyOfRange(index * 32, (index + 1) * 32)

    private fun proofWordValue(proofBytes: ByteArray, index: Int): BigInteger =
        BigInteger(1, proofWord(proofBytes, index))

    private fun proofWordIsZero(proofBytes: ByteArray, index: Int): Boolean =
        proofWord(proofBytes, index).all { it.toInt() == 0 }

    private fun requireBaseFieldWord(proofBytes: ByteArray, index: Int, label: String) {
        require(proofWordValue(proofBytes, index) < BN254_BASE_FIELD_MODULUS) {
            "$label must be a BN254 base-field element"
        }
    }

    private fun requireNonZeroPoint(proofBytes: ByteArray, indexes: List<Int>, label: String) {
        require(indexes.any { !proofWordIsZero(proofBytes, it) }) {
            "$label must not be zero"
        }
    }

    private data class G2ProjectivePoint(
        val x: Pair<BigInteger, BigInteger>,
        val y: Pair<BigInteger, BigInteger>,
        val z: Pair<BigInteger, BigInteger>,
        val infinity: Boolean,
    )

    private fun fq(value: BigInteger): BigInteger = value.mod(BN254_BASE_FIELD_MODULUS)

    private fun fq2Add(
        left: Pair<BigInteger, BigInteger>,
        right: Pair<BigInteger, BigInteger>,
    ): Pair<BigInteger, BigInteger> =
        Pair(fq(left.first + right.first), fq(left.second + right.second))

    private fun fq2Sub(
        left: Pair<BigInteger, BigInteger>,
        right: Pair<BigInteger, BigInteger>,
    ): Pair<BigInteger, BigInteger> =
        Pair(fq(left.first - right.first), fq(left.second - right.second))

    private fun fq2Scale(
        left: Pair<BigInteger, BigInteger>,
        scalar: Long,
    ): Pair<BigInteger, BigInteger> =
        Pair(fq(left.first * BigInteger.valueOf(scalar)), fq(left.second * BigInteger.valueOf(scalar)))

    private fun fq2Mul(
        left: Pair<BigInteger, BigInteger>,
        right: Pair<BigInteger, BigInteger>,
    ): Pair<BigInteger, BigInteger> =
        Pair(
            fq(left.first * right.first - left.second * right.second),
            fq(left.first * right.second + left.second * right.first),
        )

    private fun fq2IsZero(value: Pair<BigInteger, BigInteger>): Boolean =
        value.first == BigInteger.ZERO && value.second == BigInteger.ZERO

    private fun g2Infinity(): G2ProjectivePoint =
        G2ProjectivePoint(
            x = Pair(BigInteger.ZERO, BigInteger.ZERO),
            y = Pair(BigInteger.ONE, BigInteger.ZERO),
            z = Pair(BigInteger.ZERO, BigInteger.ZERO),
            infinity = true,
        )

    private fun g2AffineProjective(
        x: Pair<BigInteger, BigInteger>,
        y: Pair<BigInteger, BigInteger>,
    ): G2ProjectivePoint =
        G2ProjectivePoint(x = x, y = y, z = Pair(BigInteger.ONE, BigInteger.ZERO), infinity = false)

    private fun g2ProjectiveIsInfinity(point: G2ProjectivePoint): Boolean =
        point.infinity || fq2IsZero(point.z)

    private fun g2ProjectiveDouble(point: G2ProjectivePoint): G2ProjectivePoint {
        if (g2ProjectiveIsInfinity(point) || fq2IsZero(point.y)) return g2Infinity()
        val xx = fq2Mul(point.x, point.x)
        val yy = fq2Mul(point.y, point.y)
        val yyyy = fq2Mul(yy, yy)
        val s = fq2Scale(
            fq2Sub(
                fq2Sub(fq2Mul(fq2Add(point.x, yy), fq2Add(point.x, yy)), xx),
                yyyy,
            ),
            2,
        )
        val m = fq2Scale(xx, 3)
        val x3 = fq2Sub(fq2Mul(m, m), fq2Scale(s, 2))
        val y3 = fq2Sub(fq2Mul(m, fq2Sub(s, x3)), fq2Scale(yyyy, 8))
        val z3 = fq2Scale(fq2Mul(point.y, point.z), 2)
        return G2ProjectivePoint(x = x3, y = y3, z = z3, infinity = false)
    }

    private fun g2ProjectiveAddAffine(
        point: G2ProjectivePoint,
        affineX: Pair<BigInteger, BigInteger>,
        affineY: Pair<BigInteger, BigInteger>,
    ): G2ProjectivePoint {
        if (g2ProjectiveIsInfinity(point)) return g2AffineProjective(affineX, affineY)
        val z1z1 = fq2Mul(point.z, point.z)
        val u2 = fq2Mul(affineX, z1z1)
        val s2 = fq2Mul(affineY, fq2Mul(point.z, z1z1))
        val h = fq2Sub(u2, point.x)
        if (fq2IsZero(h)) {
            return if (s2 == point.y) g2ProjectiveDouble(point) else g2Infinity()
        }
        val hh = fq2Mul(h, h)
        val i = fq2Scale(hh, 4)
        val j = fq2Mul(h, i)
        val r = fq2Scale(fq2Sub(s2, point.y), 2)
        val v = fq2Mul(point.x, i)
        val x3 = fq2Sub(fq2Sub(fq2Mul(r, r), j), fq2Scale(v, 2))
        val y3 = fq2Sub(fq2Mul(r, fq2Sub(v, x3)), fq2Scale(fq2Mul(point.y, j), 2))
        val z3 = fq2Sub(fq2Sub(fq2Mul(fq2Add(point.z, h), fq2Add(point.z, h)), z1z1), hh)
        return G2ProjectivePoint(x = x3, y = y3, z = z3, infinity = false)
    }

    private fun g2PointIsInPrimeSubgroup(
        x: Pair<BigInteger, BigInteger>,
        y: Pair<BigInteger, BigInteger>,
    ): Boolean {
        var acc = g2Infinity()
        for (index in BN254_SCALAR_FIELD_MODULUS.bitLength() - 1 downTo 0) {
            acc = g2ProjectiveDouble(acc)
            if (BN254_SCALAR_FIELD_MODULUS.testBit(index)) {
                acc = g2ProjectiveAddAffine(acc, x, y)
            }
        }
        return g2ProjectiveIsInfinity(acc)
    }

    private fun requireG1Point(proofBytes: ByteArray, xIndex: Int, yIndex: Int, label: String) {
        requireNonZeroPoint(proofBytes, listOf(xIndex, yIndex), label)
        val x = proofWordValue(proofBytes, xIndex)
        val y = proofWordValue(proofBytes, yIndex)
        val left = fq(y * y)
        val right = fq(x * x * x + BigInteger.valueOf(3))
        require(left == right) { "$label must be a BN254 G1 point" }
    }

    private fun requireG2Point(
        proofBytes: ByteArray,
        x0Index: Int,
        x1Index: Int,
        y0Index: Int,
        y1Index: Int,
        label: String,
    ) {
        requireNonZeroPoint(proofBytes, listOf(x0Index, x1Index, y0Index, y1Index), label)
        val x = Pair(proofWordValue(proofBytes, x0Index), proofWordValue(proofBytes, x1Index))
        val y = Pair(proofWordValue(proofBytes, y0Index), proofWordValue(proofBytes, y1Index))
        val left = fq2Mul(y, y)
        val x2 = fq2Mul(x, x)
        val x3 = fq2Mul(x2, x)
        val right = Pair(fq(x3.first + BN254_G2_B_C0), fq(x3.second + BN254_G2_B_C1))
        require(left == right && g2PointIsInPrimeSubgroup(x, y)) { "$label must be a BN254 G2 point" }
    }

    private fun requireGroth16ProofTuple(proofBytes: ByteArray, label: String) {
        require(proofWordValue(proofBytes, 0) == BigInteger.ONE) { "$label.version must be 1" }
        require(!proofWordIsZero(proofBytes, 1)) { "$label.messageId must not be zero" }
        require(proofWordValue(proofBytes, 2) <= BigInteger.valueOf(0xffff_ffffL)) {
            "$label.sourceDomain must fit u32"
        }
        require(!proofWordIsZero(proofBytes, 3)) { "$label.commitmentRoot must not be zero" }
        listOf("a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y")
            .forEachIndexed { offset, field ->
                requireBaseFieldWord(proofBytes, 4 + offset, "$label.$field")
            }
        requireG1Point(proofBytes, 4, 5, "$label.a")
        requireG2Point(proofBytes, 6, 7, 8, 9, "$label.b")
        requireG1Point(proofBytes, 10, 11, "$label.c")
    }

    private fun requireCanonicalProofRequest(request: EvmSccpProofRequest) {
        val expected = buildProofRequest(
            EvmSccpProofRequestInput(
                publicInputs = request.publicInputs,
                bundleBytes = request.bundleBytes,
                sourceProofBytes = request.sourceProofBytes,
                statementHash = request.statementHash,
                destinationBindingHash = request.destinationBindingHash,
                backend = request.backend,
                sourceDomain = request.sourceDomain,
                destinationBinding = request.destinationBinding,
            ),
        )
        require(
            request.version == expected.version &&
                request.backend == expected.backend &&
                request.sourceDomain == expected.sourceDomain &&
                request.targetDomain == expected.targetDomain &&
                request.publicInputs == expected.publicInputs &&
                request.publicInputsBytes.contentEquals(expected.publicInputsBytes) &&
                request.publicSignalWords == expected.publicSignalWords &&
                request.bundleBytes.contentEquals(expected.bundleBytes) &&
                request.sourceProofBytes.contentEquals(expected.sourceProofBytes) &&
                request.proofContext == expected.proofContext &&
                request.statementHash == expected.statementHash &&
                request.destinationBindingHash == expected.destinationBindingHash &&
                request.requestHash == expected.requestHash &&
                request.destinationBinding == expected.destinationBinding,
        ) { "proof request must be canonical" }
    }

    internal fun requireProductionProofRequest(request: EvmSccpProofRequest) {
        requireCanonicalProofRequest(request)
        require(request.version == 1) { "proof request version must be 1" }
        require(request.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "EVM-family SCCP proof request backend must be evm-groth16-bn254-v1"
        }
        require(request.sourceDomain == DOMAIN_SORA) {
            "EVM-family SCCP production proofs must start from SORA"
        }
        require(
            request.targetDomain == request.publicInputs.targetDomain &&
                (request.targetDomain == DOMAIN_ETH || request.targetDomain == DOMAIN_BSC),
        ) {
            "EVM-family SCCP production proofs must target ETH or BSC"
        }
        require(request.bundleBytes.isNotEmpty()) {
            "EVM-family SCCP proof request bundleBytes must not be empty"
        }
        requireOptionalSourceProofBytes(
            request.sourceProofBytes,
            "EVM-family SCCP proof request sourceProofBytes",
        )
        requireProductionDestinationBinding(request)
    }

    private fun requireOptionalSourceProofBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.size <= SOURCE_STATE_MAX_PROOF_BYTES) {
            "$label must be at most $SOURCE_STATE_MAX_PROOF_BYTES bytes"
        }
        require(copy.isEmpty() || copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        return copy
    }

    private fun requireProductionDestinationBinding(request: EvmSccpProofRequest) {
        val destinationBinding = request.destinationBinding
        require(destinationBinding != null) {
            "EVM-family SCCP production proof request destinationBinding must include deployment material"
        }
        val destinationBindingHash = requireDestinationBindingHashForProofRequest(
            publicInputs = request.publicInputs,
            destinationBinding = destinationBinding,
            backend = request.backend,
            sourceDomain = request.sourceDomain,
        )
        require(request.destinationBindingHash == destinationBindingHash) {
            "destinationBindingHash must match destinationBinding deployment material"
        }
        require(request.proofContext.destinationBindingHash == destinationBindingHash) {
            "proofContext.destinationBindingHash must match destinationBinding deployment material"
        }
    }

    internal fun requireDestinationBindingHashForProofRequest(
        publicInputs: EvmSccpPublicInputsInput,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding,
        backend: String,
        sourceDomain: Int,
    ): String {
        require(destinationBinding.version == 1) {
            "destinationBinding.version must be 1"
        }
        require(destinationBinding.sourceDomain == sourceDomain) {
            "destinationBinding.sourceDomain must match sourceDomain"
        }
        require(destinationBinding.targetDomain == publicInputs.targetDomain) {
            "destinationBinding.targetDomain must match publicInputs.targetDomain"
        }
        require(destinationBinding.verifierBackend == backend) {
            "destinationBinding.verifierBackend must match backend"
        }
        require(destinationBinding.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "destinationBinding.proofFamily must be stark-fri-v1"
        }
        val expectedDestinationBinding = SccpSourceProofs.evmDestinationBinding(
            sourceDomain = sourceDomain,
            targetDomain = publicInputs.targetDomain,
            networkId = destinationBinding.networkId,
            verifierAddress = destinationBinding.verifierAddress,
            bridgeAddress = destinationBinding.bridgeAddress,
            verifierCodeHash = destinationBinding.verifierCodeHash,
            verifierKeyHash = destinationBinding.verifierKeyHash,
            verifierBackend = backend,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
        )
        require(destinationBinding == expectedDestinationBinding) {
            "destinationBinding must match deployment material"
        }
        return expectedDestinationBinding.hash
    }

    internal fun callbackRequestSnapshot(request: EvmSccpProofRequest): EvmSccpProofRequest =
        request.copy()

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

    private fun normalizeHex32(value: String, field: String): String =
        "0x" + hexLower(hex32Bytes(value, field))

    private fun normalizeProofContext(
        statementHash: String,
        destinationBindingHash: String,
    ): EvmSccpProofContext =
        EvmSccpProofContext(
            version = 1,
            statementHash = normalizeNonZeroHex32(statementHash, "statementHash"),
            destinationBindingHash = normalizeNonZeroHex32(destinationBindingHash, "destinationBindingHash"),
        )

    private fun groth16SignalWord(label: String, value: ByteArray): String {
        val labelHash = keccak256(label.toByteArray(Charsets.UTF_8))
        val payload = ByteArray(labelHash.size + value.size)
        System.arraycopy(labelHash, 0, payload, 0, labelHash.size)
        System.arraycopy(value, 0, payload, labelHash.size, value.size)
        val reduced = BigInteger(1, keccak256(payload)).mod(BN254_SCALAR_FIELD_MODULUS)
        return "0x" + hexLower(toFixedBytes(reduced, 32))
    }

    private fun keccak256(input: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(input, 0, input.size)
        val out = ByteArray(32)
        digest.doFinal(out, 0)
        return out
    }

    private fun abiWordU32(value: Int): ByteArray {
        require(value >= 0) { "domain id must not be negative" }
        val out = ByteArray(32)
        out[28] = ((value ushr 24) and 0xff).toByte()
        out[29] = ((value ushr 16) and 0xff).toByte()
        out[30] = ((value ushr 8) and 0xff).toByte()
        out[31] = (value and 0xff).toByte()
        return out
    }

    private fun abiWordU64(value: BigInteger): ByteArray {
        val out = ByteArray(32)
        var working = value
        for (index in 31 downTo 24) {
            out[index] = working.and(BigInteger.valueOf(0xffL)).toByte()
            working = working.shiftRight(8)
        }
        return out
    }

    private fun abiWordU256(value: Int): ByteArray {
        require(value >= 0) { "u256 value must not be negative" }
        val out = ByteArray(32)
        var working = value
        for (index in 31 downTo 0) {
            out[index] = (working and 0xff).toByte()
            working = working ushr 8
            if (working == 0) break
        }
        return out
    }

    private fun messageTransparentPublicInputAbiWordBytes(
        publicInputs: EvmSccpPublicInputsInput,
    ): List<ByteArray> {
        canonicalPublicInputsBytes(publicInputs)
        return listOf(
            nonZeroHex32Bytes(publicInputs.messageId, "messageId"),
            nonZeroHex32Bytes(publicInputs.payloadHash, "payloadHash"),
            abiWordU32(publicInputs.targetDomain),
            nonZeroHex32Bytes(publicInputs.commitmentRoot, "commitmentRoot"),
            abiWordU64(normalizeU64(publicInputs.finalityHeight, "finalityHeight")),
            nonZeroHex32Bytes(publicInputs.finalityBlockHash, "finalityBlockHash"),
        )
    }

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

    private fun toFixedBytes(value: BigInteger, length: Int): ByteArray {
        val raw = value.toByteArray()
        val out = ByteArray(length)
        val copyLength = Math.min(raw.size, length)
        System.arraycopy(raw, raw.size - copyLength, out, length - copyLength, copyLength)
        return out
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) builder.append(String.format("%02x", byte.toInt() and 0xff))
        return builder.toString()
    }
}

/** Ethereum mainnet SCCP Groth16 helpers with chain-id and domain checks baked in. */
object SccpEthereumMainnet {
    const val DOMAIN_SORA: Int = SccpEvm.DOMAIN_SORA
    const val DOMAIN_ETH: Int = SccpEvm.DOMAIN_ETH
    const val MAINNET_CHAIN_ID: Long = SccpSourceProofs.ETH_MAINNET_CHAIN_ID
    const val MAINNET_NETWORK_ID: String = SccpSourceProofs.ETH_MAINNET_NETWORK_ID
    const val LOCAL_ADMISSION_ENVELOPE_ENCODING_V1: String = "norito:sccp-local-admission:v1"
    const val LOCAL_ADMISSION_SUBMISSION_KIND_V1: String = "local_admission"
    const val LOCAL_ADMISSION_ENTRYPOINT_V1: String = "SubmitBridgeProof"
    const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"
    const val SOURCE_EVENT_ABI_V1: String = "SccpSourceEvent(bytes32)"
    const val SOURCE_EVENT_TOPIC_V1: String =
        "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727"

    @JvmStatic
    fun requireMainnetChainId(chainId: Long) {
        require(chainId == MAINNET_CHAIN_ID) {
            "Ethereum mainnet SCCP requires eth_chainId == 1"
        }
    }

    @JvmStatic
    fun sourceEventTopic(): String = SOURCE_EVENT_TOPIC_V1

    @JvmStatic
    @JvmOverloads
    fun destinationBinding(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): SccpSourceProofs.EvmDestinationBinding =
        SccpSourceProofs.ethereumMainnetDestinationBinding(
            verifierAddress = verifierAddress,
            bridgeAddress = bridgeAddress,
            verifierCodeHash = verifierCodeHash,
            verifierKeyHash = verifierKeyHash,
            networkId = networkId,
        )

    @JvmStatic
    @JvmOverloads
    fun destinationBindingHash(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): String = destinationBinding(
        verifierAddress = verifierAddress,
        bridgeAddress = bridgeAddress,
        verifierCodeHash = verifierCodeHash,
        verifierKeyHash = verifierKeyHash,
        networkId = networkId,
    ).hash

    @JvmStatic
    fun buildProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest {
        require(input.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet proof requests must route SORA -> ETH"
        }
        require(input.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet proof requests must target ETH"
        }
        val destinationBinding = input.destinationBinding
            ?: throw IllegalArgumentException("Ethereum mainnet proof requests require destinationBinding")
        require(destinationBinding.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet destinationBinding must start from SORA"
        }
        require(destinationBinding.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet destinationBinding must target ETH"
        }
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "Ethereum mainnet destinationBinding.networkId must be chain id 1"
        }
        val destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = input.publicInputs,
            destinationBinding = destinationBinding,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
        )
        require(input.destinationBindingHash == destinationBindingHash) {
            "destinationBindingHash must match Ethereum mainnet destinationBinding"
        }
        return SccpEvm.buildProofRequest(input)
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: EvmSccpProofRequest): EvmSccpProofResult {
        require(request.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet proof results must route SORA -> ETH"
        }
        require(request.targetDomain == DOMAIN_ETH && request.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet proof results must target ETH"
        }
        require(request.destinationBinding?.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet proof results require SORA source destinationBinding"
        }
        require(request.destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "Ethereum mainnet proof results require chain id 1 destinationBinding"
        }
        return SccpEvm.wrapProofResult(proofBytes, request)
    }

    @JvmStatic
    fun buildSubmission(input: EvmSccpSubmissionInput): EvmSccpSubmission {
        require(input.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet submissions must target ETH"
        }
        val proofResult = input.proofResult
            ?: throw IllegalArgumentException(
                "Ethereum mainnet submissions require a wrapped proofResult with destinationBinding",
            )
        require(proofResult.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet proofResult must target ETH"
        }
        val destinationBinding = proofResult.destinationBinding
            ?: throw IllegalArgumentException("Ethereum mainnet proofResult requires destinationBinding")
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "Ethereum mainnet proofResult requires chain id 1 destinationBinding"
        }
        require(destinationBinding.hash == proofResult.destinationBindingHash) {
            "Ethereum mainnet proofResult destinationBindingHash must match destinationBinding"
        }
        return SccpEvm.buildSubmission(input)
    }

    @JvmStatic
    fun buildLocalAdmissionSubmission(
        input: EthereumMainnetLocalAdmissionSubmissionInput,
    ): EthereumMainnetLocalAdmissionSubmission {
        require(input.sourceDomain == DOMAIN_ETH && input.targetDomain == DOMAIN_SORA) {
            "Ethereum mainnet local-admission submissions must route ETH -> SORA"
        }
        require(input.envelopeEncoding == LOCAL_ADMISSION_ENVELOPE_ENCODING_V1) {
            "Ethereum mainnet local-admission envelopeEncoding is not canonical"
        }
        require(input.submissionKind == LOCAL_ADMISSION_SUBMISSION_KIND_V1) {
            "Ethereum mainnet local-admission submissionKind is not canonical"
        }
        require(input.verifierEntrypoint == LOCAL_ADMISSION_ENTRYPOINT_V1) {
            "Ethereum mainnet local-admission verifierEntrypoint is not canonical"
        }
        require(input.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "Ethereum mainnet local-admission proofFamily is not canonical"
        }
        require(input.verifierBackend == SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1) {
            "Ethereum mainnet local-admission verifierBackend is not canonical"
        }
        val proofBytes = requireNativeRecursiveBytes(input.proofBytes, "proofBytes")
        val publicInputsBytes = requireNativeRecursiveBytes(
            input.publicInputsBytes,
            "publicInputsBytes",
        )
        val bundleBytes = requireNativeRecursiveBytes(input.bundleBytes, "bundleBytes")
        val envelopeBytes = requireNativeRecursiveBytes(input.envelopeBytes, "envelopeBytes")
        val statementHash = normalizeNonZeroHex32(input.statementHash, "statementHash")
        val sourceVerifierMaterialHash = normalizeNonZeroHex32(
            input.sourceVerifierMaterialHash,
            "sourceVerifierMaterialHash",
        )
        val sourceAdapterEngineDeploymentHash = normalizeNonZeroHex32(
            input.sourceAdapterEngineDeploymentHash,
            "sourceAdapterEngineDeploymentHash",
        )
        val localAdmission = EthereumMainnetLocalAdmissionPayload(
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
        )
        return EthereumMainnetLocalAdmissionSubmission(
            version = 1,
            proofFamily = input.proofFamily,
            verifierBackend = input.verifierBackend,
            platformPayload = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            envelopeEncoding = LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
            submissionKind = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            verifierEntrypoint = LOCAL_ADMISSION_ENTRYPOINT_V1,
            sourceDomain = DOMAIN_ETH,
            targetDomain = DOMAIN_SORA,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
            localAdmission = localAdmission,
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            envelopeBytes = envelopeBytes,
        )
    }

    private fun requireNativeRecursiveBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.isNotEmpty()) { "$label must not be empty" }
        require(copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        require(copy.size <= SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "$label must be at most ${SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES} bytes"
        }
        return copy
    }

    private fun normalizeNonZeroHex32(value: String, field: String): String {
        require(value.startsWith("0x") && value.length == 66) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        require(text.any { it != '0' }) { "$field must not be zero" }
        return "0x$text"
    }
}

/** BSC mainnet SCCP Groth16 helpers with chain-id and domain checks baked in. */
object SccpBsc {
    const val DOMAIN_SORA: Int = SccpEvm.DOMAIN_SORA
    const val DOMAIN_BSC: Int = SccpEvm.DOMAIN_BSC
    const val MAINNET_CHAIN_ID: Long = SccpSourceProofs.BSC_MAINNET_CHAIN_ID
    const val MAINNET_NETWORK_ID: String = SccpSourceProofs.BSC_MAINNET_NETWORK_ID
    const val LOCAL_ADMISSION_ENVELOPE_ENCODING_V1: String = "norito:sccp-local-admission:v1"
    const val LOCAL_ADMISSION_SUBMISSION_KIND_V1: String = "local_admission"
    const val LOCAL_ADMISSION_ENTRYPOINT_V1: String = "SubmitBridgeProof"
    const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"

    @JvmStatic
    fun requireMainnetChainId(chainId: Long) {
        require(chainId == MAINNET_CHAIN_ID) {
            "BSC mainnet SCCP requires eth_chainId == 56"
        }
    }

    @JvmStatic
    @JvmOverloads
    fun destinationBinding(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): SccpSourceProofs.EvmDestinationBinding =
        SccpSourceProofs.bscMainnetDestinationBinding(
            verifierAddress = verifierAddress,
            bridgeAddress = bridgeAddress,
            verifierCodeHash = verifierCodeHash,
            verifierKeyHash = verifierKeyHash,
            networkId = networkId,
        )

    @JvmStatic
    @JvmOverloads
    fun destinationBindingHash(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): String = destinationBinding(
        verifierAddress = verifierAddress,
        bridgeAddress = bridgeAddress,
        verifierCodeHash = verifierCodeHash,
        verifierKeyHash = verifierKeyHash,
        networkId = networkId,
    ).hash

    @JvmStatic
    fun buildProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest {
        require(input.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet proof requests must target BSC"
        }
        val destinationBinding = input.destinationBinding
            ?: throw IllegalArgumentException("BSC mainnet proof requests require destinationBinding")
        require(destinationBinding.targetDomain == DOMAIN_BSC) {
            "BSC mainnet destinationBinding must target BSC"
        }
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "BSC mainnet destinationBinding.networkId must be chain id 56"
        }
        val destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = input.publicInputs,
            destinationBinding = destinationBinding,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
        )
        require(input.destinationBindingHash == destinationBindingHash) {
            "destinationBindingHash must match BSC mainnet destinationBinding"
        }
        return SccpEvm.buildProofRequest(input)
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: EvmSccpProofRequest): EvmSccpProofResult {
        require(request.targetDomain == DOMAIN_BSC && request.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet proof results must target BSC"
        }
        require(request.destinationBinding?.networkId == MAINNET_NETWORK_ID) {
            "BSC mainnet proof results require chain id 56 destinationBinding"
        }
        return SccpEvm.wrapProofResult(proofBytes, request)
    }

    @JvmStatic
    fun buildSubmission(input: EvmSccpSubmissionInput): EvmSccpSubmission {
        require(input.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet submissions must target BSC"
        }
        val proofResult = input.proofResult
            ?: throw IllegalArgumentException(
                "BSC mainnet submissions require a wrapped proofResult with destinationBinding",
            )
        require(proofResult.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet proofResult must target BSC"
        }
        val destinationBinding = proofResult.destinationBinding
            ?: throw IllegalArgumentException("BSC mainnet proofResult requires destinationBinding")
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "BSC mainnet proofResult requires chain id 56 destinationBinding"
        }
        require(destinationBinding.hash == proofResult.destinationBindingHash) {
            "BSC mainnet proofResult destinationBindingHash must match destinationBinding"
        }
        return SccpEvm.buildSubmission(input)
    }

    @JvmStatic
    fun buildLocalAdmissionSubmission(
        input: BscMainnetLocalAdmissionSubmissionInput,
    ): BscMainnetLocalAdmissionSubmission {
        require(input.sourceDomain == DOMAIN_BSC && input.targetDomain == DOMAIN_SORA) {
            "BSC mainnet local-admission submissions must route BSC -> SORA"
        }
        require(input.envelopeEncoding == LOCAL_ADMISSION_ENVELOPE_ENCODING_V1) {
            "BSC mainnet local-admission envelopeEncoding is not canonical"
        }
        require(input.submissionKind == LOCAL_ADMISSION_SUBMISSION_KIND_V1) {
            "BSC mainnet local-admission submissionKind is not canonical"
        }
        require(input.verifierEntrypoint == LOCAL_ADMISSION_ENTRYPOINT_V1) {
            "BSC mainnet local-admission verifierEntrypoint is not canonical"
        }
        require(input.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "BSC mainnet local-admission proofFamily is not canonical"
        }
        require(input.verifierBackend == SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1) {
            "BSC mainnet local-admission verifierBackend is not canonical"
        }
        val proofBytes = requireNativeRecursiveBytes(input.proofBytes, "proofBytes")
        val publicInputsBytes = requireNativeRecursiveBytes(
            input.publicInputsBytes,
            "publicInputsBytes",
        )
        val bundleBytes = requireNativeRecursiveBytes(input.bundleBytes, "bundleBytes")
        val envelopeBytes = requireNativeRecursiveBytes(input.envelopeBytes, "envelopeBytes")
        val statementHash = normalizeNonZeroHex32(input.statementHash, "statementHash")
        val sourceVerifierMaterialHash = normalizeNonZeroHex32(
            input.sourceVerifierMaterialHash,
            "sourceVerifierMaterialHash",
        )
        val sourceAdapterEngineDeploymentHash = normalizeNonZeroHex32(
            input.sourceAdapterEngineDeploymentHash,
            "sourceAdapterEngineDeploymentHash",
        )
        val localAdmission = BscMainnetLocalAdmissionPayload(
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
        )
        return BscMainnetLocalAdmissionSubmission(
            version = 1,
            proofFamily = input.proofFamily,
            verifierBackend = input.verifierBackend,
            platformPayload = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            envelopeEncoding = LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
            submissionKind = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            verifierEntrypoint = LOCAL_ADMISSION_ENTRYPOINT_V1,
            sourceDomain = DOMAIN_BSC,
            targetDomain = DOMAIN_SORA,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
            localAdmission = localAdmission,
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            envelopeBytes = envelopeBytes,
        )
    }

    private fun requireNativeRecursiveBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.isNotEmpty()) { "$label must not be empty" }
        require(copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        require(copy.size <= SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "$label must be at most ${SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES} bytes"
        }
        return copy
    }

    private fun normalizeNonZeroHex32(value: String, field: String): String {
        require(value.startsWith("0x") && value.length == 66) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        require(text.any { it != '0' }) { "$field must not be zero" }
        return "0x$text"
    }
}

/** Input for Ethereum mainnet -> SORA local-admission submission packaging. */
data class EthereumMainnetLocalAdmissionSubmissionInput(
    val proofBytes: ByteArray,
    val publicInputsBytes: ByteArray,
    val bundleBytes: ByteArray,
    val envelopeBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val sourceDomain: Int = SccpEthereumMainnet.DOMAIN_ETH,
    val targetDomain: Int = SccpEthereumMainnet.DOMAIN_SORA,
    val proofFamily: String = SccpEthereumMainnet.STARK_FRI_PROOF_FAMILY_V1,
    val verifierBackend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
    val envelopeEncoding: String = SccpEthereumMainnet.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
    val submissionKind: String = SccpEthereumMainnet.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
    val verifierEntrypoint: String = SccpEthereumMainnet.LOCAL_ADMISSION_ENTRYPOINT_V1,
)

/** Ethereum mainnet local-admission payload mirrored from the core SCCP package. */
class EthereumMainnetLocalAdmissionPayload(
    val version: Int = 1,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()
}

/** Ethereum mainnet -> SORA local-admission package ready for Torii bridge-proof submission. */
class EthereumMainnetLocalAdmissionSubmission(
    val version: Int,
    val proofFamily: String,
    val verifierBackend: String,
    val platformPayload: String,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val localAdmission: EthereumMainnetLocalAdmissionPayload,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    envelopeBytes: ByteArray,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val envelopeBytesStorage: ByteArray = envelopeBytes.copyOf()

    val arguments: List<EvmSccpSubmissionArgument> = emptyList()
    val proofBytesHex: String = "0x" + localHexLower(proofBytes)
    val publicInputsBytesHex: String = "0x" + localHexLower(publicInputsBytes)
    val bundleBytesHex: String = "0x" + localHexLower(bundleBytes)
    val envelopeHex: String = "0x" + localHexLower(envelopeBytes)

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = envelopeBytesStorage.copyOf()
}

/** Input for BSC -> SORA local-admission submission packaging. */
data class BscMainnetLocalAdmissionSubmissionInput(
    val proofBytes: ByteArray,
    val publicInputsBytes: ByteArray,
    val bundleBytes: ByteArray,
    val envelopeBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val sourceDomain: Int = SccpBsc.DOMAIN_BSC,
    val targetDomain: Int = SccpBsc.DOMAIN_SORA,
    val proofFamily: String = SccpBsc.STARK_FRI_PROOF_FAMILY_V1,
    val verifierBackend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
    val envelopeEncoding: String = SccpBsc.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
    val submissionKind: String = SccpBsc.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
    val verifierEntrypoint: String = SccpBsc.LOCAL_ADMISSION_ENTRYPOINT_V1,
)

/** BSC local-admission payload mirrored from the core SCCP package. */
class BscMainnetLocalAdmissionPayload(
    val version: Int = 1,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()
}

/** BSC -> SORA local-admission package ready for Torii bridge-proof submission. */
class BscMainnetLocalAdmissionSubmission(
    val version: Int,
    val proofFamily: String,
    val verifierBackend: String,
    val platformPayload: String,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val localAdmission: BscMainnetLocalAdmissionPayload,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    envelopeBytes: ByteArray,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val envelopeBytesStorage: ByteArray = envelopeBytes.copyOf()

    val arguments: List<EvmSccpSubmissionArgument> = emptyList()
    val proofBytesHex: String = "0x" + localHexLower(proofBytes)
    val publicInputsBytesHex: String = "0x" + localHexLower(publicInputsBytes)
    val bundleBytesHex: String = "0x" + localHexLower(bundleBytes)
    val envelopeHex: String = "0x" + localHexLower(envelopeBytes)

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = envelopeBytesStorage.copyOf()
}

private fun localHexLower(bytes: ByteArray): String =
    bytes.joinToString(separator = "") { byte -> (byte.toInt() and 0xff).toString(16).padStart(2, '0') }

/** SCCP public inputs shared by EVM-family Groth16 proof requests. */
data class EvmSccpPublicInputsInput(
    val version: Int = 1,
    val messageId: String,
    val payloadHash: String,
    val targetDomain: Int = SccpEvm.DOMAIN_ETH,
    val commitmentRoot: String,
    val finalityHeight: String,
    val finalityBlockHash: String,
) {
    internal fun toTronPublicInputs(): TronSccpPublicInputsInput =
        TronSccpPublicInputsInput(
            version = version,
            messageId = messageId,
            payloadHash = payloadHash,
            targetDomain = targetDomain,
            commitmentRoot = commitmentRoot,
            finalityHeight = finalityHeight,
            finalityBlockHash = finalityBlockHash,
        )
}

/** Inputs used to build a local EVM-family SCCP Groth16 proof request. */
data class EvmSccpProofRequestInput @JvmOverloads constructor(
    val publicInputs: EvmSccpPublicInputsInput,
    val bundleBytes: ByteArray,
    val sourceProofBytes: ByteArray = ByteArray(0),
    val statementHash: String,
    val destinationBindingHash: String,
    val backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
    val sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    val destinationBinding: SccpSourceProofs.EvmDestinationBinding? = null,
) {
    constructor(
        publicInputs: EvmSccpPublicInputsInput,
        bundleBytes: ByteArray,
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding,
        backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    ) : this(
        publicInputs = publicInputs,
        bundleBytes = bundleBytes,
        sourceProofBytes = sourceProofBytes,
        statementHash = statementHash,
        destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = publicInputs,
            destinationBinding = destinationBinding,
            backend = backend,
            sourceDomain = sourceDomain,
        ),
        backend = backend,
        sourceDomain = sourceDomain,
        destinationBinding = destinationBinding,
    )
}

/** Statement and verifier deployment context proved by the local EVM-family SCCP prover. */
data class EvmSccpProofContext(
    val version: Int,
    val statementHash: String,
    val destinationBindingHash: String,
)

/** Request passed to a linked local EVM-family Groth16 prover. */
class EvmSccpProofRequest @JvmOverloads constructor(
    val version: Int,
    val backend: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: EvmSccpPublicInputsInput,
    publicInputsBytes: ByteArray,
    publicSignalWords: List<String>,
    bundleBytes: ByteArray,
    sourceProofBytes: ByteArray,
    val proofContext: EvmSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val requestHash: String,
    val destinationBinding: SccpSourceProofs.EvmDestinationBinding? = null,
) {
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val sourceProofBytesStorage: ByteArray = sourceProofBytes.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val publicSignalWords: List<String> = publicSignalWords.toList()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val sourceProofBytes: ByteArray
        get() = sourceProofBytesStorage.copyOf()

    fun copy(
        version: Int = this.version,
        backend: String = this.backend,
        sourceDomain: Int = this.sourceDomain,
        targetDomain: Int = this.targetDomain,
        publicInputs: EvmSccpPublicInputsInput = this.publicInputs,
        publicInputsBytes: ByteArray = this.publicInputsBytes,
        publicSignalWords: List<String> = this.publicSignalWords,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: EvmSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        requestHash: String = this.requestHash,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding? = this.destinationBinding,
    ): EvmSccpProofRequest =
        EvmSccpProofRequest(
            version,
            backend,
            sourceDomain,
            targetDomain,
            publicInputs,
            publicInputsBytes,
            publicSignalWords,
            bundleBytes,
            sourceProofBytes,
            proofContext,
            statementHash,
            destinationBindingHash,
            requestHash,
            destinationBinding,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): Int = sourceDomain
    operator fun component4(): Int = targetDomain
    operator fun component5(): EvmSccpPublicInputsInput = publicInputs
    operator fun component6(): ByteArray = publicInputsBytes
    operator fun component7(): List<String> = publicSignalWords
    operator fun component8(): ByteArray = bundleBytes
    operator fun component9(): ByteArray = sourceProofBytes
    operator fun component10(): EvmSccpProofContext = proofContext
    operator fun component11(): String = statementHash
    operator fun component12(): String = destinationBindingHash
    operator fun component13(): String = requestHash
    operator fun component14(): SccpSourceProofs.EvmDestinationBinding? = destinationBinding

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is EvmSccpProofRequest &&
            version == other.version &&
            backend == other.backend &&
            sourceDomain == other.sourceDomain &&
            targetDomain == other.targetDomain &&
            publicInputs == other.publicInputs &&
            publicInputsBytesStorage.contentEquals(other.publicInputsBytesStorage) &&
            publicSignalWords == other.publicSignalWords &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            sourceProofBytesStorage.contentEquals(other.sourceProofBytesStorage) &&
            proofContext == other.proofContext &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            requestHash == other.requestHash &&
            destinationBinding == other.destinationBinding

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + backend.hashCode()
        result = 31 * result + sourceDomain
        result = 31 * result + targetDomain
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + publicInputsBytesStorage.contentHashCode()
        result = 31 * result + publicSignalWords.hashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + sourceProofBytesStorage.contentHashCode()
        result = 31 * result + proofContext.hashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + requestHash.hashCode()
        result = 31 * result + (destinationBinding?.hashCode() ?: 0)
        return result
    }

    override fun toString(): String =
        "EvmSccpProofRequest(version=$version, backend=$backend, sourceDomain=$sourceDomain, " +
            "targetDomain=$targetDomain, publicInputs=$publicInputs, " +
            "publicInputsBytes=${publicInputsBytesStorage.size} bytes, " +
            "publicSignalWords=$publicSignalWords, bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, proofContext=$proofContext, " +
            "statementHash=$statementHash, destinationBindingHash=$destinationBindingHash, " +
            "requestHash=$requestHash, destinationBinding=$destinationBinding)"
}

/** Proof bytes returned by a linked local EVM-family Groth16 prover. */
class EvmSccpProofResult @JvmOverloads constructor(
    val version: Int,
    val backend: String,
    proofBytes: ByteArray,
    val proofBase64: String,
    val publicInputs: EvmSccpPublicInputsInput,
    publicSignalWords: List<String>,
    bundleBytes: ByteArray = ByteArray(0),
    sourceProofBytes: ByteArray = ByteArray(0),
    val proofContext: EvmSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val requestHash: String,
    val envelopeHash: String,
    val destinationBinding: SccpSourceProofs.EvmDestinationBinding? = null,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val sourceProofBytesStorage: ByteArray = sourceProofBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicSignalWords: List<String> = publicSignalWords.toList()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val sourceProofBytes: ByteArray
        get() = sourceProofBytesStorage.copyOf()

    fun copy(
        version: Int = this.version,
        backend: String = this.backend,
        proofBytes: ByteArray = this.proofBytes,
        proofBase64: String = this.proofBase64,
        publicInputs: EvmSccpPublicInputsInput = this.publicInputs,
        publicSignalWords: List<String> = this.publicSignalWords,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: EvmSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        requestHash: String = this.requestHash,
        envelopeHash: String = this.envelopeHash,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding? = this.destinationBinding,
    ): EvmSccpProofResult =
        EvmSccpProofResult(
            version,
            backend,
            proofBytes,
            proofBase64,
            publicInputs,
            publicSignalWords,
            bundleBytes,
            sourceProofBytes,
            proofContext,
            statementHash,
            destinationBindingHash,
            requestHash,
            envelopeHash,
            destinationBinding,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): ByteArray = proofBytes
    operator fun component4(): String = proofBase64
    operator fun component5(): EvmSccpPublicInputsInput = publicInputs
    operator fun component6(): List<String> = publicSignalWords
    operator fun component7(): ByteArray = bundleBytes
    operator fun component8(): ByteArray = sourceProofBytes
    operator fun component9(): EvmSccpProofContext = proofContext
    operator fun component10(): String = statementHash
    operator fun component11(): String = destinationBindingHash
    operator fun component12(): String = requestHash
    operator fun component13(): String = envelopeHash
    operator fun component14(): SccpSourceProofs.EvmDestinationBinding? = destinationBinding

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is EvmSccpProofResult &&
            version == other.version &&
            backend == other.backend &&
            proofBytesStorage.contentEquals(other.proofBytesStorage) &&
            proofBase64 == other.proofBase64 &&
            publicInputs == other.publicInputs &&
            publicSignalWords == other.publicSignalWords &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            sourceProofBytesStorage.contentEquals(other.sourceProofBytesStorage) &&
            proofContext == other.proofContext &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            requestHash == other.requestHash &&
            envelopeHash == other.envelopeHash &&
            destinationBinding == other.destinationBinding

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + backend.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        result = 31 * result + proofBase64.hashCode()
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + publicSignalWords.hashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + sourceProofBytesStorage.contentHashCode()
        result = 31 * result + proofContext.hashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + requestHash.hashCode()
        result = 31 * result + envelopeHash.hashCode()
        result = 31 * result + (destinationBinding?.hashCode() ?: 0)
        return result
    }

    override fun toString(): String =
        "EvmSccpProofResult(version=$version, backend=$backend, " +
            "proofBytes=${proofBytesStorage.size} bytes, proofBase64=$proofBase64, " +
            "publicInputs=$publicInputs, publicSignalWords=$publicSignalWords, " +
            "bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, " +
            "proofContext=$proofContext, statementHash=$statementHash, " +
            "destinationBindingHash=$destinationBindingHash, requestHash=$requestHash, " +
            "envelopeHash=$envelopeHash, destinationBinding=$destinationBinding)"
}

/** Inputs used to package an EVM-family Groth16 proof for verifier-contract submission. */
class EvmSccpSubmissionInput(
    val publicInputs: EvmSccpPublicInputsInput,
    proofBytes: ByteArray,
    val statementHash: String,
    val destinationBindingHash: String,
    val sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    val proofResult: EvmSccpProofResult? = null,
    publicSignalWords: List<String>? = null,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicSignalWords: List<String>? = publicSignalWords?.toList()

    constructor(
        proofResult: EvmSccpProofResult,
        publicInputs: EvmSccpPublicInputsInput = proofResult.publicInputs,
        sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    ) : this(
        publicInputs = publicInputs,
        proofBytes = proofResult.proofBytes,
        statementHash = proofResult.statementHash,
        destinationBindingHash = proofResult.destinationBindingHash,
        sourceDomain = sourceDomain,
        proofResult = proofResult,
        publicSignalWords = proofResult.publicSignalWords,
    )

    constructor(
        publicInputs: EvmSccpPublicInputsInput,
        proofBytes: ByteArray,
        statementHash: String,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding,
        sourceDomain: Int = SccpEvm.DOMAIN_SORA,
        proofResult: EvmSccpProofResult? = null,
        publicSignalWords: List<String>? = null,
    ) : this(
        publicInputs = publicInputs,
        proofBytes = proofBytes,
        statementHash = statementHash,
        destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = publicInputs,
            destinationBinding = destinationBinding,
            backend = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
            sourceDomain = sourceDomain,
        ),
        sourceDomain = sourceDomain,
        proofResult = proofResult,
        publicSignalWords = publicSignalWords,
    )
}

/** ABI argument metadata for an EVM-family SCCP verifier-contract submission. */
data class EvmSccpSubmissionArgument(
    val key: String,
    val encoding: String,
    val bytes: String,
)

/** EVM-family SCCP verifier-contract call data ready for wallet or relayer submission. */
class EvmSccpSubmission(
    val version: Int,
    val proofFamily: String,
    val verifierBackend: String,
    val platformPayload: String,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    val contractMethod: String,
    val functionSelector: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: EvmSccpPublicInputsInput,
    publicInputWords: List<String>,
    publicSignalWords: List<String>,
    val statementHash: String,
    val destinationBindingHash: String,
    arguments: List<EvmSccpSubmissionArgument>,
    val callDataHex: String,
    val envelopeHex: String,
    proofBytes: ByteArray,
    publicInputWordsBytes: ByteArray,
    callData: ByteArray,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputWordsBytesStorage: ByteArray = publicInputWordsBytes.copyOf()
    private val callDataStorage: ByteArray = callData.copyOf()

    val publicInputWords: List<String> = publicInputWords.toList()
    val publicSignalWords: List<String> = publicSignalWords.toList()
    val arguments: List<EvmSccpSubmissionArgument> = arguments.toList()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputWordsBytes: ByteArray
        get() = publicInputWordsBytesStorage.copyOf()

    val callData: ByteArray
        get() = callDataStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = callDataStorage.copyOf()
}

/** Optional witness resolver backed by app-controlled EVM RPC calls. */
fun interface EvmSccpWitnessProvider {
    fun resolveWitness(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput
}

/** Local EVM-family Groth16 proof engine linked by the application bundle. */
fun interface EvmSccpProofEngine {
    fun prove(request: EvmSccpProofRequest): ByteArray
}

/** App-supplied Ethereum JSON-RPC execution provider for native SCCP evidence collection. */
fun interface EthereumMainnetExecutionProvider {
    fun request(method: String, params: List<Any?>): Any?
}

/** App-supplied Ethereum Beacon REST finality collector for native SCCP evidence collection. */
fun interface EthereumMainnetConsensusProvider {
    fun collectFinalityEvidence(
        receipt: Map<String, Any?>?,
        block: Map<String, Any?>?,
        transactionHash: String?,
    ): Map<String, Any?>
}

/** Typed Ethereum beacon finality evidence required before inbound source proving. */
data class EthereumMainnetBeaconFinalityEvidence(
    val executionBlockNumber: String,
    val executionBlockHash: String,
    val executionReceiptsRoot: String,
    val additionalFields: Map<String, Any?> = emptyMap(),
) {
    fun toMap(): Map<String, Any?> =
        additionalFields + mapOf(
            "executionBlockNumber" to executionBlockNumber,
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
        )
}

/** Ethereum mainnet receipt-proof transcript collected from app-supplied providers. */
data class EthereumMainnetReceiptProof(
    val sourceEventDigest: String,
    val beaconSlot: String,
    val executionBlockNumber: String,
    val executionBlockHash: String,
    val executionReceiptsRoot: String,
    val beaconFinalizedRoot: String,
    val syncCommitteeRoot: String,
    val receiptRootIndex: String,
    val receiptTrieProofNodes: List<ByteArray>,
    val inclusionBranch: List<ByteArray>,
    val sourceDomain: Int = SccpEvm.DOMAIN_ETH,
) {
    fun snapshot(): EthereumMainnetReceiptProof =
        copy(
            receiptTrieProofNodes = receiptTrieProofNodes.map { it.copyOf() },
            inclusionBranch = inclusionBranch.map { it.copyOf() },
        )
}

/** Local Ethereum mainnet inbound source prover linked by the application bundle. */
fun interface EthereumMainnetInboundProver {
    fun prove(evidence: EthereumMainnetInboundEvidence): ByteArray
}

/** App-supplied Torii submitter for locally generated Ethereum inbound proofs. */
fun interface EthereumMainnetInboundSubmitter {
    fun submit(proofBytes: ByteArray): Any?
}

/** App-supplied Ethereum transaction submitter for locally generated outbound proof calldata. */
fun interface EthereumMainnetOutboundSubmitter {
    fun submit(submission: EvmSccpSubmission): Any?
}

/** Locally collected Ethereum mainnet inbound evidence before source-proof generation. */
data class EthereumMainnetInboundEvidence(
    val sourceDomain: Int = SccpEvm.DOMAIN_ETH,
    val targetDomain: Int = SccpEvm.DOMAIN_SORA,
    val transactionHash: String? = null,
    val receipt: Map<String, Any?>? = null,
    val block: Map<String, Any?>? = null,
    val beaconFinality: Map<String, Any?>? = null,
    val receiptProof: EthereumMainnetReceiptProof? = null,
    val receiptProofHash: String? = null,
    val sourceEventDigest: String? = null,
    val sourceBridgeEmitterAddress: String? = null,
) {
    companion object {
        fun withBeaconFinalityEvidence(
            sourceDomain: Int = SccpEvm.DOMAIN_ETH,
            targetDomain: Int = SccpEvm.DOMAIN_SORA,
            transactionHash: String? = null,
            receipt: Map<String, Any?>? = null,
            block: Map<String, Any?>? = null,
            beaconFinalityEvidence: EthereumMainnetBeaconFinalityEvidence? = null,
            receiptProof: EthereumMainnetReceiptProof? = null,
            receiptProofHash: String? = null,
            sourceEventDigest: String? = null,
            sourceBridgeEmitterAddress: String? = null,
        ): EthereumMainnetInboundEvidence =
            EthereumMainnetInboundEvidence(
                sourceDomain = sourceDomain,
                targetDomain = targetDomain,
                transactionHash = transactionHash,
                receipt = receipt,
                block = block,
                beaconFinality = beaconFinalityEvidence?.toMap(),
                receiptProof = receiptProof,
                receiptProofHash = receiptProofHash,
                sourceEventDigest = sourceEventDigest,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            )
    }
}

/** App-supplied BSC JSON-RPC execution provider for native SCCP evidence collection. */
fun interface BscMainnetExecutionProvider {
    fun request(method: String, params: List<Any?>): Any?
}

/** App-supplied BSC Parlia finality collector for native SCCP evidence collection. */
fun interface BscMainnetConsensusProvider {
    fun collectFinalityEvidence(
        receipt: Map<String, Any?>?,
        block: Map<String, Any?>?,
        transactionHash: String?,
    ): Map<String, Any?>
}

/** Typed BSC Parlia finality evidence required before inbound source proving. */
data class BscMainnetParliaFinalityEvidence(
    val executionBlockNumber: String,
    val executionBlockHash: String,
    val executionReceiptsRoot: String,
    val additionalFields: Map<String, Any?> = emptyMap(),
) {
    fun toMap(): Map<String, Any?> =
        additionalFields + mapOf(
            "executionBlockNumber" to executionBlockNumber,
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
        )
}

/** Local BSC mainnet inbound source prover linked by the application bundle. */
fun interface BscMainnetInboundProver {
    fun prove(evidence: BscMainnetInboundEvidence): ByteArray
}

/** App-supplied Torii submitter for locally generated BSC inbound proofs. */
fun interface BscMainnetInboundSubmitter {
    fun submit(proofBytes: ByteArray): Any?
}

/** App-supplied BSC transaction submitter for locally generated outbound proof calldata. */
fun interface BscMainnetOutboundSubmitter {
    fun submit(submission: EvmSccpSubmission): Any?
}

/** Locally collected BSC mainnet inbound evidence before source-proof generation. */
data class BscMainnetInboundEvidence(
    val sourceDomain: Int = SccpEvm.DOMAIN_BSC,
    val targetDomain: Int = SccpEvm.DOMAIN_SORA,
    val transactionHash: String? = null,
    val receipt: Map<String, Any?>? = null,
    val block: Map<String, Any?>? = null,
    val parliaFinality: Map<String, Any?>? = null,
    val receiptProofHash: String? = null,
) {
    companion object {
        fun withParliaFinalityEvidence(
            sourceDomain: Int = SccpEvm.DOMAIN_BSC,
            targetDomain: Int = SccpEvm.DOMAIN_SORA,
            transactionHash: String? = null,
            receipt: Map<String, Any?>? = null,
            block: Map<String, Any?>? = null,
            parliaFinalityEvidence: BscMainnetParliaFinalityEvidence? = null,
            receiptProofHash: String? = null,
        ): BscMainnetInboundEvidence =
            BscMainnetInboundEvidence(
                sourceDomain = sourceDomain,
                targetDomain = targetDomain,
                transactionHash = transactionHash,
                receipt = receipt,
                block = block,
                parliaFinality = parliaFinalityEvidence?.toMap(),
                receiptProofHash = receiptProofHash,
            )
    }
}

/** Local-first EVM-family SCCP Groth16 proof wrapper for UI SDKs. */
class EvmSccpProver(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
) {
    fun buildRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest =
        SccpEvm.buildProofRequest(witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input)

    fun prove(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine ?: throw IllegalStateException("EVM-family SCCP Groth16 prover is not linked")
        SccpEvm.requireProductionProofRequest(request)
        return SccpEvm.wrapProofResult(engine.prove(SccpEvm.callbackRequestSnapshot(request)), request)
    }

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )
}

/** Local-first Ethereum mainnet SCCP Groth16 proof wrapper for UI SDKs. */
class EthereumMainnetSccp(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
    private val executionProvider: EthereumMainnetExecutionProvider? = null,
    private val consensusProvider: EthereumMainnetConsensusProvider? = null,
    private val inboundProver: EthereumMainnetInboundProver? = null,
    private val inboundSubmitter: EthereumMainnetInboundSubmitter? = null,
    private val outboundSubmitter: EthereumMainnetOutboundSubmitter? = null,
) {
    fun validateExecutionProviderMainnet(provider: EthereumMainnetExecutionProvider? = executionProvider): Any? {
        val selectedProvider = provider
            ?: throw IllegalStateException("Ethereum mainnet execution provider is not linked")
        val chainId = selectedProvider.request("eth_chainId", emptyList())
        SccpEthereumMainnet.requireMainnetChainId(normalizeRpcChainId(chainId))
        return chainId
    }

    fun collectInboundEvidenceFromReceipt(
        input: EthereumMainnetInboundEvidence,
        provider: EthereumMainnetExecutionProvider? = executionProvider,
        consensusProvider: EthereumMainnetConsensusProvider? = this.consensusProvider,
    ): EthereumMainnetInboundEvidence {
        require(input.sourceDomain == SccpEvm.DOMAIN_ETH) {
            "Ethereum mainnet inbound evidence sourceDomain must be ETH"
        }
        require(input.targetDomain == SccpEvm.DOMAIN_SORA) {
            "Ethereum mainnet inbound evidence targetDomain must be SORA"
        }
        provider?.let { validateExecutionProviderMainnet(it) }

        var transactionHash = input.transactionHash?.let {
            normalizeRpcHex(it, "transactionHash", 32)
        }
        var receipt = input.receipt
        if (receipt == null && transactionHash != null) {
            val selectedProvider = provider
                ?: throw IllegalStateException(
                    "Ethereum mainnet execution provider is not linked for transactionHash evidence collection",
                )
            receipt = requireMap(
                selectedProvider.request("eth_getTransactionReceipt", listOf(transactionHash)),
                "eth_getTransactionReceipt",
            )
        }
        val receiptProof = input.receiptProof?.snapshot()
        if (receipt == null && receiptProof == null && input.receiptProofHash == null) {
            throw IllegalArgumentException(
                "Ethereum mainnet inbound evidence requires receipt, receiptProof, receiptProofHash, or transactionHash",
            )
        }

        var blockHash: String? = null
        var receiptBlockNumber: String? = null
        var blockReceiptsRoot: String? = null
        if (receipt != null) {
            require(receipt["status"] == "0x1") {
                "Ethereum mainnet inbound receipt status must be 0x1"
            }
            val receiptTransactionHash = normalizeRpcHex(
                receipt["transactionHash"] ?: receipt["transaction_hash"],
                "receipt.transactionHash",
                32,
            )
            if (transactionHash != null) {
                require(receiptTransactionHash == transactionHash) {
                    "receipt.transactionHash must match transactionHash"
                }
            }
            transactionHash = receiptTransactionHash
            blockHash = normalizeRpcHex(receipt["blockHash"] ?: receipt["block_hash"], "receipt.blockHash", 32)
            receiptBlockNumber = normalizePositiveRpcQuantity(
                receipt["blockNumber"] ?: receipt["block_number"],
                "receipt.blockNumber",
            )
        }

        var block = input.block
        if (block == null && blockHash != null && provider != null) {
            block = requireMap(
                provider.request("eth_getBlockByHash", listOf(blockHash, false)),
                "eth_getBlockByHash",
            )
        }
        if (block != null) {
            val normalizedBlockHash = normalizeRpcHex(block["hash"], "block.hash", 32)
            if (blockHash != null) {
                require(normalizedBlockHash == blockHash) {
                    "block.hash must match receipt.blockHash"
                }
            }
            val blockNumber = normalizePositiveRpcQuantity(
                block["number"] ?: block["blockNumber"] ?: block["block_number"],
                "block.number",
            )
            if (receiptBlockNumber != null) {
                require(blockNumber == receiptBlockNumber) {
                    "block.number must match receipt.blockNumber"
                }
            }
            receiptBlockNumber = blockNumber
            blockReceiptsRoot = normalizeRpcHex(block["receiptsRoot"] ?: block["receipts_root"], "block.receiptsRoot", 32)
        }

        val sourceEvent = ethereumReceiptSourceEvent(
            receipt = receipt,
            sourceEventDigestInput = input.sourceEventDigest,
            sourceBridgeEmitterAddressInput = input.sourceBridgeEmitterAddress,
            transactionHash = transactionHash,
            blockHash = blockHash,
            blockNumber = receiptBlockNumber,
        )
        val beaconFinality = input.beaconFinality
            ?: consensusProvider?.collectFinalityEvidence(receipt, block, transactionHash)
        val normalizedBeaconFinality = beaconFinality?.let {
            normalizeBeaconFinality(
                it,
                expectedBlockHash = blockHash,
                expectedBlockNumber = receiptBlockNumber,
                expectedReceiptsRoot = blockReceiptsRoot,
            )
        }
        requireReceiptProofMatchesEvidence(
            receiptProof = receiptProof,
            blockHash = blockHash,
            receiptBlockNumber = receiptBlockNumber,
            blockReceiptsRoot = blockReceiptsRoot,
            beaconFinality = normalizedBeaconFinality,
            sourceEventDigest = sourceEvent.first,
        )

        return input.copy(
            sourceDomain = SccpEvm.DOMAIN_ETH,
            targetDomain = SccpEvm.DOMAIN_SORA,
            transactionHash = transactionHash,
            receipt = receipt,
            block = block,
            beaconFinality = normalizedBeaconFinality,
            receiptProof = receiptProof,
            receiptProofHash = normalizeReceiptProofHash(receiptProof, input.receiptProofHash),
            sourceEventDigest = sourceEvent.first,
            sourceBridgeEmitterAddress = sourceEvent.second,
        )
    }

    fun proveInboundToSora(
        input: EthereumMainnetInboundEvidence,
        provider: EthereumMainnetExecutionProvider? = executionProvider,
        consensusProvider: EthereumMainnetConsensusProvider? = this.consensusProvider,
    ): ByteArray {
        val prover = inboundProver
            ?: throw IllegalStateException("Ethereum mainnet SCCP inbound prover is not linked")
        val evidence = collectInboundEvidenceFromReceipt(input, provider, consensusProvider)
        require(evidence.beaconFinality != null) {
            "Ethereum mainnet SCCP inbound proof requires beaconFinality"
        }
        require(evidence.receiptProof != null) {
            "Ethereum mainnet SCCP inbound proof requires receiptProof"
        }
        require(evidence.receipt == null || evidence.sourceEventDigest != null) {
            "Ethereum mainnet SCCP inbound proof requires receipt source event validation"
        }
        val proofBytes = prover.prove(evidence)
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        return proofBytes.copyOf()
    }

    fun submitInboundToIroha(proofBytes: ByteArray): Any? {
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        val submitter = inboundSubmitter
            ?: throw IllegalStateException("Ethereum mainnet SCCP inbound submitter is not linked")
        return submitter.submit(proofBytes.copyOf())
    }

    fun buildLocalAdmissionSubmission(
        input: EthereumMainnetLocalAdmissionSubmissionInput,
    ): EthereumMainnetLocalAdmissionSubmission =
        SccpEthereumMainnet.buildLocalAdmissionSubmission(input)

    fun buildOutboundProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest =
        SccpEthereumMainnet.buildProofRequest(
            witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input,
        )

    fun proveOutboundToEthereum(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildOutboundProofRequest(input)
        val engine = proofEngine
            ?: throw IllegalStateException("Ethereum mainnet SCCP Groth16 prover is not linked")
        return SccpEthereumMainnet.wrapProofResult(
            engine.prove(SccpEvm.callbackRequestSnapshot(request)),
            request,
        )
    }

    fun buildEthereumCalldata(input: EvmSccpSubmissionInput): EvmSccpSubmission =
        SccpEthereumMainnet.buildSubmission(input)

    fun submitOutboundToEthereum(input: EvmSccpSubmissionInput): Any? {
        val submitter = outboundSubmitter
            ?: throw IllegalStateException("Ethereum mainnet SCCP outbound submitter is not linked")
        return submitter.submit(buildEthereumCalldata(input))
    }

    fun requireExecutionMainnet(chainId: Long) =
        SccpEthereumMainnet.requireMainnetChainId(chainId)

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )

    private fun normalizeRpcChainId(value: Any?): Long {
        val quantity = normalizeRpcQuantity(value, "eth_chainId")
        val parsed = BigInteger(quantity.substring(2), 16)
        require(parsed.bitLength() <= 63) { "eth_chainId must fit positive i64" }
        return parsed.toLong()
    }

    private fun normalizeUnsignedInteger(value: Any?, label: String): Long =
        when (value) {
            is Long -> {
                require(value >= 0) { "$label must be non-negative" }
                value
            }
            is Int -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Short -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Byte -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is BigInteger -> {
                require(value.signum() >= 0 && value.bitLength() <= 63) {
                    "$label must fit positive i64"
                }
                value.toLong()
            }
            is String -> {
                require(value.trim() == value) { "$label must be canonical" }
                val parsed = if (value.startsWith("0x")) {
                    val text = value.substring(2)
                    require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
                        "$label must be a canonical JSON-RPC quantity"
                    }
                    BigInteger(text, 16)
                } else {
                    require(value.matches(Regex("0|[1-9][0-9]*"))) {
                        "$label must be a canonical decimal integer"
                    }
                    BigInteger(value, 10)
                }
                require(parsed.bitLength() <= 63) { "$label must fit positive i64" }
                parsed.toLong()
            }
            else -> throw IllegalArgumentException("$label must be a JSON-RPC quantity or integer")
        }

    private fun requireMap(value: Any?, label: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return value as? Map<String, Any?>
            ?: throw IllegalArgumentException("$label must return an object")
    }

    private fun normalizeRpcHex(value: Any?, label: String, byteLength: Int, allowZero: Boolean = false): String {
        require(value is String) { "$label must be canonical lowercase 0x hex" }
        require(value.trim() == value && value.startsWith("0x")) {
            "$label must be canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.length == byteLength * 2 && text.matches(Regex("[0-9a-f]+"))) {
            "$label must be $byteLength bytes canonical lowercase 0x hex"
        }
        require(allowZero || text.any { it != '0' }) { "$label must not be zero" }
        return value
    }

    private fun normalizeReceiptProofHash(
        receiptProof: EthereumMainnetReceiptProof?,
        suppliedHash: String?,
    ): String? {
        var normalizedHash = suppliedHash?.let { normalizeRpcHex(it, "receiptProofHash", 32) }
        if (receiptProof == null) {
            return normalizedHash
        }
        require(receiptProof.sourceDomain == SccpEvm.DOMAIN_ETH) {
            "receiptProof.sourceDomain must be ETH"
        }
        val computedHash = SccpSourceProofs.evmReceiptProofHash(
            sourceEventDigest = receiptProof.sourceEventDigest,
            beaconSlot = receiptProof.beaconSlot,
            executionBlockNumber = receiptProof.executionBlockNumber,
            executionBlockHash = receiptProof.executionBlockHash,
            executionReceiptsRoot = receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot = receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot = receiptProof.syncCommitteeRoot,
            receiptRootIndex = receiptProof.receiptRootIndex,
            receiptTrieProofNodes = receiptProof.receiptTrieProofNodes,
            inclusionBranch = receiptProof.inclusionBranch,
            sourceDomain = receiptProof.sourceDomain,
        )
        if (normalizedHash != null && normalizedHash != computedHash) {
            throw IllegalArgumentException("receiptProofHash must match receiptProof")
        }
        normalizedHash = computedHash
        return normalizedHash
    }

    private fun requireReceiptProofMatchesEvidence(
        receiptProof: EthereumMainnetReceiptProof?,
        blockHash: String?,
        receiptBlockNumber: String?,
        blockReceiptsRoot: String?,
        beaconFinality: Map<String, Any?>?,
        sourceEventDigest: String?,
    ) {
        if (receiptProof == null) {
            return
        }
        val proofBlockNumber = normalizeUnsignedInteger(
            receiptProof.executionBlockNumber,
            "receiptProof.executionBlockNumber",
        )
        if (receiptBlockNumber != null) {
            require(proofBlockNumber == normalizeUnsignedInteger(receiptBlockNumber, "block.number")) {
                "receiptProof.executionBlockNumber must match block.number"
            }
        }
        if (beaconFinality != null) {
            require(
                proofBlockNumber == normalizeUnsignedInteger(
                    beaconFinality["executionBlockNumber"],
                    "beaconFinality.executionBlockNumber",
                ),
            ) {
                "receiptProof.executionBlockNumber must match beaconFinality.executionBlockNumber"
            }
        }
        val proofBlockHash = normalizeRpcHex(
            receiptProof.executionBlockHash,
            "receiptProof.executionBlockHash",
            32,
        )
        if (blockHash != null) {
            require(proofBlockHash == blockHash) {
                "receiptProof.executionBlockHash must match block.hash"
            }
        }
        if (beaconFinality != null) {
            require(proofBlockHash == beaconFinality["executionBlockHash"]) {
                "receiptProof.executionBlockHash must match beaconFinality.executionBlockHash"
            }
        }
        val proofReceiptsRoot = normalizeRpcHex(
            receiptProof.executionReceiptsRoot,
            "receiptProof.executionReceiptsRoot",
            32,
        )
        if (blockReceiptsRoot != null) {
            require(proofReceiptsRoot == blockReceiptsRoot) {
                "receiptProof.executionReceiptsRoot must match block.receiptsRoot"
            }
        }
        if (beaconFinality != null) {
            require(proofReceiptsRoot == beaconFinality["executionReceiptsRoot"]) {
                "receiptProof.executionReceiptsRoot must match beaconFinality.executionReceiptsRoot"
            }
        }
        if (sourceEventDigest != null) {
            val proofSourceEventDigest = normalizeRpcHex(
                receiptProof.sourceEventDigest,
                "receiptProof.sourceEventDigest",
                32,
            )
            require(proofSourceEventDigest == sourceEventDigest) {
                "receiptProof.sourceEventDigest must match receipt source event"
            }
        }
    }

    private fun ethereumReceiptSourceEvent(
        receipt: Map<String, Any?>?,
        sourceEventDigestInput: String?,
        sourceBridgeEmitterAddressInput: String?,
        transactionHash: String?,
        blockHash: String?,
        blockNumber: String?,
    ): Pair<String?, String?> {
        val sourceEventDigest = sourceEventDigestInput?.let {
            normalizeRpcHex(it, "sourceEventDigest", 32)
        }
        val sourceBridgeEmitterAddress = sourceBridgeEmitterAddressInput?.let {
            normalizeRpcHex(it, "sourceBridgeEmitterAddress", 20)
        }
        if (sourceEventDigest == null && sourceBridgeEmitterAddress == null) {
            return Pair(null, null)
        }
        require(sourceBridgeEmitterAddress != null) {
            "sourceBridgeEmitterAddress is required when validating sourceEventDigest"
        }
        val logs = receipt?.get("logs") as? List<*>
            ?: throw IllegalArgumentException("receipt.logs is required for SCCP source event validation")
        var matchedDigest: String? = null
        for ((index, logInput) in logs.withIndex()) {
            val log = logInput as? Map<*, *>
                ?: throw IllegalArgumentException("receipt.logs[$index] must be an object")
            if (log["removed"] == true) {
                throw IllegalArgumentException("receipt.logs must not contain removed logs")
            }
            val logAddress = normalizeRpcHex(
                log["address"],
                "receipt.logs[$index].address",
                20,
                allowZero = true,
            )
            val topics = log["topics"] as? List<*>
                ?: throw IllegalArgumentException("receipt.logs[$index].topics must be an array")
            require(topics.size <= 4) {
                "receipt.logs[$index].topics must contain at most 4 entries"
            }
            val normalizedTopics = topics.mapIndexed { topicIndex, topic ->
                normalizeRpcHex(
                    topic,
                    "receipt.logs[$index].topics[$topicIndex]",
                    32,
                    allowZero = true,
                )
            }
            if (
                logAddress == sourceBridgeEmitterAddress &&
                normalizedTopics.size == 2 &&
                normalizedTopics[0] == SccpEthereumMainnet.SOURCE_EVENT_TOPIC_V1
            ) {
                val logTransactionHash = normalizeRpcHex(
                    log["transactionHash"] ?: log["transaction_hash"],
                    "receipt.logs[$index].transactionHash",
                    32,
                )
                require(transactionHash == null || logTransactionHash == transactionHash) {
                    "receipt.logs transactionHash must match receipt.transactionHash"
                }
                val logBlockHash = normalizeRpcHex(
                    log["blockHash"] ?: log["block_hash"],
                    "receipt.logs[$index].blockHash",
                    32,
                )
                require(blockHash == null || logBlockHash == blockHash) {
                    "receipt.logs blockHash must match receipt.blockHash"
                }
                val logBlockNumber = normalizePositiveRpcQuantity(
                    log["blockNumber"] ?: log["block_number"],
                    "receipt.logs[$index].blockNumber",
                )
                require(blockNumber == null || logBlockNumber == blockNumber) {
                    "receipt.logs blockNumber must match receipt.blockNumber"
                }
                val data = log["data"] as? String
                    ?: throw IllegalArgumentException("receipt.logs[$index].data is required")
                val candidateDigest = normalizedTopics[1]
                if (
                    candidateDigest == "0x" + "0".repeat(64) ||
                    (sourceEventDigest != null && candidateDigest != sourceEventDigest) ||
                    data != "0x"
                ) {
                    continue
                }
                require(matchedDigest == null) {
                    "receipt.logs must contain exactly one matching SCCP source event"
                }
                matchedDigest = candidateDigest
            }
        }
        return Pair(
            matchedDigest
                ?: throw IllegalArgumentException("receipt.logs must contain the expected SCCP source event"),
            sourceBridgeEmitterAddress,
        )
    }

    private fun normalizeRpcQuantity(value: Any?, label: String): String {
        require(value is String && value.trim() == value && value.startsWith("0x")) {
            "$label must be a canonical JSON-RPC quantity"
        }
        val text = value.substring(2)
        require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
            "$label must be a canonical JSON-RPC quantity"
        }
        return "0x" + BigInteger(text, 16).toString(16)
    }

    private fun normalizePositiveRpcQuantity(value: Any?, label: String): String {
        val quantity = normalizeRpcQuantity(value, label)
        require(quantity != "0x0") { "$label must be positive" }
        return quantity
    }

    private fun normalizeBeaconFinality(
        finality: Map<String, Any?>,
        expectedBlockHash: String?,
        expectedBlockNumber: String?,
        expectedReceiptsRoot: String?,
    ): Map<String, Any?> {
        val executionBlockNumber = normalizeUnsignedInteger(
            finality["executionBlockNumber"]
                ?: finality["execution_block_number"]
                ?: finality["finalityHeight"]
                ?: finality["finality_height"],
            "beaconFinality.executionBlockNumber",
        )
        require(executionBlockNumber > 0) { "beaconFinality.executionBlockNumber must be positive" }
        if (expectedBlockNumber != null) {
            require(executionBlockNumber == normalizeUnsignedInteger(expectedBlockNumber, "block.number")) {
                "beaconFinality.executionBlockNumber must match block.number"
            }
        }
        val executionBlockHash = normalizeRpcHex(
            finality["executionBlockHash"]
                ?: finality["execution_block_hash"]
                ?: finality["finalityBlockHash"]
                ?: finality["finality_block_hash"],
            "beaconFinality.executionBlockHash",
            32,
        )
        if (expectedBlockHash != null) {
            require(executionBlockHash == expectedBlockHash) {
                "beaconFinality.executionBlockHash must match block.hash"
            }
        }
        val executionReceiptsRoot = normalizeRpcHex(
            finality["executionReceiptsRoot"]
                ?: finality["execution_receipts_root"]
                ?: finality["receiptsRoot"]
                ?: finality["receipts_root"],
            "beaconFinality.executionReceiptsRoot",
            32,
        )
        if (expectedReceiptsRoot != null) {
            require(executionReceiptsRoot == expectedReceiptsRoot) {
                "beaconFinality.executionReceiptsRoot must match block.receiptsRoot"
            }
        }
        return finality + mapOf(
            "executionBlockNumber" to executionBlockNumber.toString(),
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
        )
    }
}

/** Local-first BSC mainnet SCCP facade for native UI SDKs. */
class BscMainnetSccp(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
    private val executionProvider: BscMainnetExecutionProvider? = null,
    private val consensusProvider: BscMainnetConsensusProvider? = null,
    private val inboundProver: BscMainnetInboundProver? = null,
    private val inboundSubmitter: BscMainnetInboundSubmitter? = null,
    private val outboundSubmitter: BscMainnetOutboundSubmitter? = null,
) {
    fun validateExecutionProviderMainnet(provider: BscMainnetExecutionProvider? = executionProvider): Any? {
        val selectedProvider = provider
            ?: throw IllegalStateException("BSC mainnet execution provider is not linked")
        val chainId = selectedProvider.request("eth_chainId", emptyList())
        SccpBsc.requireMainnetChainId(normalizeRpcChainId(chainId))
        return chainId
    }

    fun collectInboundEvidenceFromReceipt(
        input: BscMainnetInboundEvidence,
        provider: BscMainnetExecutionProvider? = executionProvider,
        consensusProvider: BscMainnetConsensusProvider? = this.consensusProvider,
    ): BscMainnetInboundEvidence {
        require(input.sourceDomain == SccpEvm.DOMAIN_BSC) {
            "BSC mainnet inbound evidence sourceDomain must be BSC"
        }
        require(input.targetDomain == SccpEvm.DOMAIN_SORA) {
            "BSC mainnet inbound evidence targetDomain must be SORA"
        }
        provider?.let { validateExecutionProviderMainnet(it) }

        var transactionHash = input.transactionHash?.let {
            normalizeRpcHex(it, "transactionHash", 32)
        }
        var receipt = input.receipt
        if (receipt == null && transactionHash != null && provider != null) {
            receipt = requireMap(
                provider.request("eth_getTransactionReceipt", listOf(transactionHash)),
                "eth_getTransactionReceipt",
            )
        }
        if (receipt == null && input.receiptProofHash == null) {
            throw IllegalArgumentException(
                "BSC mainnet inbound evidence requires receipt, receiptProofHash, or transactionHash",
            )
        }

        var blockHash: String? = null
        var receiptBlockNumber: String? = null
        var blockReceiptsRoot: String? = null
        if (receipt != null) {
            require(receipt["status"] == "0x1") {
                "BSC mainnet inbound receipt status must be 0x1"
            }
            val receiptTransactionHash = normalizeRpcHex(
                receipt["transactionHash"] ?: receipt["transaction_hash"],
                "receipt.transactionHash",
                32,
            )
            if (transactionHash != null) {
                require(receiptTransactionHash == transactionHash) {
                    "receipt.transactionHash must match transactionHash"
                }
            }
            transactionHash = receiptTransactionHash
            blockHash = normalizeRpcHex(receipt["blockHash"] ?: receipt["block_hash"], "receipt.blockHash", 32)
            receiptBlockNumber = normalizePositiveRpcQuantity(
                receipt["blockNumber"] ?: receipt["block_number"],
                "receipt.blockNumber",
            )
        }

        var block = input.block
        if (block == null && blockHash != null && provider != null) {
            block = requireMap(
                provider.request("eth_getBlockByHash", listOf(blockHash, false)),
                "eth_getBlockByHash",
            )
        }
        if (block != null) {
            val normalizedBlockHash = normalizeRpcHex(block["hash"], "block.hash", 32)
            if (blockHash != null) {
                require(normalizedBlockHash == blockHash) {
                    "block.hash must match receipt.blockHash"
                }
            }
            blockHash = normalizedBlockHash
            val blockNumber = normalizePositiveRpcQuantity(
                block["number"] ?: block["blockNumber"] ?: block["block_number"],
                "block.number",
            )
            if (receiptBlockNumber != null) {
                require(blockNumber == receiptBlockNumber) {
                    "block.number must match receipt.blockNumber"
                }
            }
            receiptBlockNumber = blockNumber
            blockReceiptsRoot = normalizeRpcHex(block["receiptsRoot"] ?: block["receipts_root"], "block.receiptsRoot", 32)
        }

        val parliaFinality = input.parliaFinality
            ?: consensusProvider?.collectFinalityEvidence(receipt, block, transactionHash)
        val normalizedParliaFinality = parliaFinality?.let {
            normalizeParliaFinality(
                it,
                expectedBlockHash = blockHash,
                expectedBlockNumber = receiptBlockNumber,
                expectedReceiptsRoot = blockReceiptsRoot,
            )
        }

        return input.copy(
            sourceDomain = SccpEvm.DOMAIN_BSC,
            targetDomain = SccpEvm.DOMAIN_SORA,
            transactionHash = transactionHash,
            receipt = receipt,
            block = block,
            parliaFinality = normalizedParliaFinality,
            receiptProofHash = input.receiptProofHash?.let {
                normalizeRpcHex(it, "receiptProofHash", 32)
            },
        )
    }

    fun proveInboundToSora(
        input: BscMainnetInboundEvidence,
        provider: BscMainnetExecutionProvider? = executionProvider,
        consensusProvider: BscMainnetConsensusProvider? = this.consensusProvider,
    ): ByteArray {
        val prover = inboundProver
            ?: throw IllegalStateException("BSC mainnet SCCP inbound prover is not linked")
        val evidence = collectInboundEvidenceFromReceipt(input, provider, consensusProvider)
        require(evidence.parliaFinality != null) {
            "BSC mainnet SCCP inbound proof requires parliaFinality"
        }
        val proofBytes = prover.prove(evidence)
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        return proofBytes.copyOf()
    }

    fun submitInboundToIroha(proofBytes: ByteArray): Any? {
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        val submitter = inboundSubmitter
            ?: throw IllegalStateException("BSC mainnet SCCP inbound submitter is not linked")
        return submitter.submit(proofBytes.copyOf())
    }

    fun buildOutboundProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest =
        SccpBsc.buildProofRequest(
            witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input,
        )

    fun proveOutboundToBsc(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildOutboundProofRequest(input)
        val engine = proofEngine
            ?: throw IllegalStateException("BSC mainnet SCCP Groth16 prover is not linked")
        return SccpBsc.wrapProofResult(
            engine.prove(SccpEvm.callbackRequestSnapshot(request)),
            request,
        )
    }

    fun buildBscCalldata(input: EvmSccpSubmissionInput): EvmSccpSubmission =
        SccpBsc.buildSubmission(input)

    fun buildLocalAdmissionSubmission(
        input: BscMainnetLocalAdmissionSubmissionInput,
    ): BscMainnetLocalAdmissionSubmission =
        SccpBsc.buildLocalAdmissionSubmission(input)

    fun submitOutboundToBsc(input: EvmSccpSubmissionInput): Any? {
        val submitter = outboundSubmitter
            ?: throw IllegalStateException("BSC mainnet SCCP outbound submitter is not linked")
        return submitter.submit(buildBscCalldata(input))
    }

    fun requireExecutionMainnet(chainId: Long) =
        SccpBsc.requireMainnetChainId(chainId)

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )

    private fun normalizeRpcChainId(value: Any?): Long {
        val quantity = normalizeRpcQuantity(value, "eth_chainId")
        val parsed = BigInteger(quantity.substring(2), 16)
        require(parsed.bitLength() <= 63) { "eth_chainId must fit positive i64" }
        return parsed.toLong()
    }

    private fun requireMap(value: Any?, label: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return value as? Map<String, Any?>
            ?: throw IllegalArgumentException("$label must return an object")
    }

    private fun normalizeRpcHex(value: Any?, label: String, byteLength: Int): String {
        require(value is String) { "$label must be canonical lowercase 0x hex" }
        require(value.trim() == value && value.startsWith("0x")) {
            "$label must be canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.length == byteLength * 2 && text.matches(Regex("[0-9a-f]+"))) {
            "$label must be $byteLength bytes canonical lowercase 0x hex"
        }
        require(text.any { it != '0' }) { "$label must not be zero" }
        return value
    }

    private fun normalizeRpcQuantity(value: Any?, label: String): String {
        require(value is String && value.trim() == value && value.startsWith("0x")) {
            "$label must be a canonical JSON-RPC quantity"
        }
        val text = value.substring(2)
        require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
            "$label must be a canonical JSON-RPC quantity"
        }
        return "0x" + BigInteger(text, 16).toString(16)
    }

    private fun normalizePositiveRpcQuantity(value: Any?, label: String): String {
        val quantity = normalizeRpcQuantity(value, label)
        require(quantity != "0x0") { "$label must be positive" }
        return quantity
    }

    private fun normalizeUnsignedInteger(value: Any?, label: String): Long =
        when (value) {
            is Long -> {
                require(value >= 0) { "$label must be non-negative" }
                value
            }
            is Int -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Short -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Byte -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is BigInteger -> {
                require(value.signum() >= 0 && value.bitLength() <= 63) {
                    "$label must fit positive i64"
                }
                value.toLong()
            }
            is String -> {
                require(value.trim() == value) { "$label must be canonical" }
                val parsed = if (value.startsWith("0x")) {
                    val text = value.substring(2)
                    require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
                        "$label must be a canonical JSON-RPC quantity"
                    }
                    BigInteger(text, 16)
                } else {
                    require(value.matches(Regex("0|[1-9][0-9]*"))) {
                        "$label must be a canonical decimal integer"
                    }
                    BigInteger(value, 10)
                }
                require(parsed.bitLength() <= 63) { "$label must fit positive i64" }
                parsed.toLong()
            }
            else -> throw IllegalArgumentException("$label must be a JSON-RPC quantity or integer")
        }

    private fun normalizeParliaFinality(
        finality: Map<String, Any?>,
        expectedBlockHash: String?,
        expectedBlockNumber: String?,
        expectedReceiptsRoot: String?,
    ): Map<String, Any?> {
        val executionBlockNumber = normalizeUnsignedInteger(
            finality["executionBlockNumber"]
                ?: finality["execution_block_number"]
                ?: finality["finalityHeight"]
                ?: finality["finality_height"],
            "parliaFinality.executionBlockNumber",
        )
        require(executionBlockNumber > 0) { "parliaFinality.executionBlockNumber must be positive" }
        if (expectedBlockNumber != null) {
            require(executionBlockNumber == normalizeUnsignedInteger(expectedBlockNumber, "block.number")) {
                "parliaFinality.executionBlockNumber must match block.number"
            }
        }
        val executionBlockHash = normalizeRpcHex(
            finality["executionBlockHash"]
                ?: finality["execution_block_hash"]
                ?: finality["finalityBlockHash"]
                ?: finality["finality_block_hash"],
            "parliaFinality.executionBlockHash",
            32,
        )
        if (expectedBlockHash != null) {
            require(executionBlockHash == expectedBlockHash) {
                "parliaFinality.executionBlockHash must match block.hash"
            }
        }
        val executionReceiptsRoot = normalizeRpcHex(
            finality["executionReceiptsRoot"]
                ?: finality["execution_receipts_root"]
                ?: finality["receiptsRoot"]
                ?: finality["receipts_root"],
            "parliaFinality.executionReceiptsRoot",
            32,
        )
        if (expectedReceiptsRoot != null) {
            require(executionReceiptsRoot == expectedReceiptsRoot) {
                "parliaFinality.executionReceiptsRoot must match block.receiptsRoot"
            }
        }
        return finality + mapOf(
            "executionBlockNumber" to executionBlockNumber.toString(),
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
        )
    }
}

/** Local-first BSC mainnet SCCP Groth16 proof wrapper for UI SDKs. */
class BscSccpProver(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
) {
    fun buildRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest =
        SccpBsc.buildProofRequest(witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input)

    fun prove(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine ?: throw IllegalStateException("BSC mainnet SCCP Groth16 prover is not linked")
        return SccpBsc.wrapProofResult(engine.prove(SccpEvm.callbackRequestSnapshot(request)), request)
    }

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )
}
