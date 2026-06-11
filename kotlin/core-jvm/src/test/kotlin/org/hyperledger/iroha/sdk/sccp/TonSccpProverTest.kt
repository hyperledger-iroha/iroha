package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.crypto.Blake2b
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class TonSccpProverTest {
    @Test
    fun derivesTonRouteCanaryEvidenceHash() {
        val evidence = sampleTonRouteCanaryEvidence()

        assertEquals(358, SccpTon.canonicalRouteCanaryEvidenceBytes(evidence).size)
        assertEquals(
            "0xf128e8405017b9ca7733bb10d43eeaf783e38d39740a3455aa353c76655c6942",
            SccpTon.routeCanaryEvidenceHash(evidence),
        )

        val wrongDestinationBinding = assertFailsWith<IllegalArgumentException> {
            SccpTon.routeCanaryEvidenceHash(evidence.copy(destinationBindingHash = "0x" + "78".repeat(32)))
        }
        assertTrue(
            wrongDestinationBinding.message?.contains(
                "destinationBindingHash must match canonical TON destination binding",
            ) == true,
        )
        val wrongWorkchain = assertFailsWith<IllegalArgumentException> {
            SccpTon.routeCanaryEvidenceHash(evidence.copy(verifierContractAddress = "1:" + "11".repeat(32)))
        }
        assertTrue(wrongWorkchain.message?.contains("verifierContractAddress workchain") == true)
        val inactiveAccount = assertFailsWith<IllegalArgumentException> {
            SccpTon.routeCanaryEvidenceHash(evidence.copy(accountStatus = "uninit"))
        }
        assertTrue(inactiveAccount.message?.contains("accountStatus must be active") == true)
        val paddedLt = assertFailsWith<IllegalArgumentException> {
            SccpTon.routeCanaryEvidenceHash(evidence.copy(lastTransactionLt = "0123"))
        }
        assertTrue(paddedLt.message?.contains("lastTransactionLt must be a positive decimal") == true)
        val mismatchedCodeRoot = assertFailsWith<IllegalArgumentException> {
            SccpTon.routeCanaryEvidenceHash(evidence.copy(verifierCodeBocRootHash = "0x" + "45".repeat(32)))
        }
        assertTrue(mismatchedCodeRoot.message?.contains("verifierCodeBocRootHash") == true)
        listOf(
            evidence.copy(routeAllowlistHash = evidence.destinationBindingHash),
            evidence.copy(routeAllowlistHash = evidence.sourceVerifierMaterialHash),
            evidence.copy(routeAllowlistHash = evidence.sourceAdapterEngineDeploymentHash),
            evidence.copy(sourceVerifierMaterialHash = evidence.destinationBindingHash),
            evidence.copy(sourceAdapterEngineDeploymentHash = evidence.destinationBindingHash),
            evidence.copy(sourceAdapterEngineDeploymentHash = evidence.sourceVerifierMaterialHash),
        ).forEach { replay ->
            val failure = assertFailsWith<IllegalArgumentException> {
                SccpTon.routeCanaryEvidenceHash(replay)
            }
            assertTrue(failure.message?.contains("TON route canary governed hashes") == true)
        }
    }

    @Test
    fun buildsTonMessageBodyBoc() {
        val body = SccpTon.buildMessageBodyBoc(sampleMessageBodyInput())

        assertEquals(listOf(0xb5, 0xee, 0x9c, 0x72), body.take(4).map { it.toInt() and 0xff })
        assertTrue(body.size > SccpTon.canonicalPublicInputsBytes(samplePublicInputs()).size)

        val destinationBinding = TonSccpSubmissionDestinationBindingInput(
            key = "sora:ton",
            bindingHash = "78".repeat(32),
        )
        val manifest = TonSccpSubmissionManifestInput(
            messageBackend = "sccp-message-v1",
            registryBackend = "sccp-registry-v1",
            manifestSeed = "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
            destinationBinding = destinationBinding,
        )
        val metadata = SccpTon.canonicalSubmissionMetadataBytes(
            TonSccpSubmissionMetadataInput(
                manifest = manifest,
                destinationBindingHash = "78".repeat(32),
                publicInputs = samplePublicInputs(),
                statementHash = "bb".repeat(32),
            ),
        )
        assertTrue(metadata.size > SccpTon.canonicalPublicInputsBytes(samplePublicInputs()).size)
        val mismatchedBinding = assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalSubmissionMetadataBytes(
                TonSccpSubmissionMetadataInput(
                    manifest = manifest,
                    destinationBindingHash = "56".repeat(32),
                    publicInputs = samplePublicInputs(),
                    statementHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(mismatchedBinding.message?.contains("destinationBindingHash") == true)
        val wrongManifest = assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalSubmissionMetadataBytes(
                TonSccpSubmissionMetadataInput(
                    manifest = manifest.copy(counterpartyDomain = SccpSolana.DOMAIN_SOLANA),
                    publicInputs = samplePublicInputs(),
                    statementHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(wrongManifest.message?.contains("counterpartyDomain") == true)

        val submission = SccpTon.buildSubmission(sampleMessageBodyInput())
        assertEquals(1, submission.version)
        assertEquals(SccpTon.MESSAGE_BODY_BOC_V1, submission.envelopeEncoding)
        assertEquals("internal_message", submission.submissionKind)
        assertEquals("op::submit_sccp_message_proof", submission.verifierEntrypoint)
        assertEquals(body.toList(), submission.messageBodyBoc.toList())
        assertTrue(submission.messageBodyBocHex.startsWith("0xb5ee9c72"))
        assertEquals(listOf("message_body_boc"), submission.arguments.map { it.key })
        assertEquals(listOf("ton_boc"), submission.arguments.map { it.encoding })
        assertEquals(listOf(submission.messageBodyBocHex), submission.arguments.map { it.bytesHex })
        assertContentEquals(body, submission.envelopeBytes)
        assertEquals(submission.messageBodyBocHex, submission.envelopeHex)
        val exposedBody = submission.messageBodyBoc
        exposedBody[0] = 0
        assertContentEquals(body, submission.messageBodyBoc)
        val exposedEnvelope = submission.envelopeBytes
        exposedEnvelope[0] = 0
        assertContentEquals(body, submission.envelopeBytes)

        val proofBytes = byteArrayOf(1, 2)
        val bundleBytes = sampleTonBundleBytes()
        val expectedBundleBytes = bundleBytes.copyOf()
        val metadataBytes = byteArrayOf(5, 6)
        val copiedRequest = SccpTon.buildProofRequest(
            sampleProofRequestInput(
                bundleBytes = bundleBytes,
                sourceProofBytes = byteArrayOf(9, 10),
                statementHash = "bb".repeat(32),
                destinationBindingHash = "56".repeat(32),
            ),
        )
        val copiedResult = SccpTon.wrapProofResult(proofBytes, copiedRequest)
        val copiedInput = TonSccpMessageBodyInput(
            proofResult = copiedResult,
            bundleBytes = bundleBytes,
            metadataBytes = metadataBytes,
        )
        proofBytes[0] = 9
        bundleBytes[0] = 9
        metadataBytes[0] = 9
        copiedInput.proofBytes[0] = 9
        copiedInput.bundleBytes[0] = 9
        copiedInput.metadataBytes[0] = 9
        assertContentEquals(byteArrayOf(1, 2), copiedInput.proofBytes)
        assertContentEquals(expectedBundleBytes, copiedInput.bundleBytes)
        assertContentEquals(byteArrayOf(5, 6), copiedInput.metadataBytes)
        val emptyBundle = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(sampleMessageBodyInput(bundleBytes = ByteArray(0)))
        }
        assertTrue(emptyBundle.message?.contains("bundleBytes") == true)
        val zeroBundle = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(sampleMessageBodyInput(bundleBytes = byteArrayOf(0, 0)))
        }
        assertTrue(zeroBundle.message?.contains("bundleBytes must not be all zero") == true)
        val oversizedBundle = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(
                sampleMessageBodyInput(
                    bundleBytes = ByteArray(SccpTon.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                ),
            )
        }
        assertTrue(oversizedBundle.message?.contains("bundleBytes must be at most") == true)
        val zeroProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(sampleMessageBodyInput(proofBytes = byteArrayOf(0, 0)))
        }
        assertTrue(zeroProof.message?.contains("all zero") == true)
        val zeroStatementHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(sampleMessageBodyInput(statementHash = "00".repeat(32)))
        }
        assertTrue(zeroStatementHash.message?.contains("statementHash must not be zero") == true)
        val zeroDestinationBindingHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(sampleMessageBodyInput(destinationBindingHash = "00".repeat(32)))
        }
        assertTrue(
            zeroDestinationBindingHash.message?.contains("destinationBindingHash must not be zero") == true,
        )
        val wrongTargetDomain = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(
                sampleMessageBodyInput(
                    publicInputs = samplePublicInputs().copy(targetDomain = SccpSolana.DOMAIN_SOLANA),
                ),
            )
        }
        assertTrue(wrongTargetDomain.message?.contains("targetDomain must be TON") == true)
    }

    @Test
    fun derivesTonBocRootHashFromOrdinaryCells() {
        val boc = hexBytes("b5ee9c720101020100070001020101000202")
        val checkedBoc = hexBytes("b5ee9c724101020100070001020101000202be1c1df5")
        val rootHash = "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe"

        assertEquals(listOf(rootHash), SccpTon.bocRootHashes(boc))
        assertEquals(rootHash, SccpTon.bocSingleRootHash(boc))
        assertEquals(rootHash, SccpTon.bocSingleRootHash(checkedBoc))

        val badCrc = checkedBoc.copyOf()
        badCrc[badCrc.size - 1] = (badCrc[badCrc.size - 1].toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> { SccpTon.bocSingleRootHash(badCrc) }

        val changedChild = boc.copyOf()
        changedChild[changedChild.size - 1] = (changedChild[changedChild.size - 1].toInt() xor 1).toByte()
        assertTrue(SccpTon.bocSingleRootHash(changedChild) != rootHash)

        val cyclicRef = boc.copyOf()
        cyclicRef[14] = 0
        assertFailsWith<IllegalArgumentException> { SccpTon.bocSingleRootHash(cyclicRef) }

        val exoticCell = boc.copyOf()
        exoticCell[11] = (exoticCell[11].toInt() or 0x08).toByte()
        assertFailsWith<IllegalArgumentException> { SccpTon.bocSingleRootHash(exoticCell) }

        val invalidPartialData = boc.copyOf()
        invalidPartialData[16] = 1
        invalidPartialData[17] = 0
        assertFailsWith<IllegalArgumentException> { SccpTon.bocSingleRootHash(invalidPartialData) }

        val prunedBranchBoc = hexBytes(
            "b5ee9c72010101010026002848010149725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe0001",
        )
        assertEquals(
            "0xcc9095f882fb62a27bb19ad4aa84e19571a3283988ae40b75e238ad240cf1a96",
            SccpTon.bocSingleRootHash(prunedBranchBoc),
        )

        val legacyPrunedProofBoc = hexBytes(
            "b5ee9c7201010601005f0022012001052201620203284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0040004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001",
        )
        assertEquals(
            "0x9c769b035b601b0ddc098e9b148d9bdab0761c14bfe310ac090962ba1f39739a",
            SccpTon.bocSingleRootHash(legacyPrunedProofBoc),
        )

        val merkleProofBoc = hexBytes(
            "b5ee9c7201010301002d0009460349725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe00010101020102000202",
        )
        assertEquals(
            "0xe749bc5225cabbe3fa78fc12d74a734c365379bc0d302123dcf7bfa2ee3fbd21",
            SccpTon.bocSingleRootHash(merkleProofBoc),
        )
        val mismatchedMerkleProof = merkleProofBoc.copyOf()
        mismatchedMerkleProof[14] = (mismatchedMerkleProof[14].toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> { SccpTon.bocSingleRootHash(mismatchedMerkleProof) }

        val hashmapBoc = hexBytes(
            "b5ee9c72010109010028000101c001020120020702016203050103a0c004000403090103a0c0060004006f0101de08000403e7",
        )
        val hashmapValueHash = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419"
        assertEquals(hashmapValueHash, SccpTon.hashmapECellRefValueHash(hashmapBoc, byteArrayOf(17), 8))
        assertEquals(null, SccpTon.hashmapECellRefValueHash(hashmapBoc, byteArrayOf(18), 8))
        assertFailsWith<IllegalArgumentException> {
            SccpTon.hashmapECellRefValueHash(hashmapBoc, byteArrayOf(17), 7)
        }

        val hashmapDirectProofBoc = hexBytes(
            "b5ee9c72010107010063002101c00122012002062201620304284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0050004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001",
        )
        assertEquals(
            hashmapValueHash,
            SccpTon.hashmapECellRefValueHash(hashmapDirectProofBoc, byteArrayOf(17), 8),
        )
        assertEquals(null, SccpTon.hashmapECellRefValueHash(hashmapDirectProofBoc, byteArrayOf(1), 8))

        val hashmapMerkleProofBoc = hexBytes(
            "b5ee9c72010108010089000101c001094603e714f85374c2c336ed499a5a35e6c4f87441184532e7c23be795ce71b457f1bf00030222012003072201620405284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0060004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001",
        )
        assertEquals(
            hashmapValueHash,
            SccpTon.hashmapECellRefValueHash(hashmapMerkleProofBoc, byteArrayOf(17), 8),
        )

        val shardAccountsBoc = hexBytes(
            "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000",
        )
        val shardAccountKey = ByteArray(32).also { it[0] = 17.toByte() }
        val absentShardAccountKey = ByteArray(32).also { it[0] = 18.toByte() }
        assertEquals(
            hashmapValueHash,
            SccpTon.shardAccountsLastTransactionHash(shardAccountsBoc, shardAccountKey, 256),
        )
        assertEquals(
            SccpTon.ShardAccountLastTransaction(hashmapValueHash, BigInteger.valueOf(7)),
            SccpTon.shardAccountsLastTransaction(shardAccountsBoc, shardAccountKey, 256),
        )
        assertEquals(null, SccpTon.shardAccountsLastTransactionHash(shardAccountsBoc, absentShardAccountKey, 256))
        assertFailsWith<IllegalArgumentException> {
            SccpTon.shardAccountsLastTransactionHash(shardAccountsBoc, byteArrayOf(17), 8)
        }

        val shardStateProofBoc = hexBytes(
            "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000",
        )
        assertEquals(
            "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270",
            SccpTon.shardStateProofRootHash(shardStateProofBoc),
        )
        assertEquals(
            "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3",
            SccpTon.shardStateAccountsRootHash(shardStateProofBoc),
        )
        val badShardStateTag = shardStateProofBoc.copyOf()
        val tagOffset = indexOfBytes(badShardStateTag, hexBytes("9023afe2"))
        assertTrue(tagOffset >= 0)
        badShardStateTag[tagOffset] = (badShardStateTag[tagOffset].toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> {
            SccpTon.shardStateAccountsRootHash(badShardStateTag)
        }
        val shardIdentOffset = tagOffset + 8
        val badShardIdentTag = shardStateProofBoc.copyOf()
        badShardIdentTag[shardIdentOffset] = (badShardIdentTag[shardIdentOffset].toInt() or 0x80).toByte()
        assertFailsWith<IllegalArgumentException> {
            SccpTon.shardStateAccountsRootHash(badShardIdentTag)
        }
        val badShardIdentPrefixLen = shardStateProofBoc.copyOf()
        badShardIdentPrefixLen[shardIdentOffset] = 0x3d.toByte()
        assertFailsWith<IllegalArgumentException> {
            SccpTon.shardStateAccountsRootHash(badShardIdentPrefixLen)
        }
    }

    @Test
    fun derivesTonShardProofHashFromWitnessMaterial() {
        val branch = listOf(ByteArray(32) { 0xee.toByte() })
        val shardStateBranch = listOf(ByteArray(32) { 0x12.toByte() })
        val bytes = SccpTon.canonicalShardProofBytes(
            sourceEventDigest = "34".repeat(32),
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = "cc".repeat(32),
            transactionRoot = "dd".repeat(32),
            transactionLt = "7",
            shardStateLeafIndex = "0",
            shardStateInclusionBranch = shardStateBranch,
            inclusionBranch = branch,
        )

        assertEquals(309, bytes.size)
        assertEquals(1, bytes.first().toInt())

        val hash = SccpTon.shardProofHash(
            sourceEventDigest = "34".repeat(32),
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = "cc".repeat(32),
            transactionRoot = "dd".repeat(32),
            transactionLt = "7",
            shardStateLeafIndex = "0",
            shardStateInclusionBranch = shardStateBranch,
            inclusionBranch = branch,
        )
        val changed = SccpTon.shardProofHash(
            sourceEventDigest = "34".repeat(32),
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = "cc".repeat(32),
            transactionRoot = "dd".repeat(32),
            transactionLt = "7",
            shardStateLeafIndex = "0",
            shardStateInclusionBranch = shardStateBranch,
            inclusionBranch = listOf(ByteArray(32) { 0x12.toByte() }),
        )
        val changedShardState = SccpTon.shardProofHash(
            sourceEventDigest = "34".repeat(32),
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = "cc".repeat(32),
            transactionRoot = "dd".repeat(32),
            transactionLt = "7",
            shardStateLeafIndex = "0",
            shardStateInclusionBranch = listOf(ByteArray(32) { 0xee.toByte() }),
            inclusionBranch = branch,
        )
        assertEquals("0x09c63ca1185b537f0a37b7b248600a0992e5b7ed64ace9d1d437db7caae00686", hash)
        assertTrue(hash != changed)
        assertTrue(hash != changedShardState)
        val zeroSourceEventDigest = assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "00".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = shardStateBranch,
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroSourceEventDigest.message.orEmpty().contains("sourceEventDigest must not be zero"))

        val hashmapBoc = hexBytes(
            "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000",
        )
        val shardStateProofBoc = hexBytes(
            "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000",
        )
        val shardAccountKey = ByteArray(32).also { it[0] = 17.toByte() }
        val shardStateRoot = "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270"
        val shardAccountsRoot = "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3"
        val dictionaryBytes = SccpTon.canonicalShardProofBytes(
            sourceEventDigest = "34".repeat(32),
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = shardStateRoot,
            transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt = "7",
            shardStateLeafIndex = "0",
            shardStateInclusionBranch = emptyList(),
            inclusionBranch = branch,
            shardStateDictionaryRoot = shardAccountsRoot,
            shardStateDictionaryKeyBitLen = 256,
            shardStateDictionaryKey = shardAccountKey,
            shardStateDictionaryProofBoc = hashmapBoc,
            shardStateProofBoc = shardStateProofBoc,
        )
        val dictionaryHash = SccpTon.shardProofHash(
            sourceEventDigest = "34".repeat(32),
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = shardStateRoot,
            transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt = "7",
            shardStateLeafIndex = "0",
            shardStateInclusionBranch = emptyList(),
            inclusionBranch = branch,
            shardStateDictionaryRoot = shardAccountsRoot,
            shardStateDictionaryKeyBitLen = 256,
            shardStateDictionaryKey = shardAccountKey,
            shardStateDictionaryProofBoc = hashmapBoc,
            shardStateProofBoc = shardStateProofBoc,
        )
        assertEquals(662, dictionaryBytes.size)
        assertEquals("0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf", dictionaryHash)
        assertTrue(dictionaryHash != hash)
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = shardStateRoot,
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "8",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = shardStateProofBoc,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = shardStateRoot,
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = shardStateBranch,
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = shardStateProofBoc,
            )
        }
        val wrongGlobalIdProofBoc = shardStateProofBoc.copyOf()
        val wrongGlobalIdTagOffset = indexOfBytes(wrongGlobalIdProofBoc, hexBytes("9023afe2"))
        assertTrue(wrongGlobalIdTagOffset >= 0)
        wrongGlobalIdProofBoc.fill(0, wrongGlobalIdTagOffset + 4, wrongGlobalIdTagOffset + 8)
        assertEquals(shardAccountsRoot, SccpTon.shardStateAccountsRootHash(wrongGlobalIdProofBoc))
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = SccpTon.shardStateProofRootHash(wrongGlobalIdProofBoc),
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = wrongGlobalIdProofBoc,
            )
        }
        val wrongWorkchainIdProofBoc = shardStateProofBoc.copyOf()
        val wrongWorkchainIdTagOffset = indexOfBytes(wrongWorkchainIdProofBoc, hexBytes("9023afe2"))
        assertTrue(wrongWorkchainIdTagOffset >= 0)
        val wrongWorkchainShardIdentOffset = wrongWorkchainIdTagOffset + 8
        wrongWorkchainIdProofBoc.fill(
            0xff.toByte(),
            wrongWorkchainShardIdentOffset + 1,
            wrongWorkchainShardIdentOffset + 5,
        )
        assertEquals(shardAccountsRoot, SccpTon.shardStateAccountsRootHash(wrongWorkchainIdProofBoc))
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = SccpTon.shardStateProofRootHash(wrongWorkchainIdProofBoc),
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = wrongWorkchainIdProofBoc,
            )
        }
        val zeroGenUtimeProofBoc = shardStateProofBoc.copyOf()
        val zeroGenUtimeTagOffset = indexOfBytes(zeroGenUtimeProofBoc, hexBytes("9023afe2"))
        assertTrue(zeroGenUtimeTagOffset >= 0)
        zeroGenUtimeProofBoc.fill(0, zeroGenUtimeTagOffset + 29, zeroGenUtimeTagOffset + 33)
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = SccpTon.shardStateProofRootHash(zeroGenUtimeProofBoc),
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = zeroGenUtimeProofBoc,
            )
        }
        val futureMinRefMcSeqnoProofBoc = shardStateProofBoc.copyOf()
        val futureMinRefMcSeqnoTagOffset = indexOfBytes(futureMinRefMcSeqnoProofBoc, hexBytes("9023afe2"))
        assertTrue(futureMinRefMcSeqnoTagOffset >= 0)
        futureMinRefMcSeqnoProofBoc[futureMinRefMcSeqnoTagOffset + 44] = 0x14.toByte()
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = SccpTon.shardStateProofRootHash(futureMinRefMcSeqnoProofBoc),
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = futureMinRefMcSeqnoProofBoc,
            )
        }
        val basechainCustomProofBoc = shardStateProofBoc.copyOf()
        val basechainCustomTagOffset = indexOfBytes(basechainCustomProofBoc, hexBytes("9023afe2"))
        assertTrue(basechainCustomTagOffset >= 0)
        basechainCustomProofBoc[basechainCustomTagOffset + 45] =
            (basechainCustomProofBoc[basechainCustomTagOffset + 45].toInt() or 0x40).toByte()
        assertFailsWith<IllegalArgumentException> {
            SccpTon.shardStateAccountsRootHash(basechainCustomProofBoc)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = SccpTon.shardStateProofRootHash(basechainCustomProofBoc),
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = basechainCustomProofBoc,
            )
        }
        val mismatchedShardPrefixProofBoc = shardStateProofBoc.copyOf()
        val mismatchedTagOffset = indexOfBytes(mismatchedShardPrefixProofBoc, hexBytes("9023afe2"))
        assertTrue(mismatchedTagOffset >= 0)
        val mismatchedShardIdentOffset = mismatchedTagOffset + 8
        mismatchedShardPrefixProofBoc[mismatchedShardIdentOffset] = 0x08.toByte()
        mismatchedShardPrefixProofBoc[mismatchedShardIdentOffset + 5] = 0x12.toByte()
        assertEquals(shardAccountsRoot, SccpTon.shardStateAccountsRootHash(mismatchedShardPrefixProofBoc))
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "1333065489701666816",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = SccpTon.shardStateProofRootHash(mismatchedShardPrefixProofBoc),
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = mismatchedShardPrefixProofBoc,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "66".repeat(32),
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = shardStateProofBoc,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = shardStateRoot,
                transactionRoot = "66".repeat(32),
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = shardStateProofBoc,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = shardStateRoot,
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = emptyList(),
                inclusionBranch = branch,
                shardStateDictionaryRoot = "66".repeat(32),
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = shardStateProofBoc,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = shardStateRoot,
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = shardStateBranch,
                inclusionBranch = branch,
                shardStateDictionaryRoot = "00".repeat(32),
                shardStateDictionaryKeyBitLen = 256,
                shardStateDictionaryKey = shardAccountKey,
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = shardStateProofBoc,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = shardStateRoot,
                transactionRoot = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = shardStateBranch,
                inclusionBranch = branch,
                shardStateDictionaryRoot = shardAccountsRoot,
                shardStateDictionaryKeyBitLen = 7,
                shardStateDictionaryKey = byteArrayOf(17),
                shardStateDictionaryProofBoc = hashmapBoc,
                shardStateProofBoc = shardStateProofBoc,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = shardStateBranch,
                inclusionBranch = listOf(byteArrayOf(1, 2, 3)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = listOf(byteArrayOf(1, 2, 3)),
                inclusionBranch = branch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalShardProofBytes(
                sourceEventDigest = "34".repeat(32),
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                transactionLt = "7",
                shardStateLeafIndex = "0",
                shardStateInclusionBranch = shardStateBranch,
                inclusionBranch = List(65) { ByteArray(32) { 0xee.toByte() } },
            )
        }
    }

    @Test
    fun buildsTonShardStateOpenVerifyProofRequestFromWitnessMaterial() {
        val input = sampleShardStateProofRequestInput()
        val statement = SccpTon.canonicalShardStateProofPublicInputsBytes(input)
        val witness = SccpTon.canonicalShardStateWitnessCommitmentBytes(input)
        val context = SccpTon.canonicalShardStateVerificationContextBytes(input)
        val schema = SccpTon.shardStateOpenVerifySchemaDescriptor(input)
        val publicInputsHash = SccpTon.shardStateProofPublicInputsHash(input)
        val columns = SccpTon.shardStatePublicInputColumns(input)
        val request = SccpTon.buildShardStateProofRequest(input)

        assertEquals(603, statement.size)
        assertEquals(
            "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19",
            publicInputsHash,
        )
        assertEquals(480, witness.size)
        assertEquals(467, context.size)
        assertEquals(436, schema.size)
        assertEquals(SccpTon.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1, request.circuitId)
        assertEquals("stark-fri-v1", request.proofFamily)
        assertEquals("0x27e44edc7d124906a8176e94557996c3", request.fastpqPublicInputs.dsid)
        assertEquals(publicInputsHash, request.fastpqPublicInputs.txSetHash)
        assertEquals(publicInputsHash, request.shardStateProofPublicInputsHash)
        assertEquals(publicInputsHash, columns[15][0])
        assertEquals(publicInputsHash, request.publicInputColumns[15][0])
        assertEquals("sccp:ton:shard-state:v1:statement", request.fastpqTransitions[0].key)
        assertEquals("sccp:ton:shard-state:v1:witness", request.fastpqTransitions[1].key)
        assertEquals("sccp:ton:shard-state:v1:context", request.fastpqTransitions[2].key)
        assertContentEquals(statement, request.statementBytes)
        assertContentEquals(witness, request.witnessCommitmentBytes)
        assertContentEquals(context, request.verificationContextBytes)
        assertContentEquals(schema, request.schemaDescriptor)
        val transitionProof = sampleValidatorSetTransitionProofInput()
        val transitionBoundInput = sampleShardStateProofRequestInput(
            validatorSetTransitionProofs = listOf(transitionProof),
        )
        val tamperedTransitionSignature = transitionProof.validatorSignatureProof.signatures[0].copyOf()
        tamperedTransitionSignature[0] = (tamperedTransitionSignature[0].toInt() xor 0x01).toByte()
        val tamperedTransitionProof = sampleValidatorSetTransitionProofInput(
            signatures = listOf(
                tamperedTransitionSignature,
                transitionProof.validatorSignatureProof.signatures[1],
            ),
        )
        assertFalse(
            SccpTon.canonicalShardStateProofPublicInputsBytes(transitionBoundInput).contentEquals(
                SccpTon.canonicalShardStateProofPublicInputsBytes(
                    transitionBoundInput.copy(
                        validatorSetTransitionProofs = listOf(tamperedTransitionProof),
                    ),
                ),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpTon.buildShardStateProofRequest(
                sampleShardStateProofRequestInput(
                    sourceStateVerifierHash =
                        "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
                ),
            )
        }

        val exposedStatement = request.statementBytes
        exposedStatement[0] = 9
        assertContentEquals(statement, request.statementBytes)

        assertFailsWith<IllegalArgumentException> {
            SccpTon.shardStateProofPublicInputsHash(input.copy(transactionRoot = "66".repeat(32)))
        }
    }

    @Test
    fun buildsTonFullLightClientAuditRoleProofRequests() {
        val input = sampleFullLightClientAuditProofInput()
        val requests = SccpTon.buildFullLightClientAuditProofRequests(input)
        val shardStateProofPublicInputsHash = SccpTon.shardStateProofPublicInputsHash(input.shardState)
        val shardStateVerificationProofHash =
            SccpTon.shardStateVerificationProofHash(input.shardStateVerificationProof)
        val wrongShardStateVerificationProofVersion = assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalSourceStateVerificationProofBytes(input.shardStateVerificationProof.copy(version = 0))
        }
        assertTrue(wrongShardStateVerificationProofVersion.message!!.contains("sourceStateVerificationProof"))
        val proofBytes = byteArrayOf(4, 5, 6)
        val copiedProof = TonSccpSourceStateVerificationProof(proofBytes = proofBytes)
        proofBytes[0] = 0
        copiedProof.proofBytes[1] = 0
        assertContentEquals(byteArrayOf(4, 5, 6), copiedProof.proofBytes)
        assertEquals("BAUG", copiedProof.proofBase64)
        val allRequests = listOf(
            requests.masterchainConfig,
            requests.validatorSetTransition,
            requests.shardAccountsDictionary,
        )

        assertEquals(SccpTon.MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1, requests.masterchainConfig.circuitId)
        assertEquals(
            SccpTon.VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
            requests.validatorSetTransition.circuitId,
        )
        assertEquals(
            SccpTon.SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
            requests.shardAccountsDictionary.circuitId,
        )
        assertEquals(3, allRequests.map { it.circuitId }.toSet().size)
        assertEquals(
            listOf("masterchain_config", "validator_set_transition", "shard_accounts_dictionary"),
            allRequests.map { it.role },
        )
        assertTrue(
            SccpTon.canonicalFullLightClientAuditStatementBytes(
                input,
                TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG,
            ).isNotEmpty(),
        )

        allRequests.forEach { request ->
            val role = when (request.roleCode) {
                1 -> TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG
                2 -> TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION
                3 -> TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY
                else -> throw AssertionError("unexpected TON audit role")
            }
            assertEquals(1, request.version)
            assertEquals("stark-fri-v1", request.proofFamily)
            assertEquals("fastpq-lane-balanced", request.parameterSet)
            assertEquals(SccpTon.DOMAIN_TON, request.sourceDomain)
            assertEquals("19", request.masterchainSeqno)
            assertEquals("7", request.shardSeqno)
            assertEquals(SccpTon.MAINNET_SHARD_STATE_VERIFIER_ID_V1, request.sourceStateVerifierId)
            assertEquals(input.fullLightClientGateHash, request.fullLightClientGateHash)
            assertEquals(shardStateProofPublicInputsHash, request.shardStateProofPublicInputsHash)
            assertEquals(shardStateVerificationProofHash, request.shardStateVerificationProofHash)
            assertEquals(SccpTon.fullLightClientAuditStatementHash(input, role), request.auditStatementHash)
            assertEquals(3, request.fastpqTransitions.size)
            assertEquals(request.fastpqTransitions.map { it.key }.sorted(), request.fastpqTransitions.map { it.key })
            assertTrue(request.fastpqTransitions.all { it.key.startsWith("0x") })
        }

        assertEquals(17, requests.masterchainConfig.publicInputColumns.size)
        assertEquals(16, requests.validatorSetTransition.publicInputColumns.size)
        assertEquals(17, requests.shardAccountsDictionary.publicInputColumns.size)
        assertEquals(
            SccpTon.fullLightClientAuditPublicInputColumns(
                input,
                TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG,
            ),
            requests.masterchainConfig.publicInputColumns,
        )
        assertContentEquals(
            SccpTon.fullLightClientAuditOpenVerifySchemaDescriptor(
                input,
                TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG,
            ),
            requests.masterchainConfig.schemaDescriptor,
        )
        assertEquals(input.shardState.masterchainConfigRoot, requests.masterchainConfig.fastpqPublicInputs.oldRoot)
        assertEquals(input.shardState.sourceTrustAnchorHash, requests.validatorSetTransition.fastpqPublicInputs.oldRoot)
        assertEquals(input.shardState.transactionRoot, requests.shardAccountsDictionary.fastpqPublicInputs.newRoot)
        val originalAuditStatement = requests.masterchainConfig.statementBytes
        requests.masterchainConfig.statementBytes[0] = 0
        assertContentEquals(originalAuditStatement, requests.masterchainConfig.statementBytes)
        val originalAuditSchema = requests.shardAccountsDictionary.schemaDescriptor
        requests.shardAccountsDictionary.schemaDescriptor[0] = 0
        assertContentEquals(originalAuditSchema, requests.shardAccountsDictionary.schemaDescriptor)

        val shardRequest = SccpTon.buildShardStateProofRequest(input.shardState)
        val wrappedShard = SccpTon.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), shardRequest)
        assertEquals(SccpTon.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1, wrappedShard.circuitId)
        assertContentEquals(byteArrayOf(9, 8, 7), wrappedShard.proofBytes)
        assertEquals("CQgH", wrappedShard.proofBase64)
        wrappedShard.proofBytes[0] = 0
        assertContentEquals(byteArrayOf(9, 8, 7), wrappedShard.proofBytes)
        assertEquals("CQgH", wrappedShard.proofBase64)
        assertTrue(SccpTon.canonicalSourceStateVerificationProofBytes(wrappedShard).isNotEmpty())
        val wrappedAudit = SccpTon.wrapSourceStateVerificationProof(byteArrayOf(1, 2, 3), requests.masterchainConfig)
        assertEquals(SccpTon.MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1, wrappedAudit.circuitId)
        assertEquals("AQID", wrappedAudit.proofBase64)
        assertTrue(SccpTon.canonicalSourceStateVerificationProofBytes(wrappedAudit).isNotEmpty())
        val allZeroWrappedProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapSourceStateVerificationProof(byteArrayOf(0, 0), shardRequest)
        }
        assertTrue(allZeroWrappedProof.message!!.contains("all zero"))
        val oversizedSourceStateProofBytes = ByteArray(SccpTon.SOURCE_STATE_MAX_PROOF_BYTES + 1) { 1 }
        val oversizedWrappedProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapSourceStateVerificationProof(oversizedSourceStateProofBytes, shardRequest)
        }
        assertTrue(oversizedWrappedProof.message!!.contains("proofBytes must be at most"))
        val tamperedShardTransitions = shardRequest.fastpqTransitions.toMutableList()
        tamperedShardTransitions[0] = tamperedShardTransitions[0].copy(newValue = "0x00")
        val tamperedShardRequest = TonShardStateProofRequest(
            version = shardRequest.version,
            proofFamily = shardRequest.proofFamily,
            circuitId = shardRequest.circuitId,
            parameterSet = shardRequest.parameterSet,
            sourceDomain = shardRequest.sourceDomain,
            masterchainSeqno = shardRequest.masterchainSeqno,
            shardSeqno = shardRequest.shardSeqno,
            sourceStateVerifierId = shardRequest.sourceStateVerifierId,
            sourceStateVerifierHash = shardRequest.sourceStateVerifierHash,
            shardStateProofPublicInputsHash = shardRequest.shardStateProofPublicInputsHash,
            statementBytes = shardRequest.statementBytes,
            witnessCommitmentBytes = shardRequest.witnessCommitmentBytes,
            verificationContextBytes = shardRequest.verificationContextBytes,
            schemaDescriptor = shardRequest.schemaDescriptor,
            publicInputColumns = shardRequest.publicInputColumns,
            fastpqPublicInputs = shardRequest.fastpqPublicInputs,
            fastpqTransitions = tamperedShardTransitions,
        )
        val tamperedShardTransition = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), tamperedShardRequest)
        }
        assertTrue(tamperedShardTransition.message!!.contains("canonical TON source-state request"))
        val tamperedShardDsidRequest = TonShardStateProofRequest(
            version = shardRequest.version,
            proofFamily = shardRequest.proofFamily,
            circuitId = shardRequest.circuitId,
            parameterSet = shardRequest.parameterSet,
            sourceDomain = shardRequest.sourceDomain,
            masterchainSeqno = shardRequest.masterchainSeqno,
            shardSeqno = shardRequest.shardSeqno,
            sourceStateVerifierId = shardRequest.sourceStateVerifierId,
            sourceStateVerifierHash = shardRequest.sourceStateVerifierHash,
            shardStateProofPublicInputsHash = shardRequest.shardStateProofPublicInputsHash,
            statementBytes = shardRequest.statementBytes,
            witnessCommitmentBytes = shardRequest.witnessCommitmentBytes,
            verificationContextBytes = shardRequest.verificationContextBytes,
            schemaDescriptor = shardRequest.schemaDescriptor,
            publicInputColumns = shardRequest.publicInputColumns,
            fastpqPublicInputs = shardRequest.fastpqPublicInputs.copy(dsid = "0x" + "00".repeat(16)),
            fastpqTransitions = shardRequest.fastpqTransitions,
        )
        val tamperedShardDsid = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), tamperedShardDsidRequest)
        }
        assertTrue(tamperedShardDsid.message!!.contains("fastpqPublicInputs.dsid"))
        val tamperedAuditTransitions = requests.masterchainConfig.fastpqTransitions.toMutableList()
        tamperedAuditTransitions[0] = tamperedAuditTransitions[0].copy(newValue = "0x00")
        val tamperedAuditRequest = TonSccpFullLightClientAuditProofRequest(
            version = requests.masterchainConfig.version,
            proofFamily = requests.masterchainConfig.proofFamily,
            circuitId = requests.masterchainConfig.circuitId,
            parameterSet = requests.masterchainConfig.parameterSet,
            role = requests.masterchainConfig.role,
            roleCode = requests.masterchainConfig.roleCode,
            sourceDomain = requests.masterchainConfig.sourceDomain,
            masterchainSeqno = requests.masterchainConfig.masterchainSeqno,
            shardSeqno = requests.masterchainConfig.shardSeqno,
            verifierId = requests.masterchainConfig.verifierId,
            verifierHash = requests.masterchainConfig.verifierHash,
            sourceStateVerifierId = requests.masterchainConfig.sourceStateVerifierId,
            sourceStateVerifierHash = requests.masterchainConfig.sourceStateVerifierHash,
            sourceVerifierMaterialHash = requests.masterchainConfig.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = requests.masterchainConfig.sourceAdapterDeploymentHash,
            fullLightClientGateHash = requests.masterchainConfig.fullLightClientGateHash,
            shardStateProofPublicInputsHash = requests.masterchainConfig.shardStateProofPublicInputsHash,
            shardStateVerificationProofHash = requests.masterchainConfig.shardStateVerificationProofHash,
            auditStatementHash = requests.masterchainConfig.auditStatementHash,
            statementBytes = requests.masterchainConfig.statementBytes,
            verificationContextBytes = requests.masterchainConfig.verificationContextBytes,
            schemaDescriptor = requests.masterchainConfig.schemaDescriptor,
            publicInputColumns = requests.masterchainConfig.publicInputColumns,
            fastpqPublicInputs = requests.masterchainConfig.fastpqPublicInputs,
            fastpqTransitions = tamperedAuditTransitions,
        )
        val tamperedAuditTransition = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), tamperedAuditRequest)
        }
        assertTrue(tamperedAuditTransition.message!!.contains("canonical TON source-state request"))
        val tamperedAuditTxRequest = TonSccpFullLightClientAuditProofRequest(
            version = requests.masterchainConfig.version,
            proofFamily = requests.masterchainConfig.proofFamily,
            circuitId = requests.masterchainConfig.circuitId,
            parameterSet = requests.masterchainConfig.parameterSet,
            role = requests.masterchainConfig.role,
            roleCode = requests.masterchainConfig.roleCode,
            sourceDomain = requests.masterchainConfig.sourceDomain,
            masterchainSeqno = requests.masterchainConfig.masterchainSeqno,
            shardSeqno = requests.masterchainConfig.shardSeqno,
            verifierId = requests.masterchainConfig.verifierId,
            verifierHash = requests.masterchainConfig.verifierHash,
            sourceStateVerifierId = requests.masterchainConfig.sourceStateVerifierId,
            sourceStateVerifierHash = requests.masterchainConfig.sourceStateVerifierHash,
            sourceVerifierMaterialHash = requests.masterchainConfig.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = requests.masterchainConfig.sourceAdapterDeploymentHash,
            fullLightClientGateHash = requests.masterchainConfig.fullLightClientGateHash,
            shardStateProofPublicInputsHash = requests.masterchainConfig.shardStateProofPublicInputsHash,
            shardStateVerificationProofHash = requests.masterchainConfig.shardStateVerificationProofHash,
            auditStatementHash = requests.masterchainConfig.auditStatementHash,
            statementBytes = requests.masterchainConfig.statementBytes,
            verificationContextBytes = requests.masterchainConfig.verificationContextBytes,
            schemaDescriptor = requests.masterchainConfig.schemaDescriptor,
            publicInputColumns = requests.masterchainConfig.publicInputColumns,
            fastpqPublicInputs = requests.masterchainConfig.fastpqPublicInputs.copy(txSetHash = "0x" + "aa".repeat(32)),
            fastpqTransitions = requests.masterchainConfig.fastpqTransitions,
        )
        val tamperedAuditTxSetHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapSourceStateVerificationProof(byteArrayOf(9, 8, 7), tamperedAuditTxRequest)
        }
        assertTrue(tamperedAuditTxSetHash.message!!.contains("fastpqPublicInputs.txSetHash"))

        var preflightCallbackInvoked = false
        val preflightCheckingProver = TonSccpSourceStateProver(
            shardStateProofEngine = TonSccpShardStateProofEngine {
                preflightCallbackInvoked = true
                byteArrayOf(9, 8, 7)
            },
            fullLightClientAuditProofEngine = TonSccpFullLightClientAuditProofEngine {
                preflightCallbackInvoked = true
                byteArrayOf(9, 8, 7)
            },
        )
        val shardPreflightError = assertFailsWith<IllegalArgumentException> {
            preflightCheckingProver.proveShardState(tamperedShardRequest)
        }
        assertTrue(shardPreflightError.message!!.contains("canonical TON source-state request"))
        assertFalse(preflightCallbackInvoked)
        val auditPreflightError = assertFailsWith<IllegalArgumentException> {
            preflightCheckingProver.proveFullLightClientAudit(tamperedAuditRequest)
        }
        assertTrue(auditPreflightError.message!!.contains("canonical TON source-state request"))
        assertFalse(preflightCallbackInvoked)

        val oversizedCallbackProver = TonSccpSourceStateProver(
            shardStateProofEngine = TonSccpShardStateProofEngine {
                oversizedSourceStateProofBytes
            },
        )
        val oversizedCallbackProof = assertFailsWith<IllegalArgumentException> {
            oversizedCallbackProver.proveShardState(shardRequest)
        }
        assertTrue(oversizedCallbackProof.message!!.contains("proofBytes must be at most"))

        val seenRoles = mutableListOf<String>()
        val sourceStateProver = TonSccpSourceStateProver(
            shardStateProofEngine = TonSccpShardStateProofEngine { request ->
                seenRoles += "shard_state"
                assertEquals(SccpTon.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1, request.circuitId)
                byteArrayOf(9, 8, 7)
            },
            fullLightClientAuditProofEngine = TonSccpFullLightClientAuditProofEngine { request ->
                seenRoles += request.role
                byteArrayOf(9, 8, 7)
            },
        )
        val linkedShardProof = sourceStateProver.proveShardState(input.shardState)
        val linkedProofs = sourceStateProver.proveFullLightClientAudit(input)
        assertEquals(SccpTon.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1, linkedShardProof.circuitId)
        assertEquals("CQgH", linkedShardProof.proofBase64)
        assertEquals(
            listOf("shard_state", "masterchain_config", "validator_set_transition", "shard_accounts_dictionary"),
            seenRoles,
        )
        assertEquals(
            SccpTon.VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
            linkedProofs.validatorSetTransition.circuitId,
        )
        assertEquals(
            SccpTon.SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
            linkedProofs.shardAccountsDictionary.circuitId,
        )
        assertContentEquals(byteArrayOf(9, 8, 7), linkedProofs.shardAccountsDictionary.proofBytes)
        assertEquals("CQgH", linkedProofs.shardAccountsDictionary.proofBase64)
        var sawShardSnapshot = false
        var sawAuditSnapshot = false
        val snapshotCheckingProver = TonSccpSourceStateProver(
            shardStateProofEngine = TonSccpShardStateProofEngine { callbackRequest ->
                sawShardSnapshot = true
                assertFalse(callbackRequest === shardRequest)
                assertContentEquals(shardRequest.statementBytes, callbackRequest.statementBytes)
                assertContentEquals(shardRequest.witnessCommitmentBytes, callbackRequest.witnessCommitmentBytes)
                byteArrayOf(9, 8, 7)
            },
            fullLightClientAuditProofEngine = TonSccpFullLightClientAuditProofEngine { callbackRequest ->
                sawAuditSnapshot = true
                assertFalse(callbackRequest === requests.masterchainConfig)
                assertContentEquals(
                    requests.masterchainConfig.statementBytes,
                    callbackRequest.statementBytes,
                )
                byteArrayOf(9, 8, 7)
            },
        )
        snapshotCheckingProver.proveShardState(shardRequest)
        snapshotCheckingProver.proveFullLightClientAudit(requests.masterchainConfig)
        assertTrue(sawShardSnapshot)
        assertTrue(sawAuditSnapshot)
        val missingSourceStateProver = assertFailsWith<IllegalStateException> {
            TonSccpSourceStateProver().proveFullLightClientAudit(input)
        }
        assertTrue(missingSourceStateProver.message!!.contains("source-state prover is not linked"))

        assertFailsWith<IllegalArgumentException> {
            SccpTon.buildFullLightClientAuditProofRequests(
                input.copy(tonValidatorSetTransitionVerifierHash = "0x" + "b1".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.buildFullLightClientAuditProofRequests(
                input.copy(tonMasterchainConfigVerifierHash = "0x" + "d4".repeat(32)),
            )
        }
        val requestHashReplayError = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildFullLightClientAuditProofRequests(
                input.copy(
                    tonMasterchainConfigVerifierHash =
                        SccpTon.fullLightClientAuditStatementHash(
                            input,
                            TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG,
                        ),
                ),
            )
        }
        assertTrue(requestHashReplayError.message!!.contains("request-bound hashes"))
        val templateAuditError = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildFullLightClientAuditProofRequests(
                input.copy(
                    tonMasterchainConfigVerifierHash =
                        "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
                ),
            )
        }
        assertTrue(templateAuditError.message!!.contains("template material"))
        assertFailsWith<IllegalArgumentException> {
            SccpTon.buildFullLightClientAuditProofRequests(
                input.copy(shardStateVerificationProofHash = "0x" + "aa".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.buildFullLightClientAuditProofRequests(
                sampleFullLightClientAuditProofInput(masterchainConfigProofHash = "0x" + "aa".repeat(32)),
            )
        }
    }

    @Test
    fun derivesTonValidatorSetTransitionHashesFromWitnessMaterial() {
        val validatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val validatorWeights = listOf("1", "2")
        val validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938"
        val nextValidatorSetPayload = SccpTon.canonicalValidatorSetPayloadBytes(
            listOf(ByteArray(32) { 0x33.toByte() }, ByteArray(32) { 0x44.toByte() }),
            listOf("3", "4"),
        )
        val nextValidatorSetHash = "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f"
        val nextValidatorSetPayloadHash = "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983"
        val transitionMessageHash = "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19"
        val transitionSignatureHash = "0xd784461f68495981c2c00e60316dc9353ea4b5be3bc261b26feadc7c83c4f6a7"
        val signatureProof = TonValidatorSignatureProofInput(
            totalWeight = "3",
            signedWeight = "3",
            blockMessageHash = transitionMessageHash,
            validatorPublicKeys = validatorPublicKeys,
            validatorWeights = validatorWeights,
            signersBitmap = byteArrayOf(0x03),
            signatures = listOf(ByteArray(64) { 0xab.toByte() }, ByteArray(64) { 0xcd.toByte() }),
        )

        assertEquals(85, SccpTon.canonicalValidatorSetBytes(validatorPublicKeys, validatorWeights).size)
        assertEquals(validatorSetHash, SccpTon.validatorSetHash(validatorPublicKeys, validatorWeights))
        assertEquals(nextValidatorSetPayloadHash, SccpTon.validatorSetPayloadHash(nextValidatorSetPayload))
        assertEquals(nextValidatorSetHash, SccpTon.validatorSetHashFromPayload(nextValidatorSetPayload))
        assertEquals(
            233,
            SccpTon.canonicalValidatorSetTransitionMessageBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
            ).size,
        )
        assertEquals(
            transitionMessageHash,
            SccpTon.validatorSetTransitionMessageHash(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
            ),
        )
        assertEquals(
            676,
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof,
            ).size,
        )
        assertEquals(
            transitionSignatureHash,
            SccpTon.validatorSetTransitionSignatureHash(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof,
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = "dd".repeat(32),
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = "dd".repeat(32),
                validatorSignatureProof = signatureProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionMessageBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "9",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof.copy(blockMessageHash = "dd".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetBytes(validatorPublicKeys, listOf("1", "0"))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetBytes(
                listOf(ByteArray(32), validatorPublicKeys[1]),
                validatorWeights,
            )
        }
        val zeroKeyValidatorSetPayload =
            SccpTon.canonicalValidatorSetPayloadBytes(validatorPublicKeys, validatorWeights)
        zeroKeyValidatorSetPayload.fill(0.toByte(), 5, 37)
        assertFailsWith<IllegalArgumentException> {
            SccpTon.validatorSetHashFromPayload(zeroKeyValidatorSetPayload)
        }
        val oversizedValidatorPublicKeys = (0..1024).map { index ->
            ByteArray(32).also { publicKey ->
                publicKey[0] = 0x80.toByte()
                publicKey[28] = (index and 0xff).toByte()
                publicKey[29] = ((index ushr 8) and 0xff).toByte()
                publicKey[30] = ((index ushr 16) and 0xff).toByte()
                publicKey[31] = ((index ushr 24) and 0xff).toByte()
            }
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetBytes(
                oversizedValidatorPublicKeys,
                List(oversizedValidatorPublicKeys.size) { "1" },
            )
        }
        val oversizedValidatorSetPayload = ByteArrayOutputStream().also { out ->
            out.write(1)
            val count = oversizedValidatorPublicKeys.size
            out.write(count and 0xff)
            out.write((count ushr 8) and 0xff)
            out.write((count ushr 16) and 0xff)
            out.write((count ushr 24) and 0xff)
            for (publicKey in oversizedValidatorPublicKeys) {
                out.write(publicKey)
                out.write(byteArrayOf(1, 0, 0, 0, 0, 0, 0, 0))
            }
        }.toByteArray()
        assertFailsWith<IllegalArgumentException> {
            SccpTon.validatorSetHashFromPayload(oversizedValidatorSetPayload)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof.copy(signatures = listOf(ByteArray(64))),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof.copy(
                    signedWeight = "1",
                    signersBitmap = byteArrayOf(0x01),
                    signatures = listOf(ByteArray(64)),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof.copy(
                    signatures = listOf(ByteArray(63), ByteArray(64)),
                ),
            )
        }
        val zeroSignature = assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalValidatorSetTransitionSignatureBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = validatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = signatureProof.copy(
                    signatures = listOf(ByteArray(64), ByteArray(64) { 0x01.toByte() }),
                ),
            )
        }
        assertTrue(zeroSignature.message?.contains("all zero") == true)
    }

    @Test
    fun derivesTonMasterchainConfigProofHashesFromWitnessMaterial() {
        val validatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val validatorWeights = listOf("1", "2")
        val validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938"
        val validatorSetPayloadHash = "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0"
        val configLeafHash = "0xed92ba8082850092da7cc296a2184cc4576877aaee08c72748d96ea449b16e39"
        val configRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af"
        val configValueHash = "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50"
        val configDictionaryProofBoc = hexBytes(
            "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0",
        )
        val configProofHash = "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c"
        val validatorSetPayload = SccpTon.canonicalValidatorSetPayloadBytes(validatorPublicKeys, validatorWeights)

        assertEquals(validatorSetPayloadHash, SccpTon.validatorSetPayloadHash(validatorSetPayload))
        assertEquals(
            validatorSetPayload.toList(),
            SccpTon.configValidatorSetPayloadFromProofBoc(configDictionaryProofBoc)?.toList(),
        )
        assertEquals(
            validatorSetPayloadHash,
            SccpTon.configValidatorSetPayloadHashFromProofBoc(configDictionaryProofBoc),
        )
        assertEquals(configRoot, SccpTon.hashmapEProofRootHash(configDictionaryProofBoc))
        assertEquals(
            configValueHash,
            SccpTon.hashmapECellRefValueHash(
                configDictionaryProofBoc,
                byteArrayOf(0, 0, 0, SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toByte()),
                SccpTon.CONFIG_PARAM_KEY_BITS,
            ),
        )
        assertEquals(
            141,
            SccpTon.canonicalMasterchainConfigLeafBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
            ).size,
        )
        assertEquals(
            configLeafHash,
            SccpTon.masterchainConfigLeafHash(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigLeafBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                version = 0,
            )
        }
        assertEquals(
            411,
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = configLeafHash,
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = emptyList(),
            ).size,
        )
        assertEquals(
            configProofHash,
            SccpTon.masterchainConfigProofHash(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = configLeafHash,
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = emptyList(),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = configLeafHash,
                configLeafIndex = "0",
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = emptyList(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = "ee".repeat(32),
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = emptyList(),
            )
        }
        val wrongValidatorSetHash = "ee".repeat(32)
        val wrongValidatorSetLeafHash = SccpTon.masterchainConfigLeafHash(
            sourceDomain = SccpTon.DOMAIN_TON,
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            shardStateRoot = "cc".repeat(32),
            validatorSetHash = wrongValidatorSetHash,
            validatorSetPayloadHash = validatorSetPayloadHash,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = wrongValidatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = wrongValidatorSetLeafHash,
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = emptyList(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = 3,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = configLeafHash,
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = emptyList(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = "ee".repeat(32),
                configLeafHash = configLeafHash,
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = emptyList(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = configLeafHash,
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = listOf(byteArrayOf(1, 2, 3)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainConfigProofBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainBlockHash = "aa".repeat(32),
                shardStateRoot = "cc".repeat(32),
                configRoot = configRoot,
                validatorSetHash = validatorSetHash,
                validatorSetPayloadHash = validatorSetPayloadHash,
                configLeafHash = configLeafHash,
                configLeafIndex = SccpTon.CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
                configValueHash = configValueHash,
                configDictionaryProofBoc = configDictionaryProofBoc,
                configInclusionBranch = List(65) { ByteArray(32) { 0xee.toByte() } },
            )
        }
    }

    @Test
    fun derivesTonMasterchainBlockMessageAndSignatureHashesFromWitnessMaterial() {
        val validatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val validatorWeights = listOf("1", "2")
        val validatorSetHash = SccpTon.validatorSetHash(validatorPublicKeys, validatorWeights)
        val blockMessageHash = SccpTon.masterchainBlockMessageHash(
            sourceDomain = SccpTon.DOMAIN_TON,
            masterchainSeqno = "19",
            masterchainWorkchainId = -1,
            masterchainShard = "9223372036854775808",
            masterchainBlockHash = "aa".repeat(32),
            masterchainFileHash = "a5".repeat(32),
            validatorSetHash = validatorSetHash,
            masterchainConfigRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
            masterchainConfigProofHash = "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c",
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = "cc".repeat(32),
            transactionRoot = "dd".repeat(32),
            shardProofHash = "ee".repeat(32),
        )
        val signatureProof = TonValidatorSignatureProofInput(
            totalWeight = "3",
            signedWeight = "3",
            blockMessageHash = blockMessageHash,
            validatorPublicKeys = validatorPublicKeys,
            validatorWeights = validatorWeights,
            signersBitmap = byteArrayOf(0x03),
            signatures = listOf(ByteArray(64) { 0xab.toByte() }, ByteArray(64) { 0xcd.toByte() }),
        )

        assertEquals(
            365,
            SccpTon.canonicalMasterchainBlockMessageBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                validatorSetHash = validatorSetHash,
                masterchainConfigRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
                masterchainConfigProofHash = "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c",
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                shardProofHash = "ee".repeat(32),
            ).size,
        )
        assertEquals("0x0ca07d5072adb7db3d6a0f831294c7e119c451884aaa1afcbb23e0df0911d8bd", blockMessageHash)
        assertEquals(
            322,
            SccpTon.canonicalMasterchainValidatorSignaturesBytes(
                signatureProof,
                providedValidatorSetHash = validatorSetHash,
            ).size,
        )
        assertEquals(
            "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15",
            SccpTon.masterchainValidatorSignaturesHash(
                signatureProof,
                providedValidatorSetHash = validatorSetHash,
            ),
        )
        val zeroSignature = assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainValidatorSignaturesBytes(
                signatureProof.copy(signatures = listOf(ByteArray(64), ByteArray(64) { 0x01.toByte() })),
                providedValidatorSetHash = validatorSetHash,
            )
        }
        assertTrue(zeroSignature.message?.contains("all zero") == true)
        assertFailsWith<IllegalArgumentException> {
            SccpTon.canonicalMasterchainBlockMessageBytes(
                sourceDomain = SccpTon.DOMAIN_TON,
                masterchainSeqno = "19",
                masterchainWorkchainId = 0,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                validatorSetHash = validatorSetHash,
                masterchainConfigRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
                masterchainConfigProofHash = "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c",
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                shardProofHash = "ee".repeat(32),
            )
        }
    }

    @Test
    fun callbackRequestSnapshotCopiesTonProofRequestBytes() {
        val request = SccpTon.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val snapshot = SccpTon.callbackRequestSnapshot(request)

        assertFalse(snapshot === request)
        assertEquals(request, snapshot)
        assertContentEquals(request.publicInputsBytes, snapshot.publicInputsBytes)
        assertContentEquals(request.bundleBytes, snapshot.bundleBytes)
        assertContentEquals(request.sourceProofBytes, snapshot.sourceProofBytes)

        val exposedPublicInputs = snapshot.publicInputsBytes
        val exposedBundle = snapshot.bundleBytes
        val exposedSourceProof = snapshot.sourceProofBytes
        exposedPublicInputs[0] = (exposedPublicInputs[0].toInt() xor 0x01).toByte()
        exposedBundle[0] = (exposedBundle[0].toInt() xor 0x01).toByte()
        exposedSourceProof[0] = (exposedSourceProof[0].toInt() xor 0x01).toByte()

        assertContentEquals(request.publicInputsBytes, snapshot.publicInputsBytes)
        assertContentEquals(request.bundleBytes, snapshot.bundleBytes)
        assertContentEquals(request.sourceProofBytes, snapshot.sourceProofBytes)
    }

    @Test
    fun proverRequiresLinkedProofEngine() {
        val error = assertFailsWith<IllegalStateException> {
            TonSccpProver().prove(sampleProofRequestInput())
        }
        assertTrue(error.message?.contains("not linked") == true)
    }

    @Test
    fun proverRejectsNonProductionInputBeforeLinkedProofEngine() {
        var invoked = false
        val prover = TonSccpProver(
            proofEngine = TonSccpProofEngine {
                invoked = true
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val error = assertFailsWith<IllegalArgumentException> {
            prover.prove(
                sampleProofRequestInput(
                    sourceProofBytes = byteArrayOf(9, 10),
                    sourceStateVerifierHash = SccpSolana.ZERO_HASH_V1,
                ),
            )
        }
        assertTrue(error.message?.contains("sourceStateVerifierHash") == true)
        assertFalse(invoked)
    }

    @Test
    fun proverResolvesWitnessProviderBeforeBuildingRequest() {
        var resolved = false
        val bundleBytes = sampleTonBundleBytes()
        val input = sampleProofRequestInput(bundleBytes = bundleBytes)
        val prover = TonSccpProver(
            witnessProvider = TonSccpWitnessProvider { input ->
                assertContentEquals(ByteArray(0), input.sourceProofBytes)
                assertFalse(input.bundleBytes === bundleBytes)
                input.bundleBytes[0] = 0x7f
                resolved = true
                input.copy(bundleBytes = sampleTonBundleBytes(), sourceProofBytes = byteArrayOf(9, 10))
            },
            proofEngine = TonSccpProofEngine { request ->
                assertTrue(resolved)
                assertContentEquals(byteArrayOf(9, 10), request.sourceProofBytes)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(input)

        assertContentEquals(byteArrayOf(9, 10), result.sourceProofBytes)
        assertContentEquals(sampleTonBundleBytes(), input.bundleBytes)
        assertContentEquals(sampleTonBundleBytes(), bundleBytes)
    }

    @Test
    fun proverWrapsExternalProofBytes() {
        val seenRequests = mutableListOf<TonSccpProofRequest>()
        val prover = TonSccpProver(
            proofEngine = TonSccpProofEngine { request ->
                seenRequests.add(request)
                assertEquals(SccpTon.CONTRACT_PROOF_BACKEND_V1, request.backend)
                assertEquals("0x" + "56".repeat(32), request.statementHash)
                assertEquals("0x" + "78".repeat(32), request.destinationBindingHash)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        val omittedSourceResult = prover.prove(sampleProofRequestInput())
        val expectedRequest = SccpTon.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val expectedOmittedSourceRequest = SccpTon.buildProofRequest(sampleProofRequestInput())

        assertEquals(listOf(expectedRequest, expectedOmittedSourceRequest), seenRequests)
        assertFalse(seenRequests[0] === expectedRequest)
        assertFalse(seenRequests[1] === expectedOmittedSourceRequest)

        assertEquals(listOf(1, 2, 3, 4), result.proofBytes.map { it.toInt() })
        assertContentEquals(ByteArray(0), omittedSourceResult.sourceProofBytes)
        assertEquals("AQIDBA==", result.proofBase64)
        assertEquals("0x" + "56".repeat(32), result.statementHash)
        assertEquals("0x" + "78".repeat(32), result.destinationBindingHash)
        assertEquals(SccpTon.MAINNET_SHARD_STATE_VERIFIER_ID_V1, result.sourceStateVerifierId)
        assertEquals("0x" + "cc".repeat(32), result.sourceStateVerifierHash)
        assertTrue(result.requestHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(result.envelopeHash.matches(Regex("0x[0-9a-f]{64}")))

        val submissionInput = TonSccpMessageBodyInput(
            proofResult = result,
            bundleBytes = sampleTonBundleBytes(),
            metadataBytes = byteArrayOf(8, 9),
            queryId = "7",
        )
        assertEquals(result.publicInputs, submissionInput.publicInputs)
        assertContentEquals(result.proofBytes, submissionInput.proofBytes)
        assertContentEquals(sampleTonBundleBytes(), result.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), result.sourceProofBytes)
        assertEquals(result.proofContext.statementHash, submissionInput.statementHash)
        assertEquals(result.proofContext.destinationBindingHash, submissionInput.destinationBindingHash)
        val submission = SccpTon.buildSubmission(submissionInput)
        assertEquals("internal_message", submission.submissionKind)
        assertEquals("op::submit_sccp_message_proof", submission.verifierEntrypoint)
        val oversizedTonMessageResult = SccpTon.wrapProofResult(
            ByteArray(4096 * 127) { 1 },
            SccpTon.buildProofRequest(sampleProofRequestInput()),
        )
        val oversizedTonMessage = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildSubmission(
                TonSccpMessageBodyInput(
                    proofResult = oversizedTonMessageResult,
                    bundleBytes = sampleTonBundleBytes(),
                    metadataBytes = byteArrayOf(8, 9),
                ),
            )
        }
        assertTrue(oversizedTonMessage.message?.contains("TON BOC contains too many cells") == true)
        val omittedSourceProofResult = SccpTon.wrapProofResult(
            result.proofBytes,
            SccpTon.buildProofRequest(sampleProofRequestInput()),
        )
        val omittedSourceMessageBody = SccpTon.buildSubmission(
            TonSccpMessageBodyInput(
                proofResult = omittedSourceProofResult,
                bundleBytes = sampleTonBundleBytes(),
            ),
        )
        assertContentEquals(ByteArray(0), omittedSourceProofResult.sourceProofBytes)
        assertTrue(omittedSourceMessageBody.messageBodyBoc.isNotEmpty())
        val mismatchedBundle = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result,
                bundleBytes = sampleTonBundleBytes(finalityProof = byteArrayOf(0x71, 0x73)),
            )
        }
        assertTrue(mismatchedBundle.message?.contains("proofResult.bundleBytes") == true)

        val tamperedResultBundle = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result.copy(bundleBytes = sampleTonBundleBytes(finalityProof = byteArrayOf(0x71, 0x73))),
                bundleBytes = sampleTonBundleBytes(finalityProof = byteArrayOf(0x71, 0x73)),
            )
        }
        assertTrue(tamperedResultBundle.message?.contains("requestHash") == true)

        val mismatchedProofBase64 = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result.copy(proofBase64 = "AAAA"),
                bundleBytes = sampleTonBundleBytes(),
            )
        }
        assertTrue(mismatchedProofBase64.message?.contains("proofBase64") == true)

        val missingEnvelope = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result.copy(envelopeHash = SccpSolana.ZERO_HASH_V1),
                bundleBytes = sampleTonBundleBytes(),
            )
        }
        assertTrue(missingEnvelope.message?.contains("proofResult.envelopeHash") == true)

        val tamperedEnvelope = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result.copy(envelopeHash = "0x" + "aa".repeat(32)),
                bundleBytes = sampleTonBundleBytes(),
            )
        }
        assertTrue(tamperedEnvelope.message?.contains("wrapped proof bytes") == true)

        val mismatchedProofContext = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result.copy(
                    proofContext = result.proofContext.copy(statementHash = "0x" + "99".repeat(32)),
                ),
                bundleBytes = sampleTonBundleBytes(),
            )
        }
        assertTrue(mismatchedProofContext.message?.contains("proofContext") == true)

        val wrongSourceStateVerifier = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result.copy(sourceStateVerifierHash = SccpSolana.ZERO_HASH_V1),
                bundleBytes = sampleTonBundleBytes(),
            )
        }
        assertTrue(wrongSourceStateVerifier.message?.contains("sourceStateVerifierHash") == true)

        val wrongResultDeploymentBinding = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = result.copy(
                    sourceAdapterDeploymentBinding =
                        result.sourceAdapterDeploymentBinding.copy(targetDomain = SccpTon.DOMAIN_TON),
                ),
                bundleBytes = sampleTonBundleBytes(),
            )
        }
        assertTrue(wrongResultDeploymentBinding.message?.contains("targetDomain") == true)

        val request = SccpTon.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapProofResult(byteArrayOf(1), request.copy(backend = "debug-ton-backend"))
        }
        assertTrue(wrongBackend.message?.contains("backend must be ton-contract-v1") == true)

        val zeroProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapProofResult(byteArrayOf(0, 0), request)
        }
        assertTrue(zeroProof.message?.contains("all zero") == true)

        val oversizedProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapProofResult(
                ByteArray(SccpTon.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                request,
            )
        }
        assertTrue(oversizedProof.message?.contains("at most") == true)

        val wrongDeploymentBinding = assertFailsWith<IllegalArgumentException> {
            SccpTon.wrapProofResult(
                byteArrayOf(1),
                request.copy(sourceAdapterDeploymentBindingHash = "0x" + "99".repeat(32)),
            )
        }
        assertTrue(wrongDeploymentBinding.message?.contains("canonical") == true)

        val exposedProof = result.proofBytes
        exposedProof[0] = 9
        assertContentEquals(byteArrayOf(1, 2, 3, 4), result.proofBytes)

        val mutatedRequestView = SccpTon.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        mutatedRequestView.bundleBytes[0] = 9
        SccpTon.wrapProofResult(byteArrayOf(1), mutatedRequestView)
        assertContentEquals(sampleTonBundleBytes(), mutatedRequestView.bundleBytes)
    }

    @Test
    fun proofRequestBindsRelayContextAndDeployment() {
        val bundleBytes = sampleTonBundleBytes()
        val sourceProofBytes = byteArrayOf(9, 10)
        val request = SccpTon.buildProofRequest(
            sampleProofRequestInput(
                bundleBytes = bundleBytes,
                sourceProofBytes = sourceProofBytes,
                sourceAdapterDeploymentHash = "aa".repeat(32),
                sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
            ),
        )

        assertEquals("0x" + "56".repeat(32), request.proofContext.statementHash)
        assertEquals("0x" + "78".repeat(32), request.proofContext.destinationBindingHash)
        assertEquals(SccpTon.MAINNET_SHARD_STATE_VERIFIER_ID_V1, request.sourceStateVerifierId)
        assertEquals("0x" + "cc".repeat(32), request.sourceStateVerifierHash)
        assertEquals(SccpTon.DOMAIN_TON, request.sourceAdapterDeploymentBinding.sourceDomain)
        assertEquals(SccpSolana.DOMAIN_SORA, request.sourceAdapterDeploymentBinding.targetDomain)
        assertEquals("0x" + "aa".repeat(32), request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash)
        assertEquals(
            "0x" + "bb".repeat(32),
            request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
        )
        assertEquals(
            SccpSolana.sourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding),
            request.sourceAdapterDeploymentBindingHash,
        )
        assertTrue(request.requestHash.matches(Regex("0x[0-9a-f]{64}")))
        val deploymentBinding = SolanaSccpSourceAdapterDeploymentBinding(
            version = 1,
            sourceDomain = SccpTon.DOMAIN_TON,
            targetDomain = SccpSolana.DOMAIN_SORA,
            sourceAdapterDeploymentHash = "aa".repeat(32),
            sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
        )
        val bindingRequest = SccpTon.buildProofRequest(
            TonSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = sampleTonBundleBytes(),
                sourceProofBytes = byteArrayOf(9, 10),
                statementHash = "56".repeat(32),
                destinationBindingHash = "78".repeat(32),
                sourceStateVerifierHash = "cc".repeat(32),
                sourceAdapterDeploymentBinding = deploymentBinding,
            ),
        )
        assertEquals(request.requestHash, bindingRequest.requestHash)
        val wrongBindingTarget = assertFailsWith<IllegalArgumentException> {
            TonSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = sampleTonBundleBytes(),
                statementHash = "56".repeat(32),
                destinationBindingHash = "78".repeat(32),
                sourceStateVerifierHash = "cc".repeat(32),
                sourceAdapterDeploymentBinding =
                    deploymentBinding.copy(targetDomain = SccpTon.DOMAIN_TON),
            )
        }
        assertTrue(
            wrongBindingTarget.message?.contains(
                "sourceAdapterDeploymentBinding.targetDomain must be SORA",
            ) == true,
        )
        val sourceStateBoundRequest = SccpTon.buildProofRequest(
            sampleProofRequestInput(sourceStateVerifierHash = "dd".repeat(32)),
        )
        assertNotEquals(request.requestHash, sourceStateBoundRequest.requestHash)
        val shiftedSplitRequest = SccpTon.buildProofRequest(
            sampleProofRequestInput(
                bundleBytes = sampleTonBundleBytes(finalityProof = byteArrayOf(0x71, 0x73)),
                sourceProofBytes = byteArrayOf(10),
                sourceAdapterDeploymentHash = "aa".repeat(32),
                sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
            ),
        )
        assertNotEquals(request.requestHash, shiftedSplitRequest.requestHash)
        val sourceStateError = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceStateVerifierId = "debug-ton-state-verifier",
                    sourceStateVerifierHash = "cc".repeat(32),
                ),
            )
        }
        assertTrue(
            sourceStateError.message?.contains("sourceStateVerifierId must match TON") == true,
        )
        val zeroSourceStateVerifier = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(sourceStateVerifierHash = SccpSolana.ZERO_HASH_V1),
            )
        }
        assertTrue(
            zeroSourceStateVerifier.message?.contains("sourceStateVerifierHash must not be zero") == true,
        )
        val paddedPayloadHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    publicInputs = samplePublicInputs().copy(payloadHash = " " + "ee".repeat(32)),
                ),
            )
        }
        assertTrue(paddedPayloadHash.message?.contains("payloadHash must be canonical hex") == true)
        val paddedStatementHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(statementHash = "56".repeat(32) + "\n"),
            )
        }
        assertTrue(paddedStatementHash.message?.contains("statementHash must be canonical hex") == true)
        val zeroStatementHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(statementHash = "00".repeat(32)),
            )
        }
        assertTrue(zeroStatementHash.message?.contains("statementHash must not be zero") == true)
        val zeroDestinationBindingHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(destinationBindingHash = "00".repeat(32)),
            )
        }
        assertTrue(
            zeroDestinationBindingHash.message?.contains("destinationBindingHash must not be zero") == true,
        )
        val paddedDeploymentHash = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceAdapterDeploymentHash = "\n" + "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(
            paddedDeploymentHash.message?.contains("sourceAdapterDeploymentHash must be canonical hex") == true,
        )
        val nonCanonicalFinalityHeight = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs().copy(finalityHeight = "019")),
            )
        }
        assertTrue(
            nonCanonicalFinalityHeight.message?.contains("finalityHeight must be a canonical unsigned integer") == true,
        )
        val templateSourceStateVerifier = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceStateVerifierHash =
                        "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
                ),
            )
        }
        assertTrue(
            templateSourceStateVerifier.message?.contains("TON template verifier hash") == true,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = SccpSolana.ZERO_HASH_V1,
                ),
            )
        }
        val zeroDeploymentError = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceAdapterDeploymentHash = SccpSolana.ZERO_HASH_V1,
                    sourceAdapterDeploymentReceiptHash = SccpSolana.ZERO_HASH_V1,
                ),
            )
        }
        assertTrue(
            zeroDeploymentError.message?.contains("requires non-zero source adapter deployment binding") == true,
        )
        val emptyBundle = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    bundleBytes = ByteArray(0),
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(emptyBundle.message?.contains("bundleBytes") == true)
        val zeroBundle = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    bundleBytes = byteArrayOf(0, 0),
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(zeroBundle.message?.contains("bundleBytes must not be all zero") == true)
        val oversizedBundle = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    bundleBytes = ByteArray(SccpTon.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(oversizedBundle.message?.contains("bundleBytes must be at most") == true)
        val zeroSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceProofBytes = byteArrayOf(0, 0),
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(zeroSourceProof.message?.contains("sourceProofBytes must not be all zero") == true)
        val oversizedSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceProofBytes = ByteArray(SccpTon.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            )
        }
        assertTrue(oversizedSourceProof.message?.contains("sourceProofBytes must be at most") == true)
        assertContentEquals(
            ByteArray(0),
            SccpTon.buildProofRequest(
                sampleProofRequestInput(
                    sourceProofBytes = ByteArray(0),
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            ).sourceProofBytes,
        )
        val sourceDomainError = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(sampleProofRequestInput(sourceDomain = SccpSolana.DOMAIN_SOLANA))
        }
        assertTrue(sourceDomainError.message?.contains("sourceDomain must be TON") == true)
        val targetDomainError = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                sampleProofRequestInput().copy(
                    publicInputs = samplePublicInputs().copy(targetDomain = SccpSolana.DOMAIN_SOLANA),
                ),
            )
        }
        assertTrue(targetDomainError.message?.contains("targetDomain must be TON") == true)
        val backendError = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(sampleProofRequestInput(backend = "debug-ton-backend"))
        }
        assertTrue(backendError.message?.contains("backend must be ton-contract-v1") == true)

        bundleBytes[0] = 99
        sourceProofBytes[0] = 99
        assertContentEquals(sampleTonBundleBytes(), request.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), request.sourceProofBytes)

        val exposedPublicInputs = request.publicInputsBytes
        val exposedBundle = request.bundleBytes
        val exposedSourceProof = request.sourceProofBytes
        exposedPublicInputs[0] = 99
        exposedBundle[0] = 99
        exposedSourceProof[0] = 99
        assertTrue(request.publicInputsBytes[0].toInt() != 99)
        assertContentEquals(sampleTonBundleBytes(), request.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), request.sourceProofBytes)
    }

    @Test
    fun proofRequestRejectsNoncanonicalOrMismatchedBundleBytes() {
        val base = sampleProofRequestInput(
            sourceAdapterDeploymentHash = "aa".repeat(32),
            sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
        )
        val placeholder = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(base.copy(bundleBytes = byteArrayOf(5, 6, 7)))
        }
        assertTrue(placeholder.message?.contains("bundleBytes.version must be 1") == true)

        val swapped = sampleTonBundleFixture(amount = BigInteger.valueOf(43))
        val mismatchedPublicInputs = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(base.copy(bundleBytes = swapped.bundleBytes))
        }
        assertTrue(mismatchedPublicInputs.message?.contains("bundleBytes must match publicInputs") == true)

        val tamperedCommitment = sampleTonBundleBytes()
        tamperedCommitment[37 + 69] = (tamperedCommitment[37 + 69].toInt() xor 0x01).toByte()
        val badCommitment = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(base.copy(bundleBytes = tamperedCommitment))
        }
        assertTrue(badCommitment.message?.contains("bundleBytes.commitment must match payload") == true)

        val tamperedRoot = sampleTonBundleBytes()
        tamperedRoot[1] = (tamperedRoot[1].toInt() xor 0x01).toByte()
        val badRoot = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(base.copy(bundleBytes = tamperedRoot))
        }
        assertTrue(badRoot.message?.contains("bundleBytes.commitment_root must match merkle proof") == true)

        val ranges = splitTestSccpMessageProofBundleBytes(sampleTonBundleBytes())
        val payloadWithTrailingByte = ranges.getValue("payload").bytes + byteArrayOf(0)
        val trailingPayload = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    bundleBytes = replaceTestSccpMessageProofBundleVec(
                        sampleTonBundleBytes(),
                        ranges.getValue("payload"),
                        payloadWithTrailingByte,
                    ),
                ),
            )
        }
        assertTrue(trailingPayload.message?.contains("bundleBytes.payload must not contain trailing bytes") == true)

        val unsupportedPayloadKind = byteArrayOf(0xff.toByte()) + ranges.getValue("payload").bytes.copyOfRange(
            1,
            ranges.getValue("payload").bytes.size,
        )
        val unsupportedPayload = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    bundleBytes = replaceTestSccpMessageProofBundleVec(
                        sampleTonBundleBytes(),
                        ranges.getValue("payload"),
                        unsupportedPayloadKind,
                    ),
                ),
            )
        }
        assertTrue(
            unsupportedPayload.message?.contains("bundleBytes.payload contains unsupported SCCP payload kind") == true,
        )

        val nulPrefixedNameBundle = sampleTonTokenAddBundleFixture(
            name = byteArrayOf(0) + "Token".toByteArray(Charsets.UTF_8) + ByteArray(26),
        )
        val nulPrefixedName = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    publicInputs = nulPrefixedNameBundle.publicInputs,
                    bundleBytes = nulPrefixedNameBundle.bundleBytes,
                ),
            )
        }
        assertTrue(nulPrefixedName.message?.contains("bundleBytes.payload.name") == true)
        val nulPrefixedSymbolBundle = sampleTonTokenAddBundleFixture(
            symbol = byteArrayOf(0) + "TOK".toByteArray(Charsets.UTF_8) + ByteArray(28),
        )
        val nulPrefixedSymbol = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    publicInputs = nulPrefixedSymbolBundle.publicInputs,
                    bundleBytes = nulPrefixedSymbolBundle.bundleBytes,
                ),
            )
        }
        assertTrue(nulPrefixedSymbol.message?.contains("bundleBytes.payload.symbol") == true)

        val merkleProofWithTrailingByte = ranges.getValue("merkleProof").bytes + byteArrayOf(0)
        val trailingMerkleProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    bundleBytes = replaceTestSccpMessageProofBundleVec(
                        sampleTonBundleBytes(),
                        ranges.getValue("merkleProof"),
                        merkleProofWithTrailingByte,
                    ),
                ),
            )
        }
        assertTrue(
            trailingMerkleProof.message?.contains("bundleBytes.merkle_proof must not contain trailing bytes") == true,
        )

        val oneStep = sampleTonBundleFixture(
            merkleProofSteps = listOf(hexBytes("cc".repeat(32)) to 1),
        )
        val oneStepRanges = splitTestSccpMessageProofBundleBytes(oneStep.bundleBytes)
        val invalidDirection = oneStepRanges.getValue("merkleProof").bytes
        invalidDirection[4 + 32] = 2
        val badDirection = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    publicInputs = oneStep.publicInputs,
                    bundleBytes = replaceTestSccpMessageProofBundleVec(
                        oneStep.bundleBytes,
                        oneStepRanges.getValue("merkleProof"),
                        invalidDirection,
                    ),
                ),
            )
        }
        assertTrue(badDirection.message?.contains("sibling_is_left must be 0 or 1") == true)

        val nonSora = sampleTonBundleFixture(
            sourceDomain = SccpSolana.DOMAIN_SOLANA,
            senderCodec = SccpTon.CODEC_SOLANA_BASE58,
            sender = "11111111111111111111111111111111",
        )
        val missingSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    publicInputs = nonSora.publicInputs,
                    bundleBytes = nonSora.bundleBytes,
                    sourceProofBytes = ByteArray(0),
                ),
            )
        }
        assertTrue(missingSourceProof.message?.contains("sourceProofBytes required for non-SORA") == true)

        val nonSoraRequest = SccpTon.buildProofRequest(
            base.copy(
                publicInputs = nonSora.publicInputs,
                bundleBytes = nonSora.bundleBytes,
                sourceProofBytes = byteArrayOf(9, 10),
            ),
        )
        val nonSoraResult = SccpTon.wrapProofResult(byteArrayOf(1, 2, 3, 4), nonSoraRequest)
        val strippedSourceProof = assertFailsWith<IllegalArgumentException> {
            TonSccpMessageBodyInput(
                proofResult = nonSoraResult.copy(sourceProofBytes = ByteArray(0)),
                bundleBytes = nonSora.bundleBytes,
            )
        }
        assertTrue(strippedSourceProof.message?.contains("sourceProofBytes required for non-SORA") == true)

        val canonicalEip55Sender = "0x52908400098527886E0F7030069857D2E4169EE7"
        val lowercaseRequiredEip55Sender = "0xde709f2102306220921060314715629080e2fb77"
        val lowercaseRequiredEip55Source = sampleTonBundleFixture(
            sourceDomain = SccpSourceProofs.DOMAIN_ETH,
            senderCodec = SccpTon.CODEC_EVM_HEX,
            sender = lowercaseRequiredEip55Sender,
        )
        SccpTon.buildProofRequest(
            base.copy(
                publicInputs = lowercaseRequiredEip55Source.publicInputs,
                bundleBytes = lowercaseRequiredEip55Source.bundleBytes,
                sourceProofBytes = byteArrayOf(9, 10),
            ),
        )
        val noncanonicalEip55Source = sampleTonBundleFixture(
            sourceDomain = SccpSourceProofs.DOMAIN_ETH,
            senderCodec = SccpTon.CODEC_EVM_HEX,
            sender = canonicalEip55Sender.lowercase(),
        )
        val noncanonicalEip55 = assertFailsWith<IllegalArgumentException> {
            SccpTon.buildProofRequest(
                base.copy(
                    publicInputs = noncanonicalEip55Source.publicInputs,
                    bundleBytes = noncanonicalEip55Source.bundleBytes,
                    sourceProofBytes = byteArrayOf(9, 10),
                ),
            )
        }
        assertTrue(noncanonicalEip55.message?.contains("bundleBytes.payload.sender") == true)
        assertTrue(noncanonicalEip55.message?.contains("EIP-55") == true)
        for (invalidSender in listOf(
            lowercaseRequiredEip55Sender.uppercase(),
            "0X" + canonicalEip55Sender.drop(2),
            "0x52908400098527886E0F7030069857D2E4169EEZ",
        )) {
            val invalidSource = sampleTonBundleFixture(
                sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                senderCodec = SccpTon.CODEC_EVM_HEX,
                sender = invalidSender,
            )
            val invalidEip55 = assertFailsWith<IllegalArgumentException> {
                SccpTon.buildProofRequest(
                    base.copy(
                        publicInputs = invalidSource.publicInputs,
                        bundleBytes = invalidSource.bundleBytes,
                        sourceProofBytes = byteArrayOf(9, 10),
                    ),
                )
            }
            assertTrue(invalidEip55.message?.contains("bundleBytes.payload.sender") == true)
        }
    }

    @Test
    fun proofRequestHashMatchesCrossSdkVector() {
        val publicInputs = samplePublicInputs()
        val request = SccpTon.buildProofRequest(
            TonSccpProofRequestInput(
                publicInputs = publicInputs,
                bundleBytes = sampleTonBundleBytes(),
                sourceProofBytes = byteArrayOf(0x51, 0x52, 0x53),
                statementHash = "55".repeat(32),
                destinationBindingHash = "66".repeat(32),
                sourceStateVerifierHash = "42".repeat(32),
                sourceAdapterDeploymentBinding = TonSccpSourceAdapterDeploymentBinding(
                    version = 1,
                    sourceDomain = SccpTon.DOMAIN_TON,
                    targetDomain = SccpSolana.DOMAIN_SORA,
                    sourceAdapterDeploymentHash = "aa".repeat(32),
                    sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
                ),
            ),
        )
        val expectedPublicInputsBytes = hexBytes(
            "01" +
                "806384e356636c10ee3bbbb90674a80410a86be034616abb811586b21ac81fc4367a4f" +
                "9061f46a282eeeda95bc68c727888bde665bd89d0ebbc6dae266e3a264" +
                "04000000" +
                "377eb92928595d90759d66529f96acf34afd4ef64cd2327ab6f65876fb3cf93e" +
                "1300000000000000" +
                "aa".repeat(32),
        )

        assertContentEquals(expectedPublicInputsBytes, SccpTon.canonicalPublicInputsBytes(publicInputs))
        assertEquals(
            "0x7d35b186e3d49aed31693e33d33355fa8fa9032160c929f2c7fe260094f6ccdf",
            request.sourceAdapterDeploymentBindingHash,
        )
        assertEquals(
            "0x2a292741b8e8d8454699eda954592904e8260e6b8a41cc840f5d9c48732c3bbe",
            request.requestHash,
        )
        val proofResult = SccpTon.wrapProofResult(
            byteArrayOf(0x91.toByte(), 0x92.toByte(), 0x93.toByte(), 0x94.toByte(), 0x95.toByte()),
            request,
        )
        assertEquals(
            "0x9ed8e54d81c13a61939dedffb36c487f33d32a128ba95a0d29b33c5d25be6489",
            proofResult.envelopeHash,
        )
    }

    private fun sampleMessageBodyInput(
        publicInputs: TonSccpPublicInputsInput = samplePublicInputs(),
        proofBytes: ByteArray = byteArrayOf(1, 2, 3, 4),
        bundleBytes: ByteArray = sampleTonBundleBytes(),
        statementHash: String = "bb".repeat(32),
        destinationBindingHash: String = "56".repeat(32),
    ): TonSccpMessageBodyInput {
        val request = SccpTon.buildProofRequest(
            sampleProofRequestInput(
                publicInputs = publicInputs,
                bundleBytes = bundleBytes,
                sourceProofBytes = byteArrayOf(9, 10),
                statementHash = statementHash,
                destinationBindingHash = destinationBindingHash,
                sourceAdapterDeploymentHash = "aa".repeat(32),
                sourceAdapterDeploymentReceiptHash = "bb".repeat(32),
            ),
        )
        val proofResult = SccpTon.wrapProofResult(proofBytes, request)
        return TonSccpMessageBodyInput(
            proofResult = proofResult,
            bundleBytes = bundleBytes,
            metadataBytes = byteArrayOf(8, 9),
        )
    }

    private fun sampleProofRequestInput(
        publicInputs: TonSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = sampleTonBundleBytes(),
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String = "56".repeat(32),
        destinationBindingHash: String = "78".repeat(32),
        sourceStateVerifierId: String = SccpTon.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        sourceStateVerifierHash: String = "cc".repeat(32),
        sourceAdapterDeploymentHash: String = "aa".repeat(32),
        sourceAdapterDeploymentReceiptHash: String = "bb".repeat(32),
        backend: String = SccpTon.CONTRACT_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpTon.DOMAIN_TON,
    ): TonSccpProofRequestInput =
        TonSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
            sourceStateVerifierId = sourceStateVerifierId,
            sourceStateVerifierHash = sourceStateVerifierHash,
            sourceAdapterDeploymentHash = sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash = sourceAdapterDeploymentReceiptHash,
            backend = backend,
            sourceDomain = sourceDomain,
        )

    private fun samplePublicInputs(): TonSccpPublicInputsInput =
        sampleTonBundleFixture().publicInputs

    private fun sampleTonBundleBytes(
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
        senderCodec: Int = SccpTon.CODEC_TEXT_UTF8,
        sender: String = "alice@sora",
        nonce: Long = 327L,
        amount: BigInteger = BigInteger.valueOf(42),
        routeId: String = "sccp-ton-proof-request",
        merkleProofSteps: List<Pair<ByteArray, Int>> = emptyList(),
        finalityProof: ByteArray = byteArrayOf(0x71, 0x72),
    ): ByteArray =
        sampleTonBundleFixture(
            sourceDomain = sourceDomain,
            senderCodec = senderCodec,
            sender = sender,
            nonce = nonce,
            amount = amount,
            routeId = routeId,
            merkleProofSteps = merkleProofSteps,
            finalityProof = finalityProof,
        ).bundleBytes

    private fun sampleTonBundleFixture(
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
        senderCodec: Int = SccpTon.CODEC_TEXT_UTF8,
        sender: String = "alice@sora",
        nonce: Long = 327L,
        amount: BigInteger = BigInteger.valueOf(42),
        routeId: String = "sccp-ton-proof-request",
        merkleProofSteps: List<Pair<ByteArray, Int>> = emptyList(),
        finalityProof: ByteArray = byteArrayOf(0x71, 0x72),
    ): SampleTonBundleFixture {
        val payloadBody = ByteArrayOutputStream()
        payloadBody.write(1)
        writeTestU32Le(payloadBody, sourceDomain)
        writeTestU32Le(payloadBody, SccpTon.DOMAIN_TON)
        writeTestU64Le(payloadBody, BigInteger.valueOf(nonce))
        writeTestU32Le(payloadBody, SccpSolana.DOMAIN_SORA)
        payloadBody.write(SccpTon.CODEC_TEXT_UTF8)
        writeTestBytes(payloadBody, "xor#ton".toByteArray(Charsets.UTF_8))
        writeTestU128Le(payloadBody, amount)
        payloadBody.write(senderCodec)
        writeTestBytes(payloadBody, sender.toByteArray(Charsets.UTF_8))
        payloadBody.write(SccpTon.CODEC_TON_RAW)
        writeTestBytes(payloadBody, ("0:" + "12".repeat(32)).toByteArray(Charsets.UTF_8))
        payloadBody.write(SccpTon.CODEC_TEXT_UTF8)
        writeTestBytes(payloadBody, routeId.toByteArray(Charsets.UTF_8))

        val payloadBodyBytes = payloadBody.toByteArray()
        val payloadBytes = byteArrayOf(0x02) + payloadBodyBytes
        val messageId = "0x" + hexLower(
            prefixedKeccakBytes("sccp:transfer:v1", payloadBodyBytes),
        )
        val payloadHash = "0x" + hexLower(
            Blake2b.digest256("sccp:payload:v1".toByteArray(Charsets.UTF_8) + payloadBytes),
        )

        val commitment = ByteArrayOutputStream()
        commitment.write(1)
        commitment.write(6)
        writeTestU32Le(commitment, SccpTon.DOMAIN_TON)
        commitment.write(hexBytes(messageId.removePrefix("0x")))
        commitment.write(hexBytes(payloadHash.removePrefix("0x")))
        val commitmentBytes = commitment.toByteArray()

        var currentRoot = Blake2b.digest256("sccp:hub:leaf:v1".toByteArray(Charsets.UTF_8) + commitmentBytes)
        val merkleProof = ByteArrayOutputStream()
        writeTestU32Le(merkleProof, merkleProofSteps.size)
        for ((sibling, siblingIsLeft) in merkleProofSteps) {
            require(sibling.size == 32) { "test Merkle sibling must be 32 bytes" }
            merkleProof.write(sibling)
            merkleProof.write(siblingIsLeft)
            currentRoot = Blake2b.digest256(
                "sccp:hub:node:v1".toByteArray(Charsets.UTF_8) +
                    if (siblingIsLeft == 1) sibling + currentRoot else currentRoot + sibling,
            )
        }
        val commitmentRoot = "0x" + hexLower(currentRoot)

        val bundle = ByteArrayOutputStream()
        bundle.write(1)
        bundle.write(currentRoot)
        writeTestBytes(bundle, commitmentBytes)
        writeTestBytes(bundle, merkleProof.toByteArray())
        writeTestBytes(bundle, payloadBytes)
        writeTestBytes(bundle, finalityProof)

        return SampleTonBundleFixture(
            publicInputs = TonSccpPublicInputsInput(
                version = 1,
                messageId = messageId,
                payloadHash = payloadHash,
                targetDomain = SccpTon.DOMAIN_TON,
                commitmentRoot = commitmentRoot,
                finalityHeight = "19",
                finalityBlockHash = "aa".repeat(32),
            ),
            bundleBytes = bundle.toByteArray(),
        )
    }

    private fun sampleTonTokenAddBundleFixture(
        targetDomain: Int = SccpTon.DOMAIN_TON,
        nonce: Long = 327L,
        name: ByteArray = fixedTestAscii32("Token"),
        symbol: ByteArray = fixedTestAscii32("TOK"),
        finalityProof: ByteArray = byteArrayOf(0x71, 0x72),
    ): SampleTonBundleFixture {
        require(name.size == 32)
        require(symbol.size == 32)

        val payloadBody = ByteArrayOutputStream()
        payloadBody.write(1)
        writeTestU32Le(payloadBody, targetDomain)
        writeTestU64Le(payloadBody, BigInteger.valueOf(nonce))
        payloadBody.write(ByteArray(32) { 0x11.toByte() })
        payloadBody.write(18)
        payloadBody.write(name)
        payloadBody.write(symbol)

        val payloadBodyBytes = payloadBody.toByteArray()
        val payloadBytes = byteArrayOf(0x03) + payloadBodyBytes
        val messageId = "0x" + hexLower(prefixedKeccakBytes("sccp:token:add:v1", payloadBodyBytes))
        val payloadHash = "0x" + hexLower(
            Blake2b.digest256("sccp:payload:v1".toByteArray(Charsets.UTF_8) + payloadBytes),
        )

        val commitment = ByteArrayOutputStream()
        commitment.write(1)
        commitment.write(1)
        writeTestU32Le(commitment, targetDomain)
        commitment.write(hexBytes(messageId.removePrefix("0x")))
        commitment.write(hexBytes(payloadHash.removePrefix("0x")))
        val commitmentBytes = commitment.toByteArray()
        val currentRoot = Blake2b.digest256("sccp:hub:leaf:v1".toByteArray(Charsets.UTF_8) + commitmentBytes)
        val commitmentRoot = "0x" + hexLower(currentRoot)

        val merkleProof = ByteArrayOutputStream()
        writeTestU32Le(merkleProof, 0)

        val bundle = ByteArrayOutputStream()
        bundle.write(1)
        bundle.write(currentRoot)
        writeTestBytes(bundle, commitmentBytes)
        writeTestBytes(bundle, merkleProof.toByteArray())
        writeTestBytes(bundle, payloadBytes)
        writeTestBytes(bundle, finalityProof)

        return SampleTonBundleFixture(
            publicInputs = TonSccpPublicInputsInput(
                version = 1,
                messageId = messageId,
                payloadHash = payloadHash,
                targetDomain = targetDomain,
                commitmentRoot = commitmentRoot,
                finalityHeight = "19",
                finalityBlockHash = "aa".repeat(32),
            ),
            bundleBytes = bundle.toByteArray(),
        )
    }

    private fun fixedTestAscii32(value: String): ByteArray {
        val bytes = value.toByteArray(Charsets.UTF_8)
        require(bytes.size <= 32)
        return bytes.copyOf(32)
    }

    private fun splitTestSccpMessageProofBundleBytes(
        bundleBytes: ByteArray,
    ): Map<String, TestBundleVecRange> {
        var offset = 33
        val commitment = readTestCanonicalVecRange(bundleBytes, offset)
        offset = commitment.nextOffset
        val merkleProof = readTestCanonicalVecRange(bundleBytes, offset)
        offset = merkleProof.nextOffset
        val payload = readTestCanonicalVecRange(bundleBytes, offset)
        offset = payload.nextOffset
        val finalityProof = readTestCanonicalVecRange(bundleBytes, offset)
        return mapOf(
            "commitment" to commitment,
            "merkleProof" to merkleProof,
            "payload" to payload,
            "finalityProof" to finalityProof,
        )
    }

    private fun readTestCanonicalVecRange(bundleBytes: ByteArray, offset: Int): TestBundleVecRange {
        val length = readTestU32Le(bundleBytes, offset)
        val start = offset + 4
        val end = start + length
        require(end <= bundleBytes.size) { "test vector exceeds bundle length" }
        return TestBundleVecRange(
            lengthOffset = offset,
            bytesStart = start,
            bytesEnd = end,
            bytes = bundleBytes.copyOfRange(start, end),
            nextOffset = end,
        )
    }

    private fun replaceTestSccpMessageProofBundleVec(
        bundleBytes: ByteArray,
        vecRange: TestBundleVecRange,
        replacement: ByteArray,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(bundleBytes.copyOfRange(0, vecRange.lengthOffset))
        writeTestU32Le(out, replacement.size)
        out.write(replacement)
        out.write(bundleBytes.copyOfRange(vecRange.bytesEnd, bundleBytes.size))
        return out.toByteArray()
    }

    private fun writeTestBytes(out: ByteArrayOutputStream, value: ByteArray) {
        writeTestU32Le(out, value.size)
        out.write(value)
    }

    private fun writeTestU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0)
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun writeTestU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        repeat(8) {
            out.write(working.and(BigInteger.valueOf(0xffL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun writeTestU128Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        repeat(16) {
            out.write(working.and(BigInteger.valueOf(0xffL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun readTestU32Le(raw: ByteArray, offset: Int): Int {
        require(offset + 4 <= raw.size)
        return (raw[offset].toInt() and 0xff) or
            ((raw[offset + 1].toInt() and 0xff) shl 8) or
            ((raw[offset + 2].toInt() and 0xff) shl 16) or
            ((raw[offset + 3].toInt() and 0xff) shl 24)
    }

    private fun prefixedKeccakBytes(prefix: String, payload: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        val input = prefix.toByteArray(Charsets.UTF_8) + payload
        digest.update(input, 0, input.size)
        val out = ByteArray(32)
        digest.doFinal(out, 0)
        return out
    }

    private fun sampleShardStateProofRequestInput(
        transactionRoot: String = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
        sourceStateVerifierHash: String = "d4".repeat(32),
        masterchainConfigProofHash: String =
            "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3",
        validatorSetTransitionProofs: List<TonValidatorSetTransitionProofInput> = emptyList(),
    ): TonShardStateProofRequestInput =
        TonShardStateProofRequestInput(
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            masterchainFileHash = "a5".repeat(32),
            validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            masterchainConfigRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
            masterchainConfigProofHash = masterchainConfigProofHash,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270",
            transactionRoot = transactionRoot,
            transactionLt = "7",
            shardStateDictionaryRoot =
                "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3",
            shardStateDictionaryKeyBitLen = 256,
            shardStateDictionaryKey = ByteArray(32).also { it[0] = 17.toByte() },
            masterchainSignatureHash =
                "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15",
            shardProofHash = "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf",
            shardStateProofBoc = hexBytes(
                "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000",
            ),
            shardStateDictionaryProofBoc = hexBytes(
                "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000",
            ),
            configDictionaryProofBoc = hexBytes(
                "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0",
            ),
            validatorSetTransitionProofs = validatorSetTransitionProofs,
            sourceStateVerifierHash = sourceStateVerifierHash,
            sourceTrustAnchorHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            consensusVerifierHash = "b2".repeat(32),
            messageInclusionVerifierHash = "c3".repeat(32),
            finalityPolicyHash = "c4".repeat(32),
        )

    private fun sampleValidatorSetTransitionProofInput(
        signatures: List<ByteArray> = listOf(
            ByteArray(64) { 0xab.toByte() },
            ByteArray(64) { 0xcd.toByte() },
        ),
    ): TonValidatorSetTransitionProofInput {
        val transitionMessageHash = "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19"
        val nextValidatorSetPayload = hexBytes(
            "0102000000${"33".repeat(32)}0300000000000000${"44".repeat(32)}0400000000000000",
        )
        val validatorSignatureProof = TonValidatorSignatureProofInput(
            totalWeight = "3",
            signedWeight = "3",
            blockMessageHash = transitionMessageHash,
            validatorPublicKeys = listOf(
                ByteArray(32) { 0x11.toByte() },
                ByteArray(32) { 0x22.toByte() },
            ),
            validatorWeights = listOf("1", "2"),
            signersBitmap = byteArrayOf(0x03),
            signatures = signatures,
        )
        return TonValidatorSetTransitionProofInput(
            fromValidatorSetSeqno = "7",
            toValidatorSetSeqno = "8",
            masterchainSeqno = "19",
            masterchainBlockHash = "aa".repeat(32),
            masterchainFileHash = "a5".repeat(32),
            parentValidatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            nextValidatorSetHash = "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f",
            nextValidatorSetPayload = nextValidatorSetPayload,
            nextValidatorSetPayloadHash =
                "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983",
            nextValidatorSetConfigHash = "cc".repeat(32),
            transitionMessageHash = transitionMessageHash,
            transitionSignatureHash = SccpTon.validatorSetTransitionSignatureHash(
                sourceDomain = SccpTon.DOMAIN_TON,
                fromValidatorSetSeqno = "7",
                toValidatorSetSeqno = "8",
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                parentValidatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
                nextValidatorSetHash = "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f",
                nextValidatorSetPayload = nextValidatorSetPayload,
                nextValidatorSetPayloadHash =
                    "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983",
                nextValidatorSetConfigHash = "cc".repeat(32),
                transitionMessageHash = transitionMessageHash,
                validatorSignatureProof = validatorSignatureProof,
            ),
            validatorSignatureProof = validatorSignatureProof,
        )
    }

    private fun sampleFullLightClientAuditProofInput(
        masterchainConfigVerifierHash: String = "0x" + "b1".repeat(32),
        validatorSetTransitionVerifierHash: String = "0x" + "c2".repeat(32),
        shardAccountsDictionaryVerifierHash: String = "0x" + "d3".repeat(32),
        shardStateVerificationProofHash: String? = null,
        masterchainConfigProofHash: String =
            "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3",
    ): TonSccpFullLightClientAuditProofInput {
        val shardState = sampleShardStateProofRequestInput(masterchainConfigProofHash = masterchainConfigProofHash)
        val validatorSetPayloadHash =
            "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0"
        val configLeafHash = SccpTon.masterchainConfigLeafHash(
            sourceDomain = SccpTon.DOMAIN_TON,
            masterchainSeqno = shardState.masterchainSeqno,
            masterchainBlockHash = shardState.masterchainBlockHash,
            shardStateRoot = shardState.shardStateRoot,
            validatorSetHash = shardState.validatorSetHash,
            validatorSetPayloadHash = validatorSetPayloadHash,
        )
        val configValueHash =
            "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50"
        val sourceVerifierMaterialHash = SccpSourceProofs.sourceVerifierMaterialHash(
            sourceDomain = SccpSourceProofs.DOMAIN_TON,
            sourceTrustAnchorHash = shardState.sourceTrustAnchorHash,
            consensusVerifierHash = shardState.consensusVerifierHash,
            messageInclusionVerifierHash = shardState.messageInclusionVerifierHash,
            finalityPolicyHash = shardState.finalityPolicyHash,
            sourceStateVerifierHash = shardState.sourceStateVerifierHash,
        )
        val deploymentReceiptHash = "0x" + "aa".repeat(32)
        val sourceAdapterDeploymentHash = SccpSourceProofs.sourceAdapterEngineDeploymentHash(
            sourceDomain = SccpSourceProofs.DOMAIN_TON,
            sourceTrustAnchorHash = shardState.sourceTrustAnchorHash,
            consensusVerifierHash = shardState.consensusVerifierHash,
            messageInclusionVerifierHash = shardState.messageInclusionVerifierHash,
            finalityPolicyHash = shardState.finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            sourceStateVerifierHash = shardState.sourceStateVerifierHash,
            tonMasterchainConfigVerifierHash = masterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash = validatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash = shardAccountsDictionaryVerifierHash,
        )
        val fullLightClientGateHash = SccpSourceProofs.tonFullLightClientGateHash(
            sourceDomain = SccpSourceProofs.DOMAIN_TON,
            sourceTrustAnchorHash = shardState.sourceTrustAnchorHash,
            consensusVerifierHash = shardState.consensusVerifierHash,
            messageInclusionVerifierHash = shardState.messageInclusionVerifierHash,
            finalityPolicyHash = shardState.finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            tonMasterchainConfigVerifierHash = masterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash = validatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash = shardAccountsDictionaryVerifierHash,
            sourceStateVerifierHash = shardState.sourceStateVerifierHash,
        )
        return TonSccpFullLightClientAuditProofInput(
            shardState = shardState,
            shardStateVerificationProof = TonSccpSourceStateVerificationProof(
                proofBytes = byteArrayOf(0x11, 0x22, 0x33, 0x44),
            ),
            validatorSetPayloadHash = validatorSetPayloadHash,
            configLeafHash = configLeafHash,
            configValueHash = configValueHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = sourceAdapterDeploymentHash,
            fullLightClientGateHash = fullLightClientGateHash,
            tonMasterchainConfigVerifierHash = masterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash = validatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash = shardAccountsDictionaryVerifierHash,
            shardStateVerificationProofHash = shardStateVerificationProofHash,
        )
    }

    private fun sampleTonRouteCanaryEvidence(
        destinationBindingHash: String = SccpSourceProofs.destinationBindingHash(SccpTon.DOMAIN_TON),
        expectedDestinationBindingHash: String? = null,
        verifierContractAddress: String = "0:" + "11".repeat(32),
        accountStatus: String = "active",
        lastTransactionLt: String = "123456789",
        verifierCodeBocRootHash: String = "0x" + "44".repeat(32),
    ): TonSccpRouteCanaryEvidenceInput =
        TonSccpRouteCanaryEvidenceInput(
            routeAllowlistHash = "0x" + "31".repeat(32),
            destinationBindingHash = destinationBindingHash,
            expectedDestinationBindingHash = expectedDestinationBindingHash,
            sourceVerifierMaterialHash = "0x" + "33".repeat(32),
            sourceAdapterEngineDeploymentHash = "0x" + "34".repeat(32),
            verifierContractAddress = verifierContractAddress,
            verifierCodeHash = "0x" + "44".repeat(32),
            accountStatus = accountStatus,
            accountStateHash = "0x" + "55".repeat(32),
            lastTransactionLt = lastTransactionLt,
            lastTransactionHash = "0x" + "66".repeat(32),
            verifierCodeBocRootHash = verifierCodeBocRootHash,
        )

    private fun hexBytes(hex: String): ByteArray {
        require(hex.length % 2 == 0)
        return ByteArray(hex.length / 2) { index ->
            hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun hexLower(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02x".format(it.toInt() and 0xff) }

    private fun indexOfBytes(haystack: ByteArray, needle: ByteArray): Int {
        for (offset in 0..(haystack.size - needle.size)) {
            var matches = true
            for (index in needle.indices) {
                matches = matches && haystack[offset + index] == needle[index]
            }
            if (matches) return offset
        }
        return -1
    }

    private data class SampleTonBundleFixture(
        val publicInputs: TonSccpPublicInputsInput,
        val bundleBytes: ByteArray,
    )

    private data class TestBundleVecRange(
        val lengthOffset: Int,
        val bytesStart: Int,
        val bytesEnd: Int,
        val bytes: ByteArray,
        val nextOffset: Int,
    )
}
