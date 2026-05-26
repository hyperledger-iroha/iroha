package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest

/**
 * Pure-Kotlin port of the deterministic STARK/FRI envelope prover from
 * `iroha_core::zk_stark` (`prove_stark_fri_air_envelope_bytes` +
 * `prove_stark_fri_composition_envelope_bytes`). Restricted to the
 * `stark/fri/sha256-goldilocks` backend used by the offline-cash routes.
 *
 * Correctness is locked by byte-parity fixture tests generated from the
 * canonical Rust helpers in `crates/iroha_torii/src/offline_lineage.rs`.
 */
internal object OfflineStarkEnvelopeProver {

    private const val STARK_HASH_SHA256_V1: Int = 1
    private val MOD_P: BigInteger = BigInteger.ONE.shiftLeft(64)
        .subtract(BigInteger.ONE.shiftLeft(32))
        .add(BigInteger.ONE) // 2^64 - 2^32 + 1
    private val GOLDILOCKS_GENERATOR: BigInteger = BigInteger.valueOf(7)
    private const val OFFLINE_STARK_DOMAIN_LOG2: Int = 4
    private const val OFFLINE_STARK_BLOWUP_LOG2: Int = 3
    private const val OFFLINE_STARK_QUERY_COUNT: Int = 8
    private val OFFLINE_STARK_BINDING_CONSTANT: BigInteger = BigInteger.valueOf(23)
    private val OFFLINE_STARK_BINDING_Z_COEFF: BigInteger = BigInteger.valueOf(29)
    private const val STARK_AIR_TRACE_WIDTH: Int = 6

    fun buildEnvelope(domainTag: String, transcriptLabel: String): OfflineStarkVerifyEnvelope {
        val params = OfflineStarkFriParams(
            version = 1,
            nLog2 = OFFLINE_STARK_DOMAIN_LOG2,
            blowupLog2 = OFFLINE_STARK_BLOWUP_LOG2,
            foldArity = 2,
            queries = OFFLINE_STARK_QUERY_COUNT,
            merkleArity = 2,
            hashFn = STARK_HASH_SHA256_V1,
            domainTag = domainTag,
        )
        val terms = offlineStarkBindingTerms(domainTag, transcriptLabel)
        return proveCompositionEnvelope(
            params,
            transcriptLabel,
            OFFLINE_STARK_BINDING_CONSTANT,
            OFFLINE_STARK_BINDING_Z_COEFF,
            terms,
        )
    }

    private fun offlineStarkBindingTerms(
        domainTag: String,
        transcriptLabel: String,
    ): List<OfflineStarkCompositionTerm> {
        val dtBytes = domainTag.toByteArray(StandardCharsets.UTF_8)
        val tlBytes = transcriptLabel.toByteArray(StandardCharsets.UTF_8)
        val preamble = "iroha:offline:stark-binding-air".toByteArray(StandardCharsets.US_ASCII)
        val preimage = ByteArray(preamble.size + 8 + dtBytes.size + 8 + tlBytes.size)
        var p = 0
        System.arraycopy(preamble, 0, preimage, p, preamble.size); p += preamble.size
        writeLeU64(preimage, p, dtBytes.size.toLong()); p += 8
        System.arraycopy(dtBytes, 0, preimage, p, dtBytes.size); p += dtBytes.size
        writeLeU64(preimage, p, tlBytes.size.toLong()); p += 8
        System.arraycopy(tlBytes, 0, preimage, p, tlBytes.size)
        val digest = sha256(preimage)
        val terms = ArrayList<OfflineStarkCompositionTerm>(4)
        for (i in 0..3) {
            val word = readLeU64(digest, i * 8)
            val value = word.mod(MOD_P)
            terms.add(OfflineStarkCompositionTerm(i.toLong(), value, BigInteger.valueOf(31L + i.toLong())))
        }
        return terms
    }

