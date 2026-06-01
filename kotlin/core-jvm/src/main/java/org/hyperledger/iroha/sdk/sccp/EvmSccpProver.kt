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
