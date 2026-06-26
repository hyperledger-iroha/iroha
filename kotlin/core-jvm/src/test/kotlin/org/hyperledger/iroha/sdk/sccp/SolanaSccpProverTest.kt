package org.hyperledger.iroha.sdk.sccp

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class SolanaSccpProverTest {
    private val solanaSignature55 =
        "2hxGyn4y9Mjkii76BqmxVoNYbTs3tw97bmtZRXnDoZPAw7VZTWhhk1aV11DtFgYGVibPaty4PQLHVLaKrT24NxGU"
    private val solanaSignature01 =
        "2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2"
    private val solanaZeroSignature = "1".repeat(64)
    private val solanaProgram42 = "5TeWSsjg2gbxCyWVniXeCmwM7UtHTCK7svzJr5xYJzHf"
    private val solanaProgram02 = "8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR"
    private val solanaZeroProgram = "1".repeat(32)
    private val solanaMainnetGenesisPublicInput =
        "0x8dbaadfbc441ded0257a4700cd26d814b5a196be44b963454cff8dd9543f13b5"

    @Test
    fun derivesSolanaRouteCanaryEvidenceHash() {
        val evidence = sampleSolanaRouteCanaryEvidence()

        assertEquals(475, SccpSolana.canonicalRouteCanaryEvidenceBytes(evidence).size)
        assertEquals(
            "0x77296e47d5681f97136dc79d66dbda4478c3c5ec80271bfd4f1f3b3dbb8e15ca",
            SccpSolana.routeCanaryEvidenceHash(evidence),
        )
        assertEquals(
            "BPFLoaderUpgradeab1e11111111111111111111111",
            SccpSolana.UPGRADEABLE_LOADER_ID,
        )

        val slotMismatch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.routeCanaryEvidenceHash(evidence.copy(solanaProgramdataSlot = "4322"))
        }
        assertTrue(slotMismatch.message?.contains("solanaExpectedProgramdataSlot") == true)
        val nonElf = assertFailsWith<IllegalArgumentException> {
            SccpSolana.routeCanaryEvidenceHash(
                evidence.copy(solanaProgramdataExecutableBase64 = "AQIDBA=="),
            )
        }
        assertTrue(nonElf.message?.contains("BPF ELF") == true)
        val wrongDestinationBinding = assertFailsWith<IllegalArgumentException> {
            SccpSolana.routeCanaryEvidenceHash(evidence.copy(destinationBindingHash = "0x" + "78".repeat(32)))
        }
        assertTrue(
            wrongDestinationBinding.message?.contains(
                "destinationBindingHash must match canonical Solana destination binding",
            ) == true,
        )
        val wrongExpectedDestinationBinding = assertFailsWith<IllegalArgumentException> {
            SccpSolana.routeCanaryEvidenceHash(evidence.copy(expectedDestinationBindingHash = "0x" + "78".repeat(32)))
        }
        assertTrue(
            wrongExpectedDestinationBinding.message?.contains(
                "expectedDestinationBindingHash must match canonical Solana destination binding",
            ) == true,
        )
        listOf(
            evidence.copy(routeAllowlistHash = evidence.destinationBindingHash),
            evidence.copy(routeAllowlistHash = evidence.sourceVerifierMaterialHash),
            evidence.copy(routeAllowlistHash = evidence.sourceAdapterEngineDeploymentHash),
            evidence.copy(sourceVerifierMaterialHash = evidence.destinationBindingHash),
            evidence.copy(sourceAdapterEngineDeploymentHash = evidence.destinationBindingHash),
            evidence.copy(sourceAdapterEngineDeploymentHash = evidence.sourceVerifierMaterialHash),
        ).forEach { replay ->
            val failure = assertFailsWith<IllegalArgumentException> {
                SccpSolana.routeCanaryEvidenceHash(replay)
            }
            assertTrue(failure.message?.contains("Solana route canary governed hashes") == true)
        }
    }

    private fun sampleSolanaStakeStateV2StakeAccount(): ByteArray {
        val data = ByteArray(200)
        writeU32Le(data, 0, 2)
        data.fill(0x81.toByte(), 12, 44)
        data.fill(0x91.toByte(), 44, 76)
        data.fill(0xa1.toByte(), 124, 156)
        writeU64Le(data, 156, 1_000L)
        writeU64Le(data, 164, 2L)
        writeU64Le(data, 172, 9L)
        byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f)
            .copyInto(data, destinationOffset = 180)
        writeU64Le(data, 188, 123L)
        data[196] = 1
        return data
    }

    private fun sampleSolanaVoteStateAccount(hasLatency: Boolean = true): ByteArray {
        val data = ByteArray(3_762)
        var cursor = 0

        fun writeU8(value: Int) {
            data[cursor] = value.toByte()
            cursor += 1
        }

        fun writeU32(value: Int) {
            writeU32Le(data, cursor, value)
            cursor += 4
        }

        fun writeU64(value: Long) {
            writeU64Le(data, cursor, value)
            cursor += 8
        }

        fun writeRepeated(value: Byte, length: Int) {
            data.fill(value, cursor, cursor + length)
            cursor += length
        }

        writeU32(if (hasLatency) 2 else 1)
        writeRepeated(0x51.toByte(), 32)
        writeRepeated(0x71.toByte(), 32)
        writeU8(7)
        writeU64(SccpSolana.TOWER_VOTE_STACK_DEPTH)
        for (index in 0 until SccpSolana.TOWER_VOTE_STACK_DEPTH.toInt()) {
            if (hasLatency) {
                writeU8(0)
            }
            writeU64(11L + index)
            writeU32(SccpSolana.TOWER_VOTE_STACK_DEPTH.toInt() - index)
        }
        writeU8(1)
        writeU64(10L)
        writeU64(2L)
        writeU64(1L)
        writeRepeated(0x60.toByte(), 32)
        writeU64(3L)
        writeRepeated(0x61.toByte(), 32)
        return data
    }

    private fun sampleSolanaVoteStateV4Account(authorizedVoterCount: Int = 2): ByteArray {
        val data = ByteArray(3_762)
        var cursor = 0

        fun writeU8(value: Int) {
            data[cursor] = value.toByte()
            cursor += 1
        }

        fun writeU16(value: Int) {
            data[cursor] = (value and 0xFF).toByte()
            data[cursor + 1] = ((value ushr 8) and 0xFF).toByte()
            cursor += 2
        }

        fun writeU32(value: Int) {
            writeU32Le(data, cursor, value)
            cursor += 4
        }

        fun writeU64(value: Long) {
            writeU64Le(data, cursor, value)
            cursor += 8
        }

        fun writeRepeated(value: Byte, length: Int) {
            data.fill(value, cursor, cursor + length)
            cursor += length
        }

        writeU32(3)
        writeRepeated(0x51.toByte(), 32)
        writeRepeated(0x71.toByte(), 32)
        writeRepeated(0x81.toByte(), 32)
        writeRepeated(0x91.toByte(), 32)
        writeU16(1_234)
        writeU16(9_876)
        writeU64(456L)
        writeU8(1)
        writeRepeated(0xa5.toByte(), 48)
        writeU64(SccpSolana.TOWER_VOTE_STACK_DEPTH)
        for (index in 0 until SccpSolana.TOWER_VOTE_STACK_DEPTH.toInt()) {
            writeU8(0)
            writeU64(11L + index)
            writeU32(SccpSolana.TOWER_VOTE_STACK_DEPTH.toInt() - index)
        }
        writeU8(1)
        writeU64(10L)
        writeU64(authorizedVoterCount.toLong())
        for (index in 0 until authorizedVoterCount) {
            writeU64((index + 1).toLong())
            writeRepeated((0x60 + index).toByte(), 32)
        }
        return data
    }

    private fun writeU32Le(data: ByteArray, offset: Int, value: Int) {
        data[offset] = (value and 0xFF).toByte()
        data[offset + 1] = ((value ushr 8) and 0xFF).toByte()
        data[offset + 2] = ((value ushr 16) and 0xFF).toByte()
        data[offset + 3] = ((value ushr 24) and 0xFF).toByte()
    }

    private fun writeU64Le(data: ByteArray, offset: Int, value: Long) {
        var working = value
        for (index in 0 until 8) {
            data[offset + index] = (working and 0xFFL).toByte()
            working = working ushr 8
        }
    }

    @Test
    fun buildsSolanaSccpProofRequest() {
        val request = SccpSolana.buildProofRequest(
            sampleWitness(
                sourceAdapterDeploymentHash = "ab".repeat(32),
                sourceAdapterDeploymentReceiptHash = "cd".repeat(32),
            ),
        )

        assertEquals(1, request.version)
        assertEquals(SccpSolana.RECURSIVE_PROOF_BACKEND_V1, request.backend)
        assertEquals(SccpSolana.DOMAIN_SOLANA, request.sourceDomain)
        assertEquals(SccpSolana.DOMAIN_SORA, request.targetDomain)
        assertTrue(request.witness.blockhash.matches(Regex("0x[0-9a-f]{64}")))
        val canonicalBlockhashRequest = SccpSolana.buildProofRequest(
            sampleWitness(
                blockhash = request.witness.blockhash,
                sourceAdapterDeploymentHash = "ab".repeat(32),
                sourceAdapterDeploymentReceiptHash = "cd".repeat(32),
            ),
        )
        assertEquals(request.witnessHash, canonicalBlockhashRequest.witnessHash)
        assertContentEquals(
            SccpSolana.canonicalWitnessBytes(request.witness),
            SccpSolana.canonicalWitnessBytes(canonicalBlockhashRequest.witness),
        )
        assertEquals("0x" + "dd".repeat(32), request.publicInputs.messageId)
        assertEquals("0x" + "aa".repeat(32), request.publicInputs.bankHash)
        assertEquals("0x" + "bb".repeat(32), request.publicInputs.transactionStatusRoot)
        assertEquals("0x" + "cc".repeat(32), request.publicInputs.messageProofHash)
        assertEquals("0x" + "56".repeat(32), request.publicInputs.statementHash)
        assertEquals("0x" + "78".repeat(32), request.publicInputs.destinationBindingHash)
        assertEquals("0x" + "ab".repeat(32), request.publicInputs.sourceAdapterDeploymentHash)
        assertEquals(
            "0x" + "cd".repeat(32),
            request.publicInputs.sourceAdapterDeploymentReceiptHash,
        )
        assertEquals(SccpSolana.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1, request.sourceStateVerifierId)
        assertEquals(SccpSolana.ZERO_HASH_V1, request.sourceStateVerifierHash)
        assertEquals(
            SccpSolana.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
            request.publicInputs.sourceStateVerifierId,
        )
        assertEquals(SccpSolana.ZERO_HASH_V1, request.publicInputs.sourceStateVerifierHash)
        assertEquals(
            SccpSolana.sourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding),
            request.sourceAdapterDeploymentBindingHash,
        )
        assertEquals(request.publicInputs.statementHash, request.proofContext.statementHash)
        assertTrue(request.witnessHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(request.proofContextHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(request.sourceAdapterDeploymentBindingHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(SccpSolana.canonicalProofContextBytes(request.proofContext).isNotEmpty())
    }

    @Test
    fun rejectsNonSoraSolanaProofRequestTargetDomain() {
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildProofRequest(sampleWitness(targetDomain = 4))
        }

        assertTrue(error.message!!.contains("targetDomain must be SORA"))
    }

    @Test
    fun requiresSourceEventDigest() {
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeWitness(sampleWitness(sourceEventDigest = ""))
        }
        assertTrue(error.message?.contains("sourceEventDigest") == true)
    }

    @Test
    fun rejectsUnexpectedSolanaSourceStateVerifierProfile() {
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeWitness(
                sampleWitness(
                    sourceStateVerifierId = "debug-solana-state-verifier",
                    sourceStateVerifierHash = "ab".repeat(32),
                ),
            )
        }
        assertTrue(error.message?.contains("AccountsDB verifier profile") == true)
    }

    @Test
    fun buildsMessageProofHashFromInclusionWitness() {
        val branch = listOf(ByteArray(32) { 0x56.toByte() })
        val transactionStatusRoot = SccpSolana.transactionStatusRootFromBranch(
            sourceEventDigest = "34".repeat(32),
            transactionSignature = solanaSignature55,
            emitterProgramId = solanaProgram42,
            inclusionBranch = branch,
        )
        assertEquals(
            "0xb048ca31d8ad7b2a0d15cbeb81d536350743483d44dd93136e859df93d3863b2",
            transactionStatusRoot,
        )
        val hash = SccpSolana.messageProofHash(
            sourceEventDigest = "34".repeat(32),
            transactionStatusRoot = transactionStatusRoot,
            transactionSignature = solanaSignature55,
            emitterProgramId = solanaProgram42,
            inclusionBranch = branch,
        )

        assertTrue(hash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(
            SccpSolana.canonicalTransactionStatusLeafBytes(
                sourceEventDigest = "34".repeat(32),
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
            ).isNotEmpty(),
        )
        assertEquals(
            "0x4e12efed6d53466de0596f05aa6cc767df1efd6a4d1549276c4ec8b69118515d",
            SccpSolana.transactionStatusLeafHash(
                sourceEventDigest = "34".repeat(32),
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
            ),
        )
        val zeroLeafSignature = assertFailsWith<IllegalArgumentException> {
            SccpSolana.transactionStatusLeafHash(
                sourceEventDigest = "34".repeat(32),
                transactionSignature = solanaZeroSignature,
                emitterProgramId = solanaProgram42,
            )
        }
        assertTrue(zeroLeafSignature.message?.contains("transactionSignature") == true)
        val zeroLeafProgram = assertFailsWith<IllegalArgumentException> {
            SccpSolana.transactionStatusLeafHash(
                sourceEventDigest = "34".repeat(32),
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaZeroProgram,
            )
        }
        assertTrue(zeroLeafProgram.message?.contains("emitterProgramId") == true)
        assertTrue(
            SccpSolana.canonicalMessageProofBytes(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
                inclusionBranch = branch,
            ).isNotEmpty(),
        )
        val zeroDigest = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "00".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroDigest.message?.contains("sourceEventDigest") == true)
        val zeroRoot = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = "00".repeat(32),
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroRoot.message?.contains("transactionStatusRoot") == true)
        val zeroProofSignature = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaZeroSignature,
                emitterProgramId = solanaProgram42,
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroProofSignature.message?.contains("transactionSignature") == true)
        val zeroProofProgram = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaZeroProgram,
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroProofProgram.message?.contains("emitterProgramId") == true)
        assertTrue(
            hash != SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature01,
                emitterProgramId = solanaProgram42,
                inclusionBranch = branch,
            ),
        )
        assertTrue(
            hash != SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram02,
                inclusionBranch = branch,
            ),
        )
        val emptyBranch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
                inclusionBranch = emptyList(),
            )
        }
        assertTrue(emptyBranch.message?.contains("inclusionBranch") == true)
        val oversizedBranch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
                inclusionBranch = List(SccpSolana.MAX_SOURCE_MERKLE_BRANCH_NODES + 1) {
                    ByteArray(32) { 0x56.toByte() }
                },
            )
        }
        assertTrue(oversizedBranch.message?.contains("at most") == true)
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = solanaSignature55,
                emitterProgramId = solanaProgram42,
                inclusionBranch = listOf(ByteArray(31) { 0xab.toByte() }),
            )
        }
        assertTrue(error.message?.contains("inclusionBranch[0]") == true)
        val base58Error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = transactionStatusRoot,
                transactionSignature = "not-a-solana-signature",
                emitterProgramId = solanaProgram42,
                inclusionBranch = branch,
            )
        }
        assertTrue(base58Error.message?.contains("transactionSignature") == true)
    }

    @Test
    fun buildsEpochStakeRootForVoteWitnesses() {
        val validatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val validatorStakes = listOf("1", "2")

        assertEquals(432_000L, SccpSolana.MAINNET_SLOTS_PER_EPOCH)
        assertEquals("2", SccpSolana.mainnetEpochForSlot("864000"))
        assertEquals(
            134,
            SccpSolana.canonicalEpochStakeRootBytes("3", validatorPublicKeys, validatorStakes).size,
        )
        assertEquals(
            "0x1d86a5ecfac6e63bfcefdc1a3bfefd962a33e2a4cf65cd4e8518bcebea771f0a",
            SccpSolana.epochStakeRoot("3", validatorPublicKeys, validatorStakes),
        )
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.epochStakeRoot("3", listOf(ByteArray(31) { 0x11.toByte() }), listOf("1"))
        }
        assertTrue(error.message?.contains("validatorPublicKeys[0]") == true)
        val zeroKey = assertFailsWith<IllegalArgumentException> {
            SccpSolana.epochStakeRoot("3", listOf(ByteArray(32) { 0x00.toByte() }), listOf("1"))
        }
        assertTrue(zeroKey.message?.contains("validatorPublicKeys[0]") == true)
        val oversizedValidatorPublicKeys = List(SccpSolana.MAX_VALIDATORS + 1) { index ->
            ByteArray(32).also {
                val value = index + 1
                it[30] = (value and 0xff).toByte()
                it[31] = ((value ushr 8) and 0xff).toByte()
            }
        }
        val tooManyValidators = assertFailsWith<IllegalArgumentException> {
            SccpSolana.epochStakeRoot(
                "3",
                oversizedValidatorPublicKeys,
                List(oversizedValidatorPublicKeys.size) { "1" },
            )
        }
        assertTrue(tooManyValidators.message?.contains("1..8192") == true)
    }

    @Test
    fun buildsStakeActivationHashForFinalityContext() {
        val validatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val validatorStakes = listOf("1", "2")
        val activationEpochs = listOf("0", "2")
        val deactivationEpochs = listOf("18446744073709551615", "9")

        assertEquals(
            165,
            SccpSolana.canonicalStakeActivationBytes(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
            ).size,
        )
        assertEquals(
            "0xdb418c62a1aeb8ae15cb26e3a198d46890cefa3545df8e1921be2e83f57dabf3",
            SccpSolana.stakeActivationHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
            ),
        )
        val futureActivation = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeActivationHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                listOf("4", "2"),
                deactivationEpochs,
            )
        }
        assertTrue(futureActivation.message?.contains("validatorActivationEpochs[0]") == true)
        val currentEpochActivation = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeActivationHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                listOf("3", "2"),
                deactivationEpochs,
            )
        }
        assertTrue(currentEpochActivation.message?.contains("validatorActivationEpochs[0]") == true)
        val expired = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeActivationHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                listOf("18446744073709551615", "2"),
            )
        }
        assertTrue(expired.message?.contains("validatorDeactivationEpochs[1]") == true)
        assertEquals(
            66,
            SccpSolana.stakeActivationHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                listOf("18446744073709551615", "3"),
            ).length,
        )
        val lengthMismatch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeActivationHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                listOf("0"),
                deactivationEpochs,
            )
        }
        assertTrue(lengthMismatch.message?.contains("validatorActivationEpochs") == true)
    }

    @Test
    fun buildsAccountOpeningHashForFinalityContext() {
        val address = ByteArray(32) { 0x31.toByte() }
        val dataHash = "0x" + "71".repeat(32)

        assertEquals(
            122,
            SccpSolana.canonicalAccountOpeningBytes(
                address,
                hexBytes(SccpSolana.VOTE_PROGRAM_ID),
                "1000000",
                "0",
                false,
                dataHash,
            ).size,
        )
        val accountHash = SccpSolana.accountOpeningHash(
            address,
            hexBytes(SccpSolana.VOTE_PROGRAM_ID),
            "1000000",
            "0",
            false,
            dataHash,
        )
        assertTrue(accountHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(
            accountHash != SccpSolana.accountOpeningHash(
                address,
                hexBytes(SccpSolana.STAKE_PROGRAM_ID),
                "1000000",
                "0",
                false,
                dataHash,
            ),
        )
        assertTrue(
            accountHash != SccpSolana.accountOpeningHash(
                address,
                hexBytes(SccpSolana.VOTE_PROGRAM_ID),
                "1000000",
                "0",
                true,
                dataHash,
            ),
        )
        val zeroLamports = assertFailsWith<IllegalArgumentException> {
            SccpSolana.accountOpeningHash(
                address,
                hexBytes(SccpSolana.VOTE_PROGRAM_ID),
                "0",
                "0",
                false,
                dataHash,
            )
        }
        assertTrue(zeroLamports.message?.contains("lamports") == true)
    }

    @Test
    fun buildsOpenedAccountsLtHashContributionBindings() {
        fun ltHash(value: Int): ByteArray {
            val out = ByteArray(2048)
            out[0] = value.toByte()
            out[1] = (value ushr 8).toByte()
            return out
        }
        fun add(left: ByteArray, right: ByteArray): ByteArray {
            val out = left.copyOf()
            val mixed = ((out[0].toInt() and 0xff) or ((out[1].toInt() and 0xff) shl 8)) +
                ((right[0].toInt() and 0xff) or ((right[1].toInt() and 0xff) shl 8))
            out[0] = mixed.toByte()
            out[1] = (mixed ushr 8).toByte()
            return out
        }
        val voteOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x31.toByte() },
            owner = hexBytes(SccpSolana.VOTE_PROGRAM_ID),
            lamports = "1000000",
            rentEpoch = "0",
            dataHash = "0x" + "91".repeat(32),
        )
        val stakeOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x32.toByte() },
            owner = hexBytes(SccpSolana.STAKE_PROGRAM_ID),
            lamports = "2000000",
            rentEpoch = "0",
            dataHash = "0x" + "92".repeat(32),
        )
        val stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address = hexBytes(SccpSolana.STAKE_HISTORY_SYSVAR_ID),
            owner = hexBytes(SccpSolana.SYSVAR_PROGRAM_ID),
            lamports = "1",
            rentEpoch = "0",
            dataHash = "0x" + "93".repeat(32),
        )
        val unopenedOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x34.toByte() },
            owner = hexBytes(SccpSolana.STAKE_PROGRAM_ID),
            lamports = "3000000",
            rentEpoch = "0",
            dataHash = "0x" + "94".repeat(32),
        )
        val voteRawData = byteArrayOf(1, 2, 3)
        val stakeRawData = byteArrayOf(4, 5, 6)
        val stakeHistoryRawData = byteArrayOf(7, 8, 9)
        val unopenedRawData = byteArrayOf(10, 11, 12)
        val voteLtHash = SccpSolana.accountLtHash(voteOpening, voteRawData)
        val stakeLtHash = SccpSolana.accountLtHash(stakeOpening, stakeRawData)
        val stakeHistoryLtHash = SccpSolana.accountLtHash(stakeHistoryOpening, stakeHistoryRawData)
        val openedLtHash = SccpSolana.accountsLtHashFromOpenings(
            listOf(voteOpening, stakeOpening, stakeHistoryOpening),
            listOf(voteRawData, stakeRawData, stakeHistoryRawData),
        )
        val unopenedLtHash = SccpSolana.accountLtHash(unopenedOpening, unopenedRawData)
        val accountsLtHash = SccpSolana.accountsLtHashFromOpenings(
            listOf(voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening),
            listOf(voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData),
        )
        val input = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot = "1296096",
            accountInclusionRoot = "0x" + "77".repeat(32),
            accountsLtHashChecksum = SccpSolana.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash = accountsLtHash,
            validatorVoteAccountOpenings = listOf(voteOpening),
            validatorVoteAccountRawData = listOf(voteRawData),
            validatorVoteAccountLtHashes = listOf(voteLtHash),
            validatorStakeAccountOpenings = listOf(stakeOpening),
            validatorStakeAccountRawData = listOf(stakeRawData),
            validatorStakeAccountLtHashes = listOf(stakeLtHash),
            stakeHistorySysvarOpening = stakeHistoryOpening,
            stakeHistorySysvarRawData = stakeHistoryRawData,
            stakeHistorySysvarAccountLtHash = stakeHistoryLtHash,
        )

        assertContentEquals(unopenedLtHash, SccpSolana.openedAccountsLtHashResidual(input))
        assertEquals(
            SccpSolana.accountsLtHashChecksum(unopenedLtHash),
            SccpSolana.openedAccountsLtHashResidualChecksum(input),
        )
        assertEquals(10_696, SccpSolana.canonicalOpenedAccountsLtHashContributionsBytes(input).size)
        assertEquals(
            "0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9",
            SccpSolana.openedAccountsLtHashContributionsHash(input),
        )
        val mismatch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountsLtHashContributionsHash(
                input.copy(accountsLtHashChecksum = "0x" + "88".repeat(32)),
            )
        }
        assertTrue(mismatch.message?.contains("accountsLtHashChecksum") == true)
        val zeroResidual = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountsLtHashContributionsHash(
                input.copy(
                    accountsLtHashChecksum = SccpSolana.accountsLtHashChecksum(openedLtHash),
                    accountsLtHash = openedLtHash,
                ),
            )
        }
        assertTrue(zeroResidual.message?.contains("openedAccountsLtHashResidual") == true)
        val duplicateOpened = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountsLtHashContributionsHash(
                input.copy(
                    validatorStakeAccountOpenings = listOf(
                        stakeOpening.copy(address = voteOpening.address.copyOf()),
                    ),
                ),
            )
        }
        assertTrue(duplicateOpened.message?.contains("opened account addresses") == true)
        val zeroLamportsOpened = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountsLtHashContributionsHash(
                input.copy(
                    validatorVoteAccountOpenings = listOf(
                        voteOpening.copy(lamports = "0"),
                    ),
                    validatorVoteAccountLtHashes = listOf(ByteArray(2048)),
                ),
            )
        }
        assertTrue(zeroLamportsOpened.message?.contains("lamports") == true)
        val oversizedVoteOpened = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountsLtHashContributionsHash(
                input.copy(
                    validatorVoteAccountOpenings = List(SccpSolana.MAX_VALIDATORS + 1) { voteOpening },
                    validatorVoteAccountRawData = List(SccpSolana.MAX_VALIDATORS + 1) { voteRawData },
                    validatorVoteAccountLtHashes = List(SccpSolana.MAX_VALIDATORS + 1) { voteLtHash },
                ),
            )
        }
        assertTrue(oversizedVoteOpened.message?.contains("validatorVoteAccountOpenings") == true)
    }

    @Test
    fun derivesAccountLtHashFromOpeningsAndRawData() {
        fun add(left: ByteArray, right: ByteArray): ByteArray {
            val out = left.copyOf()
            for (index in out.indices step 2) {
                val mixed = (
                    ((out[index].toInt() and 0xff) or ((out[index + 1].toInt() and 0xff) shl 8)) +
                        ((right[index].toInt() and 0xff) or ((right[index + 1].toInt() and 0xff) shl 8))
                    ) and 0xffff
                out[index] = mixed.toByte()
                out[index + 1] = (mixed ushr 8).toByte()
            }
            return out
        }

        val voteOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x31.toByte() },
            owner = hexBytes(SccpSolana.VOTE_PROGRAM_ID),
            lamports = "1000000",
            rentEpoch = "0",
            dataHash = "0x" + "91".repeat(32),
        )
        val voteRawData = byteArrayOf(1, 2, 3)
        val voteLtHash = SccpSolana.accountLtHash(voteOpening, voteRawData)
        assertEquals(2048, voteLtHash.size)
        assertEquals(
            "0x56a868657e9113c76dc94321040b8f01a35ea4996c6fa235581510cd18be4bfe",
            SccpSolana.accountsLtHashChecksum(voteLtHash),
        )
        val maxRawData = ByteArray(65_536) { (it and 0xff).toByte() }
        val maxLtHash = SccpSolana.accountLtHash(voteOpening, maxRawData)
        assertEquals(
            "0xc467c59f47747fdae4d87f8c79413ae24d3674ea3ca02aad0a1216a20d4fe147",
            SccpSolana.accountsLtHashChecksum(maxLtHash),
        )
        assertEquals(
            "c972db5d20a5a451a44daa674d0511382480d6e9060f750129723812e0e3c66a4" +
                "deddbb7975e2ff4d4c753aebcb703e61122d1ca1cfcd4f0c002a2cad30f4949",
            hexLower(maxLtHash.copyOfRange(0, 64)),
        )
        assertEquals(
            "b4159fa2d334c4209bfb59997f7da42a56e2e921e0bbc4ebd916f3c55353b630" +
                "e26303b0af0b23e91870e9815f7ed6348395fbc7c0f07bf605da23589fa9fb51",
            hexLower(maxLtHash.copyOfRange(maxLtHash.size - 64, maxLtHash.size)),
        )
        val zeroLamportsOpening = voteOpening.copy(
            address = ByteArray(32) { 0x33.toByte() },
            lamports = "0",
            dataHash = "0x" + "94".repeat(32),
        )
        assertContentEquals(ByteArray(2048), SccpSolana.accountLtHash(zeroLamportsOpening, voteRawData))
        assertContentEquals(
            voteLtHash,
            SccpSolana.accountsLtHashFromOpenings(
                listOf(voteOpening, zeroLamportsOpening),
                listOf(voteRawData, voteRawData),
            ),
        )

        val stakeOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x32.toByte() },
            owner = hexBytes(SccpSolana.STAKE_PROGRAM_ID),
            lamports = "2000000",
            rentEpoch = "0",
            dataHash = "0x" + "92".repeat(32),
        )
        val stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address = hexBytes(SccpSolana.STAKE_HISTORY_SYSVAR_ID),
            owner = hexBytes(SccpSolana.SYSVAR_PROGRAM_ID),
            lamports = "1",
            rentEpoch = "0",
            dataHash = "0x" + "93".repeat(32),
        )
        val stakeRawData = byteArrayOf(4, 5, 6)
        val stakeHistoryRawData = byteArrayOf(7, 8, 9)
        val stakeLtHash = SccpSolana.accountLtHash(stakeOpening, stakeRawData)
        val stakeHistoryLtHash = SccpSolana.accountLtHash(stakeHistoryOpening, stakeHistoryRawData)
        val unopenedLtHash = ByteArray(2048) { 0x44.toByte() }
        val openedLtHash = SccpSolana.accountsLtHashFromOpenings(
            listOf(voteOpening, stakeOpening, stakeHistoryOpening),
            listOf(voteRawData, stakeRawData, stakeHistoryRawData),
        )
        val accountsLtHash = add(openedLtHash, unopenedLtHash)
        val derivedInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot = "1296096",
            accountInclusionRoot = "0x" + "77".repeat(32),
            accountsLtHashChecksum = SccpSolana.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash = accountsLtHash,
            validatorVoteAccountOpenings = listOf(voteOpening),
            validatorVoteAccountRawData = listOf(voteRawData),
            validatorStakeAccountOpenings = listOf(stakeOpening),
            validatorStakeAccountRawData = listOf(stakeRawData),
            stakeHistorySysvarOpening = stakeHistoryOpening,
            stakeHistorySysvarRawData = stakeHistoryRawData,
        )
        val precomputedInput = derivedInput.copy(
            validatorVoteAccountLtHashes = listOf(voteLtHash),
            validatorStakeAccountLtHashes = listOf(stakeLtHash),
            stakeHistorySysvarAccountLtHash = stakeHistoryLtHash,
        )

        assertContentEquals(unopenedLtHash, SccpSolana.openedAccountsLtHashResidual(derivedInput))
        assertContentEquals(
            SccpSolana.canonicalOpenedAccountsLtHashContributionsBytes(precomputedInput),
            SccpSolana.canonicalOpenedAccountsLtHashContributionsBytes(derivedInput),
        )
        assertEquals(
            SccpSolana.openedAccountsLtHashContributionsHash(precomputedInput),
            SccpSolana.openedAccountsLtHashContributionsHash(derivedInput),
        )
        val wrongVoteLtHash = voteLtHash.copyOf()
        wrongVoteLtHash[0] = (wrongVoteLtHash[0].toInt() xor 1).toByte()
        val badPrecomputed = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountsLtHashContributionsHash(
                derivedInput.copy(
                    validatorVoteAccountLtHashes = listOf(wrongVoteLtHash),
                    validatorStakeAccountLtHashes = listOf(stakeLtHash),
                    stakeHistorySysvarAccountLtHash = stakeHistoryLtHash,
                ),
            )
        }
        assertTrue(badPrecomputed.message?.contains("validatorVoteAccountLtHashes[0]") == true)
    }

    @Test
    fun buildsAccountsLtHashSourceStateProofRequests() {
        fun ltHash(value: Int): ByteArray {
            val bytes = ByteArray(2048)
            for (index in bytes.indices step 2) {
                bytes[index] = value.toByte()
            }
            return bytes
        }
        fun add(left: ByteArray, right: ByteArray): ByteArray {
            val out = left.copyOf()
            for (index in out.indices step 2) {
                val mixed = (
                    ((out[index].toInt() and 0xff) or ((out[index + 1].toInt() and 0xff) shl 8)) +
                        ((right[index].toInt() and 0xff) or ((right[index + 1].toInt() and 0xff) shl 8))
                    ) and 0xffff
                out[index] = mixed.toByte()
                out[index + 1] = (mixed ushr 8).toByte()
            }
            return out
        }
        val voteOpening = SolanaSccpAccountOpeningInput(
            address = hexBytes("31".repeat(32)),
            owner = hexBytes(SccpSolana.VOTE_PROGRAM_ID),
            lamports = "1000000",
            rentEpoch = "0",
            dataHash = "0x" + "91".repeat(32),
        )
        val stakeOpening = SolanaSccpAccountOpeningInput(
            address = hexBytes("32".repeat(32)),
            owner = hexBytes(SccpSolana.STAKE_PROGRAM_ID),
            lamports = "2000000",
            rentEpoch = "0",
            dataHash = "0x" + "92".repeat(32),
        )
        val stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address = hexBytes(SccpSolana.STAKE_HISTORY_SYSVAR_ID),
            owner = hexBytes(SccpSolana.SYSVAR_PROGRAM_ID),
            lamports = "1",
            rentEpoch = "0",
            dataHash = "0x" + "93".repeat(32),
        )
        val voteRawData = byteArrayOf(1, 2, 3)
        val stakeRawData = byteArrayOf(4, 5, 6)
        val stakeHistoryRawData = byteArrayOf(7, 8, 9)
        val voteLtHash = SccpSolana.accountLtHash(voteOpening, voteRawData)
        val stakeLtHash = SccpSolana.accountLtHash(stakeOpening, stakeRawData)
        val stakeHistoryLtHash = SccpSolana.accountLtHash(stakeHistoryOpening, stakeHistoryRawData)
        val unopenedLtHash = ltHash(4)
        val accountsLtHash = add(add(add(voteLtHash, stakeLtHash), stakeHistoryLtHash), unopenedLtHash)
        val opened = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot = "1296096",
            accountInclusionRoot = "0x" + "77".repeat(32),
            accountsLtHashChecksum = SccpSolana.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash = accountsLtHash,
            validatorVoteAccountOpenings = listOf(voteOpening),
            validatorVoteAccountRawData = listOf(voteRawData),
            validatorVoteAccountLtHashes = listOf(voteLtHash),
            validatorStakeAccountOpenings = listOf(stakeOpening),
            validatorStakeAccountRawData = listOf(stakeRawData),
            validatorStakeAccountLtHashes = listOf(stakeLtHash),
            stakeHistorySysvarOpening = stakeHistoryOpening,
            stakeHistorySysvarRawData = stakeHistoryRawData,
            stakeHistorySysvarAccountLtHash = stakeHistoryLtHash,
        )
        val bankHash = SccpSolana.agaveBankHash(
            parentBankHash = "c0".repeat(32),
            bankSignatureCount = "8",
            blockhash = "42".repeat(32),
            accountsLtHash = accountsLtHash,
        )
        val zeroAccountsLtHash = ByteArray(2_048)
        val zeroAccountsLtHashChecksum = SccpSolana.accountsLtHashChecksum(zeroAccountsLtHash)
        assertTrue(zeroAccountsLtHashChecksum.startsWith("0x"))
        val zeroBankHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.agaveBankHash(
                parentBankHash = "c0".repeat(32),
                bankSignatureCount = "8",
                blockhash = "42".repeat(32),
                accountsLtHash = zeroAccountsLtHash,
            )
        }
        assertTrue(zeroBankHash.message?.contains("accountsLtHash") == true)
        val zeroOpened = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountsLtHashContributionsHash(
                opened.copy(
                    accountsLtHashChecksum = zeroAccountsLtHashChecksum,
                    accountsLtHash = zeroAccountsLtHash,
                ),
            )
        }
        assertTrue(zeroOpened.message?.contains("accountsLtHash") == true)
        val witness = sampleWitness(
            sourceStateVerifierHash = "aa".repeat(32),
        ).copy(
            finalizedSlot = "1296096",
            parentSlot = "1296095",
            parentBankHash = "c0".repeat(32),
            blockhash = "42".repeat(32),
            bankHash = bankHash,
            accountInclusionRoot = opened.accountInclusionRoot,
            accountsLtHashChecksum = opened.accountsLtHashChecksum,
            accountsLtHash = accountsLtHash,
        )

        val request = SccpSolana.buildAccountsLtHashProofRequest(witness, opened)
        val mismatchedWitnessLtHash = accountsLtHash.copyOf()
        mismatchedWitnessLtHash[0] = (mismatchedWitnessLtHash[0].toInt() xor 1).toByte()
        val mismatchedWitnessAccountsLtHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildAccountsLtHashProofRequest(
                witness.copy(accountsLtHash = mismatchedWitnessLtHash),
                opened,
            )
        }
        assertTrue(mismatchedWitnessAccountsLtHash.message?.contains("accountsLtHash") == true)

        assertEquals(1, request.version)
        assertEquals("stark-fri-v1", request.proofFamily)
        assertEquals(SccpSolana.ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1, request.circuitId)
        assertEquals("fastpq-lane-balanced", request.parameterSet)
        assertEquals(SccpSolana.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1, request.sourceStateVerifierId)
        assertEquals("0x" + "aa".repeat(32), request.sourceStateVerifierHash)
        assertEquals(
            SccpSolana.accountsLtHashProofPublicInputsHash(
                finalizedSlot = witness.finalizedSlot,
                parentSlot = witness.parentSlot,
                bankSignatureCount = witness.bankSignatureCount,
                parentBankHash = witness.parentBankHash,
                bankHash = witness.bankHash,
                blockhash = witness.blockhash,
                transactionStatusRoot = witness.transactionStatusRoot,
                accountInclusionRoot = witness.accountInclusionRoot,
                accountsLtHashChecksum = witness.accountsLtHashChecksum,
                accountsLtHash = accountsLtHash,
            ),
            request.accountsLtHashProofPublicInputsHash,
        )
        val wrongPublicInputsBankHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.canonicalAccountsLtHashProofPublicInputsBytes(
                finalizedSlot = witness.finalizedSlot,
                parentSlot = witness.parentSlot,
                bankSignatureCount = witness.bankSignatureCount,
                parentBankHash = witness.parentBankHash,
                bankHash = "44".repeat(32),
                blockhash = witness.blockhash,
                transactionStatusRoot = witness.transactionStatusRoot,
                accountInclusionRoot = witness.accountInclusionRoot,
                accountsLtHashChecksum = witness.accountsLtHashChecksum,
                accountsLtHash = accountsLtHash,
            )
        }
        assertTrue(wrongPublicInputsBankHash.message?.contains("bankHash") == true)
        val wrongPublicInputsChecksum = assertFailsWith<IllegalArgumentException> {
            SccpSolana.accountsLtHashProofPublicInputsHash(
                finalizedSlot = witness.finalizedSlot,
                parentSlot = witness.parentSlot,
                bankSignatureCount = witness.bankSignatureCount,
                parentBankHash = witness.parentBankHash,
                bankHash = witness.bankHash,
                blockhash = witness.blockhash,
                transactionStatusRoot = witness.transactionStatusRoot,
                accountInclusionRoot = witness.accountInclusionRoot,
                accountsLtHashChecksum = "44".repeat(32),
                accountsLtHash = accountsLtHash,
            )
        }
        assertTrue(wrongPublicInputsChecksum.message?.contains("accountsLtHashChecksum") == true)
        assertEquals(SccpSolana.openedAccountsLtHashContributionsHash(opened), request.openedAccountsLtHashContributionsHash)
        assertEquals(SccpSolana.openedAccountsLtHashResidualChecksum(opened), request.openedAccountsLtHashResidualChecksum)
        assertContentEquals(SccpSolana.canonicalAccountsLtHashCommitmentBytes(witness, opened), request.accountCommitmentBytes)
        assertContentEquals(
            SccpSolana.canonicalAccountsLtHashVerificationContextBytes(witness, opened),
            request.verificationContextBytes,
        )
        assertEquals(SccpSolana.accountsLtHashPublicInputColumns(witness, opened), request.publicInputColumns)
        assertEquals(solanaMainnetGenesisPublicInput, request.publicInputColumns[1][0])
        assertEquals(request.openedAccountsLtHashContributionsHash, request.publicInputColumns[12][0])
        assertEquals(request.openedAccountsLtHashResidualChecksum, request.publicInputColumns[13][0])
        assertTrue(String(request.schemaDescriptor).contains("opened_accounts_lt_hash_residual_checksum"))
        assertTrue(String(request.schemaDescriptor).contains("mainnet_genesis_hash"))
        assertTrue(String(request.schemaDescriptor).contains("source_state_verifier_id"))
        assertTrue(String(request.schemaDescriptor).contains(SccpSolana.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1))
        assertTrue(String(request.schemaDescriptor).contains("source_state_verifier_hash"))
        assertTrue(byteArrayContains(request.schemaDescriptor, hexBytes(request.sourceStateVerifierHash)))
        assertEquals(
            listOf(
                "sccp:solana:accounts-lt:v1:statement",
                "sccp:solana:accounts-lt:v1:accounts",
                "sccp:solana:accounts-lt:v1:opened-contributions",
                "sccp:solana:accounts-lt:v1:residual",
                "sccp:solana:accounts-lt:v1:context",
            ),
            request.fastpqTransitions.map { it.key },
        )
        assertEquals("0x" + witness.parentBankHash, request.fastpqPublicInputs.oldRoot)
        assertEquals(witness.bankHash, request.fastpqPublicInputs.newRoot)
        val wrappedProof = SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(1, 2, 3), request)
        assertEquals(request.version, wrappedProof.version)
        assertEquals(request.proofFamily, wrappedProof.proofFamily)
        assertEquals(request.circuitId, wrappedProof.circuitId)
        assertContentEquals(byteArrayOf(1, 2, 3), wrappedProof.proofBytes)
        assertEquals("AQID", wrappedProof.proofBase64)
        val exposedProofBytes = wrappedProof.proofBytes
        exposedProofBytes[0] = 9
        assertContentEquals(byteArrayOf(1, 2, 3), wrappedProof.proofBytes)
        assertEquals("AQID", wrappedProof.proofBase64)
        var seenAccountsRequest: SolanaSccpAccountsLtHashProofRequest? = null
        val sourceStateProver = SolanaSccpSourceStateProver(
            accountsLtHashProofEngine = SolanaSccpAccountsLtHashProofEngine { linkedRequest ->
                seenAccountsRequest = linkedRequest
                assertEquals(SccpSolana.ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1, linkedRequest.circuitId)
                byteArrayOf(1, 2, 3)
            },
        )
        val linkedProof = sourceStateProver.proveAccountsLtHash(request)
        assertTrue(seenAccountsRequest !== request)
        val accountsSnapshot = requireNotNull(seenAccountsRequest)
        assertEquals(request.circuitId, accountsSnapshot.circuitId)
        assertContentEquals(request.statementBytes, accountsSnapshot.statementBytes)
        assertContentEquals(request.accountCommitmentBytes, accountsSnapshot.accountCommitmentBytes)
        assertContentEquals(
            request.fastpqTransitions[0].newValue,
            accountsSnapshot.fastpqTransitions[0].newValue,
        )
        val seenStatementBytes = accountsSnapshot.statementBytes
        seenStatementBytes[0] = 0
        assertContentEquals(request.statementBytes, accountsSnapshot.statementBytes)
        assertEquals(request.circuitId, linkedProof.circuitId)
        assertContentEquals(byteArrayOf(1, 2, 3), linkedProof.proofBytes)
        assertEquals("AQID", linkedProof.proofBase64)
        val missingSourceStateProver = assertFailsWith<IllegalStateException> {
            SolanaSccpSourceStateProver().proveAccountsLtHash(request)
        }
        assertTrue(missingSourceStateProver.message!!.contains("source-state prover is not linked"))
        assertEquals(
            SccpSolana.accountsLtHashProofHash(
                SolanaSccpSourceStateVerificationProof(
                    circuitId = request.circuitId,
                    proofBytes = byteArrayOf(1, 2, 3),
                ),
            ),
            SccpSolana.accountsLtHashProofHash(wrappedProof),
        )
        val allZeroWrappedProof = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(0, 0), request)
        }
        assertTrue(allZeroWrappedProof.message!!.contains("all zero"))
        val oversizedProofBytes = ByteArray(SccpSolana.SOURCE_STATE_MAX_PROOF_BYTES + 1) { 1 }
        val oversizedWrappedProof = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(oversizedProofBytes, request)
        }
        assertTrue(oversizedWrappedProof.message!!.contains("at most"))
        val oversizedCanonicalProof = assertFailsWith<IllegalArgumentException> {
            SccpSolana.canonicalSourceStateVerificationProofBytes(
                SolanaSccpSourceStateVerificationProof(
                    circuitId = request.circuitId,
                    proofBytes = oversizedProofBytes,
                ),
            )
        }
        assertTrue(oversizedCanonicalProof.message!!.contains("at most"))
        val oversizedProofFamily = assertFailsWith<IllegalArgumentException> {
            SccpSolana.canonicalSourceStateVerificationProofBytes(
                SolanaSccpSourceStateVerificationProof(
                    proofFamily = "x".repeat(SccpSolana.SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
                    circuitId = request.circuitId,
                    proofBytes = byteArrayOf(1),
                ),
            )
        }
        assertTrue(oversizedProofFamily.message!!.contains("proofFamily"))
        val oversizedCircuitId = assertFailsWith<IllegalArgumentException> {
            SccpSolana.canonicalSourceStateVerificationProofBytes(
                SolanaSccpSourceStateVerificationProof(
                    circuitId = "x".repeat(SccpSolana.SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
                    proofBytes = byteArrayOf(1),
                ),
            )
        }
        assertTrue(oversizedCircuitId.message!!.contains("circuitId"))
        val wrongGenesisColumns = request.publicInputColumns.map { it.toMutableList() }.toMutableList()
        wrongGenesisColumns[1][0] = "0x" + "aa".repeat(32)
        val wrongGenesisRequest = SolanaSccpAccountsLtHashProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            sourceDomain = request.sourceDomain,
            finalizedSlot = request.finalizedSlot,
            parentSlot = request.parentSlot,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash = request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum = request.openedAccountsLtHashResidualChecksum,
            statementBytes = request.statementBytes,
            accountCommitmentBytes = request.accountCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = wrongGenesisColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = request.fastpqTransitions,
        )
        val wrongGenesisError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(1), wrongGenesisRequest)
        }
        assertTrue(wrongGenesisError.message!!.contains("mainnet_genesis_hash"))
        val wrongResidualColumns = request.publicInputColumns.map { it.toMutableList() }.toMutableList()
        wrongResidualColumns[13][0] = "0x" + "cc".repeat(32)
        val wrongResidualRequest = SolanaSccpAccountsLtHashProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            sourceDomain = request.sourceDomain,
            finalizedSlot = request.finalizedSlot,
            parentSlot = request.parentSlot,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash = request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum = request.openedAccountsLtHashResidualChecksum,
            statementBytes = request.statementBytes,
            accountCommitmentBytes = request.accountCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = wrongResidualColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = request.fastpqTransitions,
        )
        val wrongResidualError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(1), wrongResidualRequest)
        }
        assertTrue(wrongResidualError.message!!.contains("opened_accounts_lt_hash_residual_checksum"))
        val staleAccountsHashError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(
                byteArrayOf(1),
                accountsLtHashRequest(
                    request,
                    accountsLtHashProofPublicInputsHash = "0x" + "cc".repeat(32),
                ),
            )
        }
        assertTrue(staleAccountsHashError.message!!.contains("accountsLtHashProofPublicInputsHash"))
        val wrongAccountsDsidError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(
                byteArrayOf(1),
                accountsLtHashRequest(
                    request,
                    fastpqPublicInputs = request.fastpqPublicInputs.copy(dsid = "0x" + "00".repeat(16)),
                ),
            )
        }
        assertTrue(wrongAccountsDsidError.message!!.contains("request.fastpqPublicInputs.dsid"))
        val wrongAccountsTxSetError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(
                byteArrayOf(1),
                accountsLtHashRequest(
                    request,
                    fastpqPublicInputs = request.fastpqPublicInputs.copy(txSetHash = "0x" + "cc".repeat(32)),
                ),
            )
        }
        assertTrue(wrongAccountsTxSetError.message!!.contains("request.fastpqPublicInputs.txSetHash"))
        val wrongTransitions = request.fastpqTransitions.mapIndexed { index, transition ->
            if (index == 0) {
                SolanaSccpAccountsLtHashFastpqTransition(
                    key = transition.key,
                    operation = transition.operation,
                    oldValue = transition.oldValue,
                    newValue = byteArrayOf(0),
                )
            } else {
                transition
            }
        }
        val wrongTransitionRequest = SolanaSccpAccountsLtHashProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            sourceDomain = request.sourceDomain,
            finalizedSlot = request.finalizedSlot,
            parentSlot = request.parentSlot,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash = request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum = request.openedAccountsLtHashResidualChecksum,
            statementBytes = request.statementBytes,
            accountCommitmentBytes = request.accountCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = wrongTransitions,
        )
        val wrongTransitionError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(1), wrongTransitionRequest)
        }
        assertTrue(wrongTransitionError.message!!.contains("canonical Solana source-state request"))
        val wrongOldValueTransitions = request.fastpqTransitions.mapIndexed { index, transition ->
            if (index == 0) {
                SolanaSccpAccountsLtHashFastpqTransition(
                    key = transition.key,
                    operation = transition.operation,
                    oldValue = byteArrayOf(0),
                    newValue = transition.newValue,
                )
            } else {
                transition
            }
        }
        val wrongOldValueRequest = SolanaSccpAccountsLtHashProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            sourceDomain = request.sourceDomain,
            finalizedSlot = request.finalizedSlot,
            parentSlot = request.parentSlot,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash = request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum = request.openedAccountsLtHashResidualChecksum,
            statementBytes = request.statementBytes,
            accountCommitmentBytes = request.accountCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = wrongOldValueTransitions,
        )
        val wrongOldValueError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(1), wrongOldValueRequest)
        }
        assertTrue(wrongOldValueError.message!!.contains("canonical Solana source-state request"))
        val missingStatementRequest = SolanaSccpAccountsLtHashProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            sourceDomain = request.sourceDomain,
            finalizedSlot = request.finalizedSlot,
            parentSlot = request.parentSlot,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash = request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum = request.openedAccountsLtHashResidualChecksum,
            statementBytes = ByteArray(0),
            accountCommitmentBytes = request.accountCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = request.fastpqTransitions,
        )
        val missingStatementError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(1), missingStatementRequest)
        }
        assertTrue(missingStatementError.message!!.contains("request.statementBytes"))
        var rejectedAccountsCallbackRan = false
        val guardingAccountsProver = SolanaSccpSourceStateProver(
            accountsLtHashProofEngine = SolanaSccpAccountsLtHashProofEngine {
                rejectedAccountsCallbackRan = true
                byteArrayOf(1)
            },
        )
        val rejectedAccountsRequest = assertFailsWith<IllegalArgumentException> {
            guardingAccountsProver.proveAccountsLtHash(missingStatementRequest)
        }
        assertTrue(rejectedAccountsRequest.message!!.contains("request.statementBytes"))
        assertFalse(rejectedAccountsCallbackRan)
        val originalStatementBytes = request.statementBytes
        request.statementBytes[0] = 0
        assertContentEquals(originalStatementBytes, request.statementBytes)
        val originalTransitionValue = request.fastpqTransitions[0].newValue
        request.fastpqTransitions[0].newValue[0] = 0
        assertContentEquals(originalTransitionValue, request.fastpqTransitions[0].newValue)

        val zeroHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildAccountsLtHashProofRequest(
                witness.copy(sourceStateVerifierHash = SccpSolana.ZERO_HASH_V1),
                opened,
            )
        }
        assertTrue(zeroHash.message?.contains("sourceStateVerifierHash") == true)
        val templateHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildAccountsLtHashProofRequest(
                witness.copy(sourceStateVerifierHash = SccpSolana.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1),
                opened,
            )
        }
        assertTrue(templateHash.message?.contains("Solana template verifier hash") == true)
        val badBankHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildAccountsLtHashProofRequest(witness.copy(bankHash = "cc".repeat(32)), opened)
        }
        assertTrue(badBankHash.message?.contains("bankHash") == true)
    }

    @Test
    fun buildsSolanaFullLightClientAuditRoleProofRequests() {
        val voteOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x31.toByte() },
            owner = hexBytes(SccpSolana.VOTE_PROGRAM_ID),
            lamports = "1000000",
            rentEpoch = "0",
            dataHash = "0x" + "91".repeat(32),
        )
        val stakeOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x32.toByte() },
            owner = hexBytes(SccpSolana.STAKE_PROGRAM_ID),
            lamports = "2000000",
            rentEpoch = "0",
            dataHash = "0x" + "92".repeat(32),
        )
        val stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address = hexBytes(SccpSolana.STAKE_HISTORY_SYSVAR_ID),
            owner = hexBytes(SccpSolana.SYSVAR_PROGRAM_ID),
            lamports = "1",
            rentEpoch = "0",
            dataHash = "0x" + "93".repeat(32),
        )
        val unopenedOpening = SolanaSccpAccountOpeningInput(
            address = ByteArray(32) { 0x34.toByte() },
            owner = hexBytes(SccpSolana.STAKE_PROGRAM_ID),
            lamports = "3000000",
            rentEpoch = "0",
            dataHash = "0x" + "94".repeat(32),
        )
        val voteRawData = byteArrayOf(1, 2, 3)
        val stakeRawData = byteArrayOf(4, 5, 6)
        val stakeHistoryRawData = byteArrayOf(7, 8, 9)
        val unopenedRawData = byteArrayOf(10, 11, 12)
        val accountsLtHash = SccpSolana.accountsLtHashFromOpenings(
            listOf(voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening),
            listOf(voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData),
        )
        val parentBankHash = "c0".repeat(32)
        val blockhash = "42".repeat(32)
        val bankHash = SccpSolana.agaveBankHash(
            parentBankHash = parentBankHash,
            bankSignatureCount = "8",
            blockhash = blockhash,
            accountsLtHash = accountsLtHash,
        )
        val opened = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot = "1296096",
            accountInclusionRoot = "0x" + "77".repeat(32),
            accountsLtHashChecksum = SccpSolana.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash = accountsLtHash,
            validatorVoteAccountOpenings = listOf(voteOpening),
            validatorVoteAccountRawData = listOf(voteRawData),
            validatorStakeAccountOpenings = listOf(stakeOpening),
            validatorStakeAccountRawData = listOf(stakeRawData),
            stakeHistorySysvarOpening = stakeHistoryOpening,
            stakeHistorySysvarRawData = stakeHistoryRawData,
        )
        val sourceStateVerifierHash = "99".repeat(32)
        val sourceTrustAnchorHash = "44".repeat(32)
        val consensusVerifierHash = "55".repeat(32)
        val messageInclusionVerifierHash = "66".repeat(32)
        val finalityPolicyHash = "88".repeat(32)
        val deploymentReceiptHash = "aa".repeat(32)
        val towerVerifierHash = "b1".repeat(32)
        val accountsdbVerifierHash = "c2".repeat(32)
        val bankVerifierHash = "d3".repeat(32)
        val sourceVerifierMaterialHash = SccpSourceProofs.sourceVerifierMaterialHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SOL,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
        )
        val sourceAdapterDeploymentHash = SccpSourceProofs.sourceAdapterEngineDeploymentHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SOL,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            solanaTowerReplayVerifierHash = towerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash = bankVerifierHash,
        )
        val fullLightClientGateHash = SccpSourceProofs.solanaFullLightClientGateHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SOL,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            solanaTowerReplayVerifierHash = towerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash = bankVerifierHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
        )
        val duplicatedGateAuditHash = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.solanaFullLightClientGateHash(
                sourceDomain = SccpSourceProofs.DOMAIN_SOL,
                sourceTrustAnchorHash = sourceTrustAnchorHash,
                consensusVerifierHash = consensusVerifierHash,
                messageInclusionVerifierHash = messageInclusionVerifierHash,
                finalityPolicyHash = finalityPolicyHash,
                deploymentReceiptHash = deploymentReceiptHash,
                solanaTowerReplayVerifierHash = accountsdbVerifierHash,
                solanaFullAccountsdbLatticeVerifierHash = accountsdbVerifierHash,
                solanaBankForkChoiceVerifierHash = bankVerifierHash,
                sourceStateVerifierHash = sourceStateVerifierHash,
            )
        }
        assertTrue(duplicatedGateAuditHash.message!!.contains("role-separated"))
        val reusedGateDeploymentReceiptHash = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.solanaFullLightClientGateHash(
                sourceDomain = SccpSourceProofs.DOMAIN_SOL,
                sourceTrustAnchorHash = sourceTrustAnchorHash,
                consensusVerifierHash = consensusVerifierHash,
                messageInclusionVerifierHash = messageInclusionVerifierHash,
                finalityPolicyHash = finalityPolicyHash,
                deploymentReceiptHash = deploymentReceiptHash,
                solanaTowerReplayVerifierHash = deploymentReceiptHash,
                solanaFullAccountsdbLatticeVerifierHash = accountsdbVerifierHash,
                solanaBankForkChoiceVerifierHash = bankVerifierHash,
                sourceStateVerifierHash = sourceStateVerifierHash,
            )
        }
        assertTrue(reusedGateDeploymentReceiptHash.message!!.contains("source-adapter material"))
        val witness = sampleWitness(
            sourceStateVerifierHash = sourceStateVerifierHash,
            sourceAdapterDeploymentHash = sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash = deploymentReceiptHash,
        ).copy(
            finalizedSlot = "1296096",
            parentSlot = "1296095",
            parentBankHash = parentBankHash,
            blockhash = blockhash,
            bankHash = bankHash,
            accountInclusionRoot = opened.accountInclusionRoot,
            accountsLtHashChecksum = opened.accountsLtHashChecksum,
            accountsLtHash = accountsLtHash,
        )
        val input = SolanaSccpFullLightClientAuditProofInput(
            witnessInput = witness,
            openedAccounts = opened,
            accountsLtHashProof = SolanaSccpSourceStateVerificationProof(
                circuitId = SccpSolana.ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
                proofBytes = byteArrayOf(1, 2, 3, 4),
            ),
            rootedSlot = "1296065",
            towerVoteSlots = (1296066..1296096).map { it.toString() },
            epochStakeRoot = "13".repeat(32),
            stakeActivationHash = "14".repeat(32),
            stakeAccountStateHash = "15".repeat(32),
            stakeHistoryHash = "16".repeat(32),
            stakeHistorySysvarAccountHash = "17".repeat(32),
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceAdapterDeploymentReceiptHash = deploymentReceiptHash,
            solanaTowerReplayVerifierHash = towerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash = bankVerifierHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = sourceAdapterDeploymentHash,
            fullLightClientGateHash = fullLightClientGateHash,
        )

        val requests = SccpSolana.buildFullLightClientAuditProofRequests(input)
        val mismatchedAuditWitnessLtHash = accountsLtHash.copyOf()
        mismatchedAuditWitnessLtHash[0] = (mismatchedAuditWitnessLtHash[0].toInt() xor 1).toByte()
        val mismatchedAuditAccountsLtHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(witnessInput = witness.copy(accountsLtHash = mismatchedAuditWitnessLtHash)),
            )
        }
        assertTrue(mismatchedAuditAccountsLtHash.message?.contains("accountsLtHash") == true)
        val requestHashReusedTowerVerifierHash = requests.towerReplay.auditStatementHash
        val requestHashReusedDeploymentHash = SccpSourceProofs.sourceAdapterEngineDeploymentHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SOL,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            solanaTowerReplayVerifierHash = requestHashReusedTowerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash = bankVerifierHash,
        )
        val requestHashReusedGateHash = SccpSourceProofs.solanaFullLightClientGateHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SOL,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            solanaTowerReplayVerifierHash = requestHashReusedTowerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash = bankVerifierHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
        )
        val requestHashReused = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildTowerReplayProofRequest(
                input.copy(
                    witnessInput = witness.copy(sourceAdapterDeploymentHash = requestHashReusedDeploymentHash),
                    solanaTowerReplayVerifierHash = requestHashReusedTowerVerifierHash,
                    sourceAdapterDeploymentHash = requestHashReusedDeploymentHash,
                    fullLightClientGateHash = requestHashReusedGateHash,
                ),
            )
        }
        assertTrue(requestHashReused.message!!.contains("role-separated"))
        val expectedTowerReplayColumns = listOf(
            listOf("0x0100000000000000000000000000000000000000000000000000000000000000"),
            listOf("0x0300000000000000000000000000000000000000000000000000000000000000"),
            listOf(solanaMainnetGenesisPublicInput),
            listOf("0xe0c6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"),
            listOf("0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3"),
            listOf("0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"),
            listOf("0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"),
            listOf("0x8584e42713d21415970ceb9d51b71c6b7a5999093b3d6cec84dfc13a38e47c0f"),
            listOf("0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1"),
            listOf("0x0300000000000000000000000000000000000000000000000000000000000000"),
            listOf("0xc1c6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0xdfc6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"),
            listOf("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"),
            listOf("0x922a426e06d6263986a0c9ff0f956f5429288c9c1310cb67fbaf30918de58b40"),
            listOf("0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"),
            listOf("0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"),
            listOf("0x1313131313131313131313131313131313131313131313131313131313131313"),
            listOf("0x1414141414141414141414141414141414141414141414141414141414141414"),
            listOf("0x1515151515151515151515151515151515151515151515151515151515151515"),
            listOf("0x1616161616161616161616161616161616161616161616161616161616161616"),
            listOf("0x1717171717171717171717171717171717171717171717171717171717171717"),
            listOf("0x7777777777777777777777777777777777777777777777777777777777777777"),
        )
        val expectedAccountsdbColumns = listOf(
            listOf("0x0200000000000000000000000000000000000000000000000000000000000000"),
            listOf("0x0300000000000000000000000000000000000000000000000000000000000000"),
            listOf(solanaMainnetGenesisPublicInput),
            listOf("0xe0c6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"),
            listOf("0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0"),
            listOf("0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"),
            listOf("0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"),
            listOf("0x8584e42713d21415970ceb9d51b71c6b7a5999093b3d6cec84dfc13a38e47c0f"),
            listOf("0xc2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"),
            listOf("0x0300000000000000000000000000000000000000000000000000000000000000"),
            listOf("0xc1c6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0xdfc6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"),
            listOf("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"),
            listOf("0x7777777777777777777777777777777777777777777777777777777777777777"),
            listOf("0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"),
            listOf("0xc1b7c880344a2551d0842848f68b8519027e8b228a4c92c4e754141821d63810"),
            listOf("0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9"),
            listOf("0x336bb79a5e96c331ddca555aedde346438de4ca1b227ae09f7faaa5e0e455be0"),
            listOf("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"),
        )
        val expectedBankForkChoiceColumns = listOf(
            listOf("0x0300000000000000000000000000000000000000000000000000000000000000"),
            listOf("0x0300000000000000000000000000000000000000000000000000000000000000"),
            listOf(solanaMainnetGenesisPublicInput),
            listOf("0xe0c6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"),
            listOf("0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8"),
            listOf("0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"),
            listOf("0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"),
            listOf("0x8584e42713d21415970ceb9d51b71c6b7a5999093b3d6cec84dfc13a38e47c0f"),
            listOf("0xd3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3"),
            listOf("0x0300000000000000000000000000000000000000000000000000000000000000"),
            listOf("0xc1c6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0xdfc6130000000000000000000000000000000000000000000000000000000000"),
            listOf("0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"),
            listOf("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"),
            listOf("0xc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0"),
            listOf("0x46bf9f58208a9c61b931640824eb13d636d3af5b0268cce866c958367bd6a451"),
            listOf("0x4242424242424242424242424242424242424242424242424242424242424242"),
            listOf("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            listOf("0x7777777777777777777777777777777777777777777777777777777777777777"),
            listOf("0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"),
            listOf("0x0800000000000000000000000000000000000000000000000000000000000000"),
            listOf("0x1d2a51ef7c068fe46c9f588c252ce9cea8b66d87453bf73c9920005802e738bc"),
            listOf("0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"),
            listOf("0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"),
        )

        assertEquals(SccpSolana.TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1, requests.towerReplay.circuitId)
        assertEquals(
            "0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3",
            requests.towerReplay.auditStatementHash,
        )
        assertEquals(777, requests.towerReplay.statementBytes.size)
        assertEquals(expectedTowerReplayColumns, requests.towerReplay.publicInputColumns)
        assertEquals(
            SccpSolana.FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
            requests.fullAccountsdbLattice.circuitId,
        )
        assertEquals(
            "0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0",
            requests.fullAccountsdbLattice.auditStatementHash,
        )
        assertEquals(440, requests.fullAccountsdbLattice.statementBytes.size)
        assertEquals(expectedAccountsdbColumns, requests.fullAccountsdbLattice.publicInputColumns)
        assertEquals(SccpSolana.BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1, requests.bankForkChoice.circuitId)
        assertEquals(
            "0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8",
            requests.bankForkChoice.auditStatementHash,
        )
        assertEquals(509, requests.bankForkChoice.statementBytes.size)
        assertEquals(expectedBankForkChoiceColumns, requests.bankForkChoice.publicInputColumns)
        assertEquals(listOf(input.witnessInput.accountInclusionRoot), requests.bankForkChoice.publicInputColumns[19])
        assertTrue(requests.towerReplay.schemaDescriptor.decodeToString().contains("mainnet_genesis_hash"))
        assertEquals(
            listOf("0x1515151515151515151515151515151515151515151515151515151515151515"),
            requests.towerReplay.publicInputColumns[20],
        )
        assertEquals(
            listOf("0x1717171717171717171717171717171717171717171717171717171717171717"),
            requests.towerReplay.publicInputColumns[22],
        )
        assertEquals(listOf(input.witnessInput.accountInclusionRoot), requests.towerReplay.publicInputColumns[23])
        assertTrue(requests.towerReplay.schemaDescriptor.decodeToString().contains("stake_account_state_hash"))
        assertTrue(requests.towerReplay.schemaDescriptor.decodeToString().contains("stake_history_sysvar_account_hash"))
        assertTrue(requests.towerReplay.schemaDescriptor.decodeToString().contains("account_inclusion_root"))
        assertTrue(requests.bankForkChoice.schemaDescriptor.decodeToString().contains("account_inclusion_root"))
        assertTrue(requests.bankForkChoice.schemaDescriptor.decodeToString().contains("bank_hash_hard_fork_data_hash"))
        assertEquals(3, listOf(
            requests.towerReplay.auditStatementHash,
            requests.fullAccountsdbLattice.auditStatementHash,
            requests.bankForkChoice.auditStatementHash,
        ).toSet().size)
        assertEquals(fullLightClientGateHash, requests.towerReplay.fullLightClientGateHash)
        assertEquals(SccpSolana.fullLightClientAuditFinalityContextHash(input), requests.towerReplay.finalityContextHash)
        assertEquals(SccpSolana.fullLightClientAuditVoteMessageHash(input), requests.towerReplay.voteMessageHash)
        assertEquals(SccpSolana.accountsLtHashProofHash(input.accountsLtHashProof), requests.towerReplay.accountsLtHashProofHash)
        listOf(requests.towerReplay, requests.fullAccountsdbLattice, requests.bankForkChoice).forEach { request ->
            val proofCapsule = SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), request)
            assertEquals(request.version, proofCapsule.version)
            assertEquals(request.proofFamily, proofCapsule.proofFamily)
            assertEquals(request.circuitId, proofCapsule.circuitId)
            assertContentEquals(byteArrayOf(9, 8, 7), proofCapsule.proofBytes)
            assertEquals("CQgH", proofCapsule.proofBase64)
            assertTrue(SccpSolana.canonicalSourceStateVerificationProofBytes(proofCapsule).isNotEmpty())
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.accountsLtHashProofHash(proofCapsule)
            }
            val exposedProofBytes = proofCapsule.proofBytes
            exposedProofBytes[0] = 1
            assertContentEquals(byteArrayOf(9, 8, 7), proofCapsule.proofBytes)
            assertEquals("CQgH", proofCapsule.proofBase64)
        }
        val seenRoles = mutableListOf<String>()
        val sourceStateProver = SolanaSccpSourceStateProver(
            fullLightClientAuditProofEngine = SolanaSccpFullLightClientAuditProofEngine { request ->
                seenRoles.add(request.role)
                byteArrayOf(9, 8, 7)
            },
        )
        val linkedProofs = sourceStateProver.proveFullLightClientAudit(input)
        assertEquals(listOf("tower_replay", "full_accountsdb_lattice", "bank_fork_choice"), seenRoles)
        assertEquals(SccpSolana.TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1, linkedProofs.towerReplay.circuitId)
        assertEquals(
            SccpSolana.FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
            linkedProofs.fullAccountsdbLattice.circuitId,
        )
        assertEquals(SccpSolana.BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1, linkedProofs.bankForkChoice.circuitId)
        assertEquals("CQgH", linkedProofs.bankForkChoice.proofBase64)
        var seenAuditRequest: SolanaSccpFullLightClientAuditProofRequest? = null
        val snapshotCheckingProver = SolanaSccpSourceStateProver(
            fullLightClientAuditProofEngine = SolanaSccpFullLightClientAuditProofEngine { request ->
                seenAuditRequest = request
                byteArrayOf(9, 8, 7)
            },
        )
        snapshotCheckingProver.proveFullLightClientAudit(requests.towerReplay)
        assertTrue(seenAuditRequest !== requests.towerReplay)
        val auditSnapshot = requireNotNull(seenAuditRequest)
        assertContentEquals(requests.towerReplay.statementBytes, auditSnapshot.statementBytes)
        assertContentEquals(
            requests.towerReplay.verificationContextBytes,
            auditSnapshot.verificationContextBytes,
        )
        assertContentEquals(
            requests.towerReplay.fastpqTransitions[0].newValue,
            auditSnapshot.fastpqTransitions[0].newValue,
        )
        val seenAuditStatementBytes = auditSnapshot.statementBytes
        seenAuditStatementBytes[0] = 0
        assertContentEquals(requests.towerReplay.statementBytes, auditSnapshot.statementBytes)
        val missingSourceStateProver = assertFailsWith<IllegalStateException> {
            SolanaSccpSourceStateProver().proveFullLightClientAudit(input)
        }
        assertTrue(missingSourceStateProver.message!!.contains("source-state prover is not linked"))
        val wrongAuditGenesisColumns =
            requests.bankForkChoice.publicInputColumns.map { it.toMutableList() }.toMutableList()
        wrongAuditGenesisColumns[2][0] = "0x" + "aa".repeat(32)
        val wrongAuditGenesisRequest = SolanaSccpFullLightClientAuditProofRequest(
            version = requests.bankForkChoice.version,
            proofFamily = requests.bankForkChoice.proofFamily,
            circuitId = requests.bankForkChoice.circuitId,
            parameterSet = requests.bankForkChoice.parameterSet,
            role = requests.bankForkChoice.role,
            roleCode = requests.bankForkChoice.roleCode,
            sourceDomain = requests.bankForkChoice.sourceDomain,
            finalizedSlot = requests.bankForkChoice.finalizedSlot,
            verifierId = requests.bankForkChoice.verifierId,
            verifierHash = requests.bankForkChoice.verifierHash,
            sourceStateVerifierId = requests.bankForkChoice.sourceStateVerifierId,
            sourceStateVerifierHash = requests.bankForkChoice.sourceStateVerifierHash,
            sourceVerifierMaterialHash = requests.bankForkChoice.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = requests.bankForkChoice.sourceAdapterDeploymentHash,
            fullLightClientGateHash = requests.bankForkChoice.fullLightClientGateHash,
            finalityContextHash = requests.bankForkChoice.finalityContextHash,
            voteMessageHash = requests.bankForkChoice.voteMessageHash,
            accountsLtHashProofHash = requests.bankForkChoice.accountsLtHashProofHash,
            auditStatementHash = requests.bankForkChoice.auditStatementHash,
            statementBytes = requests.bankForkChoice.statementBytes,
            verificationContextBytes = requests.bankForkChoice.verificationContextBytes,
            schemaDescriptor = requests.bankForkChoice.schemaDescriptor,
            publicInputColumns = wrongAuditGenesisColumns,
            fastpqPublicInputs = requests.bankForkChoice.fastpqPublicInputs,
            fastpqTransitions = requests.bankForkChoice.fastpqTransitions,
        )
        val wrongAuditGenesisError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), wrongAuditGenesisRequest)
        }
        assertTrue(wrongAuditGenesisError.message!!.contains("mainnet_genesis_hash"))
        val wrongAuditStatementColumns =
            requests.towerReplay.publicInputColumns.map { it.toMutableList() }.toMutableList()
        wrongAuditStatementColumns[5][0] = "0x" + "cc".repeat(32)
        val wrongAuditStatementRequest = SolanaSccpFullLightClientAuditProofRequest(
            version = requests.towerReplay.version,
            proofFamily = requests.towerReplay.proofFamily,
            circuitId = requests.towerReplay.circuitId,
            parameterSet = requests.towerReplay.parameterSet,
            role = requests.towerReplay.role,
            roleCode = requests.towerReplay.roleCode,
            sourceDomain = requests.towerReplay.sourceDomain,
            finalizedSlot = requests.towerReplay.finalizedSlot,
            verifierId = requests.towerReplay.verifierId,
            verifierHash = requests.towerReplay.verifierHash,
            sourceStateVerifierId = requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash = requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash = requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash = requests.towerReplay.fullLightClientGateHash,
            finalityContextHash = requests.towerReplay.finalityContextHash,
            voteMessageHash = requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash = requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash = requests.towerReplay.auditStatementHash,
            statementBytes = requests.towerReplay.statementBytes,
            verificationContextBytes = requests.towerReplay.verificationContextBytes,
            schemaDescriptor = requests.towerReplay.schemaDescriptor,
            publicInputColumns = wrongAuditStatementColumns,
            fastpqPublicInputs = requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions = requests.towerReplay.fastpqTransitions,
        )
        val wrongAuditStatementError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), wrongAuditStatementRequest)
        }
        assertTrue(wrongAuditStatementError.message!!.contains("audit_statement_hash"))
        val staleAuditHashError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(
                byteArrayOf(9, 8, 7),
                fullLightClientAuditRequest(
                    requests.towerReplay,
                    auditStatementHash = "0x" + "cc".repeat(32),
                ),
            )
        }
        assertTrue(staleAuditHashError.message!!.contains("request.auditStatementHash"))
        val wrongAuditDsidError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(
                byteArrayOf(9, 8, 7),
                fullLightClientAuditRequest(
                    requests.towerReplay,
                    fastpqPublicInputs = requests.towerReplay.fastpqPublicInputs.copy(dsid = "0x" + "00".repeat(16)),
                ),
            )
        }
        assertTrue(wrongAuditDsidError.message!!.contains("request.fastpqPublicInputs.dsid"))
        val wrongAuditTxSetError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(
                byteArrayOf(9, 8, 7),
                fullLightClientAuditRequest(
                    requests.towerReplay,
                    fastpqPublicInputs = requests.towerReplay.fastpqPublicInputs.copy(txSetHash = "0x" + "cc".repeat(32)),
                ),
            )
        }
        assertTrue(wrongAuditTxSetError.message!!.contains("request.fastpqPublicInputs.txSetHash"))
        val reusedSourceStateVerifierError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(
                byteArrayOf(9, 8, 7),
                fullLightClientAuditRequest(
                    requests.towerReplay,
                    verifierHash = requests.towerReplay.sourceStateVerifierHash,
                ),
            )
        }
        assertTrue(reusedSourceStateVerifierError.message!!.contains("role-separated"))
        val wrongAuditTransitions = requests.towerReplay.fastpqTransitions.mapIndexed { index, transition ->
            if (index == 0) {
                SolanaSccpFullLightClientAuditFastpqTransition(
                    key = transition.key,
                    operation = transition.operation,
                    oldValue = transition.oldValue,
                    newValue = byteArrayOf(0),
                )
            } else {
                transition
            }
        }
        val wrongAuditTransitionRequest = SolanaSccpFullLightClientAuditProofRequest(
            version = requests.towerReplay.version,
            proofFamily = requests.towerReplay.proofFamily,
            circuitId = requests.towerReplay.circuitId,
            parameterSet = requests.towerReplay.parameterSet,
            role = requests.towerReplay.role,
            roleCode = requests.towerReplay.roleCode,
            sourceDomain = requests.towerReplay.sourceDomain,
            finalizedSlot = requests.towerReplay.finalizedSlot,
            verifierId = requests.towerReplay.verifierId,
            verifierHash = requests.towerReplay.verifierHash,
            sourceStateVerifierId = requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash = requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash = requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash = requests.towerReplay.fullLightClientGateHash,
            finalityContextHash = requests.towerReplay.finalityContextHash,
            voteMessageHash = requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash = requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash = requests.towerReplay.auditStatementHash,
            statementBytes = requests.towerReplay.statementBytes,
            verificationContextBytes = requests.towerReplay.verificationContextBytes,
            schemaDescriptor = requests.towerReplay.schemaDescriptor,
            publicInputColumns = requests.towerReplay.publicInputColumns,
            fastpqPublicInputs = requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions = wrongAuditTransitions,
        )
        val wrongAuditTransitionError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), wrongAuditTransitionRequest)
        }
        assertTrue(wrongAuditTransitionError.message!!.contains("canonical Solana source-state request"))
        val wrongAuditOldValueTransitions = requests.towerReplay.fastpqTransitions.mapIndexed { index, transition ->
            if (index == 0) {
                SolanaSccpFullLightClientAuditFastpqTransition(
                    key = transition.key,
                    operation = transition.operation,
                    oldValue = byteArrayOf(0),
                    newValue = transition.newValue,
                )
            } else {
                transition
            }
        }
        val wrongAuditOldValueRequest = SolanaSccpFullLightClientAuditProofRequest(
            version = requests.towerReplay.version,
            proofFamily = requests.towerReplay.proofFamily,
            circuitId = requests.towerReplay.circuitId,
            parameterSet = requests.towerReplay.parameterSet,
            role = requests.towerReplay.role,
            roleCode = requests.towerReplay.roleCode,
            sourceDomain = requests.towerReplay.sourceDomain,
            finalizedSlot = requests.towerReplay.finalizedSlot,
            verifierId = requests.towerReplay.verifierId,
            verifierHash = requests.towerReplay.verifierHash,
            sourceStateVerifierId = requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash = requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash = requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash = requests.towerReplay.fullLightClientGateHash,
            finalityContextHash = requests.towerReplay.finalityContextHash,
            voteMessageHash = requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash = requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash = requests.towerReplay.auditStatementHash,
            statementBytes = requests.towerReplay.statementBytes,
            verificationContextBytes = requests.towerReplay.verificationContextBytes,
            schemaDescriptor = requests.towerReplay.schemaDescriptor,
            publicInputColumns = requests.towerReplay.publicInputColumns,
            fastpqPublicInputs = requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions = wrongAuditOldValueTransitions,
        )
        val wrongAuditOldValueError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), wrongAuditOldValueRequest)
        }
        assertTrue(wrongAuditOldValueError.message!!.contains("canonical Solana source-state request"))
        val malformedAuditRequest = SolanaSccpFullLightClientAuditProofRequest(
            version = requests.towerReplay.version,
            proofFamily = requests.towerReplay.proofFamily,
            circuitId = requests.towerReplay.circuitId,
            parameterSet = requests.towerReplay.parameterSet,
            role = requests.towerReplay.role,
            roleCode = requests.towerReplay.roleCode,
            sourceDomain = requests.towerReplay.sourceDomain,
            finalizedSlot = requests.towerReplay.finalizedSlot,
            verifierId = requests.towerReplay.verifierId,
            verifierHash = requests.towerReplay.verifierHash,
            sourceStateVerifierId = requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash = requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash = requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash = requests.towerReplay.fullLightClientGateHash,
            finalityContextHash = requests.towerReplay.finalityContextHash,
            voteMessageHash = requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash = requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash = requests.towerReplay.auditStatementHash,
            statementBytes = requests.towerReplay.statementBytes,
            verificationContextBytes = requests.towerReplay.verificationContextBytes,
            schemaDescriptor = requests.towerReplay.schemaDescriptor,
            publicInputColumns = emptyList(),
            fastpqPublicInputs = requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions = requests.towerReplay.fastpqTransitions,
        )
        val malformedAuditError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), malformedAuditRequest)
        }
        assertTrue(malformedAuditError.message!!.contains("request.publicInputColumns"))
        var rejectedAuditCallbackRan = false
        val guardingAuditProver = SolanaSccpSourceStateProver(
            fullLightClientAuditProofEngine = SolanaSccpFullLightClientAuditProofEngine {
                rejectedAuditCallbackRan = true
                byteArrayOf(9, 8, 7)
            },
        )
        val rejectedAuditRequest = assertFailsWith<IllegalArgumentException> {
            guardingAuditProver.proveFullLightClientAudit(malformedAuditRequest)
        }
        assertTrue(rejectedAuditRequest.message!!.contains("request.publicInputColumns"))
        assertFalse(rejectedAuditCallbackRan)
        val allZeroAccountsLtHashProof = assertFailsWith<IllegalArgumentException> {
            SccpSolana.accountsLtHashProofHash(input.accountsLtHashProof.copy(proofBytes = ByteArray(3)))
        }
        assertTrue(allZeroAccountsLtHashProof.message!!.contains("all zero"))
        val wrongAccountsLtHashProofVersion = assertFailsWith<IllegalArgumentException> {
            SccpSolana.canonicalSourceStateVerificationProofBytes(input.accountsLtHashProof.copy(version = 0))
        }
        assertTrue(wrongAccountsLtHashProofVersion.message!!.contains("version"))
        val proofBytes = byteArrayOf(4, 5, 6)
        val copiedProof = SolanaSccpSourceStateVerificationProof(
            circuitId = SccpSolana.ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
            proofBytes = proofBytes,
        )
        proofBytes[0] = 0
        copiedProof.proofBytes[1] = 0
        assertContentEquals(byteArrayOf(4, 5, 6), copiedProof.proofBytes)
        assertTrue(String(requests.fullAccountsdbLattice.schemaDescriptor).contains("full_light_client_gate_hash"))
        assertTrue(requests.bankForkChoice.fastpqTransitions.all { it.key.startsWith("0x") })
        val originalAuditStatement = requests.towerReplay.statementBytes
        requests.towerReplay.statementBytes[0] = 0
        assertContentEquals(originalAuditStatement, requests.towerReplay.statementBytes)
        val originalAuditTransitionValue = requests.bankForkChoice.fastpqTransitions[0].newValue
        requests.bankForkChoice.fastpqTransitions[0].newValue[0] = 0
        assertContentEquals(originalAuditTransitionValue, requests.bankForkChoice.fastpqTransitions[0].newValue)
        val mismatchedGateHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(fullLightClientGateHash = "0x" + "ab".repeat(32)),
            )
        }
        assertTrue(mismatchedGateHash.message!!.contains("fullLightClientGateHash"))
        val mismatchedMaterialHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(sourceVerifierMaterialHash = "0x" + "ab".repeat(32)),
            )
        }
        assertTrue(mismatchedMaterialHash.message!!.contains("sourceVerifierMaterialHash"))
        val mismatchedDeploymentReceiptHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(sourceAdapterDeploymentReceiptHash = "0x" + "ab".repeat(32)),
            )
        }
        assertTrue(mismatchedDeploymentReceiptHash.message!!.contains("sourceAdapterDeploymentReceiptHash"))
        val mismatchedWitnessDeploymentHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(
                    witnessInput = witness.copy(
                        sourceAdapterDeploymentHash = "0x" + "ab".repeat(32),
                    ),
                ),
            )
        }
        assertTrue(mismatchedWitnessDeploymentHash.message!!.contains("sourceAdapterDeploymentHash"))
        val mismatchedWitnessDeploymentReceiptHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(
                    witnessInput = witness.copy(
                        sourceAdapterDeploymentReceiptHash = "0x" + "ab".repeat(32),
                    ),
                ),
            )
        }
        assertTrue(
            mismatchedWitnessDeploymentReceiptHash.message!!.contains(
                "sourceAdapterDeploymentReceiptHash",
            ),
        )
        val duplicatedAuditHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(solanaTowerReplayVerifierHash = accountsdbVerifierHash),
            )
        }
        assertTrue(duplicatedAuditHash.message!!.contains("role-separated"))
        val reusedSourceStateHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(solanaTowerReplayVerifierHash = sourceStateVerifierHash),
            )
        }
        assertTrue(reusedSourceStateHash.message!!.contains("source-adapter material"))
        val reusedSourceTrustAnchorHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(solanaTowerReplayVerifierHash = sourceTrustAnchorHash),
            )
        }
        assertTrue(reusedSourceTrustAnchorHash.message!!.contains("source-adapter material"))
        val reusedAdapterVerifierHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(
                    solanaTowerReplayVerifierHash =
                        SccpSourceProofs.sourceAdapterVerifierVkHash(SccpSourceProofs.DOMAIN_SOL),
                ),
            )
        }
        assertTrue(reusedAdapterVerifierHash.message!!.contains("source-adapter material"))
        val reusedDeploymentReceiptHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(
                    sourceAdapterDeploymentReceiptHash = towerVerifierHash,
                    witnessInput = witness.copy(sourceAdapterDeploymentReceiptHash = towerVerifierHash),
                ),
            )
        }
        assertTrue(reusedDeploymentReceiptHash.message!!.contains("source-adapter material"))
        val reusedTemplateHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildFullLightClientAuditProofRequests(
                input.copy(
                    solanaTowerReplayVerifierHash =
                        "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
                ),
            )
        }
        assertTrue(reusedTemplateHash.message!!.contains("template material"))
    }

    @Test
    fun buildsVoteAndStakeAccountDataHashes() {
        val towerVoteSlots = (11..41).map { it.toString() }
        assertEquals(
            457,
            SccpSolana.canonicalVoteAccountDataBytes(
                ByteArray(32) { 0x51.toByte() },
                ByteArray(32) { 0x61.toByte() },
                ByteArray(32) { 0x71.toByte() },
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x51.toByte() },
                "700",
                "10000",
                "123",
                ByteArray(0),
                "10",
                towerVoteSlots,
            ).size,
        )
        val voteHash = SccpSolana.voteAccountDataHash(
            ByteArray(32) { 0x51.toByte() },
            ByteArray(32) { 0x61.toByte() },
            ByteArray(32) { 0x71.toByte() },
            ByteArray(32) { 0x81.toByte() },
            ByteArray(32) { 0x51.toByte() },
            "700",
            "10000",
            "123",
            ByteArray(0),
            "10",
            towerVoteSlots,
        )
        assertTrue(voteHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(
            voteHash != SccpSolana.voteAccountDataHash(
                ByteArray(32) { 0x51.toByte() },
                ByteArray(32) { 0x62.toByte() },
                ByteArray(32) { 0x71.toByte() },
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x51.toByte() },
                "700",
                "10000",
                "123",
                ByteArray(0),
                "10",
                towerVoteSlots,
            ),
        )
        val unsortedVoteSlots = towerVoteSlots.toMutableList()
        unsortedVoteSlots[0] = "10"
        val badVoteSlots = assertFailsWith<IllegalArgumentException> {
            SccpSolana.voteAccountDataHash(
                ByteArray(32) { 0x51.toByte() },
                ByteArray(32) { 0x61.toByte() },
                ByteArray(32) { 0x71.toByte() },
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x51.toByte() },
                "700",
                "10000",
                "123",
                ByteArray(0),
                "10",
                unsortedVoteSlots,
            )
        }
        assertTrue(badVoteSlots.message?.contains("towerVoteSlots[0]") == true)

        assertEquals(
            154,
            SccpSolana.canonicalStakeAccountDataBytes(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa1.toByte() },
                "1000",
                "2",
                "9",
                "123",
                "1",
                byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f),
            ).size,
        )
        val stakeHash = SccpSolana.stakeAccountDataHash(
            ByteArray(32) { 0x81.toByte() },
            ByteArray(32) { 0x91.toByte() },
            ByteArray(32) { 0xa1.toByte() },
            "1000",
            "2",
            "9",
            "123",
            "1",
            byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f),
        )
        assertTrue(stakeHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(
            stakeHash != SccpSolana.stakeAccountDataHash(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa2.toByte() },
                "1000",
                "2",
                "9",
                "123",
                "1",
                byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f),
            ),
        )
        assertTrue(
            SccpSolana.stakeAccountDataHash(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa1.toByte() },
                "1000",
                "2",
                "9",
                "123",
                "1",
                byteArrayOf(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0.toByte(), 0x3f),
            ).matches(Regex("0x[0-9a-f]{64}")),
        )
        val badWarmupCooldownRate = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountDataHash(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa1.toByte() },
                "1000",
                "2",
                "9",
                "123",
                "1",
                ByteArray(8),
            )
        }
        assertTrue(
            badWarmupCooldownRate.message?.contains("warmupCooldownRateBytes") == true,
        )
        assertTrue(
            stakeHash != SccpSolana.stakeAccountDataHash(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa1.toByte() },
                "1000",
                "2",
                "9",
                "123",
                "0",
                byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f),
            ),
        )
        val badStakeEpoch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountDataHash(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa1.toByte() },
                "1000",
                "2",
                "2",
                "123",
                "1",
                byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f),
            )
        }
        assertTrue(badStakeEpoch.message?.contains("deactivationEpoch") == true)
        val badStakeFlags = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountDataHash(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa1.toByte() },
                "1000",
                "2",
                "9",
                "123",
                "2",
                byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f),
            )
        }
        assertTrue(badStakeFlags.message?.contains("stakeFlags") == true)
        val badWarmupCooldownBytes = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountDataHash(
                ByteArray(32) { 0x81.toByte() },
                ByteArray(32) { 0x91.toByte() },
                ByteArray(32) { 0xa1.toByte() },
                "1000",
                "2",
                "9",
                "123",
                "1",
                ByteArray(7),
            )
        }
        assertTrue(badWarmupCooldownBytes.message?.contains("warmupCooldownRateBytes") == true)
    }

    @Test
    fun buildsVoteAccountDataHashFromRawVoteState() {
        val voteAccountAddress = ByteArray(32) { 0x81.toByte() }
        val rawV3 = sampleSolanaVoteStateAccount(hasLatency = true)
        val parsed = SccpSolana.voteAccountDataFromRawVoteState(rawV3, "3", voteAccountAddress)
        assertContentEquals(ByteArray(32) { 0x51.toByte() }, parsed.nodePubkey)
        assertContentEquals(ByteArray(32) { 0x61.toByte() }, parsed.authorizedVoter)
        assertContentEquals(ByteArray(32) { 0x71.toByte() }, parsed.authorizedWithdrawer)
        assertContentEquals(voteAccountAddress, parsed.inflationRewardsCollector)
        assertContentEquals(ByteArray(32) { 0x51.toByte() }, parsed.blockRevenueCollector)
        assertEquals("700", parsed.inflationRewardsCommissionBps)
        assertEquals("10000", parsed.blockRevenueCommissionBps)
        assertEquals("0", parsed.pendingDelegatorRewards)
        assertContentEquals(ByteArray(0), parsed.blsPubkeyCompressed)
        assertEquals("10", parsed.rootSlot)
        assertEquals((11..41).map { it.toString() }, parsed.towerVoteSlots)
        assertEquals(
            SccpSolana.voteAccountDataHash(
                parsed.nodePubkey,
                parsed.authorizedVoter,
                parsed.authorizedWithdrawer,
                parsed.inflationRewardsCollector,
                parsed.blockRevenueCollector,
                parsed.inflationRewardsCommissionBps,
                parsed.blockRevenueCommissionBps,
                parsed.pendingDelegatorRewards,
                parsed.blsPubkeyCompressed,
                parsed.rootSlot,
                parsed.towerVoteSlots,
            ),
            SccpSolana.voteAccountDataHashFromRawVoteState(rawV3, "3", voteAccountAddress),
        )

        val rawV1 = sampleSolanaVoteStateAccount(hasLatency = false)
        assertEquals(
            parsed.towerVoteSlots,
            SccpSolana.voteAccountDataFromRawVoteState(rawV1, "3", voteAccountAddress).towerVoteSlots,
        )

        val parsedV4 = SccpSolana.voteAccountDataFromRawVoteState(sampleSolanaVoteStateV4Account(), "3", voteAccountAddress)
        assertContentEquals(ByteArray(32) { 0x81.toByte() }, parsedV4.inflationRewardsCollector)
        assertContentEquals(ByteArray(32) { 0x91.toByte() }, parsedV4.blockRevenueCollector)
        assertEquals("1234", parsedV4.inflationRewardsCommissionBps)
        assertEquals("9876", parsedV4.blockRevenueCommissionBps)
        assertEquals("456", parsedV4.pendingDelegatorRewards)
        assertContentEquals(ByteArray(48) { 0xa5.toByte() }, parsedV4.blsPubkeyCompressed)
        val v4InflationCommissionBpsOffset = 4 + (4 * 32)
        val excessiveInflationCommissionV4 = sampleSolanaVoteStateV4Account()
        excessiveInflationCommissionV4[v4InflationCommissionBpsOffset] = 0x11.toByte()
        excessiveInflationCommissionV4[v4InflationCommissionBpsOffset + 1] = 0x27.toByte()
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(
                    excessiveInflationCommissionV4,
                    "3",
                    voteAccountAddress,
                )
            }.message?.contains("inflationRewardsCommissionBps") == true,
        )
        val excessiveBlockCommissionV4 = sampleSolanaVoteStateV4Account()
        excessiveBlockCommissionV4[v4InflationCommissionBpsOffset + 2] = 0x11.toByte()
        excessiveBlockCommissionV4[v4InflationCommissionBpsOffset + 3] = 0x27.toByte()
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(
                    excessiveBlockCommissionV4,
                    "3",
                    voteAccountAddress,
                )
            }.message?.contains("blockRevenueCommissionBps") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataHash(
                    parsedV4.nodePubkey,
                    parsedV4.authorizedVoter,
                    parsedV4.authorizedWithdrawer,
                    parsedV4.inflationRewardsCollector,
                    parsedV4.blockRevenueCollector,
                    parsedV4.inflationRewardsCommissionBps,
                    parsedV4.blockRevenueCommissionBps,
                    parsedV4.pendingDelegatorRewards,
                    ByteArray(48),
                    parsedV4.rootSlot,
                    parsedV4.towerVoteSlots,
                )
            }.message?.contains("blsPubkeyCompressed") == true,
        )
        val allZeroBlsV4 = sampleSolanaVoteStateV4Account()
        val v4BlsPubkeyOffset = 4 + (4 * 32) + 2 + 2 + 8 + 1
        allZeroBlsV4.fill(0, v4BlsPubkeyOffset, v4BlsPubkeyOffset + 48)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(allZeroBlsV4, "3", voteAccountAddress)
            }.message?.contains("blsPubkeyCompressed") == true,
        )
        val parsedV4FourAuthorized =
            SccpSolana.voteAccountDataFromRawVoteState(sampleSolanaVoteStateV4Account(4), "3", voteAccountAddress)
        assertContentEquals(ByteArray(32) { 0x62.toByte() }, parsedV4FourAuthorized.authorizedVoter)

        val wrongVoteCount = rawV3.copyOf()
        writeU64Le(wrongVoteCount, 4 + 32 + 32 + 1, 30L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(wrongVoteCount, "3", voteAccountAddress)
            }.message?.contains("31 active post-root slots") == true,
        )

        val voteEntryOffset = 4 + 32 + 32 + 1 + 8
        val firstVoteSlotOffset = voteEntryOffset + 1
        val firstConfirmationOffset = firstVoteSlotOffset + 8
        val secondVoteSlotOffset = voteEntryOffset + (1 + 8 + 4) + 1
        val rootOptionOffset = voteEntryOffset + (31 * (1 + 8 + 4))

        val wrongConfirmationCount = rawV3.copyOf()
        writeU32Le(wrongConfirmationCount, firstConfirmationOffset, 30)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(wrongConfirmationCount, "3", voteAccountAddress)
            }.message?.contains("invalid Tower confirmation count") == true,
        )

        val repeatedVoteSlot = rawV3.copyOf()
        writeU64Le(repeatedVoteSlot, secondVoteSlotOffset, 11L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(repeatedVoteSlot, "3", voteAccountAddress)
            }.message?.contains("greater than the previous slot") == true,
        )

        val noRoot = rawV3.copyOf()
        noRoot[rootOptionOffset] = 0
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(noRoot, "3", voteAccountAddress)
            }.message?.contains("rooted vote state") == true,
        )

        val rootOverlapsVoteStack = rawV3.copyOf()
        writeU64Le(rootOverlapsVoteStack, rootOptionOffset + 1, 11L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(rootOverlapsVoteStack, "3", voteAccountAddress)
            }.message?.contains("greater than the previous slot") == true,
        )

        val badPriorVoters = rawV3.copyOf()
        val priorVotersOffset = rootOptionOffset + 1 + 8 + 8 + (2 * (8 + 32))
        val zeroPriorVoterWithEpochBounds = rawV3.copyOf()
        writeU64Le(zeroPriorVoterWithEpochBounds, priorVotersOffset + 32, 1L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(
                    zeroPriorVoterWithEpochBounds,
                    "3",
                    voteAccountAddress,
                )
            }.message?.contains("priorVoters[0]") == true,
        )
        badPriorVoters[priorVotersOffset + (32 * (32 + 8 + 8)) + 8] = 2
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(badPriorVoters, "3", voteAccountAddress)
            }.message?.contains("priorVoters") == true,
        )

        val tooManyEpochCredits = sampleSolanaVoteStateV4Account()
        val v4AuthorizedVotersOffset = 4 + 32 + 32 + 32 + 32 + 2 + 2 + 8 + 1 + 48 + 8 +
            (31 * (1 + 8 + 4)) + 1 + 8
        val zeroFutureAuthorizedVoter = sampleSolanaVoteStateV4Account(4)
        val fourthAuthorizedVoterKeyOffset = v4AuthorizedVotersOffset + 8 + (3 * (8 + 32)) + 8
        zeroFutureAuthorizedVoter.fill(0, fourthAuthorizedVoterKeyOffset, fourthAuthorizedVoterKeyOffset + 32)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(zeroFutureAuthorizedVoter, "3", voteAccountAddress)
            }.message?.contains("authorizedVoters[3].authorizedVoter") == true,
        )
        val tooManyV4AuthorizedVoters = sampleSolanaVoteStateV4Account(5)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(
                    tooManyV4AuthorizedVoters,
                    "3",
                    voteAccountAddress,
                )
            }.message?.contains("1..4 entries for VoteStateV4") == true,
        )

        val v4EpochCreditsOffset = v4AuthorizedVotersOffset + 8 + (2 * (8 + 32))
        writeU64Le(tooManyEpochCredits, v4EpochCreditsOffset, 65L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(tooManyEpochCredits, "3", voteAccountAddress)
            }.message?.contains("epochCredits") == true,
        )

        val v3EpochCreditsOffset = priorVotersOffset + (32 * (32 + 8 + 8)) + 8 + 1
        val futureEpochCredit = rawV3.copyOf()
        writeU64Le(futureEpochCredit, v3EpochCreditsOffset, 1L)
        writeU64Le(futureEpochCredit, v3EpochCreditsOffset + 8, 4L)
        writeU64Le(futureEpochCredit, v3EpochCreditsOffset + 16, 1L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(futureEpochCredit, "3", voteAccountAddress)
            }.message?.contains("epochCredits") == true,
        )

        val lastTimestampSlotOffset = v3EpochCreditsOffset + 8
        val futureLastTimestampSlot = rawV3.copyOf()
        writeU64Le(futureLastTimestampSlot, lastTimestampSlotOffset, 42L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(futureLastTimestampSlot, "3", voteAccountAddress)
            }.message?.contains("lastTimestamp") == true,
        )

        val negativeLastTimestamp = rawV3.copyOf()
        writeU64Le(negativeLastTimestamp, lastTimestampSlotOffset, 41L)
        writeU64Le(negativeLastTimestamp, lastTimestampSlotOffset + 8, -1L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(negativeLastTimestamp, "3", voteAccountAddress)
            }.message?.contains("lastTimestamp") == true,
        )

        val nonzeroPadding = rawV3.copyOf()
        nonzeroPadding[nonzeroPadding.lastIndex] = 1.toByte()
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(nonzeroPadding, "3", voteAccountAddress)
            }.message?.contains("padding") == true,
        )

        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.voteAccountDataFromRawVoteState(rawV3, "0", voteAccountAddress)
            }.message?.contains("at or before epoch") == true,
        )
    }

    @Test
    fun buildsStakeAccountDataHashFromRawStakeStateV2() {
        val raw = sampleSolanaStakeStateV2StakeAccount()
        val parsed = SccpSolana.stakeAccountDataFromRawStakeStateV2(raw)
        assertContentEquals(ByteArray(32) { 0x81.toByte() }, parsed.staker)
        assertContentEquals(ByteArray(32) { 0x91.toByte() }, parsed.withdrawer)
        assertContentEquals(ByteArray(32) { 0xa1.toByte() }, parsed.voterPubkey)
        assertEquals("1000", parsed.delegatedStake)
        assertEquals("2", parsed.activationEpoch)
        assertEquals("9", parsed.deactivationEpoch)
        assertContentEquals(
            byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f),
            parsed.warmupCooldownRateBytes,
        )
        assertEquals("123", parsed.creditsObserved)
        assertEquals("1", parsed.stakeFlags)
        assertEquals(
            SccpSolana.stakeAccountDataHash(
                parsed.staker,
                parsed.withdrawer,
                parsed.voterPubkey,
                parsed.delegatedStake,
                parsed.activationEpoch,
                parsed.deactivationEpoch,
                parsed.creditsObserved,
                parsed.stakeFlags,
                parsed.warmupCooldownRateBytes,
            ),
            SccpSolana.stakeAccountDataHashFromRawStakeStateV2(raw),
        )

        val wrongVariant = raw.copyOf()
        writeU32Le(wrongVariant, 0, 1)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(wrongVariant)
            }.message?.contains("StakeStateV2::Stake") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(raw.copyOf(199))
            }.message?.contains("200-byte") == true,
        )

        val hiddenPadding = raw.copyOf()
        hiddenPadding[197] = 1
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(hiddenPadding)
            }.message?.contains("padding") == true,
        )

        val unknownFlags = raw.copyOf()
        unknownFlags[196] = 2
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(unknownFlags)
            }.message?.contains("StakeFlags") == true,
        )

        val zeroVoter = raw.copyOf()
        zeroVoter.fill(0, 124, 156)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(zeroVoter)
            }.message?.contains("voterPubkey") == true,
        )

        val zeroDelegation = raw.copyOf()
        writeU64Le(zeroDelegation, 156, 0L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(zeroDelegation)
            }.message?.contains("delegatedStake") == true,
        )

        val legacyWarmupCooldownRate = raw.copyOf()
        byteArrayOf(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0.toByte(), 0x3f).copyInto(
            legacyWarmupCooldownRate,
            180,
        )
        assertContentEquals(
            byteArrayOf(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0.toByte(), 0x3f),
            SccpSolana.stakeAccountDataFromRawStakeStateV2(legacyWarmupCooldownRate)
                .warmupCooldownRateBytes,
        )

        val zeroWarmupCooldownRate = raw.copyOf()
        zeroWarmupCooldownRate.fill(0, 180, 188)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(zeroWarmupCooldownRate)
            }.message?.contains("warmupCooldownRateBytes") == true,
        )

        val invalidEpochOrder = raw.copyOf()
        writeU64Le(invalidEpochOrder, 172, 2L)
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSolana.stakeAccountDataFromRawStakeStateV2(invalidEpochOrder)
            }.message?.contains("deactivationEpoch") == true,
        )
    }

    @Test
    fun buildsStakeAccountStateHashForFinalityContext() {
        val validatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val validatorStakes = listOf("1", "2")
        val activationEpochs = listOf("0", "2")
        val deactivationEpochs = listOf("18446744073709551615", "9")
        val voteAccounts = listOf(ByteArray(32) { 0x33.toByte() }, ByteArray(32) { 0x44.toByte() })
        val stakeAccounts = listOf(ByteArray(32) { 0x55.toByte() }, ByteArray(32) { 0x66.toByte() })
        val voteAccountHashes = listOf(ByteArray(32) { 0x77.toByte() }, ByteArray(32) { 0x88.toByte() })
        val stakeAccountHashes = listOf(ByteArray(32) { 0x99.toByte() }, ByteArray(32) { 0xaa.toByte() })

        assertEquals(
            437,
            SccpSolana.canonicalStakeAccountStateBytes(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
            ).size,
        )
        assertEquals(
            "0x34f6086dd8c1770770802be17b833ed7c973fdaa002c866c0462c33d6938f5b5",
            SccpSolana.stakeAccountStateHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
            ),
        )
        val lengthMismatch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountStateHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                listOf(ByteArray(32) { 0x33.toByte() }),
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
            )
        }
        assertTrue(lengthMismatch.message?.contains("validatorVoteAccountAddresses") == true)
        val duplicateVoteAccount = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountStateHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                listOf(ByteArray(32) { 0x33.toByte() }, ByteArray(32) { 0x33.toByte() }),
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
            )
        }
        assertTrue(duplicateVoteAccount.message?.contains("validatorVoteAccountAddresses") == true)
        val sameVoteAndStake = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountStateHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                listOf(ByteArray(32) { 0x55.toByte() }, ByteArray(32) { 0x44.toByte() }),
                voteAccountHashes,
                stakeAccountHashes,
            )
        }
        assertTrue(sameVoteAndStake.message?.contains("validatorStakeAccountAddresses[1]") == true)
        val crossRoleOverlap = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountStateHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                listOf(ByteArray(32) { 0x66.toByte() }, ByteArray(32) { 0x44.toByte() }),
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
            )
        }
        assertTrue(crossRoleOverlap.message?.contains("validatorVoteAccountAddresses[0]") == true)
        val zeroVoteHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeAccountStateHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                listOf(ByteArray(32) { 0x77.toByte() }, ByteArray(32) { 0x00.toByte() }),
                stakeAccountHashes,
            )
        }
        assertTrue(zeroVoteHash.message?.contains("validatorVoteAccountHashes[1]") == true)
    }

    @Test
    fun buildsStakeHistoryHashForFinalityContext() {
        val validatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val validatorEffectiveStakes = listOf("1", "2")
        val validatorDelegatedStakes = listOf("1", "3")
        val activationEpochs = listOf("0", "2")
        val deactivationEpochs = listOf("18446744073709551615", "9")
        val voteAccounts = listOf(ByteArray(32) { 0x33.toByte() }, ByteArray(32) { 0x44.toByte() })
        val stakeAccounts = listOf(ByteArray(32) { 0x55.toByte() }, ByteArray(32) { 0x66.toByte() })
        val voteAccountHashes = listOf(ByteArray(32) { 0x77.toByte() }, ByteArray(32) { 0x88.toByte() })
        val stakeAccountHashes = listOf(ByteArray(32) { 0x99.toByte() }, ByteArray(32) { 0xaa.toByte() })
        val stakeHistoryEntries = listOf(
            SolanaSccpStakeHistoryEntry("2", "23", "3", "0"),
            SolanaSccpStakeHistoryEntry("3", "3", "1", "0"),
        )

        assertEquals(
            249,
            SccpSolana.canonicalStakeHistoryBytes(
                "3",
                validatorPublicKeys,
                validatorEffectiveStakes,
                validatorDelegatedStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
                stakeHistoryEntries,
            ).size,
        )
        assertEquals(
            "0xd75957eec3cf9f5b88076c8dc18e81c5debd627adfbed7e03e35443bcc4d14b6",
            SccpSolana.stakeHistoryHash(
                "3",
                validatorPublicKeys,
                validatorEffectiveStakes,
                validatorDelegatedStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
                stakeHistoryEntries,
            ),
        )
        val delegatedTooSmall = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistoryHash(
                "3",
                validatorPublicKeys,
                validatorEffectiveStakes,
                listOf("0", "3"),
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
                stakeHistoryEntries,
            )
        }
        assertTrue(delegatedTooSmall.message?.contains("validatorDelegatedStakes[0]") == true)
        val wrongEffectiveStake = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistoryHash(
                "3",
                validatorPublicKeys,
                listOf("1", "1"),
                validatorDelegatedStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
                stakeHistoryEntries,
            )
        }
        assertTrue(wrongEffectiveStake.message?.contains("validatorEffectiveStakes[1]") == true)
        val extraSignedEffectiveStake = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistoryHash(
                "3",
                validatorPublicKeys,
                validatorEffectiveStakes,
                validatorDelegatedStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
                listOf(
                    stakeHistoryEntries[0],
                    SolanaSccpStakeHistoryEntry("3", "4", "1", "0"),
                ),
            )
        }
        assertTrue(
            extraSignedEffectiveStake.message?.contains("must equal replayed validator effective stake") == true,
        )
        val missingSignedEpoch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistoryHash(
                "3",
                validatorPublicKeys,
                validatorEffectiveStakes,
                validatorDelegatedStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
                stakeHistoryEntries.take(1),
            )
        }
        assertTrue(missingSignedEpoch.message?.contains("stakeHistoryEntries") == true)
    }

    @Test
    fun buildsStakeHistorySysvarDataHash() {
        val stakeHistoryEntries = listOf(
            SolanaSccpStakeHistoryEntry("2", "10", "3", "1"),
            SolanaSccpStakeHistoryEntry("3", "12", "0", "0"),
        )

        assertEquals(32, SccpSolana.SYSVAR_PROGRAM_ID.removePrefix("0x").length / 2)
        assertEquals(32, SccpSolana.STAKE_HISTORY_SYSVAR_ID.removePrefix("0x").length / 2)
        val canonical = SccpSolana.canonicalStakeHistorySysvarDataBytes(stakeHistoryEntries)
        assertEquals(72, canonical.size)
        assertEquals(3, canonical[8].toInt() and 0xFF)
        val dataHash = SccpSolana.stakeHistorySysvarDataHash(stakeHistoryEntries)
        assertTrue(dataHash.matches(Regex("0x[0-9a-f]{64}")))
        assertEquals(dataHash, SccpSolana.stakeHistorySysvarDataHashFromRawData(canonical))
        assertTrue(
            dataHash != SccpSolana.stakeHistorySysvarDataHash(
                listOf(
                    stakeHistoryEntries[0],
                    SolanaSccpStakeHistoryEntry("3", "13", "0", "0"),
                ),
            ),
        )
        val unsorted = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistorySysvarDataHash(stakeHistoryEntries.reversed())
        }
        assertTrue(unsorted.message?.contains("strictly increasing epoch") == true)
        val truncatedRaw = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistorySysvarDataHashFromRawData(canonical.copyOf(9))
        }
        assertTrue(truncatedRaw.message?.contains("bincode Vec") == true)
        val wrongCount = canonical.copyOf()
        writeU64Le(wrongCount, 0, 3L)
        val wrongCountError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistorySysvarDataHashFromRawData(wrongCount)
        }
        assertTrue(wrongCountError.message?.contains("1..512") == true)
        val ascendingRaw = canonical.copyOf()
        val newestEntry = canonical.copyOfRange(8, 40)
        val oldestEntry = canonical.copyOfRange(40, 72)
        oldestEntry.copyInto(ascendingRaw, 8)
        newestEntry.copyInto(ascendingRaw, 40)
        val wrongOrderError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.stakeHistorySysvarDataHashFromRawData(ascendingRaw)
        }
        assertTrue(wrongOrderError.message?.contains("newest-first") == true)
    }

    @Test
    fun buildsTowerLockoutHashForFinalityContext() {
        val finalizedSlot = "1296096"
        val rootedSlot = "1296065"
        val parentSlot = "1296095"
        val parentBankHash = "0x" + "33".repeat(32)

        assertEquals(32L, SccpSolana.TOWER_LOCKOUT_CONFIRMATION_DEPTH)
        assertEquals(31L, SccpSolana.TOWER_VOTE_STACK_DEPTH)
        assertEquals(
            73,
            SccpSolana.canonicalTowerLockoutBytes(
                finalizedSlot = finalizedSlot,
                rootedSlot = rootedSlot,
                parentSlot = parentSlot,
                parentBankHash = parentBankHash,
            ).size,
        )
        assertTrue(
            SccpSolana.towerLockoutHash(
                finalizedSlot = finalizedSlot,
                rootedSlot = rootedSlot,
                parentSlot = parentSlot,
                parentBankHash = parentBankHash,
            ).matches(Regex("0x[0-9a-f]{64}")),
        )
        assertEquals(
            SccpSolana.towerLockoutHash(finalizedSlot, rootedSlot, parentSlot, parentBankHash),
            SccpSolana.towerLockoutHash(finalizedSlot, rootedSlot, parentSlot, parentBankHash, "3"),
        )
        val wrongEpoch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerLockoutHash(finalizedSlot, rootedSlot, parentSlot, parentBankHash, "4")
        }
        assertTrue(wrongEpoch.message?.contains("epoch") == true)
        val shallowRoot = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerLockoutHash(finalizedSlot, "1296066", parentSlot, parentBankHash)
        }
        assertTrue(shallowRoot.message?.contains("rootedSlot") == true)
        val indirectParent = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerLockoutHash(finalizedSlot, rootedSlot, "1296094", parentBankHash)
        }
        assertTrue(indirectParent.message?.contains("parentSlot") == true)
        val zeroParentBankHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerLockoutHash(finalizedSlot, rootedSlot, parentSlot, "0x" + "00".repeat(32))
        }
        assertTrue(zeroParentBankHash.message?.contains("parentBankHash") == true)
    }

    @Test
    fun buildsTowerReplayHashForFinalityContext() {
        val finalizedSlot = "1296096"
        val rootedSlot = "1296065"
        val parentSlot = "1296095"
        val bankForkHash = "0x" + "a5".repeat(32)
        val towerVoteSlots = (1_296_066..1_296_096).map { it.toString() }

        assertEquals(
            573,
            SccpSolana.canonicalTowerReplayBytes(
                finalizedSlot = finalizedSlot,
                rootedSlot = rootedSlot,
                parentSlot = parentSlot,
                bankForkHash = bankForkHash,
                towerVoteSlots = towerVoteSlots,
            ).size,
        )
        assertTrue(
            SccpSolana.towerReplayHash(
                finalizedSlot,
                rootedSlot,
                parentSlot,
                bankForkHash,
                towerVoteSlots,
            ).matches(Regex("0x[0-9a-f]{64}")),
        )
        assertEquals(
            SccpSolana.towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots),
            SccpSolana.towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots, "3"),
        )
        assertTrue(
            SccpSolana.towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots) !=
                SccpSolana.towerReplayHash(
                    finalizedSlot,
                    rootedSlot,
                    parentSlot,
                    "0x" + "a6".repeat(32),
                    towerVoteSlots,
                ),
        )
        val zeroBankForkHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerReplayHash(
                finalizedSlot,
                rootedSlot,
                parentSlot,
                "0x" + "00".repeat(32),
                towerVoteSlots,
            )
        }
        assertTrue(zeroBankForkHash.message?.contains("bankForkHash") == true)
        val wrongEpoch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots, "4")
        }
        assertTrue(wrongEpoch.message?.contains("epoch") == true)
        val shortStack = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots.drop(1))
        }
        assertTrue(shortStack.message?.contains("towerVoteSlots") == true)
        val unsortedVoteSlots = towerVoteSlots.toMutableList()
        val first = unsortedVoteSlots[0]
        unsortedVoteSlots[0] = unsortedVoteSlots[1]
        unsortedVoteSlots[1] = first
        val unsortedStack = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, unsortedVoteSlots)
        }
        assertTrue(unsortedStack.message?.contains("strictly increasing") == true)
        val wrongLastVoteSlots = towerVoteSlots.toMutableList()
        wrongLastVoteSlots[wrongLastVoteSlots.lastIndex] = "1296095"
        val wrongLast = assertFailsWith<IllegalArgumentException> {
            SccpSolana.towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, wrongLastVoteSlots)
        }
        assertTrue(wrongLast.message?.contains("last towerVoteSlots") == true)
    }

    @Test
    fun buildsAccountInclusionWitness() {
        data class AccountOpening(
            val address: ByteArray,
            val owner: ByteArray,
            val lamports: String,
            val rentEpoch: String,
            val executable: Boolean,
            val dataHash: String,
        )

        val finalizedSlot = "1296096"
        val openings = listOf(
            AccountOpening(ByteArray(32) { 0x31.toByte() }, ByteArray(32) { 0x61.toByte() }, "1000000", "0", false, "0x" + "91".repeat(32)),
            AccountOpening(ByteArray(32) { 0x41.toByte() }, ByteArray(32) { 0x62.toByte() }, "1000001", "0", false, "0x" + "92".repeat(32)),
            AccountOpening(ByteArray(32) { 0x51.toByte() }, ByteArray(32) { 0x63.toByte() }, "1000002", "0", false, "0x" + "93".repeat(32)),
        )
        val openingInputs = openings.map { opening ->
            SolanaSccpAccountOpeningInput(
                address = opening.address,
                owner = opening.owner,
                lamports = opening.lamports,
                rentEpoch = opening.rentEpoch,
                executable = opening.executable,
                dataHash = opening.dataHash,
            )
        }
        val rawData = listOf(
            ByteArray(64) { 0x01.toByte() },
            ByteArray(64) { 0x02.toByte() },
            ByteArray(64) { 0x03.toByte() },
        )
        assertEquals(
            109,
            SccpSolana.canonicalAccountInclusionLeafBytes(
                finalizedSlot,
                openings[0].address,
                openings[0].owner,
                openings[0].lamports,
                openings[0].rentEpoch,
                openings[0].executable,
                openings[0].dataHash,
                SccpSolana.accountRawDataHash(rawData[0]),
            ).size,
        )
        val leaves = openings.mapIndexed { index, opening ->
            SccpSolana.accountInclusionLeafHash(
                finalizedSlot,
                opening.address,
                opening.owner,
                opening.lamports,
                opening.rentEpoch,
                opening.executable,
                opening.dataHash,
                rawData[index],
            )
        }
        assertEquals(65, SccpSolana.canonicalAccountInclusionNodeBytes(leaves[0], leaves[1]).size)
        assertTrue(SccpSolana.accountInclusionNodeHash(leaves[0], leaves[1]).startsWith("0x"))
        val witness = SccpSolana.accountInclusionRootAndBranches(leaves)
        assertEquals(leaves.size, witness.branches.size)
        assertEquals(witness.root, SccpSolana.accountInclusionRootFromBranch(leaves[0], witness.branches[0]))
        assertEquals(witness.root, SccpSolana.accountInclusionRootFromBranch(leaves[1], witness.branches[1]))
        val openedWitness = SccpSolana.openedAccountInclusionWitness(
            SolanaSccpOpenedAccountInclusionWitnessInput(
                finalizedSlot = finalizedSlot,
                validatorVoteAccountOpenings = listOf(openingInputs[0]),
                validatorVoteAccountRawData = listOf(rawData[0]),
                validatorStakeAccountOpenings = listOf(openingInputs[1]),
                validatorStakeAccountRawData = listOf(rawData[1]),
                stakeHistorySysvarOpening = openingInputs[2],
                stakeHistorySysvarRawData = rawData[2],
                expectedAccountInclusionRoot = witness.root,
            ),
        )
        assertEquals(witness.branches, openedWitness.branches)
        assertEquals(listOf(witness.branches[0]), openedWitness.validatorVoteAccountBranches)
        assertEquals(listOf(witness.branches[1]), openedWitness.validatorStakeAccountBranches)
        assertEquals(witness.branches[2], openedWitness.stakeHistorySysvarBranch)
        val duplicateOpenedAddress = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountInclusionWitness(
                SolanaSccpOpenedAccountInclusionWitnessInput(
                    finalizedSlot = finalizedSlot,
                    validatorVoteAccountOpenings = listOf(openingInputs[0]),
                    validatorVoteAccountRawData = listOf(rawData[0]),
                    validatorStakeAccountOpenings = listOf(openingInputs[1].copy(address = openingInputs[0].address)),
                    validatorStakeAccountRawData = listOf(rawData[1]),
                    stakeHistorySysvarOpening = openingInputs[2],
                    stakeHistorySysvarRawData = rawData[2],
                ),
            )
        }
        assertTrue(duplicateOpenedAddress.message?.contains("opened account addresses") == true)
        val mismatchedRoot = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountInclusionWitness(
                SolanaSccpOpenedAccountInclusionWitnessInput(
                    finalizedSlot = finalizedSlot,
                    validatorVoteAccountOpenings = listOf(openingInputs[0]),
                    validatorVoteAccountRawData = listOf(rawData[0]),
                    validatorStakeAccountOpenings = listOf(openingInputs[1]),
                    validatorStakeAccountRawData = listOf(rawData[1]),
                    stakeHistorySysvarOpening = openingInputs[2],
                    stakeHistorySysvarRawData = rawData[2],
                    expectedAccountInclusionRoot = "0x" + "77".repeat(32),
                ),
            )
        }
        assertTrue(mismatchedRoot.message?.contains("accountInclusionRoot") == true)
        val mutatedLeaf = SccpSolana.accountInclusionLeafHash(
            finalizedSlot,
            openings[0].address,
            openings[0].owner,
            openings[0].lamports,
            openings[0].rentEpoch,
            openings[0].executable,
            openings[0].dataHash,
            ByteArray(64) { 0x04.toByte() },
        )
        assertTrue(SccpSolana.accountInclusionRootFromBranch(mutatedLeaf, witness.branches[0]) != witness.root)
        val zeroLeaf = assertFailsWith<IllegalArgumentException> {
            SccpSolana.accountInclusionRootFromBranch("0x" + "00".repeat(32), emptyList())
        }
        assertTrue(zeroLeaf.message?.contains("leaf") == true)
        val oversizedBranch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.accountInclusionRootFromBranch(
                leaves[0],
                List(SccpSolana.MAX_SOURCE_MERKLE_BRANCH_NODES + 1) { "0x" + "56".repeat(32) },
            )
        }
        assertTrue(oversizedBranch.message?.contains("at most") == true)
        val oversizedOpened = assertFailsWith<IllegalArgumentException> {
            SccpSolana.openedAccountInclusionWitness(
                SolanaSccpOpenedAccountInclusionWitnessInput(
                    finalizedSlot = finalizedSlot,
                    validatorVoteAccountOpenings = List(SccpSolana.MAX_VALIDATORS + 1) { openingInputs[0] },
                    validatorVoteAccountRawData = List(SccpSolana.MAX_VALIDATORS + 1) { rawData[0] },
                    validatorStakeAccountOpenings = listOf(openingInputs[1]),
                    validatorStakeAccountRawData = listOf(rawData[1]),
                    stakeHistorySysvarOpening = openingInputs[2],
                    stakeHistorySysvarRawData = rawData[2],
                ),
            )
        }
        assertTrue(oversizedOpened.message?.contains("validatorVoteAccountOpenings") == true)
        assertFailsWith<IllegalArgumentException> { SccpSolana.accountRawDataHash(ByteArray(0)) }
        assertFailsWith<IllegalArgumentException> { SccpSolana.accountInclusionRootAndBranches(listOf(leaves[0], leaves[0])) }
    }

    @Test
    fun buildsBankForkHashForFinalityContext() {
        val finalizedSlot = "1296096"
        val parentSlot = "1296095"
        val parentBankHash = "0x" + "33".repeat(32)
        val bankSignatureCount = "8"
        val blockhash = "0x" + "55".repeat(32)
        val accountsLtHash = ByteArray(2_048) { 0x99.toByte() }
        val bankHash = SccpSolana.agaveBankHash(
            parentBankHash = parentBankHash,
            bankSignatureCount = bankSignatureCount,
            blockhash = blockhash,
            accountsLtHash = accountsLtHash,
        )
        val transactionStatusRoot = "0x" + "66".repeat(32)
        val accountInclusionRoot = "0x" + "77".repeat(32)
        val accountsLtHashChecksum = SccpSolana.accountsLtHashChecksum(accountsLtHash)

        assertEquals(
            229,
            SccpSolana.canonicalBankForkBytes(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            ).size,
        )
        assertEquals(
            "0x8c496fb25a4499947e454a84f638211a84445748bc5242fbb6fb511edd82e531",
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            ),
        )
        assertEquals(
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            ),
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
                epoch = "3",
            ),
        )
        val wrongEpoch = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
                epoch = "4",
            )
        }
        assertTrue(wrongEpoch.message?.contains("epoch") == true)
        val indirectParent = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = "1296094",
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            )
        }
        assertTrue(indirectParent.message?.contains("parentSlot") == true)
        val zeroSignatureCount = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = "0",
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            )
        }
        assertTrue(zeroSignatureCount.message?.contains("bankSignatureCount") == true)
        val repeatedBankHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = parentBankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            )
        }
        assertTrue(repeatedBankHash.message?.contains("bankHash") == true)
        val wrongBankHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = "0x" + "44".repeat(32),
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            )
        }
        assertTrue(wrongBankHash.message?.contains("bankHash") == true)
        val zeroBlockhash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = "0x" + "00".repeat(32),
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = accountsLtHashChecksum,
            )
        }
        assertTrue(zeroBlockhash.message?.contains("blockhash") == true)
        val zeroAccountRoot = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = "0x" + "00".repeat(32),
                accountsLtHashChecksum = accountsLtHashChecksum,
            )
        }
        assertTrue(zeroAccountRoot.message?.contains("accountInclusionRoot") == true)
        val zeroAccountsLtHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.bankForkHash(
                finalizedSlot = finalizedSlot,
                parentSlot = parentSlot,
                bankSignatureCount = bankSignatureCount,
                parentBankHash = parentBankHash,
                bankHash = bankHash,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                transactionStatusRoot = transactionStatusRoot,
                accountInclusionRoot = accountInclusionRoot,
                accountsLtHashChecksum = "0x" + "00".repeat(32),
            )
        }
        assertTrue(zeroAccountsLtHash.message?.contains("accountsLtHashChecksum") == true)
        val hugeHardForkData = assertFailsWith<IllegalArgumentException> {
            SccpSolana.agaveBankHash(
                parentBankHash = parentBankHash,
                bankSignatureCount = bankSignatureCount,
                blockhash = blockhash,
                accountsLtHash = accountsLtHash,
                bankHashHardForkData = ByteArray(1_025),
            )
        }
        assertTrue(hugeHardForkData.message?.contains("bankHashHardForkData") == true)
    }

    @Test
    fun derivesAndValidatesMessageProofHashFromWitnessBranch() {
        val branch = listOf(ByteArray(32) { 0x56.toByte() })
        val input = sampleWitness(messageProofHash = "", inclusionBranch = branch)
        val derived = SccpSolana.messageProofHash(
            sourceEventDigest = input.sourceEventDigest,
            transactionStatusRoot = input.transactionStatusRoot,
            transactionSignature = input.transactionSignature,
            emitterProgramId = input.emitterProgramId,
            inclusionBranch = branch,
        )
        val normalized = SccpSolana.normalizeWitness(input)

        assertEquals(derived, normalized.messageProofHash)
        assertEquals(1, normalized.inclusionBranch.size)
        assertTrue(
            SccpSolana.canonicalWitnessBytes(input).size >
                SccpSolana.canonicalWitnessBytes(sampleWitness()).size,
        )
        val zeroWitnessSignature = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeWitness(sampleWitness(transactionSignature = solanaZeroSignature))
        }
        assertTrue(zeroWitnessSignature.message?.contains("transactionSignature") == true)
        val zeroWitnessProgram = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeWitness(sampleWitness(emitterProgramId = solanaZeroProgram))
        }
        assertTrue(zeroWitnessProgram.message?.contains("emitterProgramId") == true)

        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeWitness(
                sampleWitness(messageProofHash = "cc".repeat(32), inclusionBranch = branch),
            )
        }
        assertTrue(error.message?.contains("messageProofHash") == true)
    }

    @Test
    fun proverRequiresLinkedProofEngine() {
        val error = assertFailsWith<IllegalStateException> {
            SolanaSccpProver().prove(sampleProductionWitness())
        }
        assertTrue(error.message?.contains("not linked") == true)
    }

    @Test
    fun proverResolvesWitnessProviderBeforeBuildingRequest() {
        var resolved = false
        val input = sampleProductionWitness()
        val inputAccountsLtHash = input.accountsLtHash!!.copyOf()
        val inputInclusionNode = input.inclusionBranch[0].copyOf()
        val prover = SolanaSccpProver(
            witnessProvider = SolanaSccpWitnessProvider { witness ->
                assertFalse(witness === input)
                assertEquals("321", witness.finalizedSlot)
                witness.accountsLtHash!![0] = 0
                witness.inclusionBranch[0][0] = 0
                resolved = true
                input
            },
            proofEngine = SolanaSccpProofEngine { request ->
                assertTrue(resolved)
                assertEquals("321", request.witness.finalizedSlot)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(input)

        assertContentEquals(inputAccountsLtHash, input.accountsLtHash)
        assertContentEquals(inputInclusionNode, input.inclusionBranch[0])
        assertEquals(SccpSolana.buildProofRequest(input).witnessHash, result.witnessHash)
    }

    @Test
    fun proverSnapshotsMutableRequestBytesBeforeLocalEngine() {
        val input = sampleProductionWitness()
        val expectedRequest = SccpSolana.buildProofRequest(input)
        val expectedAccountsLtHash = expectedRequest.witness.accountsLtHash!!
        val expectedInclusionNode = expectedRequest.witness.inclusionBranch[0]
        lateinit var callbackRequest: SolanaSccpProofRequest
        val prover = SolanaSccpProver(
            proofEngine = SolanaSccpProofEngine { request ->
                callbackRequest = request
                assertEquals(expectedRequest.witnessHash, request.witnessHash)
                request.witness.accountsLtHash!![0] = 0
                request.witness.inclusionBranch[0][0] = 0
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(input)

        assertEquals(expectedRequest.witnessHash, result.witnessHash)
        assertContentEquals(expectedAccountsLtHash, callbackRequest.witness.accountsLtHash!!)
        assertContentEquals(expectedInclusionNode, callbackRequest.witness.inclusionBranch[0])
        val rebuiltRequest = SccpSolana.buildProofRequest(input)
        assertContentEquals(expectedAccountsLtHash, rebuiltRequest.witness.accountsLtHash!!)
        assertContentEquals(expectedInclusionNode, rebuiltRequest.witness.inclusionBranch[0])
        assertEquals(expectedRequest.witness, rebuiltRequest.witness)
    }

    @Test
    fun bindsSourceAdapterDeploymentContextForUiProvers() {
        val deploymentHash = "ab".repeat(32)
        val receiptHash = "cd".repeat(32)
        val zeroBinding = SccpSolana.normalizeSourceAdapterDeploymentBinding()
        assertEquals(SccpSolana.ZERO_HASH_V1, zeroBinding.sourceAdapterDeploymentHash)
        assertEquals(SccpSolana.ZERO_HASH_V1, zeroBinding.sourceAdapterDeploymentReceiptHash)
        val zeroRequest = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildProofRequest(sampleWitness())
        }
        assertTrue(zeroRequest.message?.contains("requires non-zero source adapter deployment binding") == true)
        val request = SccpSolana.buildProofRequest(
            sampleWitness(
                sourceAdapterDeploymentHash = deploymentHash,
                sourceAdapterDeploymentReceiptHash = receiptHash,
            ),
        )

        assertEquals("0x$deploymentHash", request.publicInputs.sourceAdapterDeploymentHash)
        assertEquals("0x$receiptHash", request.publicInputs.sourceAdapterDeploymentReceiptHash)
        assertEquals(73, SccpSolana.canonicalSourceAdapterDeploymentBindingBytes(request.sourceAdapterDeploymentBinding).size)
        assertEquals(
            SccpSolana.sourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding),
            request.sourceAdapterDeploymentBindingHash,
        )

        val unsupportedSource = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceDomain = 99,
                targetDomain = SccpSolana.DOMAIN_SORA,
                sourceAdapterDeploymentHash = deploymentHash,
                sourceAdapterDeploymentReceiptHash = receiptHash,
            )
        }
        assertTrue(unsupportedSource.message?.contains("launch-scope remote domain") == true)

        val localSource = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceDomain = SccpSolana.DOMAIN_SORA,
                targetDomain = SccpSolana.DOMAIN_SORA,
                sourceAdapterDeploymentHash = deploymentHash,
                sourceAdapterDeploymentReceiptHash = receiptHash,
            )
        }
        assertTrue(localSource.message?.contains("launch-scope remote domain") == true)
        // Zero/zero deployment hashes are diagnostic-only and must not bypass route validation.
        val zeroLocalSource = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceDomain = SccpSolana.DOMAIN_SORA,
                targetDomain = SccpSolana.DOMAIN_SORA,
            )
        }
        assertTrue(zeroLocalSource.message?.contains("launch-scope remote domain") == true)

        val nonSoraTarget = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceDomain = SccpSolana.DOMAIN_SOLANA,
                targetDomain = SccpSourceProofs.DOMAIN_TON,
                sourceAdapterDeploymentHash = deploymentHash,
                sourceAdapterDeploymentReceiptHash = receiptHash,
            )
        }
        assertTrue(nonSoraTarget.message?.contains("targetDomain must be SORA") == true)
        val zeroNonSoraTarget = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceDomain = SccpSolana.DOMAIN_SOLANA,
                targetDomain = SccpSourceProofs.DOMAIN_TON,
            )
        }
        assertTrue(zeroNonSoraTarget.message?.contains("targetDomain must be SORA") == true)

        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceAdapterDeploymentHash = deploymentHash,
                sourceAdapterDeploymentReceiptHash = SccpSolana.ZERO_HASH_V1,
            )
        }
        assertTrue(error.message?.contains("must both be zero") == true)

        // Receipt-only deployment bindings must fail, not only deployment-only bindings.
        val receiptOnlyBinding = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceAdapterDeploymentHash = SccpSolana.ZERO_HASH_V1,
                sourceAdapterDeploymentReceiptHash = receiptHash,
            )
        }
        assertTrue(receiptOnlyBinding.message?.contains("must both be zero") == true)

        val reusedRoleHash = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeSourceAdapterDeploymentBinding(
                sourceAdapterDeploymentHash = deploymentHash,
                sourceAdapterDeploymentReceiptHash = deploymentHash,
            )
        }
        assertTrue(reusedRoleHash.message?.contains("must differ") == true)
    }

    @Test
    fun requiresProofContext() {
        val missingStatement = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeProofContext("", "78".repeat(32))
        }
        assertTrue(missingStatement.message?.contains("statementHash") == true)

        val zeroStatement = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeProofContext(SccpSolana.ZERO_HASH_V1, "78".repeat(32))
        }
        assertTrue(zeroStatement.message?.contains("statementHash") == true)

        val missingBinding = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeProofContext("56".repeat(32), "")
        }
        assertTrue(missingBinding.message?.contains("destinationBindingHash") == true)

        val zeroBinding = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeProofContext("56".repeat(32), SccpSolana.ZERO_HASH_V1)
        }
        assertTrue(zeroBinding.message?.contains("destinationBindingHash") == true)
    }

    @Test
    fun buildsSolanaProgramInstructionSubmission() {
        val solanaDestinationBindingHash =
            SccpSourceProofs.destinationBindingHash(SccpSolana.DOMAIN_SOLANA)
        val canonicalRequest = SccpSolana.buildProofRequest(
            sampleProductionWitness().copy(destinationBindingHash = solanaDestinationBindingHash),
        )
        val canonicalProofResult = SccpSolana.wrapProofResult(byteArrayOf(1, 2, 3, 4), canonicalRequest)
        val canonicalPublicInputs = sampleSubmissionPublicInputs(
            messageId = canonicalRequest.publicInputs.messageId,
            payloadHash = canonicalRequest.publicInputs.payloadHash,
            commitmentRoot = canonicalRequest.publicInputs.commitmentRoot,
            finalityHeight = canonicalRequest.publicInputs.finalizedSlot,
            finalityBlockHash = canonicalRequest.publicInputs.bankHash,
        )
        val submission = SccpSolana.buildSubmission(
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult,
                bundleBytes = byteArrayOf(5, 6, 7),
            ),
        )

        assertEquals(SccpSolana.BORSH_INSTRUCTION_V1, submission.envelopeEncoding)
        assertEquals("program_instruction", submission.submissionKind)
        assertEquals("submit_sccp_message_proof", submission.verifierEntrypoint)
        assertEquals(
            listOf(
                "proof_bytes",
                "public_inputs",
                "bundle_bytes",
                "statement_hash",
                "destination_binding_hash",
                "proof_context_hash",
            ),
            submission.arguments.map { it.key },
        )
        assertEquals(141, submission.publicInputsBytes.size)
        assertEquals(
            SccpSolana.proofContextHash(
                SccpSolana.normalizeProofContext("56".repeat(32), solanaDestinationBindingHash),
            ),
            submission.proofContextHash,
        )
        assertEquals(submission.instructionDataHex, submission.envelopeHex)
        assertEquals(
            "submit_sccp_message_proof",
            String(submission.instructionData.copyOfRange(4, 29), Charsets.UTF_8),
        )
        val exposedProof = submission.proofBytes
        val exposedPublicInputs = submission.publicInputsBytes
        val exposedBundle = submission.bundleBytes
        val exposedInstruction = submission.instructionData
        val exposedEnvelope = submission.envelopeBytes
        exposedProof[0] = 99
        exposedPublicInputs[0] = 99
        exposedBundle[0] = 99
        exposedInstruction[0] = 99
        exposedEnvelope[0] = 99
        assertContentEquals(byteArrayOf(1, 2, 3, 4), submission.proofBytes)
        assertTrue(submission.publicInputsBytes[0].toInt() != 99)
        assertContentEquals(byteArrayOf(5, 6, 7), submission.bundleBytes)
        assertTrue(submission.instructionData[0].toInt() != 99)
        assertTrue(submission.envelopeBytes[0].toInt() != 99)

        val proofResultSubmission = SccpSolana.buildSubmission(
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult,
                bundleBytes = byteArrayOf(5, 6, 7),
            ),
        )
        assertEquals(canonicalProofResult.proofContextHash, proofResultSubmission.proofContextHash)
        val uppercaseProofResult = canonicalProofResult.copy(
            publicInputs = canonicalProofResult.publicInputs.copy(
                statementHash = canonicalProofResult.publicInputs.statementHash.uppercase(),
                destinationBindingHash = canonicalProofResult.publicInputs.destinationBindingHash.uppercase(),
            ),
            proofContext = canonicalProofResult.proofContext.copy(
                statementHash = canonicalProofResult.proofContext.statementHash.uppercase(),
                destinationBindingHash = canonicalProofResult.proofContext.destinationBindingHash.uppercase(),
            ),
        )
        val uppercaseMetadata = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = canonicalPublicInputs.copy(
                        messageId = canonicalPublicInputs.messageId.uppercase(),
                        payloadHash = canonicalPublicInputs.payloadHash.uppercase(),
                        commitmentRoot = canonicalPublicInputs.commitmentRoot.uppercase(),
                        finalityBlockHash = canonicalPublicInputs.finalityBlockHash.uppercase(),
                    ),
                    proofBytes = canonicalProofResult.proofBytes,
                    bundleBytes = byteArrayOf(5, 6, 7),
                    statementHash = canonicalProofResult.proofContext.statementHash,
                    destinationBindingHash = solanaDestinationBindingHash,
                    proofContextHash = canonicalProofResult.proofContextHash,
                    proofResult = canonicalProofResult,
                ),
            )
        }
        assertTrue(uppercaseMetadata.message?.contains("publicInputs.messageId must be canonical hex") == true)
        val uppercaseProofResultError = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = canonicalPublicInputs,
                    proofBytes = uppercaseProofResult.proofBytes,
                    bundleBytes = byteArrayOf(5, 6, 7),
                    statementHash = canonicalProofResult.proofContext.statementHash,
                    destinationBindingHash = solanaDestinationBindingHash,
                    proofContextHash = canonicalProofResult.proofContextHash,
                    proofResult = uppercaseProofResult,
                ),
            )
        }
        assertTrue(
            uppercaseProofResultError.message?.contains(
                "proofResult.proofContext.statementHash must be canonical hex",
            ) == true || uppercaseProofResultError.message?.contains(
                "statementHash must be canonical hex",
            ) == true,
            "actual error: ${uppercaseProofResultError.message}",
        )
        val missingEnvelope = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(envelopeHash = SccpSolana.ZERO_HASH_V1),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(missingEnvelope.message?.contains("envelopeHash") == true)
        val tamperedEnvelope = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(envelopeHash = "0x" + "aa".repeat(32)),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(tamperedEnvelope.message?.contains("wrapped proof bytes") == true)
        val mismatchedProofContext = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(proofContextHash = "0x" + "99".repeat(32)),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(mismatchedProofContext.message?.contains("proofContextHash") == true)
        val mismatchedProofResultVersion = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(version = 2),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(mismatchedProofResultVersion.message?.contains("proofResult.version") == true)
        val mismatchedProofBase64 = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(proofBase64 = "AAAA"),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(mismatchedProofBase64.message?.contains("proofBase64") == true)
        val zeroWitnessHash = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(witnessHash = SccpSolana.ZERO_HASH_V1),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(zeroWitnessHash.message?.contains("proofResult.witnessHash") == true)
        val mismatchedProofContextVersion = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    proofContext = canonicalProofResult.proofContext.copy(version = 2),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(mismatchedProofContextVersion.message?.contains("proofContext.version") == true)
        val mismatchedSourceVerifier = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    sourceStateVerifierHash = SccpSolana.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(mismatchedSourceVerifier.message?.contains("template verifier") == true)
        val mismatchedDeploymentBindingVersion = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    sourceAdapterDeploymentBinding = canonicalProofResult.sourceAdapterDeploymentBinding.copy(
                        version = 2,
                    ),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(
            mismatchedDeploymentBindingVersion.message?.contains(
                "sourceAdapterDeploymentBinding.version",
            ) == true,
        )
        val mismatchedDeploymentBinding = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    sourceAdapterDeploymentBinding = canonicalProofResult.sourceAdapterDeploymentBinding.copy(
                        sourceAdapterDeploymentHash = "ee".repeat(32),
                    ),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(
            mismatchedDeploymentBinding.message?.contains(
                "sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding",
            ) == true,
        )
        val mismatchedDeploymentPublicInputs = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    publicInputs = canonicalProofResult.publicInputs.copy(
                        sourceAdapterDeploymentHash = "ee".repeat(32),
                    ),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(
            mismatchedDeploymentPublicInputs.message?.contains(
                "publicInputs.sourceAdapterDeploymentHash",
            ) == true,
        )
        val mismatchedPublicInputSourceVerifierId = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    publicInputs = canonicalProofResult.publicInputs.copy(
                        sourceStateVerifierId = "sccp:solana:wrong-source-state-verifier:v1",
                    ),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(
            mismatchedPublicInputSourceVerifierId.message?.contains(
                "publicInputs.sourceStateVerifierId",
            ) == true,
        )
        val mismatchedPublicInputSourceVerifierHash = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    publicInputs = canonicalProofResult.publicInputs.copy(
                        sourceStateVerifierHash = "0x" + "dd".repeat(32),
                    ),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(
            mismatchedPublicInputSourceVerifierHash.message?.contains(
                "publicInputs.sourceStateVerifierHash",
            ) == true,
        )
        val mismatchedPublicInputParentSlot = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    publicInputs = canonicalProofResult.publicInputs.copy(
                        parentSlot = canonicalProofResult.publicInputs.finalizedSlot,
                    ),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(
            mismatchedPublicInputParentSlot.message?.contains("publicInputs.parentSlot") == true,
        )
        val zeroPublicInputMessageProofHash = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs,
                proofResult = canonicalProofResult.copy(
                    publicInputs = canonicalProofResult.publicInputs.copy(
                        messageProofHash = SccpSolana.ZERO_HASH_V1,
                    ),
                ),
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(
            zeroPublicInputMessageProofHash.message?.contains("publicInputs.messageProofHash") == true,
        )
        val mismatchedMessage = assertFailsWith<IllegalArgumentException> {
            SolanaSccpSubmissionInput(
                publicInputs = canonicalPublicInputs.copy(messageId = "aa".repeat(32)),
                proofResult = canonicalProofResult,
                bundleBytes = byteArrayOf(5, 6, 7),
            )
        }
        assertTrue(mismatchedMessage.message?.contains("messageId") == true)

        val inputProofBytes = byteArrayOf(9)
        val inputBundleBytes = byteArrayOf(8)
        val copiedInput = SolanaSccpSubmissionInput(
            publicInputs = sampleSubmissionPublicInputs(),
            proofBytes = inputProofBytes,
            bundleBytes = inputBundleBytes,
            statementHash = "56".repeat(32),
            destinationBindingHash = solanaDestinationBindingHash,
        )
        inputProofBytes[0] = 0
        inputBundleBytes[0] = 0
        copiedInput.proofBytes[0] = 0
        copiedInput.bundleBytes[0] = 0
        assertContentEquals(byteArrayOf(9), copiedInput.proofBytes)
        assertContentEquals(byteArrayOf(8), copiedInput.bundleBytes)
        val rawInput = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                copiedInput,
            )
        }
        assertTrue(rawInput.message?.contains("proofResult") == true)
        val wrongTarget = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = sampleSubmissionPublicInputs(targetDomain = SccpSolana.DOMAIN_SORA),
                    proofBytes = byteArrayOf(1, 2),
                    bundleBytes = byteArrayOf(5, 6, 7),
                    statementHash = "56".repeat(32),
                    destinationBindingHash = solanaDestinationBindingHash,
                ),
            )
        }
        assertTrue(wrongTarget.message?.contains("publicInputs.targetDomain") == true)
        val wrongPublicInputVersion = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = sampleSubmissionPublicInputs(version = 2),
                    proofBytes = byteArrayOf(1, 2),
                    bundleBytes = byteArrayOf(5, 6, 7),
                    statementHash = "56".repeat(32),
                    destinationBindingHash = solanaDestinationBindingHash,
                ),
            )
        }
        assertTrue(wrongPublicInputVersion.message?.contains("publicInputs.version") == true)
        val wrongBinding = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = canonicalPublicInputs,
                    proofBytes = canonicalProofResult.proofBytes,
                    bundleBytes = byteArrayOf(2),
                    statementHash = canonicalProofResult.proofContext.statementHash,
                    destinationBindingHash = "78".repeat(32),
                    proofContextHash = canonicalProofResult.proofContextHash,
                    proofResult = canonicalProofResult,
                ),
            )
        }
        assertTrue(wrongBinding.message?.contains("destinationBindingHash") == true)
        val zeroBundle = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = canonicalPublicInputs,
                    proofBytes = canonicalProofResult.proofBytes,
                    bundleBytes = byteArrayOf(0, 0),
                    statementHash = canonicalProofResult.proofContext.statementHash,
                    destinationBindingHash = solanaDestinationBindingHash,
                    proofContextHash = canonicalProofResult.proofContextHash,
                    proofResult = canonicalProofResult,
                ),
            )
        }
        assertTrue(zeroBundle.message?.contains("bundleBytes must not be all zero") == true)
        val oversizedBundle = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = canonicalPublicInputs,
                    proofBytes = canonicalProofResult.proofBytes,
                    bundleBytes = ByteArray(SccpSolana.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                    statementHash = canonicalProofResult.proofContext.statementHash,
                    destinationBindingHash = solanaDestinationBindingHash,
                    proofContextHash = canonicalProofResult.proofContextHash,
                    proofResult = canonicalProofResult,
                ),
            )
        }
        assertTrue(oversizedBundle.message?.contains("bundleBytes must be at most") == true)
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.buildSubmission(
                SolanaSccpSubmissionInput(
                    publicInputs = canonicalPublicInputs,
                    proofBytes = canonicalProofResult.proofBytes,
                    bundleBytes = byteArrayOf(2),
                    statementHash = canonicalProofResult.proofContext.statementHash,
                    destinationBindingHash = solanaDestinationBindingHash,
                    proofContextHash = "cc".repeat(32),
                    proofResult = canonicalProofResult,
                ),
            )
        }
        assertTrue(error.message?.contains("proofContextHash") == true)
    }

    @Test
    fun proverWrapsExternalProofBytes() {
        val productionWitness = sampleProductionWitness()
        var seenRequest: SolanaSccpProofRequest? = null
        val prover = SolanaSccpProver(
            proofEngine = SolanaSccpProofEngine { request ->
                seenRequest = request
                assertEquals(SccpSolana.RECURSIVE_PROOF_BACKEND_V1, request.backend)
                assertEquals("0x" + "56".repeat(32), request.proofContext.statementHash)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(productionWitness)
        val expectedRequest = SccpSolana.buildProofRequest(productionWitness)

        assertEquals(listOf(1, 2, 3, 4), result.proofBytes.map { it.toInt() })
        assertEquals("AQIDBA==", result.proofBase64)
        assertEquals(expectedRequest.proofContextHash, result.proofContextHash)
        assertFalse(seenRequest === expectedRequest)
        assertEquals(expectedRequest.witnessHash, seenRequest?.witnessHash)
        assertEquals(expectedRequest.proofContextHash, seenRequest?.proofContextHash)
        assertEquals(
            expectedRequest.sourceAdapterDeploymentBindingHash,
            seenRequest?.sourceAdapterDeploymentBindingHash,
        )
        assertEquals(expectedRequest.sourceStateVerifierId, seenRequest?.sourceStateVerifierId)
        assertEquals(expectedRequest.sourceStateVerifierHash, seenRequest?.sourceStateVerifierHash)
        assertTrue(result.envelopeHash.matches(Regex("0x[0-9a-f]{64}")))

        val exposedProof = result.proofBytes
        exposedProof[0] = 9
        assertContentEquals(byteArrayOf(1, 2, 3, 4), result.proofBytes)

        val request = expectedRequest
        val zeroProof = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapProofResult(byteArrayOf(0, 0), request)
        }
        assertTrue(zeroProof.message?.contains("all zero") == true)
        val oversizedProof = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapProofResult(
                ByteArray(SccpSolana.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                request,
            )
        }
        assertTrue(oversizedProof.message?.contains("at most") == true)
        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapProofResult(byteArrayOf(1), request.copy(backend = "debug-solana-backend"))
        }
        assertTrue(wrongBackend.message?.contains("sccp-solana-recursive-mainnet-v1") == true)

        val wrongProofContext = assertFailsWith<IllegalArgumentException> {
            SccpSolana.wrapProofResult(byteArrayOf(1), request.copy(proofContextHash = "0x" + "99".repeat(32)))
        }
        assertTrue(wrongProofContext.message?.contains("canonical") == true)

        val wrongGenesis = assertFailsWith<IllegalArgumentException> {
            SolanaSccpProver(
                proofEngine = SolanaSccpProofEngine {
                    error("local prover should not be invoked")
                },
            ).prove(sampleProductionWitness(mainnetGenesisHash = "devnet"))
        }
        assertTrue(wrongGenesis.message?.contains("mainnetGenesisHash") == true)

        val missingAccountsLtHash = assertFailsWith<IllegalArgumentException> {
            SolanaSccpProver(
                proofEngine = SolanaSccpProofEngine {
                    error("local prover should not be invoked")
                },
            ).prove(sampleProductionWitness(accountsLtHash = null))
        }
        assertTrue(missingAccountsLtHash.message?.contains("accountsLtHash") == true)

        val missingProductionBinding = assertFailsWith<IllegalArgumentException> {
            SolanaSccpProver(
                proofEngine = SolanaSccpProofEngine {
                    error("local prover should not be invoked")
                },
            ).prove(
                sampleWitness(
                    sourceAdapterDeploymentHash = "ab".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "cd".repeat(32),
                ),
            )
        }
        assertTrue(missingProductionBinding.message?.contains("sourceStateVerifierHash") == true)
        val templateProductionBinding = assertFailsWith<IllegalArgumentException> {
            SolanaSccpProver(
                proofEngine = SolanaSccpProofEngine {
                    error("local prover should not be invoked")
                },
            ).prove(sampleProductionWitness(sourceStateVerifierHash = SccpSolana.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1))
        }
        assertTrue(templateProductionBinding.message?.contains("Solana template verifier hash") == true)
        val missingInclusionBranch = assertFailsWith<IllegalArgumentException> {
            SolanaSccpProver(
                proofEngine = SolanaSccpProofEngine {
                    error("local prover should not be invoked")
                },
            ).prove(
                sampleWitness(
                    sourceStateVerifierHash = "ef".repeat(32),
                    sourceAdapterDeploymentHash = "ab".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "cd".repeat(32),
                ),
            )
        }
        assertTrue(missingInclusionBranch.message?.contains("inclusionBranch") == true)
    }

    private fun sampleProductionWitness(
        mainnetGenesisHash: String = SccpSolana.MAINNET_GENESIS_HASH,
        sourceStateVerifierHash: String = "ef".repeat(32),
        accountsLtHash: ByteArray? = ByteArray(2_048) { ((it % 251) + 1).toByte() },
    ): SolanaSccpWitnessInput {
        val branch = listOf(ByteArray(32) { 0x56.toByte() })
        val sourceEventDigest = "34".repeat(32)
        val blockhash = "9a".repeat(32)
        val transactionStatusRoot = SccpSolana.transactionStatusRootFromBranch(
            sourceEventDigest,
            solanaSignature55,
            solanaProgram42,
            branch,
        )
        val messageProofHash = SccpSolana.messageProofHash(
            sourceEventDigest,
            transactionStatusRoot,
            solanaSignature55,
            solanaProgram42,
            branch,
        )
        val accountsLtHashChecksum = accountsLtHash?.let {
            SccpSolana.accountsLtHashChecksum(it)
        } ?: "88".repeat(32)
        val bankHash = accountsLtHash?.let {
            SccpSolana.agaveBankHash(
                parentBankHash = "c0".repeat(32),
                bankSignatureCount = "8",
                blockhash = blockhash,
                accountsLtHash = it,
            )
        } ?: "aa".repeat(32)
        return sampleWitness(
            mainnetGenesisHash = mainnetGenesisHash,
            blockhash = blockhash,
            bankHash = bankHash,
            accountsLtHashChecksum = accountsLtHashChecksum,
            accountsLtHash = accountsLtHash,
            messageProofHash = messageProofHash,
            inclusionBranch = branch,
            sourceStateVerifierHash = sourceStateVerifierHash,
            sourceAdapterDeploymentHash = "ab".repeat(32),
            sourceAdapterDeploymentReceiptHash = "cd".repeat(32),
        )
    }

    private fun sampleSolanaRouteCanaryEvidence(
        solanaProgramdataSlot: String = "4321",
        solanaProgramdataExecutableBase64: String = "f0VMRgECAwQF",
    ): SolanaSccpRouteCanaryEvidenceInput =
        SolanaSccpRouteCanaryEvidenceInput(
            routeAllowlistHash = "0x" + "31".repeat(32),
            destinationBindingHash = SccpSourceProofs.destinationBindingHash(SccpSolana.DOMAIN_SOLANA),
            sourceVerifierMaterialHash = "0x" + "33".repeat(32),
            sourceAdapterEngineDeploymentHash = "0x" + "34".repeat(32),
            verifierIdentity = "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
            verifierCodeHash = "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
            solanaProgramAccountDataBase64 = "AgAAABERERERERERERERERERERERERERERERERERERERERER",
            solanaProgramdataAddress = "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2",
            solanaProgramdataSlot = solanaProgramdataSlot,
            solanaExpectedProgramdataSlot = "4321",
            solanaProgramAccountContextSlot = "5000",
            solanaProgramdataAccountContextSlot = "5001",
            solanaProgramdataMetadataBlake2b256 =
                "0x2b5f26278ea949463e97c1dc5e53a821b82515b405454a1b0e3cd652c3b00209",
            solanaProgramdataMetadataBase64 =
                "AwAAAOEQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            solanaProgramdataExecutableBlake2b256 =
                "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
            solanaProgramdataExecutableBase64 = solanaProgramdataExecutableBase64,
        )

    private fun accountsLtHashRequest(
        request: SolanaSccpAccountsLtHashProofRequest,
        accountsLtHashProofPublicInputsHash: String = request.accountsLtHashProofPublicInputsHash,
        publicInputColumns: List<List<String>> = request.publicInputColumns,
        fastpqPublicInputs: SolanaSccpAccountsLtHashFastpqPublicInputs = request.fastpqPublicInputs,
        fastpqTransitions: List<SolanaSccpAccountsLtHashFastpqTransition> = request.fastpqTransitions,
    ): SolanaSccpAccountsLtHashProofRequest =
        SolanaSccpAccountsLtHashProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            sourceDomain = request.sourceDomain,
            finalizedSlot = request.finalizedSlot,
            parentSlot = request.parentSlot,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash = accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum = request.openedAccountsLtHashResidualChecksum,
            statementBytes = request.statementBytes,
            accountCommitmentBytes = request.accountCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = publicInputColumns,
            fastpqPublicInputs = fastpqPublicInputs,
            fastpqTransitions = fastpqTransitions,
        )

    private fun fullLightClientAuditRequest(
        request: SolanaSccpFullLightClientAuditProofRequest,
        verifierHash: String = request.verifierHash,
        auditStatementHash: String = request.auditStatementHash,
        publicInputColumns: List<List<String>> = request.publicInputColumns,
        fastpqPublicInputs: SolanaSccpFullLightClientAuditFastpqPublicInputs = request.fastpqPublicInputs,
        fastpqTransitions: List<SolanaSccpFullLightClientAuditFastpqTransition> = request.fastpqTransitions,
    ): SolanaSccpFullLightClientAuditProofRequest =
        SolanaSccpFullLightClientAuditProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            role = request.role,
            roleCode = request.roleCode,
            sourceDomain = request.sourceDomain,
            finalizedSlot = request.finalizedSlot,
            verifierId = request.verifierId,
            verifierHash = verifierHash,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            sourceVerifierMaterialHash = request.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = request.sourceAdapterDeploymentHash,
            fullLightClientGateHash = request.fullLightClientGateHash,
            finalityContextHash = request.finalityContextHash,
            voteMessageHash = request.voteMessageHash,
            accountsLtHashProofHash = request.accountsLtHashProofHash,
            auditStatementHash = auditStatementHash,
            statementBytes = request.statementBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = publicInputColumns,
            fastpqPublicInputs = fastpqPublicInputs,
            fastpqTransitions = fastpqTransitions,
        )

    private fun sampleWitness(
        targetDomain: Int = SccpSolana.DOMAIN_SORA,
        mainnetGenesisHash: String = SccpSolana.MAINNET_GENESIS_HASH,
        sourceEventDigest: String = "34".repeat(32),
        messageProofHash: String = "cc".repeat(32),
        inclusionBranch: List<ByteArray> = emptyList(),
        sourceStateVerifierId: String = SccpSolana.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
        sourceStateVerifierHash: String = SccpSolana.ZERO_HASH_V1,
        sourceAdapterDeploymentHash: String = SccpSolana.ZERO_HASH_V1,
        sourceAdapterDeploymentReceiptHash: String = SccpSolana.ZERO_HASH_V1,
        blockhash: String = "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
        bankHash: String = "aa".repeat(32),
        accountsLtHashChecksum: String = "88".repeat(32),
        accountsLtHash: ByteArray? = null,
        transactionSignature: String = solanaSignature55,
        emitterProgramId: String = solanaProgram42,
    ): SolanaSccpWitnessInput {
        val transactionStatusRoot = if (inclusionBranch.isEmpty()) {
            "bb".repeat(32)
        } else {
            SccpSolana.transactionStatusRootFromBranch(
                sourceEventDigest,
                transactionSignature,
                emitterProgramId,
                inclusionBranch,
            )
        }
        return SolanaSccpWitnessInput(
            targetDomain = targetDomain,
            mainnetGenesisHash = mainnetGenesisHash,
            finalizedSlot = "321",
            parentSlot = "320",
            bankSignatureCount = "8",
            parentBankHash = "c0".repeat(32),
            blockhash = blockhash,
            bankHash = bankHash,
            transactionStatusRoot = transactionStatusRoot,
            messageProofHash = messageProofHash,
            accountInclusionRoot = "77".repeat(32),
            accountsLtHashChecksum = accountsLtHashChecksum,
            accountsLtHash = accountsLtHash,
            transactionSignature = transactionSignature,
            emitterProgramId = emitterProgramId,
            messageId = "dd".repeat(32),
            payloadHash = "ee".repeat(32),
            commitmentRoot = "12".repeat(32),
            sourceEventDigest = sourceEventDigest,
            sourceStateVerifierId = sourceStateVerifierId,
            sourceStateVerifierHash = sourceStateVerifierHash,
            statementHash = "56".repeat(32),
            destinationBindingHash = "78".repeat(32),
            inclusionBranch = inclusionBranch,
            sourceAdapterDeploymentHash = sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash = sourceAdapterDeploymentReceiptHash,
        )
    }

    private fun hexBytes(value: String): ByteArray {
        val hex = value.removePrefix("0x").removePrefix("0X")
        return ByteArray(hex.length / 2) { index ->
            hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun byteArrayContains(haystack: ByteArray, needle: ByteArray): Boolean =
        needle.isEmpty() || haystack.asList().windowed(needle.size).any { it == needle.asList() }

    private fun hexLower(bytes: ByteArray): String = bytes.joinToString(separator = "") {
        (it.toInt() and 0xff).toString(16).padStart(2, '0')
    }

    private fun sampleSubmissionPublicInputs(
        version: Int = 1,
        targetDomain: Int = SccpSolana.DOMAIN_SOLANA,
        messageId: String = "dd".repeat(32),
        payloadHash: String = "ee".repeat(32),
        commitmentRoot: String = "12".repeat(32),
        finalityHeight: String = "321",
        finalityBlockHash: String = "aa".repeat(32),
    ): SolanaSccpSubmissionPublicInputs =
        SolanaSccpSubmissionPublicInputs(
            version = version,
            messageId = messageId,
            payloadHash = payloadHash,
            targetDomain = targetDomain,
            commitmentRoot = commitmentRoot,
            finalityHeight = finalityHeight,
            finalityBlockHash = finalityBlockHash,
        )
}