    private fun proveCompositionEnvelope(
        params: OfflineStarkFriParams,
        transcriptLabel: String,
        constant: BigInteger,
        zCoeff: BigInteger,
        auxTerms: List<OfflineStarkCompositionTerm>,
    ): OfflineStarkVerifyEnvelope {
        validateCompositionTerms(constant, zCoeff, auxTerms)
        val publicDigest = starkAirPublicDigestFromComposition(constant, zCoeff, auxTerms)
        val envelope = proveAirEnvelope(params, transcriptLabel, "composition-v1", publicDigest)

        val firstChain = envelope.proof.queries.first()
        val zFinal = firstChain.last().z
        var expected = fqAdd(constant, fqMul(zCoeff, zFinal))
        for (term in auxTerms) {
            expected = fqAdd(expected, fqMul(term.coeff, term.value))
        }
        val compLevels = merkleLevelsFromValues(params, listOf(expected))
            ?: error("failed to build STARK composition commitment")
        val compRoot = merkleRootFromLevels(compLevels)
            ?: error("failed to derive STARK composition root")
        val compPath = merklePathFromLevels(0, compLevels)
            ?: error("failed to derive STARK composition path")
        val compValue = OfflineStarkCompositionValue(
            leaf = expected,
            constant = constant,
            zCoeff = zCoeff,
            auxTerms = auxTerms,
            path = compPath,
        )
        val compValues = List(envelope.proof.queries.size) { compValue }

        val newCommits = OfflineStarkCommitments(
            version = envelope.proof.commits.version,
            roots = envelope.proof.commits.roots,
            compRoot = compRoot,
        )
        val newProof = OfflineStarkProof(
            version = envelope.proof.version,
            commits = newCommits,
            queries = envelope.proof.queries,
            compValues = compValues,
            air = envelope.proof.air,
        )
        return OfflineStarkVerifyEnvelope(envelope.params, newProof, envelope.transcriptLabel)
    }

    private fun proveAirEnvelope(
        params: OfflineStarkFriParams,
        transcriptLabel: String,
        circuitId: String,
        publicDigest: ByteArray,
    ): OfflineStarkVerifyEnvelope {
        require(circuitId.isNotEmpty()) { "invalid STARK AIR circuit_id" }
        val domain = 1 shl params.nLog2
        val rows = List(domain) { starkAirRow(it, publicDigest) }
        val traceLeaves = rows.map { starkAirTraceLeafHash(params, it) }
        val traceLevels = merkleLevelsFromHashes(params, traceLeaves)
            ?: error("failed to build STARK AIR trace commitment")
        val traceRoot = merkleRootFromLevels(traceLevels)
            ?: error("failed to derive STARK AIR trace root")

        val compositionValues = List(domain) { idx ->
            starkAirCompositionValue(idx, domain, publicDigest, rows[idx], rows[(idx + 1) % domain])
        }
        val compositionLevels = merkleLevelsFromValues(params, compositionValues)
            ?: error("failed to build STARK AIR composition commitment")
        val compositionRoot = merkleRootFromLevels(compositionLevels)
            ?: error("failed to derive STARK AIR composition root")

        val extraQueryRoots = listOf(traceRoot, compositionRoot, publicDigest)
        val envelope = synthesizeFriEnvelopeFromValues(
            params,
            transcriptLabel,
            compositionValues,
            extraQueryRoots,
        )
        require(envelope.proof.commits.roots.firstOrNull()?.contentEquals(compositionRoot) == true) {
            "STARK AIR composition root does not match FRI base root"
        }

        val queryRoots = envelope.proof.commits.roots + extraQueryRoots
        val openings = ArrayList<OfflineStarkAirOpening>(envelope.proof.queries.size)
        for (qi in envelope.proof.queries.indices) {
            val index = deriveQueryIndex(envelope.transcriptLabel, envelope.params, queryRoots, qi)
                ?: error("failed to derive STARK AIR query index")
            val nextIndex = (index + 1) % domain
            val rowPath = merklePathFromLevels(index, traceLevels)
                ?: error("failed to open STARK AIR row")
            val nextRowPath = merklePathFromLevels(nextIndex, traceLevels)
                ?: error("failed to open next STARK AIR row")
            val compositionPath = merklePathFromLevels(index, compositionLevels)
                ?: error("failed to open STARK AIR composition")
            val compositionValue = starkAirCompositionValue(
                index, domain, publicDigest, rows[index], rows[nextIndex],
            )
            openings.add(
                OfflineStarkAirOpening(
                    index = index.toLong(),
                    row = rows[index],
                    nextRow = rows[nextIndex],
                    rowPath = rowPath,
                    nextRowPath = nextRowPath,
                    compositionValue = compositionValue,
                    compositionPath = compositionPath,
                ),
            )
        }

        val airProof = OfflineStarkAirProof(
            version = 1,
            circuitId = circuitId,
            publicDigest = publicDigest,
            traceRoot = traceRoot,
            compositionRoot = compositionRoot,
            traceWidth = STARK_AIR_TRACE_WIDTH,
            openings = openings,
        )
        val newProof = OfflineStarkProof(
            version = envelope.proof.version,
            commits = envelope.proof.commits,
            queries = envelope.proof.queries,
            compValues = envelope.proof.compValues,
            air = airProof,
        )
        return OfflineStarkVerifyEnvelope(envelope.params, newProof, envelope.transcriptLabel)
    }

    private fun synthesizeFriEnvelopeFromValues(
        params: OfflineStarkFriParams,
        transcriptLabel: String,
        baseValues: List<BigInteger>,
        extraQueryRoots: List<ByteArray>,
    ): OfflineStarkVerifyEnvelope {
        val requiredLayers = layersRequired(params)
            ?: error("invalid STARK folding parameters")
        val totalDomain = 1 shl params.nLog2
        require(baseValues.size == totalDomain) {
            "STARK base evaluations do not match domain size"
        }
        val foldArity = params.foldArity

        val layerValues = ArrayList<List<BigInteger>>(requiredLayers + 1)
        val layerMerkle = ArrayList<List<List<ByteArray>>>(requiredLayers + 1)
        val roots = ArrayList<ByteArray>(requiredLayers + 1)
        layerValues.add(baseValues)

        for (round in 0 until requiredLayers) {
            val current = layerValues[round]
            val levels = merkleLevelsFromValues(params, current)
                ?: error("failed to build STARK FRI Merkle layer")
            val root = merkleRootFromLevels(levels)
                ?: error("failed to derive STARK FRI root")
            val beta = friRoundChallenge(params, transcriptLabel, root)
                ?: error("failed to derive STARK FRI challenge")
            roots.add(root)
            layerMerkle.add(levels)
            val next = ArrayList<BigInteger>(current.size / foldArity)
            var pairIndex = 0
            var cursor = 0
            while (cursor + foldArity <= current.size) {
                val x = domainXForPair(current.size, pairIndex)
                    ?: error("failed to derive STARK FRI domain element")
                val folded = friFoldPair(current[cursor], current[cursor + 1], beta, x)
                next.add(folded)
                cursor += foldArity
                pairIndex++
            }
            layerValues.add(next)
        }
        val finalValues = layerValues.last()
        val finalLevels = merkleLevelsFromValues(params, finalValues)
            ?: error("failed to build STARK final FRI Merkle layer")
        val finalRoot = merkleRootFromLevels(finalLevels)
            ?: error("failed to derive STARK final FRI root")
        roots.add(finalRoot)
        layerMerkle.add(finalLevels)

        val queryRoots = roots + extraQueryRoots
        val queryCount = params.queries
        val queries = ArrayList<List<OfflineFoldDecommit>>(queryCount)
        for (qi in 0 until queryCount) {
            var idxLayer = deriveQueryIndex(transcriptLabel, params, queryRoots, qi)
                ?: error("failed to derive STARK query index")
            val chain = ArrayList<OfflineFoldDecommit>(requiredLayers)
            for (k in 0 until requiredLayers) {
                val j = idxLayer / 2
                val y0Idx = j * 2
                val y1Idx = y0Idx + 1
                val pathY0 = merklePathFromLevels(y0Idx, layerMerkle[k])
                    ?: error("failed to build y0 path")
                val pathY1 = merklePathFromLevels(y1Idx, layerMerkle[k])
                    ?: error("failed to build y1 path")
                val pathZ = merklePathFromLevels(j, layerMerkle[k + 1])
                    ?: error("failed to build z path")
                val y0 = layerValues[k][y0Idx]
                val y1 = layerValues[k][y1Idx]
                val z = layerValues[k + 1][j]
                chain.add(
                    OfflineFoldDecommit(
                        j = j.toLong(),
                        y0 = y0,
                        y1 = y1,
                        pathY0 = pathY0,
                        pathY1 = pathY1,
                        z = z,
                        pathZ = pathZ,
                    ),
                )
                idxLayer = j
            }
            require(idxLayer == 0) { "final query index must collapse to zero" }
            queries.add(chain)
        }

        val commits = OfflineStarkCommitments(version = 1, roots = roots, compRoot = null)
        val proof = OfflineStarkProof(
            version = 1,
            commits = commits,
            queries = queries,
            compValues = null,
            air = null,
        )
        return OfflineStarkVerifyEnvelope(params, proof, transcriptLabel)
    }

    private fun layersRequired(params: OfflineStarkFriParams): Int? {
        if (params.foldArity < 2) return null
        var domain = 1 shl params.nLog2
        val fold = params.foldArity
        if (fold.countOneBits() != 1) return null
        var layers = 0
        while (domain > 1) {
            if (domain % fold != 0) return null
            domain /= fold
            layers++
        }
        return layers
    }

    private fun validateCompositionTerms(
        constant: BigInteger,
        zCoeff: BigInteger,
        auxTerms: List<OfflineStarkCompositionTerm>,
    ) {
        require(fromCanonical(constant) != null) { "invalid STARK constant" }
        require(fromCanonical(zCoeff) != null) { "invalid STARK z coefficient" }
        var lastWire: Long? = null
        for (term in auxTerms) {
            require(fromCanonical(term.value) != null && fromCanonical(term.coeff) != null) {
                "invalid STARK composition auxiliary field element"
            }
            val prev = lastWire
            require(prev == null || term.wireIndex > prev) {
                "STARK composition auxiliary wires must be strictly ordered"
            }
            lastWire = term.wireIndex
        }
    }

    private fun starkAirPublicDigestFromComposition(
        constant: BigInteger,
        zCoeff: BigInteger,
        auxTerms: List<OfflineStarkCompositionTerm>,
    ): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        md.update("iroha:zk:stark:air-public-digest:v1".toByteArray(StandardCharsets.US_ASCII))
        md.update(fqToLeBytes(constant))
        md.update(fqToLeBytes(zCoeff))
        md.update(u64ToLeBytes(auxTerms.size.toLong()))
        for (term in auxTerms) {
            md.update(u32ToLeBytes(term.wireIndex))
            md.update(fqToLeBytes(term.value))
            md.update(fqToLeBytes(term.coeff))
        }
        return md.digest()
    }

    private fun starkAirDigestLimbs(publicDigest: ByteArray): Array<BigInteger> =
        Array(4) { i -> readLeU64(publicDigest, i * 8).mod(MOD_P) }

    private fun starkAirRow(index: Int, publicDigest: ByteArray): List<BigInteger> {
        val limbs = starkAirDigestLimbs(publicDigest)
        val reducedIndex = BigInteger.valueOf(index.toLong()).mod(MOD_P)
        return listOf(
            reducedIndex,
            limbs[0],
            limbs[1],
            limbs[2],
            limbs[3],
            BigInteger.valueOf(STARK_AIR_TRACE_WIDTH.toLong()),
        )
    }

    private fun starkAirTraceLeafHash(params: OfflineStarkFriParams, row: List<BigInteger>): ByteArray {
        require(params.hashFn == STARK_HASH_SHA256_V1) { "only SHA-256 STARK backend is supported" }
        val md = MessageDigest.getInstance("SHA-256")
        md.update("STARK:AIR:TRACE:ROW:V1".toByteArray(StandardCharsets.US_ASCII))
        md.update(u64ToLeBytes(row.size.toLong()))
        for (value in row) {
            md.update(fqToLeBytes(value))
        }
        return md.digest()
    }

    private fun starkAirCompositionValue(
        index: Int,
        domainSize: Int,
        publicDigest: ByteArray,
        row: List<BigInteger>,
        nextRow: List<BigInteger>,
    ): BigInteger {
        require(domainSize > 0 && row.size == STARK_AIR_TRACE_WIDTH && nextRow.size == STARK_AIR_TRACE_WIDTH) {
            "invalid STARK AIR composition inputs"
        }
        val expected = starkAirRow(index, publicDigest)
        val expectedNext = starkAirRow((index + 1) % domainSize, publicDigest)
        var acc = BigInteger.ZERO
        var coeff = BigInteger.valueOf(3)
        for (i in 0 until STARK_AIR_TRACE_WIDTH) {
            val residue = fqSub(row[i], expected[i])
            acc = fqAdd(acc, fqMul(coeff, residue))
            coeff = fqAdd(coeff, BigInteger.valueOf(2))
        }
        for (i in 0 until STARK_AIR_TRACE_WIDTH) {
            val residue = fqSub(nextRow[i], expectedNext[i])
            acc = fqAdd(acc, fqMul(coeff, residue))
            coeff = fqAdd(coeff, BigInteger.valueOf(2))
        }
        return acc
    }

    private fun friRoundChallenge(
        params: OfflineStarkFriParams,
        transcriptLabel: String,
        root: ByteArray,
    ): BigInteger? {
        val labelBytes = transcriptLabel.toByteArray(StandardCharsets.UTF_8)
        val domainTagBytes = params.domainTag.toByteArray(StandardCharsets.UTF_8)
        val totalLen = labelBytes.size + 2 + 5 + 2 + 4 + domainTagBytes.size + root.size
        val tb = ByteArray(totalLen)
        var p = 0
        System.arraycopy(labelBytes, 0, tb, p, labelBytes.size); p += labelBytes.size
        writeLeU16(tb, p, params.version); p += 2
        tb[p++] = params.nLog2.toByte()
        tb[p++] = params.blowupLog2.toByte()
        tb[p++] = params.foldArity.toByte()
        tb[p++] = params.merkleArity.toByte()
        tb[p++] = params.hashFn.toByte()
        writeLeU16(tb, p, params.queries); p += 2
        writeLeU32(tb, p, domainTagBytes.size.toLong()); p += 4
        System.arraycopy(domainTagBytes, 0, tb, p, domainTagBytes.size); p += domainTagBytes.size
        System.arraycopy(root, 0, tb, p, root.size)
        return challenge(params, "stark:fri:r:k", tb)
    }

    private fun challenge(
        params: OfflineStarkFriParams,
        label: String,
        bytes: ByteArray,
    ): BigInteger? {
        if (params.hashFn != STARK_HASH_SHA256_V1) return null
        val md = MessageDigest.getInstance("SHA-256")
        md.update(label.toByteArray(StandardCharsets.UTF_8))
        md.update(byteArrayOf(0))
        md.update(bytes)
        val digest = md.digest()
        return readLeU64(digest, 0).mod(MOD_P)
    }

    private fun deriveQueryIndex(
        label: String,
        params: OfflineStarkFriParams,
        roots: List<ByteArray>,
        queryIdx: Int,
    ): Int? {
        if (params.nLog2 >= Int.SIZE_BITS) return null
        val domain = 1 shl params.nLog2
        if (domain == 0) return null
        if (params.hashFn != STARK_HASH_SHA256_V1) return null

        val md = MessageDigest.getInstance("SHA-256")
        md.update("STARK:query-index".toByteArray(StandardCharsets.US_ASCII))
        md.update(label.toByteArray(StandardCharsets.UTF_8))
        md.update(u16ToLeBytes(params.version))
        md.update(byteArrayOf(
            params.nLog2.toByte(),
            params.blowupLog2.toByte(),
            params.foldArity.toByte(),
            params.merkleArity.toByte(),
            params.hashFn.toByte(),
        ))
        md.update(u16ToLeBytes(params.queries))
        val domainTagBytes = params.domainTag.toByteArray(StandardCharsets.UTF_8)
        md.update(u32ToLeBytes(domainTagBytes.size.toLong()))
        md.update(domainTagBytes)
        md.update(u64ToLeBytes(queryIdx.toLong()))
        for (root in roots) {
            md.update(root)
        }
        val digest = md.digest()
        val reduced = readLeU64(digest, 0).mod(BigInteger.valueOf(domain.toLong()))
        return reduced.toInt()
    }

    private fun domainXForPair(layerDomain: Int, pairIndex: Int): BigInteger? {
        if (layerDomain < 2 || layerDomain.countOneBits() != 1 || pairIndex >= layerDomain / 2) {
            return null
        }
        val exponent = MOD_P.subtract(BigInteger.ONE).divide(BigInteger.valueOf(layerDomain.toLong()))
        val root = GOLDILOCKS_GENERATOR.modPow(exponent, MOD_P)
        return root.modPow(BigInteger.valueOf(pairIndex.toLong()), MOD_P)
    }

    private fun friFoldPair(y0: BigInteger, y1: BigInteger, beta: BigInteger, x: BigInteger): BigInteger {
        val two = BigInteger.valueOf(2)
        val twoX = fqMul(two, x)
        val invTwoX = fqInv(twoX) ?: error("failed to invert STARK fold denominator")
        val invTwo = fqInv(two) ?: error("failed to invert STARK 2")
        val even = fqMul(fqAdd(y0, y1), invTwo)
        val odd = fqMul(fqSub(y0, y1), invTwoX)
        return fqAdd(even, fqMul(beta, odd))
    }

    private fun merkleLevelsFromValues(
        params: OfflineStarkFriParams,
        values: List<BigInteger>,
    ): List<List<ByteArray>>? {
        if (values.isEmpty()) return null
        var current: MutableList<ByteArray> = values.mapTo(ArrayList(values.size)) { leafHash(it) }
        val levels = ArrayList<List<ByteArray>>()
        while (true) {
            levels.add(current.toList())
            if (current.size == 1) break
            if (current.size % 2 == 1) current.add(current.last())
            val next = ArrayList<ByteArray>(current.size / 2)
            var i = 0
            while (i < current.size) {
                next.add(nodeHash(current[i], current[i + 1]))
                i += 2
            }
            current = next
        }
        return levels
    }

    private fun merkleLevelsFromHashes(
        params: OfflineStarkFriParams,
        leaves: List<ByteArray>,
    ): List<List<ByteArray>>? {
        if (leaves.isEmpty()) return null
        var current: MutableList<ByteArray> = ArrayList(leaves)
        val levels = ArrayList<List<ByteArray>>()
        while (true) {
            levels.add(current.toList())
            if (current.size == 1) break
            if (current.size % 2 == 1) current.add(current.last())
            val next = ArrayList<ByteArray>(current.size / 2)
            var i = 0
            while (i < current.size) {
                next.add(nodeHash(current[i], current[i + 1]))
                i += 2
            }
            current = next
        }
        return levels
    }

    private fun merkleRootFromLevels(levels: List<List<ByteArray>>): ByteArray? =
        levels.lastOrNull()?.firstOrNull()

    private fun merklePathFromLevels(index: Int, levels: List<List<ByteArray>>): OfflineMerklePath? {
        val leafLevel = levels.firstOrNull() ?: return null
        if (index >= leafLevel.size) return null
        val depth = levels.size - 1
        if (depth < 0) return null
        val dirs = ByteArray((depth + 7) / 8)
        val siblings = ArrayList<ByteArray>(depth)
        var currentIndex = index
        for (levelIdx in 0 until depth) {
            val level = levels[levelIdx]
            if (currentIndex >= level.size) return null
            val siblingIdx = if (currentIndex % 2 == 0) currentIndex + 1 else currentIndex - 1
            val sibling = if (siblingIdx < level.size) level[siblingIdx] else level[currentIndex]
            if (currentIndex % 2 == 1) {
                dirs[levelIdx / 8] = (dirs[levelIdx / 8].toInt() or (1 shl (levelIdx % 8))).toByte()
            }
            siblings.add(sibling)
            currentIndex /= 2
        }
        return OfflineMerklePath(dirs, siblings)
    }

    private fun leafHash(value: BigInteger): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        md.update("LEAF".toByteArray(StandardCharsets.US_ASCII))
        md.update(fqToLeBytes(value))
        return md.digest()
    }

    private fun nodeHash(left: ByteArray, right: ByteArray): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        md.update(left)
        md.update(right)
        return md.digest()
    }

    private fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    private fun fqAdd(a: BigInteger, b: BigInteger): BigInteger = a.add(b).mod(MOD_P)
    private fun fqSub(a: BigInteger, b: BigInteger): BigInteger = a.subtract(b).mod(MOD_P)
    private fun fqMul(a: BigInteger, b: BigInteger): BigInteger = a.multiply(b).mod(MOD_P)
    private fun fqInv(a: BigInteger): BigInteger? =
        if (a.signum() == 0) null else a.modPow(MOD_P.subtract(BigInteger.valueOf(2)), MOD_P)

    private fun fromCanonical(value: BigInteger): BigInteger? =
        if (value.signum() < 0 || value >= MOD_P) null else value

    private fun fqToLeBytes(value: BigInteger): ByteArray {
        val out = ByteArray(8)
        var v = value
        val mask = BigInteger.valueOf(0xFF)
        for (i in 0..7) {
            out[i] = v.and(mask).toInt().toByte()
            v = v.shiftRight(8)
        }
        return out
    }

    private fun u64ToLeBytes(value: Long): ByteArray {
        val out = ByteArray(8)
        writeLeU64(out, 0, value)
        return out
    }

    private fun u32ToLeBytes(value: Long): ByteArray {
        val out = ByteArray(4)
        writeLeU32(out, 0, value)
        return out
    }

    private fun u16ToLeBytes(value: Int): ByteArray {
        val out = ByteArray(2)
        writeLeU16(out, 0, value)
        return out
    }

    private fun writeLeU16(dst: ByteArray, offset: Int, value: Int) {
        dst[offset] = (value and 0xFF).toByte()
        dst[offset + 1] = ((value ushr 8) and 0xFF).toByte()
    }

    private fun writeLeU32(dst: ByteArray, offset: Int, value: Long) {
        dst[offset] = (value and 0xFF).toByte()
        dst[offset + 1] = ((value ushr 8) and 0xFF).toByte()
        dst[offset + 2] = ((value ushr 16) and 0xFF).toByte()
        dst[offset + 3] = ((value ushr 24) and 0xFF).toByte()
    }

    private fun writeLeU64(dst: ByteArray, offset: Int, value: Long) {
        var v = value
        for (i in 0..7) {
            dst[offset + i] = (v and 0xFF).toByte()
            v = v ushr 8
        }
    }

    private fun readLeU64(src: ByteArray, offset: Int): BigInteger {
        var v = BigInteger.ZERO
        for (i in 7 downTo 0) {
            v = v.shiftLeft(8).or(BigInteger.valueOf((src[offset + i].toInt() and 0xFF).toLong()))
        }
        return v
    }
}
