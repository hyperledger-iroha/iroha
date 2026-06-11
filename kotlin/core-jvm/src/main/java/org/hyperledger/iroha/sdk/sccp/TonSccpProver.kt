package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.util.Base64
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** TON SCCP proof request and internal-message helpers for local-first UI proof generation. */
object SccpTon {
    const val DOMAIN_TON: Int = 4
    const val CONTRACT_PROOF_BACKEND_V1: String = "ton-contract-v1"
    const val MESSAGE_BODY_BOC_V1: String = "ton_message_body_boc_v1"
    const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"
    const val NATIVE_RECURSIVE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val SOURCE_STATE_MAX_PROOF_BYTES: Int = NATIVE_RECURSIVE_MAX_PROOF_BYTES
    const val CODEC_TEXT_UTF8: Int = 1
    const val CODEC_EVM_HEX: Int = 2
    const val CODEC_SOLANA_BASE58: Int = 3
    const val CODEC_TON_RAW: Int = 4
    const val CODEC_TRON_BASE58CHECK: Int = 5
    const val CODEC_SORA_ASSET_ID: Int = 6
    const val MAINNET_SHARD_STATE_VERIFIER_ID_V1: String =
        "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1"
    const val SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1: String =
        "sccp-ton-shard-state-light-client-v1"
    const val MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1: String =
        "sccp-ton-masterchain-config-v1"
    const val VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1: String =
        "sccp-ton-validator-set-transition-v1"
    const val SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1: String =
        "sccp-ton-shard-accounts-dictionary-v1"
    private val SOURCE_STATE_VERIFICATION_CIRCUIT_IDS: Set<String> = setOf(
        SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
        MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
        VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
        SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
    )
    private val TEMPLATE_SHARD_STATE_VERIFIER_HASH: ByteArray =
        hex32Bytes(
            "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
            "templateShardStateVerifierHash",
        )
    private val TEMPLATE_SOURCE_MATERIAL_HASHES: List<ByteArray> = listOf(
        hex32Bytes(
            "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
            "tonTemplateSourceTrustAnchorHash",
        ),
        hex32Bytes(
            "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
            "tonTemplateConsensusVerifierHash",
        ),
        hex32Bytes(
            "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
            "tonTemplateMessageInclusionVerifierHash",
        ),
        hex32Bytes(
            "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
            "tonTemplateFinalityPolicyHash",
        ),
        TEMPLATE_SHARD_STATE_VERIFIER_HASH,
    )
    const val CURRENT_VALIDATOR_SET_CONFIG_PARAM: Long = 34L
    const val CONFIG_PARAM_KEY_BITS: Int = 32

    private const val SUBMIT_OP_V1: Long = 0x53434350L
    private const val MESSAGE_SCHEMA_VERSION_V1: Int = 1
    private const val SHARD_PROOF_PREFIX_V1: String = "sccp:ton:shard-proof:v1"
    private const val VALIDATOR_SET_PREFIX_V1: String = "sccp:ton:validator-set:v1"
    private const val VALIDATOR_SET_PAYLOAD_PREFIX_V1: String = "sccp:ton:validator-set-payload:v1"
    private const val MASTERCHAIN_CONFIG_LEAF_PREFIX_V1: String =
        "sccp:ton:masterchain-config-leaf:v1"
    private const val MASTERCHAIN_CONFIG_PROOF_PREFIX_V1: String =
        "sccp:ton:masterchain-config-proof:v1"
    private const val MASTERCHAIN_BLOCK_MESSAGE_PREFIX_V1: String =
        "sccp:ton:masterchain-block-message:v1"
    private const val MASTERCHAIN_SIGNATURES_PREFIX_V1: String =
        "sccp:ton:masterchain-signatures:v1"
    private const val VALIDATOR_SET_TRANSITION_MESSAGE_PREFIX_V1: String =
        "sccp:ton:validator-set-transition-message:v1"
    private const val VALIDATOR_SET_TRANSITION_SIGNATURES_PREFIX_V1: String =
        "sccp:ton:validator-set-transition-signatures:v1"
    private const val VALIDATOR_SET_TRANSITION_CHAIN_PREFIX_V1: String =
        "sccp:ton:validator-set-transition-chain:v1"
    private const val SHARD_STATE_PROOF_PUBLIC_INPUTS_PREFIX_V1: String =
        "sccp:ton:shard-state-proof-public-inputs:v1"
    private const val SHARD_STATE_FASTPQ_DSID_PREFIX_V1: String =
        "sccp:ton:shard-state:fastpq:dsid:v1"
    private const val SHARD_STATE_FASTPQ_PARAMETER_SET_V1: String = "fastpq-lane-balanced"
    private const val SHARD_STATE_FASTPQ_STATEMENT_KEY_V1: String =
        "sccp:ton:shard-state:v1:statement"
    private const val SHARD_STATE_FASTPQ_WITNESS_KEY_V1: String =
        "sccp:ton:shard-state:v1:witness"
    private const val SHARD_STATE_FASTPQ_CONTEXT_KEY_V1: String =
        "sccp:ton:shard-state:v1:context"
    private const val SHARD_STATE_PROOF_BOC_PREFIX_V1: String =
        "sccp:ton:shard-state-proof-boc:v1"
    private const val SHARD_ACCOUNTS_PROOF_BOC_PREFIX_V1: String =
        "sccp:ton:shard-accounts-proof-boc:v1"
    private const val CONFIG_PROOF_BOC_PREFIX_V1: String = "sccp:ton:config-proof-boc:v1"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1: String =
        "sccp:ton:full-light-client-audit:fastpq:dsid:v1"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1: String =
        "fastpq-lane-balanced"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1: String =
        "sccp:ton:full-light-client-audit:v1:statement"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1: String =
        "sccp:ton:full-light-client-audit:v1:context"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1: String =
        "sccp:ton:full-light-client-audit:v1:gate"
    private const val FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1: String =
        "sccp:ton:full-light-client-audit:statement:v1"
    private const val ROUTE_CANARY_LIVE_ACCOUNT_PREFIX_V1: String =
        "iroha:sccp:ton-route-canary-live-account:v1"
    private const val SCCP_MSG_PREFIX_ASSET_REGISTER_V1: String = "sccp:asset:register:v1"
    private const val SCCP_MSG_PREFIX_ROUTE_ACTIVATE_V1: String = "sccp:route:activate:v1"
    private const val SCCP_MSG_PREFIX_TRANSFER_V1: String = "sccp:transfer:v1"
    private const val SCCP_MSG_PREFIX_TOKEN_ADD_V1: String = "sccp:token:add:v1"
    private const val SCCP_MSG_PREFIX_TOKEN_PAUSE_V1: String = "sccp:token:pause:v1"
    private const val SCCP_MSG_PREFIX_TOKEN_RESUME_V1: String = "sccp:token:resume:v1"
    private const val SCCP_HUB_LEAF_PREFIX_V1: String = "sccp:hub:leaf:v1"
    private const val SCCP_HUB_NODE_PREFIX_V1: String = "sccp:hub:node:v1"
    private const val SCCP_PAYLOAD_HASH_PREFIX_V1: String = "sccp:payload:v1"
    private const val BASE58_ALPHABET: String =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    private const val MAX_CELL_DATA_BYTES: Int = 127
    private const val MAX_CELL_SERIALIZED_DATA_BYTES: Int = 128
    private const val MAX_BOC_BYTES: Int = 64 * 1024
    private const val MAX_BOC_CELLS: Int = 4096
    private const val MAX_REFS: Int = 4
    private const val MAX_VALIDATORS: Int = 1024
    private const val SHARD_ACCOUNT_KEY_BITS: Int = 256
    private const val VALIDATOR_SET_KEY_BITS: Int = 16
    private const val VALIDATOR_CONSTRUCTOR: Int = 0x53
    private const val VALIDATOR_ADDR_CONSTRUCTOR: Int = 0x73
    private const val VALIDATORS_CONSTRUCTOR: Int = 0x11
    private const val VALIDATORS_EXT_CONSTRUCTOR: Int = 0x12
    private const val ED25519_PUBKEY_CONSTRUCTOR: Int = 0x8e81278a.toInt()
    private const val MAX_SOURCE_MERKLE_BRANCH_NODES: Int = 64
    private const val CRC32C_REFLECTED_POLY: Int = -2097792136
    private const val SHARD_STATE_UNSPLIT_TAG: Int = -1876709406
    private const val TON_MAINNET_GLOBAL_ID: Int = -239
    private const val TON_MASTERCHAIN_WORKCHAIN_ID: Int = -1
    private val TON_MASTERCHAIN_SHARD: BigInteger = BigInteger.ONE.shiftLeft(63)
    private const val TON_BASECHAIN_WORKCHAIN_ID: Int = 0
    private val BOC_MAGIC = byteArrayOf(0xb5.toByte(), 0xee.toByte(), 0x9c.toByte(), 0x72.toByte())
    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    /** Selected TON ShardAccount last transaction identity. */
    data class ShardAccountLastTransaction(val hash: String, val lt: BigInteger)

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

    /** Canonical live-account evidence bytes for the SORA -> TON route canary. */
    @JvmStatic
    fun canonicalRouteCanaryEvidenceBytes(input: TonSccpRouteCanaryEvidenceInput): ByteArray {
        val routeAllowlistHash = nonZeroHex32Bytes(input.routeAllowlistHash, "routeAllowlistHash")
        val destinationBindingHash =
            nonZeroHex32Bytes(input.destinationBindingHash, "destinationBindingHash")
        val canonicalTonDestinationBindingHash =
            SccpSourceProofs.destinationBindingHash(DOMAIN_TON)
        val expectedDestinationBindingHash = normalizeNonZeroHex32(
            input.expectedDestinationBindingHash ?: canonicalTonDestinationBindingHash,
            "expectedDestinationBindingHash",
        )
        require(expectedDestinationBindingHash == canonicalTonDestinationBindingHash) {
            "expectedDestinationBindingHash must match canonical TON destination binding"
        }
        require("0x" + hexLower(destinationBindingHash) == canonicalTonDestinationBindingHash) {
            "destinationBindingHash must match canonical TON destination binding"
        }
        val sourceVerifierMaterialHash =
            nonZeroHex32Bytes(input.sourceVerifierMaterialHash, "sourceVerifierMaterialHash")
        val sourceAdapterEngineDeploymentHash = nonZeroHex32Bytes(
            input.sourceAdapterEngineDeploymentHash,
            "sourceAdapterEngineDeploymentHash",
        )
        requireHashRolesDistinct(
            "TON route canary governed hashes",
            listOf(
                "routeAllowlistHash" to routeAllowlistHash,
                "destinationBindingHash" to destinationBindingHash,
                "sourceVerifierMaterialHash" to sourceVerifierMaterialHash,
                "sourceAdapterEngineDeploymentHash" to sourceAdapterEngineDeploymentHash,
            ),
        )
        val verifierContractAddress =
            normalizeTonRawAddress(input.verifierContractAddress, "verifierContractAddress")
        val verifierCodeHash = nonZeroHex32Bytes(input.verifierCodeHash, "verifierCodeHash")
        val accountStatus = normalizeTonActiveAccountStatus(input.accountStatus, "accountStatus")
        val accountStateHash = nonZeroHex32Bytes(input.accountStateHash, "accountStateHash")
        val lastTransactionLt =
            normalizePositiveDecimalText(input.lastTransactionLt, "lastTransactionLt")
        val lastTransactionHash =
            nonZeroHex32Bytes(input.lastTransactionHash, "lastTransactionHash")
        val verifierCodeBocRootHash =
            nonZeroHex32Bytes(input.verifierCodeBocRootHash, "verifierCodeBocRootHash")
        require(verifierCodeBocRootHash.contentEquals(verifierCodeHash)) {
            "verifierCodeBocRootHash must match verifierCodeHash"
        }

        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, SccpSolana.DOMAIN_SORA)
        writeU32Le(out, DOMAIN_TON)
        out.write(routeAllowlistHash)
        out.write(destinationBindingHash)
        out.write(sourceVerifierMaterialHash)
        out.write(sourceAdapterEngineDeploymentHash)
        writeVector(out, verifierContractAddress.toByteArray(Charsets.UTF_8))
        out.write(verifierCodeHash)
        writeVector(out, accountStatus.toByteArray(Charsets.UTF_8))
        out.write(accountStateHash)
        writeVector(out, lastTransactionLt.toByteArray(Charsets.UTF_8))
        out.write(lastTransactionHash)
        out.write(verifierCodeBocRootHash)
        return out.toByteArray()
    }

    /** Hash Rust verifies for the SORA -> TON live-account route canary. */
    @JvmStatic
    fun routeCanaryEvidenceHash(input: TonSccpRouteCanaryEvidenceInput): String =
        hashHex(ROUTE_CANARY_LIVE_ACCOUNT_PREFIX_V1, canonicalRouteCanaryEvidenceBytes(input))

    @JvmStatic
    fun canonicalShardProofBytes(
        sourceEventDigest: String,
        masterchainSeqno: String,
        masterchainBlockHash: String,
        shardWorkchainId: Int,
        shardShard: String,
        shardSeqno: String,
        shardBlockHash: String,
        shardFileHash: String,
        shardStateRoot: String,
        transactionRoot: String,
        transactionLt: String,
        shardStateLeafIndex: String,
        shardStateInclusionBranch: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        shardStateDictionaryRoot: String? = null,
        shardStateDictionaryKeyBitLen: Int? = null,
        shardStateDictionaryKey: ByteArray = ByteArray(0),
        shardStateDictionaryProofBoc: ByteArray = ByteArray(0),
        shardStateProofBoc: ByteArray = ByteArray(0),
    ): ByteArray {
        val shardStateBranch = normalizeInclusionBranch(shardStateInclusionBranch)
        val branch = normalizeInclusionBranch(inclusionBranch)
        val hasDictionaryOpening = shardStateDictionaryRoot != null ||
            shardStateDictionaryKeyBitLen != null ||
            shardStateDictionaryKey.isNotEmpty() ||
            shardStateDictionaryProofBoc.isNotEmpty()
        require(!hasDictionaryOpening || shardStateProofBoc.isNotEmpty()) {
            "shardStateProofBoc is required for TON shard-state dictionary openings"
        }
        require(hasDictionaryOpening || shardStateProofBoc.isEmpty()) {
            "shardStateProofBoc requires a TON shard-state dictionary opening"
        }
        require(!hasDictionaryOpening || shardStateBranch.isEmpty()) {
            "shardStateInclusionBranch must be empty for TON shard-state dictionary openings"
        }
        val normalizedMasterchainSeqno = normalizeU64(masterchainSeqno, "masterchainSeqno")
        require(shardWorkchainId == TON_BASECHAIN_WORKCHAIN_ID) {
            "shardWorkchainId must be TON basechain"
        }
        val normalizedShard = normalizeU64(shardShard, "shardShard")
        require(normalizedShard != BigInteger.ZERO) { "shardShard must not be zero" }
        val normalizedShardSeqno = normalizeU64(shardSeqno, "shardSeqno")
        require(normalizedShardSeqno != BigInteger.ZERO) { "shardSeqno must not be zero" }
        val normalizedTransactionLt = normalizeU64(transactionLt, "transactionLt")
        require(normalizedTransactionLt != BigInteger.ZERO) { "transactionLt must not be zero" }
        val shardFileHashBytes = nonZeroHex32Bytes(shardFileHash, "shardFileHash")
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"))
        writeU64Le(out, normalizedMasterchainSeqno)
        out.write(hex32Bytes(masterchainBlockHash, "masterchainBlockHash"))
        writeI32Le(out, shardWorkchainId)
        writeU64Le(out, normalizedShard)
        writeU64Le(out, normalizedShardSeqno)
        out.write(hex32Bytes(shardBlockHash, "shardBlockHash"))
        out.write(shardFileHashBytes)
        val shardStateRootBytes = hex32Bytes(shardStateRoot, "shardStateRoot")
        val transactionRootBytes = hex32Bytes(transactionRoot, "transactionRoot")
        out.write(shardStateRootBytes)
        out.write(transactionRootBytes)
        writeU64Le(out, normalizedTransactionLt)
        if (shardStateProofBoc.isNotEmpty()) {
            writeVector(out, shardStateProofBoc)
        }
        if (hasDictionaryOpening) {
            require(shardStateDictionaryRoot != null) { "shardStateDictionaryRoot is required" }
            require(shardStateDictionaryKeyBitLen != null) { "shardStateDictionaryKeyBitLen is required" }
            val dictionaryRoot = hex32Bytes(shardStateDictionaryRoot, "shardStateDictionaryRoot")
            require(dictionaryRoot.any { it.toInt() != 0 }) {
                "shardStateDictionaryRoot must not be zero"
            }
            require(shardStateDictionaryKeyBitLen in 0..0xffff) {
                "shardStateDictionaryKeyBitLen must fit u16"
            }
            require(shardStateDictionaryKeyBitLen == SHARD_ACCOUNT_KEY_BITS) {
                "TON ShardAccounts key bit length must be 256"
            }
            require(hashmapKeyIsCanonical(shardStateDictionaryKey, shardStateDictionaryKeyBitLen)) {
                "shardStateDictionaryKey length is invalid"
            }
            require(shardStateDictionaryProofBoc.isNotEmpty()) {
                "shardStateDictionaryProofBoc must not be empty"
            }
            require(shardStateProofRootHash(shardStateProofBoc) == "0x" + hexLower(shardStateRootBytes)) {
                "shardStateProofBoc root must match shardStateRoot"
            }
            val shardStateOpening = shardStateAccountsOpening(shardStateProofBoc)
            require(shardStateOpening.accountsRootHash == "0x" + hexLower(dictionaryRoot)) {
                "shardStateProofBoc accounts root must match shardStateDictionaryRoot"
            }
            require(shardStateOpening.globalId == TON_MAINNET_GLOBAL_ID) {
                "shardStateProofBoc ShardStateUnsplit global_id must be TON mainnet"
            }
            require(shardStateOpening.workchainId == TON_BASECHAIN_WORKCHAIN_ID) {
                "shardStateProofBoc ShardIdent workchain_id must be TON basechain"
            }
            require(shardStateOpening.workchainId == shardWorkchainId) {
                "shardStateProofBoc ShardIdent workchain_id must match shardWorkchainId"
            }
            require(BigInteger.valueOf(Integer.toUnsignedLong(shardStateOpening.seqNo)) == normalizedShardSeqno) {
                "shardStateProofBoc ShardStateUnsplit seq_no must match shardSeqno"
            }
            require(shardStateOpening.shardId == normalizedShard) {
                "shardStateProofBoc ShardIdent shard must match shardShard"
            }
            require(shardStateOpening.seqNo != 0) {
                "shardStateProofBoc ShardStateUnsplit seq_no must be non-zero"
            }
            require(shardStateOpening.genUtime != 0) {
                "shardStateProofBoc ShardStateUnsplit gen_utime must be non-zero"
            }
            require(shardStateOpening.genLt != 0L) {
                "shardStateProofBoc ShardStateUnsplit gen_lt must be non-zero"
            }
            require(
                BigInteger.valueOf(Integer.toUnsignedLong(shardStateOpening.minRefMcSeqno))
                    <= normalizedMasterchainSeqno,
            ) {
                "shardStateProofBoc ShardStateUnsplit min_ref_mc_seqno exceeds masterchainSeqno"
            }
            require(
                shardStateAccountKeyMatchesShardPrefix(
                    shardStateDictionaryKey,
                    shardStateDictionaryKeyBitLen,
                    shardStateOpening,
                ),
            ) {
                "shardStateDictionaryKey must match shardStateProofBoc ShardIdent prefix"
            }
            val selectedTransaction =
                shardAccountsLastTransaction(
                    shardStateDictionaryProofBoc,
                    shardStateDictionaryKey,
                    shardStateDictionaryKeyBitLen,
                )
            require(
                selectedTransaction != null && selectedTransaction.hash == "0x" + hexLower(transactionRootBytes),
            ) {
                "shardStateDictionaryProofBoc ShardAccount last transaction hash must match transactionRoot"
            }
            require(selectedTransaction.lt == normalizedTransactionLt) {
                "shardStateDictionaryProofBoc ShardAccount last transaction lt must match transactionLt"
            }
            out.write(dictionaryRoot)
            writeU16Le(out, shardStateDictionaryKeyBitLen)
            writeVector(out, shardStateDictionaryKey)
            writeVector(out, shardStateDictionaryProofBoc)
        }
        writeU64Le(out, normalizeU64(shardStateLeafIndex, "shardStateLeafIndex"))
        writeU32Le(out, shardStateBranch.size)
        shardStateBranch.forEach { out.write(it) }
        writeU32Le(out, branch.size)
        branch.forEach { out.write(it) }
        return out.toByteArray()
    }

    @JvmStatic
    fun shardProofHash(
        sourceEventDigest: String,
        masterchainSeqno: String,
        masterchainBlockHash: String,
        shardWorkchainId: Int,
        shardShard: String,
        shardSeqno: String,
        shardBlockHash: String,
        shardFileHash: String,
        shardStateRoot: String,
        transactionRoot: String,
        transactionLt: String,
        shardStateLeafIndex: String,
        shardStateInclusionBranch: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        shardStateDictionaryRoot: String? = null,
        shardStateDictionaryKeyBitLen: Int? = null,
        shardStateDictionaryKey: ByteArray = ByteArray(0),
        shardStateDictionaryProofBoc: ByteArray = ByteArray(0),
        shardStateProofBoc: ByteArray = ByteArray(0),
    ): String =
        hashHex(
            SHARD_PROOF_PREFIX_V1,
            canonicalShardProofBytes(
                sourceEventDigest,
                masterchainSeqno,
                masterchainBlockHash,
                shardWorkchainId,
                shardShard,
                shardSeqno,
                shardBlockHash,
                shardFileHash,
                shardStateRoot,
                transactionRoot,
                transactionLt,
                shardStateLeafIndex,
                shardStateInclusionBranch,
                inclusionBranch,
                shardStateDictionaryRoot,
                shardStateDictionaryKeyBitLen,
                shardStateDictionaryKey,
                shardStateDictionaryProofBoc,
                shardStateProofBoc,
            ),
        )

    @JvmStatic
    fun bocRootHashes(input: ByteArray): List<String> {
        val parsed = parseBocCompleteOrdinary(input)
        val hashes = bocCellHashes(parsed.cells)
        return parsed.roots.map { root ->
            require(root >= 0 && root < hashes.size) { "TON BoC root index is invalid" }
            "0x" + hexLower(hashes[root].hashes[3])
        }
    }

    @JvmStatic
    fun bocSingleRootHash(input: ByteArray): String {
        val roots = bocRootHashes(input)
        require(roots.size == 1) { "TON BoC must contain exactly one root" }
        return roots[0]
    }

    @JvmStatic
    fun shardStateProofRootHash(input: ByteArray): String {
        val parsed = parseBocCompleteOrdinary(input)
        val computed = bocCellHashes(parsed.cells)
        return "0x" + hexLower(bocProofRootAndChildIndex(parsed, computed).first)
    }

    @JvmStatic
    fun hashmapEProofRootHash(input: ByteArray): String {
        val parsed = parseBocCompleteOrdinary(input)
        val computed = bocCellHashes(parsed.cells)
        return "0x" + hexLower(bocProofRootAndChildIndex(parsed, computed).first)
    }

    @JvmStatic
    fun shardStateAccountsRootHash(input: ByteArray): String {
        return shardStateAccountsOpening(input).accountsRootHash
    }

    private fun shardStateAccountsOpening(input: ByteArray): ShardStateAccountsOpening {
        val parsed = parseBocCompleteOrdinary(input)
        val computed = bocCellHashes(parsed.cells)
        val childIndex = bocProofRootAndChildIndex(parsed, computed).second
        return shardStateUnsplitAccountsOpeningFromCell(parsed.cells, computed, childIndex)
    }

    @JvmStatic
    fun hashmapECellRefValueHash(input: ByteArray, key: ByteArray, keyBitLen: Int): String? {
        require(hashmapKeyIsCanonical(key, keyBitLen)) { "TON HashmapE key length is invalid" }
        val parsed = parseBocCompleteOrdinary(input)
        val computed = bocCellHashes(parsed.cells)
        require(parsed.roots.size == 1) { "TON BoC must contain exactly one root" }
        val rootIndex = hashmapUnwrapMerkleProofCell(parsed.cells, parsed.roots[0])
            ?: throw IllegalArgumentException("TON HashmapE root is pruned or unsupported")
        val root = parsed.cells[rootIndex]
        require(bocCellKind(root) == BocCellKind.ORDINARY) { "TON HashmapE root must be ordinary" }
        val reader = BocBitReader(root)
        val hasRoot = reader.readBit()
        if (!hasRoot) {
            require(reader.isExhausted()) { "TON HashmapE empty root is invalid" }
            return null
        }
        require(reader.remainingBits() == 0 && reader.remainingRefs() == 1) {
            "TON HashmapE root is invalid"
        }
        return hashmapCellRefValueHash(parsed.cells, computed, reader.readRef(), key, keyBitLen)
    }

    @JvmStatic
    fun configValidatorSetPayloadFromProofBoc(input: ByteArray): ByteArray? {
        val parsed = parseBocCompleteOrdinary(input)
        bocCellHashes(parsed.cells)
        require(parsed.roots.size == 1) { "TON BoC must contain exactly one root" }
        val rootIndex = hashmapUnwrapMerkleProofCell(parsed.cells, parsed.roots[0])
            ?: throw IllegalArgumentException("TON config dictionary root is pruned or unsupported")
        val root = parsed.cells[rootIndex]
        require(bocCellKind(root) == BocCellKind.ORDINARY) { "TON config dictionary root must be ordinary" }
        val reader = BocBitReader(root)
        val hasRoot = reader.readBit()
        if (!hasRoot) {
            require(reader.isExhausted()) { "TON config dictionary empty root is invalid" }
            return null
        }
        require(reader.remainingBits() == 0 && reader.remainingRefs() == 1) {
            "TON config dictionary root is invalid"
        }
        val valueRef = hashmapCellRefValueIndex(
            parsed.cells,
            reader.readRef(),
            currentValidatorSetConfigKey(),
            CONFIG_PARAM_KEY_BITS,
        ) ?: return null
        return validatorSetPayloadFromCell(parsed.cells, valueRef)
    }

    @JvmStatic
    fun configValidatorSetPayloadHashFromProofBoc(input: ByteArray): String? =
        configValidatorSetPayloadFromProofBoc(input)?.let { validatorSetPayloadHash(it) }

    @JvmStatic
    fun shardAccountsLastTransaction(input: ByteArray, key: ByteArray, keyBitLen: Int): ShardAccountLastTransaction? {
        require(keyBitLen == SHARD_ACCOUNT_KEY_BITS) {
            "TON ShardAccounts key bit length must be 256"
        }
        require(hashmapKeyIsCanonical(key, keyBitLen)) { "TON ShardAccounts key length is invalid" }
        val parsed = parseBocCompleteOrdinary(input)
        val computed = bocCellHashes(parsed.cells)
        require(parsed.roots.size == 1) { "TON BoC must contain exactly one root" }
        val rootIndex = hashmapUnwrapMerkleProofCell(parsed.cells, parsed.roots[0])
            ?: throw IllegalArgumentException("TON ShardAccounts root is pruned or unsupported")
        val root = parsed.cells[rootIndex]
        require(bocCellKind(root) == BocCellKind.ORDINARY) { "TON ShardAccounts root must be ordinary" }
        val reader = BocBitReader(root)
        val hasRoot = reader.readBit()
        if (!hasRoot) {
            require(reader.isExhausted()) { "TON ShardAccounts empty root is invalid" }
            return null
        }
        require(reader.remainingBits() == 0 && reader.remainingRefs() == 1) {
            "TON ShardAccounts root is invalid"
        }
        return hashmapShardAccountsLastTransaction(
            parsed.cells,
            computed,
            reader.readRef(),
            key,
            keyBitLen,
        )
    }

    @JvmStatic
    fun shardAccountsLastTransactionHash(input: ByteArray, key: ByteArray, keyBitLen: Int): String? =
        shardAccountsLastTransaction(input, key, keyBitLen)?.hash

    @JvmStatic
    fun canonicalValidatorSetBytes(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): ByteArray {
        val keysAndWeights = normalizeValidatorSet(validatorPublicKeys, validatorWeights)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, keysAndWeights.first.size)
        for (index in keysAndWeights.first.indices) {
            out.write(keysAndWeights.first[index])
            writeU64Le(out, keysAndWeights.second[index])
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun validatorSetHash(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): String =
        validatorSetHashFromPayload(canonicalValidatorSetBytes(validatorPublicKeys, validatorWeights))

    @JvmStatic
    fun canonicalValidatorSetPayloadBytes(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): ByteArray =
        canonicalValidatorSetBytes(validatorPublicKeys, validatorWeights)

    @JvmStatic
    fun validatorSetHashFromPayload(payload: ByteArray): String {
        validateValidatorSetPayload(payload)
        return hashHex(VALIDATOR_SET_PREFIX_V1, payload)
    }

    @JvmStatic
    fun validatorSetPayloadHash(payload: ByteArray): String {
        validateValidatorSetPayload(payload)
        return hashHex(VALIDATOR_SET_PAYLOAD_PREFIX_V1, payload)
    }

    @JvmStatic
    fun canonicalMasterchainConfigLeafBytes(
        sourceDomain: Int,
        masterchainSeqno: String,
        masterchainBlockHash: String,
        shardStateRoot: String,
        validatorSetHash: String,
        validatorSetPayloadHash: String,
        version: Int = 1,
    ): ByteArray {
        require(version == 1) { "TON masterchain config leaf version must be 1" }
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU32Le(out, sourceDomain)
        writeU64Le(out, normalizeU64(masterchainSeqno, "masterchainSeqno"))
        out.write(hex32Bytes(masterchainBlockHash, "masterchainBlockHash"))
        out.write(hex32Bytes(shardStateRoot, "shardStateRoot"))
        out.write(hex32Bytes(validatorSetHash, "validatorSetHash"))
        out.write(hex32Bytes(validatorSetPayloadHash, "validatorSetPayloadHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun masterchainConfigLeafHash(
        sourceDomain: Int,
        masterchainSeqno: String,
        masterchainBlockHash: String,
        shardStateRoot: String,
        validatorSetHash: String,
        validatorSetPayloadHash: String,
        version: Int = 1,
    ): String =
        hashHex(
            MASTERCHAIN_CONFIG_LEAF_PREFIX_V1,
            canonicalMasterchainConfigLeafBytes(
                sourceDomain,
                masterchainSeqno,
                masterchainBlockHash,
                shardStateRoot,
                validatorSetHash,
                validatorSetPayloadHash,
                version,
            ),
        )

    @JvmStatic
    fun canonicalMasterchainConfigProofBytes(
        sourceDomain: Int,
        masterchainSeqno: String,
        masterchainBlockHash: String,
        shardStateRoot: String,
        configRoot: String,
        validatorSetHash: String,
        validatorSetPayloadHash: String,
        configLeafHash: String,
        configLeafIndex: String,
        configValueHash: String,
        configDictionaryProofBoc: ByteArray,
        configInclusionBranch: List<ByteArray>,
        version: Int = 1,
    ): ByteArray {
        require(version == 1) { "TON masterchain config proof version must be 1" }
        require(sourceDomain == DOMAIN_TON) { "sourceDomain must be TON" }
        val normalizedMasterchainSeqno = normalizeU64(masterchainSeqno, "masterchainSeqno")
        require(normalizedMasterchainSeqno != BigInteger.ZERO) { "masterchainSeqno must be non-zero" }
        val branch = normalizeInclusionBranch(configInclusionBranch)
        require(branch.isEmpty()) { "configInclusionBranch must be empty when configDictionaryProofBoc is used" }
        val masterchainBlockHashBytes = nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash")
        val shardStateRootBytes = nonZeroHex32Bytes(shardStateRoot, "shardStateRoot")
        val configRootBytes = nonZeroHex32Bytes(configRoot, "configRoot")
        val configValueHashBytes = nonZeroHex32Bytes(configValueHash, "configValueHash")
        require(configDictionaryProofBoc.isNotEmpty()) { "configDictionaryProofBoc must be non-empty" }
        require(hashmapEProofRootHash(configDictionaryProofBoc) == "0x" + hexLower(configRootBytes)) {
            "configDictionaryProofBoc root does not match configRoot"
        }
        require(
            hashmapECellRefValueHash(
                configDictionaryProofBoc,
                currentValidatorSetConfigKey(),
                CONFIG_PARAM_KEY_BITS,
            ) == "0x" + hexLower(configValueHashBytes),
        ) {
            "configDictionaryProofBoc value does not match configValueHash"
        }
        val validatorSetPayloadHashBytes = hex32Bytes(validatorSetPayloadHash, "validatorSetPayloadHash")
        require(validatorSetPayloadHashBytes.any { it.toInt() != 0 }) {
            "validatorSetPayloadHash must be non-zero"
        }
        val validatorSetPayload = requireNotNull(configValidatorSetPayloadFromProofBoc(configDictionaryProofBoc)) {
            "configDictionaryProofBoc must open config param 34"
        }
        require(validatorSetPayloadHash(validatorSetPayload) == "0x" + hexLower(validatorSetPayloadHashBytes)) {
            "configDictionaryProofBoc ValidatorSet does not match validatorSetPayloadHash"
        }
        val validatorSetHashBytes = nonZeroHex32Bytes(validatorSetHash, "validatorSetHash")
        require(validatorSetHashFromPayload(validatorSetPayload) == "0x" + hexLower(validatorSetHashBytes)) {
            "validatorSetHash must match configDictionaryProofBoc ValidatorSet"
        }
        val configLeafHashBytes = nonZeroHex32Bytes(configLeafHash, "configLeafHash")
        val expectedConfigLeafHash = masterchainConfigLeafHash(
            sourceDomain = sourceDomain,
            masterchainSeqno = normalizedMasterchainSeqno.toString(),
            masterchainBlockHash = "0x" + hexLower(masterchainBlockHashBytes),
            shardStateRoot = "0x" + hexLower(shardStateRootBytes),
            validatorSetHash = "0x" + hexLower(validatorSetHashBytes),
            validatorSetPayloadHash = "0x" + hexLower(validatorSetPayloadHashBytes),
        )
        require(configLeafHashBytes.contentEquals(hex32Bytes(expectedConfigLeafHash, "configLeafHash"))) {
            "configLeafHash must match TON config proof fields"
        }
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU32Le(out, sourceDomain)
        writeU64Le(out, normalizedMasterchainSeqno)
        out.write(masterchainBlockHashBytes)
        out.write(shardStateRootBytes)
        out.write(configRootBytes)
        out.write(validatorSetHashBytes)
        out.write(validatorSetPayloadHashBytes)
        out.write(configLeafHashBytes)
        val normalizedConfigLeafIndex = normalizeU64(configLeafIndex, "configLeafIndex")
        if (normalizedConfigLeafIndex != BigInteger.valueOf(CURRENT_VALIDATOR_SET_CONFIG_PARAM)) {
            throw IllegalArgumentException("configLeafIndex must be TON current validator set config param 34")
        }
        writeU16Le(out, CONFIG_PARAM_KEY_BITS)
        writeU64Le(out, normalizedConfigLeafIndex)
        out.write(configValueHashBytes)
        writeVector(out, configDictionaryProofBoc)
        writeU32Le(out, branch.size)
        branch.forEach { writeVector(out, it) }
        return out.toByteArray()
    }

    @JvmStatic
    fun masterchainConfigProofHash(
        sourceDomain: Int,
        masterchainSeqno: String,
        masterchainBlockHash: String,
        shardStateRoot: String,
        configRoot: String,
        validatorSetHash: String,
        validatorSetPayloadHash: String,
        configLeafHash: String,
        configLeafIndex: String,
        configValueHash: String,
        configDictionaryProofBoc: ByteArray,
        configInclusionBranch: List<ByteArray>,
        version: Int = 1,
    ): String =
        hashHex(
            MASTERCHAIN_CONFIG_PROOF_PREFIX_V1,
            canonicalMasterchainConfigProofBytes(
                sourceDomain,
                masterchainSeqno,
                masterchainBlockHash,
                shardStateRoot,
                configRoot,
                validatorSetHash,
                validatorSetPayloadHash,
                configLeafHash,
                configLeafIndex,
                configValueHash,
                configDictionaryProofBoc,
                configInclusionBranch,
                version,
            ),
        )

    @JvmStatic
    fun canonicalMasterchainBlockMessageBytes(
        sourceDomain: Int,
        masterchainSeqno: String,
        masterchainWorkchainId: Int,
        masterchainShard: String,
        masterchainBlockHash: String,
        masterchainFileHash: String,
        validatorSetHash: String,
        masterchainConfigRoot: String,
        masterchainConfigProofHash: String,
        shardWorkchainId: Int,
        shardShard: String,
        shardSeqno: String,
        shardBlockHash: String,
        shardFileHash: String,
        shardStateRoot: String,
        transactionRoot: String,
        shardProofHash: String,
    ): ByteArray {
        require(sourceDomain == DOMAIN_TON) { "sourceDomain must be TON" }
        val normalizedMasterchainSeqno = normalizeU64(masterchainSeqno, "masterchainSeqno")
        require(normalizedMasterchainSeqno != BigInteger.ZERO) { "masterchainSeqno must be non-zero" }
        require(masterchainWorkchainId == TON_MASTERCHAIN_WORKCHAIN_ID) {
            "masterchainWorkchainId must be TON masterchain"
        }
        val normalizedShard = normalizeU64(masterchainShard, "masterchainShard")
        require(normalizedShard == TON_MASTERCHAIN_SHARD) {
            "masterchainShard must be TON masterchain shard"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, sourceDomain)
        writeU64Le(out, normalizedMasterchainSeqno)
        writeI32Le(out, masterchainWorkchainId)
        writeU64Le(out, normalizedShard)
        out.write(nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash"))
        out.write(nonZeroHex32Bytes(masterchainFileHash, "masterchainFileHash"))
        out.write(hex32Bytes(validatorSetHash, "validatorSetHash"))
        out.write(hex32Bytes(masterchainConfigRoot, "masterchainConfigRoot"))
        out.write(hex32Bytes(masterchainConfigProofHash, "masterchainConfigProofHash"))
        require(shardWorkchainId == TON_BASECHAIN_WORKCHAIN_ID) {
            "shardWorkchainId must be TON basechain"
        }
        val normalizedBasechainShard = normalizeU64(shardShard, "shardShard")
        require(normalizedBasechainShard != BigInteger.ZERO) { "shardShard must not be zero" }
        val normalizedShardSeqno = normalizeU64(shardSeqno, "shardSeqno")
        require(normalizedShardSeqno != BigInteger.ZERO) { "shardSeqno must not be zero" }
        writeI32Le(out, shardWorkchainId)
        writeU64Le(out, normalizedBasechainShard)
        writeU64Le(out, normalizedShardSeqno)
        out.write(hex32Bytes(shardBlockHash, "shardBlockHash"))
        out.write(nonZeroHex32Bytes(shardFileHash, "shardFileHash"))
        out.write(hex32Bytes(shardStateRoot, "shardStateRoot"))
        out.write(hex32Bytes(transactionRoot, "transactionRoot"))
        out.write(hex32Bytes(shardProofHash, "shardProofHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun masterchainBlockMessageHash(
        sourceDomain: Int,
        masterchainSeqno: String,
        masterchainWorkchainId: Int,
        masterchainShard: String,
        masterchainBlockHash: String,
        masterchainFileHash: String,
        validatorSetHash: String,
        masterchainConfigRoot: String,
        masterchainConfigProofHash: String,
        shardWorkchainId: Int,
        shardShard: String,
        shardSeqno: String,
        shardBlockHash: String,
        shardFileHash: String,
        shardStateRoot: String,
        transactionRoot: String,
        shardProofHash: String,
    ): String =
        hashHex(
            MASTERCHAIN_BLOCK_MESSAGE_PREFIX_V1,
            canonicalMasterchainBlockMessageBytes(
                sourceDomain,
                masterchainSeqno,
                masterchainWorkchainId,
                masterchainShard,
                masterchainBlockHash,
                masterchainFileHash,
                validatorSetHash,
                masterchainConfigRoot,
                masterchainConfigProofHash,
                shardWorkchainId,
                shardShard,
                shardSeqno,
                shardBlockHash,
                shardFileHash,
                shardStateRoot,
                transactionRoot,
                shardProofHash,
            ),
        )

    @JvmStatic
    fun canonicalMasterchainValidatorSignaturesBytes(
        input: TonValidatorSignatureProofInput,
        providedValidatorSetHash: String? = null,
    ): ByteArray {
        val derivedValidatorSetHash = validatorSetHash(input.validatorPublicKeys, input.validatorWeights)
        if (providedValidatorSetHash != null) {
            require(hex32Bytes(providedValidatorSetHash, "validatorSetHash").contentEquals(
                hex32Bytes(derivedValidatorSetHash, "validatorSetHash"),
            )) {
                "validatorSetHash must match validator public keys and weights"
            }
        }
        val out = ByteArrayOutputStream()
        out.write(canonicalValidatorSignatureProofBytes(input))
        out.write(hex32Bytes(derivedValidatorSetHash, "validatorSetHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun masterchainValidatorSignaturesHash(
        input: TonValidatorSignatureProofInput,
        providedValidatorSetHash: String? = null,
    ): String =
        hashHex(
            MASTERCHAIN_SIGNATURES_PREFIX_V1,
            canonicalMasterchainValidatorSignaturesBytes(input, providedValidatorSetHash),
        )

    @JvmStatic
    fun canonicalValidatorSetTransitionMessageBytes(
        sourceDomain: Int,
        fromValidatorSetSeqno: String,
        toValidatorSetSeqno: String,
        masterchainSeqno: String,
        masterchainWorkchainId: Int,
        masterchainShard: String,
        masterchainBlockHash: String,
        masterchainFileHash: String,
        parentValidatorSetHash: String,
        nextValidatorSetHash: String,
        nextValidatorSetPayloadHash: String,
        nextValidatorSetConfigHash: String,
    ): ByteArray {
        require(sourceDomain == DOMAIN_TON) { "sourceDomain must be TON" }
        val normalizedFromSeqno = normalizeU64(fromValidatorSetSeqno, "fromValidatorSetSeqno")
        val normalizedToSeqno = normalizeU64(toValidatorSetSeqno, "toValidatorSetSeqno")
        require(normalizedFromSeqno + BigInteger.ONE == normalizedToSeqno) {
            "toValidatorSetSeqno must be exactly one greater than fromValidatorSetSeqno"
        }
        val normalizedMasterchainSeqno = normalizeU64(masterchainSeqno, "masterchainSeqno")
        require(normalizedMasterchainSeqno != BigInteger.ZERO) { "masterchainSeqno must be non-zero" }
        require(masterchainWorkchainId == TON_MASTERCHAIN_WORKCHAIN_ID) {
            "masterchainWorkchainId must be TON masterchain"
        }
        val normalizedShard = normalizeU64(masterchainShard, "masterchainShard")
        require(normalizedShard == TON_MASTERCHAIN_SHARD) {
            "masterchainShard must be TON masterchain shard"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, sourceDomain)
        writeU64Le(out, normalizedFromSeqno)
        writeU64Le(out, normalizedToSeqno)
        writeU64Le(out, normalizedMasterchainSeqno)
        writeI32Le(out, masterchainWorkchainId)
        writeU64Le(out, normalizedShard)
        out.write(nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash"))
        out.write(nonZeroHex32Bytes(masterchainFileHash, "masterchainFileHash"))
        out.write(nonZeroHex32Bytes(parentValidatorSetHash, "parentValidatorSetHash"))
        out.write(nonZeroHex32Bytes(nextValidatorSetHash, "nextValidatorSetHash"))
        out.write(hex32Bytes(nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"))
        out.write(nonZeroHex32Bytes(nextValidatorSetConfigHash, "nextValidatorSetConfigHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun validatorSetTransitionMessageHash(
        sourceDomain: Int,
        fromValidatorSetSeqno: String,
        toValidatorSetSeqno: String,
        masterchainSeqno: String,
        masterchainWorkchainId: Int,
        masterchainShard: String,
        masterchainBlockHash: String,
        masterchainFileHash: String,
        parentValidatorSetHash: String,
        nextValidatorSetHash: String,
        nextValidatorSetPayloadHash: String,
        nextValidatorSetConfigHash: String,
    ): String =
        hashHex(
            VALIDATOR_SET_TRANSITION_MESSAGE_PREFIX_V1,
            canonicalValidatorSetTransitionMessageBytes(
                sourceDomain,
                fromValidatorSetSeqno,
                toValidatorSetSeqno,
                masterchainSeqno,
                masterchainWorkchainId,
                masterchainShard,
                masterchainBlockHash,
                masterchainFileHash,
                parentValidatorSetHash,
                nextValidatorSetHash,
                nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash,
            ),
        )

    @JvmStatic
    fun canonicalValidatorSetTransitionSignatureBytes(
        version: Int = 1,
        sourceDomain: Int,
        fromValidatorSetSeqno: String,
        toValidatorSetSeqno: String,
        masterchainSeqno: String,
        masterchainWorkchainId: Int,
        masterchainShard: String,
        masterchainBlockHash: String,
        masterchainFileHash: String,
        parentValidatorSetHash: String,
        nextValidatorSetHash: String,
        nextValidatorSetPayload: ByteArray,
        nextValidatorSetPayloadHash: String,
        nextValidatorSetConfigHash: String,
        transitionMessageHash: String,
        validatorSignatureProof: TonValidatorSignatureProofInput,
    ): ByteArray {
        require(version == 1) { "TON validator-set transition proof version must be 1" }
        val parentHash = validatorSetHash(
            validatorSignatureProof.validatorPublicKeys,
            validatorSignatureProof.validatorWeights,
        )
        val parentHashBytes = hex32Bytes(parentHash, "parentValidatorSetHash")
        val providedParentHashBytes = hex32Bytes(parentValidatorSetHash, "parentValidatorSetHash")
        require(providedParentHashBytes.contentEquals(parentHashBytes)) {
            "parentValidatorSetHash must match validatorSignatureProof"
        }
        require(validatorSetPayloadHash(nextValidatorSetPayload) == nextValidatorSetPayloadHash) {
            "nextValidatorSetPayloadHash must match nextValidatorSetPayload"
        }
        require(validatorSetHashFromPayload(nextValidatorSetPayload) == nextValidatorSetHash) {
            "nextValidatorSetHash must match nextValidatorSetPayload"
        }
        val transitionMessageHashBytes = hex32Bytes(transitionMessageHash, "transitionMessageHash")
        val expectedTransitionMessageHash = validatorSetTransitionMessageHash(
            sourceDomain = sourceDomain,
            fromValidatorSetSeqno = fromValidatorSetSeqno,
            toValidatorSetSeqno = toValidatorSetSeqno,
            masterchainSeqno = masterchainSeqno,
            masterchainWorkchainId = masterchainWorkchainId,
            masterchainShard = masterchainShard,
            masterchainBlockHash = masterchainBlockHash,
            masterchainFileHash = masterchainFileHash,
            parentValidatorSetHash = parentValidatorSetHash,
            nextValidatorSetHash = nextValidatorSetHash,
            nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash = nextValidatorSetConfigHash,
        )
        require(
            transitionMessageHashBytes.contentEquals(
                hex32Bytes(expectedTransitionMessageHash, "transitionMessageHash"),
            ),
        ) {
            "transitionMessageHash must match transition message fields"
        }
        require(
            hex32Bytes(validatorSignatureProof.blockMessageHash, "blockMessageHash")
                .contentEquals(transitionMessageHashBytes),
        ) {
            "validatorSignatureProof.blockMessageHash must match transitionMessageHash"
        }
        require(masterchainWorkchainId == TON_MASTERCHAIN_WORKCHAIN_ID) {
            "masterchainWorkchainId must be TON masterchain"
        }
        val normalizedMasterchainShard = normalizeU64(masterchainShard, "masterchainShard")
        require(normalizedMasterchainShard == TON_MASTERCHAIN_SHARD) {
            "masterchainShard must be TON masterchain shard"
        }
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU32Le(out, sourceDomain)
        writeU64Le(out, normalizeU64(fromValidatorSetSeqno, "fromValidatorSetSeqno"))
        writeU64Le(out, normalizeU64(toValidatorSetSeqno, "toValidatorSetSeqno"))
        writeU64Le(out, normalizeU64(masterchainSeqno, "masterchainSeqno"))
        writeI32Le(out, masterchainWorkchainId)
        writeU64Le(out, normalizedMasterchainShard)
        out.write(nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash"))
        out.write(nonZeroHex32Bytes(masterchainFileHash, "masterchainFileHash"))
        out.write(providedParentHashBytes)
        out.write(hex32Bytes(nextValidatorSetHash, "nextValidatorSetHash"))
        writeVector(out, nextValidatorSetPayload)
        out.write(hex32Bytes(nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"))
        out.write(hex32Bytes(nextValidatorSetConfigHash, "nextValidatorSetConfigHash"))
        out.write(transitionMessageHashBytes)
        out.write(parentHashBytes)
        out.write(canonicalValidatorSignatureProofBytes(validatorSignatureProof))
        return out.toByteArray()
    }

    @JvmStatic
    fun validatorSetTransitionSignatureHash(
        version: Int = 1,
        sourceDomain: Int,
        fromValidatorSetSeqno: String,
        toValidatorSetSeqno: String,
        masterchainSeqno: String,
        masterchainWorkchainId: Int,
        masterchainShard: String,
        masterchainBlockHash: String,
        masterchainFileHash: String,
        parentValidatorSetHash: String,
        nextValidatorSetHash: String,
        nextValidatorSetPayload: ByteArray,
        nextValidatorSetPayloadHash: String,
        nextValidatorSetConfigHash: String,
        transitionMessageHash: String,
        validatorSignatureProof: TonValidatorSignatureProofInput,
    ): String =
        hashHex(
            VALIDATOR_SET_TRANSITION_SIGNATURES_PREFIX_V1,
            canonicalValidatorSetTransitionSignatureBytes(
                version,
                sourceDomain,
                fromValidatorSetSeqno,
                toValidatorSetSeqno,
                masterchainSeqno,
                masterchainWorkchainId,
                masterchainShard,
                masterchainBlockHash,
                masterchainFileHash,
                parentValidatorSetHash,
                nextValidatorSetHash,
                nextValidatorSetPayload,
                nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash,
                transitionMessageHash,
                validatorSignatureProof,
            ),
        )

    @JvmStatic
    fun canonicalShardStateProofPublicInputsBytes(input: TonShardStateProofRequestInput): ByteArray {
        val normalized = normalizeShardStateSourceStateInput(input)
        val out = ByteArrayOutputStream()
        out.write(normalized.version)
        writeU32Le(out, normalized.sourceDomain)
        writeU64Le(out, normalized.masterchainSeqno)
        writeI32Le(out, normalized.masterchainWorkchainId)
        writeU64Le(out, normalized.masterchainShard)
        out.write(hex32Bytes(normalized.masterchainBlockHash, "masterchainBlockHash"))
        out.write(hex32Bytes(normalized.masterchainFileHash, "masterchainFileHash"))
        out.write(hex32Bytes(normalized.validatorSetHash, "validatorSetHash"))
        out.write(hex32Bytes(normalized.masterchainConfigRoot, "masterchainConfigRoot"))
        out.write(hex32Bytes(normalized.masterchainConfigProofHash, "masterchainConfigProofHash"))
        writeI32Le(out, normalized.shardWorkchainId)
        writeU64Le(out, normalized.shardShard)
        writeU64Le(out, normalized.shardSeqno)
        out.write(hex32Bytes(normalized.shardBlockHash, "shardBlockHash"))
        out.write(hex32Bytes(normalized.shardFileHash, "shardFileHash"))
        out.write(hex32Bytes(normalized.shardStateRoot, "shardStateRoot"))
        out.write(hex32Bytes(normalized.transactionRoot, "transactionRoot"))
        writeU64Le(out, normalized.transactionLt)
        out.write(hex32Bytes(normalized.shardStateDictionaryRoot, "shardStateDictionaryRoot"))
        writeU16Le(out, normalized.shardStateDictionaryKeyBitLen)
        writeVector(out, normalized.shardStateDictionaryKey)
        out.write(hex32Bytes(normalized.masterchainSignatureHash, "masterchainSignatureHash"))
        out.write(hex32Bytes(normalized.shardProofHash, "shardProofHash"))
        out.write(hex32Bytes(normalized.shardStateProofBocHash, "shardStateProofBocHash"))
        out.write(hex32Bytes(normalized.shardAccountsProofBocHash, "shardAccountsProofBocHash"))
        out.write(hex32Bytes(normalized.configProofBocHash, "configProofBocHash"))
        out.write(hex32Bytes(normalized.transitionChainHash, "transitionChainHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun shardStateProofPublicInputsHash(input: TonShardStateProofRequestInput): String =
        hashHex(
            SHARD_STATE_PROOF_PUBLIC_INPUTS_PREFIX_V1,
            canonicalShardStateProofPublicInputsBytes(input),
        )

    @JvmStatic
    fun canonicalShardStateWitnessCommitmentBytes(input: TonShardStateProofRequestInput): ByteArray {
        val normalized = normalizeShardStateSourceStateInput(input)
        val out = ByteArrayOutputStream()
        out.write(normalized.version)
        writeVector(out, normalized.shardStateProofBoc)
        writeVector(out, normalized.shardStateDictionaryProofBoc)
        writeVector(out, normalized.configDictionaryProofBoc)
        writeU32Le(out, normalized.validatorSetTransitionProofs.size)
        normalized.validatorSetTransitionProofs.forEach { transition ->
            out.write(canonicalValidatorSetTransitionProofBytes(transition))
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun canonicalShardStateVerificationContextBytes(input: TonShardStateProofRequestInput): ByteArray {
        val normalized = normalizeShardStateSourceStateInput(input)
        val out = ByteArrayOutputStream()
        out.write(normalized.version)
        writeString(out, normalized.sourceStateVerifierId, "sourceStateVerifierId")
        out.write(hex32Bytes(normalized.sourceStateVerifierHash, "sourceStateVerifierHash"))
        writeString(out, normalized.sourceTrustAnchorId, "sourceTrustAnchorId")
        out.write(hex32Bytes(normalized.sourceTrustAnchorHash, "sourceTrustAnchorHash"))
        writeString(out, normalized.consensusVerifierId, "consensusVerifierId")
        out.write(hex32Bytes(normalized.consensusVerifierHash, "consensusVerifierHash"))
        writeString(out, normalized.messageInclusionVerifierId, "messageInclusionVerifierId")
        out.write(hex32Bytes(normalized.messageInclusionVerifierHash, "messageInclusionVerifierHash"))
        writeString(out, normalized.finalityPolicyId, "finalityPolicyId")
        out.write(hex32Bytes(normalized.finalityPolicyHash, "finalityPolicyHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun shardStatePublicInputColumns(input: TonShardStateProofRequestInput): List<List<String>> {
        val normalized = normalizeShardStateSourceStateInput(input)
        val publicInputsHash = shardStateProofPublicInputsHash(input)
        return listOf(
            listOf("0x" + hexLower(sccpWordU32Le(normalized.sourceDomain))),
            listOf("0x" + hexLower(sccpWordU64Le(normalized.masterchainSeqno))),
            listOf("0x" + hexLower(sccpWordI32Le(normalized.masterchainWorkchainId))),
            listOf("0x" + hexLower(sccpWordU64Le(normalized.masterchainShard))),
            listOf(normalized.masterchainBlockHash),
            listOf(normalized.validatorSetHash),
            listOf(normalized.masterchainConfigRoot),
            listOf("0x" + hexLower(sccpWordI32Le(normalized.shardWorkchainId))),
            listOf("0x" + hexLower(sccpWordU64Le(normalized.shardShard))),
            listOf("0x" + hexLower(sccpWordU64Le(normalized.shardSeqno))),
            listOf(normalized.shardBlockHash),
            listOf(normalized.shardStateRoot),
            listOf(normalized.shardStateDictionaryRoot),
            listOf(normalized.transactionRoot),
            listOf("0x" + hexLower(sccpWordU64Le(normalized.transactionLt))),
            listOf(publicInputsHash),
        )
    }

    @JvmStatic
    fun shardStateOpenVerifySchemaDescriptor(input: TonShardStateProofRequestInput): ByteArray {
        val normalized = normalizeShardStateSourceStateInput(input)
        val out = ByteArrayOutputStream()
        out.write(normalized.version)
        writeString(out, SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1, "circuitId")
        writeString(out, SHARD_STATE_FASTPQ_PARAMETER_SET_V1, "parameterSet")
        writeI32Le(out, TON_MAINNET_GLOBAL_ID)
        writeU32Le(out, normalized.sourceDomain)
        listOf(
            "source_domain",
            "masterchain_seqno",
            "masterchain_workchain_id",
            "masterchain_shard",
            "masterchain_block_hash",
            "validator_set_hash",
            "masterchain_config_root",
            "shard_workchain_id",
            "shard_shard",
            "shard_seqno",
            "shard_block_hash",
            "shard_state_root",
            "shard_state_dictionary_root",
            "transaction_root",
            "transaction_lt",
            "shard_state_proof_public_inputs_hash",
        ).forEach { writeString(out, it, "requiredInput") }
        return out.toByteArray()
    }

    @JvmStatic
    fun buildShardStateProofRequest(input: TonShardStateProofRequestInput): TonShardStateProofRequest {
        val normalized = normalizeShardStateSourceStateInput(input)
        val statementBytes = canonicalShardStateProofPublicInputsBytes(input)
        val witnessCommitmentBytes = canonicalShardStateWitnessCommitmentBytes(input)
        val verificationContextBytes = canonicalShardStateVerificationContextBytes(input)
        val publicInputsHash = shardStateProofPublicInputsHash(input)
        val dsidHash = prefixedHashBytes(
            SHARD_STATE_FASTPQ_DSID_PREFIX_V1,
            hex32Bytes(publicInputsHash, "shardStateProofPublicInputsHash"),
        )
        return TonShardStateProofRequest(
            version = 1,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
            circuitId = SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
            parameterSet = SHARD_STATE_FASTPQ_PARAMETER_SET_V1,
            sourceDomain = normalized.sourceDomain,
            masterchainSeqno = normalized.masterchainSeqno.toString(),
            shardSeqno = normalized.shardSeqno.toString(),
            sourceStateVerifierId = normalized.sourceStateVerifierId,
            sourceStateVerifierHash = normalized.sourceStateVerifierHash,
            shardStateProofPublicInputsHash = publicInputsHash,
            statementBytes = statementBytes,
            witnessCommitmentBytes = witnessCommitmentBytes,
            verificationContextBytes = verificationContextBytes,
            schemaDescriptor = shardStateOpenVerifySchemaDescriptor(input),
            publicInputColumns = shardStatePublicInputColumns(input),
            fastpqPublicInputs = TonShardStateFastpqPublicInputs(
                dsid = "0x" + hexLower(dsidHash.copyOfRange(0, 16)),
                slot = normalized.masterchainSeqno.toString(),
                oldRoot = normalized.masterchainConfigRoot,
                newRoot = normalized.shardStateRoot,
                permRoot = normalized.shardStateDictionaryRoot,
                txSetHash = publicInputsHash,
            ),
            fastpqTransitions = listOf(
                TonShardStateFastpqTransition(
                    key = SHARD_STATE_FASTPQ_STATEMENT_KEY_V1,
                    operation = "meta_set",
                    oldValue = "0x",
                    newValue = "0x" + hexLower(statementBytes),
                ),
                TonShardStateFastpqTransition(
                    key = SHARD_STATE_FASTPQ_WITNESS_KEY_V1,
                    operation = "meta_set",
                    oldValue = "0x",
                    newValue = "0x" + hexLower(witnessCommitmentBytes),
                ),
                TonShardStateFastpqTransition(
                    key = SHARD_STATE_FASTPQ_CONTEXT_KEY_V1,
                    operation = "meta_set",
                    oldValue = "0x",
                    newValue = "0x" + hexLower(verificationContextBytes),
                ),
            ),
        )
    }

    @JvmStatic
    fun canonicalSourceStateVerificationProofBytes(
        proof: TonSccpSourceStateVerificationProof,
    ): ByteArray {
        require(
            proof.version == 1 &&
                proof.proofFamily == STARK_FRI_PROOF_FAMILY_V1 &&
                proof.circuitId in SOURCE_STATE_VERIFICATION_CIRCUIT_IDS,
        ) {
            "sourceStateVerificationProof must be a TON source-state stark-fri-v1 proof"
        }
        require(proof.proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proof.proofBytes.size <= SOURCE_STATE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $SOURCE_STATE_MAX_PROOF_BYTES bytes"
        }
        require(proof.proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        val out = ByteArrayOutputStream()
        out.write(proof.version)
        writeString(out, proof.proofFamily, "proofFamily")
        writeString(out, proof.circuitId, "circuitId")
        writeVector(out, proof.proofBytes)
        return out.toByteArray()
    }

    @JvmStatic
    fun shardStateVerificationProofHash(proof: TonSccpSourceStateVerificationProof): String {
        require(proof.circuitId == SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1) {
            "shardStateVerificationProof must be the TON shard-state stark-fri-v1 proof"
        }
        return hashHex(
            "sccp:ton:source-state-verification-proof:v1",
            canonicalSourceStateVerificationProofBytes(proof),
        )
    }

    @JvmStatic
    fun wrapSourceStateVerificationProof(
        proofBytes: ByteArray,
        request: TonShardStateProofRequest,
    ): TonSccpSourceStateVerificationProof {
        requireSourceStateProofRequestForWrapping(request)
        return wrapSourceStateVerificationProof(
            proofBytes = proofBytes,
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            sourceDomain = request.sourceDomain,
        )
    }

    @JvmStatic
    fun wrapSourceStateVerificationProof(
        proofBytes: ByteArray,
        request: TonSccpFullLightClientAuditProofRequest,
    ): TonSccpSourceStateVerificationProof {
        requireSourceStateProofRequestForWrapping(request)
        return wrapSourceStateVerificationProof(
            proofBytes = proofBytes,
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            sourceDomain = request.sourceDomain,
        )
    }

    internal fun requireSourceStateProofRequestForProverCallback(
        request: TonShardStateProofRequest,
    ) {
        requireSourceStateProofRequestForWrapping(request)
    }

    internal fun requireSourceStateProofRequestForProverCallback(
        request: TonSccpFullLightClientAuditProofRequest,
    ) {
        requireSourceStateProofRequestForWrapping(request)
    }

    private fun requireSourceStateProofRequestForWrapping(
        request: TonShardStateProofRequest,
    ) {
        require(request.version == 1) { "TON source-state proof request.version must be 1" }
        require(request.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "TON source-state proof request.proofFamily must be stark-fri-v1"
        }
        require(request.circuitId == SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1) {
            "request.circuitId must be the TON shard-state OpenVerify circuit"
        }
        require(request.parameterSet == SHARD_STATE_FASTPQ_PARAMETER_SET_V1) {
            "request.parameterSet must be fastpq-lane-balanced"
        }
        require(request.sourceDomain == DOMAIN_TON) {
            "TON source-state proof request.sourceDomain must be TON"
        }
        require(normalizeU64(request.masterchainSeqno, "request.masterchainSeqno") != BigInteger.ZERO) {
            "request.masterchainSeqno must not be zero"
        }
        require(normalizeU64(request.shardSeqno, "request.shardSeqno") != BigInteger.ZERO) {
            "request.shardSeqno must not be zero"
        }
        require(request.sourceStateVerifierId == MAINNET_SHARD_STATE_VERIFIER_ID_V1) {
            "request.sourceStateVerifierId must match TON shard-state verifier profile"
        }
        val sourceStateVerifierHash =
            nonZeroHex32Bytes(request.sourceStateVerifierHash, "request.sourceStateVerifierHash")
        require(!sourceStateVerifierHash.contentEquals(TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
            "request.sourceStateVerifierHash must not be the TON template verifier hash"
        }
        val derivedPublicInputsHash = hashHex(
            SHARD_STATE_PROOF_PUBLIC_INPUTS_PREFIX_V1,
            request.statementBytes,
        )
        require(
            normalizeNonZeroHex32(
                request.shardStateProofPublicInputsHash,
                "request.shardStateProofPublicInputsHash",
            ) == derivedPublicInputsHash,
        ) {
            "request.shardStateProofPublicInputsHash must match request.statementBytes"
        }
        val shardDsidHash = prefixedHashBytes(
            SHARD_STATE_FASTPQ_DSID_PREFIX_V1,
            hex32Bytes(derivedPublicInputsHash, "request.shardStateProofPublicInputsHash"),
        )
        require(request.fastpqPublicInputs.dsid == "0x" + hexLower(shardDsidHash.copyOfRange(0, 16))) {
            "request.fastpqPublicInputs.dsid must match request.statementBytes"
        }
        require(
            normalizeNonZeroHex32(
                request.fastpqPublicInputs.txSetHash,
                "request.fastpqPublicInputs.txSetHash",
            ) == derivedPublicInputsHash,
        ) {
            "request.fastpqPublicInputs.txSetHash must match request.statementBytes"
        }
        requireTonOpenVerifyRequestPayloadForWrapping(
            statementBytes = request.statementBytes,
            witnessCommitmentBytes = request.witnessCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqFields = listOf(
                request.fastpqPublicInputs.dsid,
                request.fastpqPublicInputs.slot,
                request.fastpqPublicInputs.oldRoot,
                request.fastpqPublicInputs.newRoot,
                request.fastpqPublicInputs.permRoot,
                request.fastpqPublicInputs.txSetHash,
            ),
            transitionEntries = request.fastpqTransitions.map(::tonTransitionCheck),
            expectedTransitionEntries = tonShardStateExpectedTransitionChecks(
                request.statementBytes,
                request.witnessCommitmentBytes,
                request.verificationContextBytes,
            ),
        )
    }

    private fun requireSourceStateProofRequestForWrapping(
        request: TonSccpFullLightClientAuditProofRequest,
    ) {
        require(request.version == 1) { "TON source-state proof request.version must be 1" }
        require(request.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "TON source-state proof request.proofFamily must be stark-fri-v1"
        }
        require(request.parameterSet == FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1) {
            "request.parameterSet must be fastpq-lane-balanced"
        }
        require(request.sourceDomain == DOMAIN_TON) {
            "TON source-state proof request.sourceDomain must be TON"
        }
        val profile = auditRoleProfileForRequest(request.role)
        require(request.roleCode == profile.code) { "request.roleCode must match request.role" }
        require(request.circuitId == profile.circuitId) { "request.circuitId must match request.role" }
        require(request.verifierId == profile.verifierId) { "request.verifierId must match request.role" }
        require(normalizeU64(request.masterchainSeqno, "request.masterchainSeqno") != BigInteger.ZERO) {
            "request.masterchainSeqno must not be zero"
        }
        require(normalizeU64(request.shardSeqno, "request.shardSeqno") != BigInteger.ZERO) {
            "request.shardSeqno must not be zero"
        }
        normalizeNonZeroHex32(request.verifierHash, "request.verifierHash")
        require(request.sourceStateVerifierId == MAINNET_SHARD_STATE_VERIFIER_ID_V1) {
            "request.sourceStateVerifierId must match TON shard-state verifier profile"
        }
        val sourceStateVerifierHash =
            nonZeroHex32Bytes(request.sourceStateVerifierHash, "request.sourceStateVerifierHash")
        require(!sourceStateVerifierHash.contentEquals(TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
            "request.sourceStateVerifierHash must not be the TON template verifier hash"
        }
        for ((field, hash) in listOf(
            "request.sourceVerifierMaterialHash" to request.sourceVerifierMaterialHash,
            "request.sourceAdapterDeploymentHash" to request.sourceAdapterDeploymentHash,
            "request.fullLightClientGateHash" to request.fullLightClientGateHash,
            "request.shardStateProofPublicInputsHash" to request.shardStateProofPublicInputsHash,
            "request.shardStateVerificationProofHash" to request.shardStateVerificationProofHash,
            "request.auditStatementHash" to request.auditStatementHash,
        )) {
            normalizeNonZeroHex32(hash, field)
        }
        val normalizedFullLightClientGateHash = normalizeNonZeroHex32(
            request.fullLightClientGateHash,
            "request.fullLightClientGateHash",
        )
        val normalizedAuditStatementHash =
            normalizeNonZeroHex32(request.auditStatementHash, "request.auditStatementHash")
        val derivedAuditStatementHash =
            hashHex(FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1, request.statementBytes)
        require(normalizedAuditStatementHash == derivedAuditStatementHash) {
            "request.auditStatementHash must match request.statementBytes"
        }
        val auditDsidPreimage = ByteArrayOutputStream()
        auditDsidPreimage.write(profile.code)
        auditDsidPreimage.write(hex32Bytes(normalizedAuditStatementHash, "request.auditStatementHash"))
        val auditDsidHash = prefixedHashBytes(
            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1,
            auditDsidPreimage.toByteArray(),
        )
        require(request.fastpqPublicInputs.dsid == "0x" + hexLower(auditDsidHash.copyOfRange(0, 16))) {
            "request.fastpqPublicInputs.dsid must match request.statementBytes"
        }
        require(
            normalizeNonZeroHex32(
                request.fastpqPublicInputs.txSetHash,
                "request.fastpqPublicInputs.txSetHash",
            ) == derivedAuditStatementHash,
        ) {
            "request.fastpqPublicInputs.txSetHash must match request.statementBytes"
        }
        requireTonOpenVerifyRequestPayloadForWrapping(
            statementBytes = request.statementBytes,
            witnessCommitmentBytes = null,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqFields = listOf(
                request.fastpqPublicInputs.dsid,
                request.fastpqPublicInputs.slot,
                request.fastpqPublicInputs.oldRoot,
                request.fastpqPublicInputs.newRoot,
                request.fastpqPublicInputs.permRoot,
                request.fastpqPublicInputs.txSetHash,
            ),
            transitionEntries = request.fastpqTransitions.map(::tonTransitionCheck),
            expectedTransitionEntries = tonAuditExpectedTransitionChecks(
                profile,
                request.statementBytes,
                request.verificationContextBytes,
                normalizedFullLightClientGateHash,
            ),
        )
    }

    private fun auditRoleProfileForRequest(role: String): TonFullLightClientAuditRoleProfile =
        when (normalizeNonEmpty(role, "request.role")) {
            "masterchainConfig", "masterchain_config" ->
                auditRoleProfile(TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG)
            "validatorSetTransition", "validator_set_transition" ->
                auditRoleProfile(TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION)
            "shardAccountsDictionary", "shard_accounts_dictionary" ->
                auditRoleProfile(TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY)
            else -> throw IllegalArgumentException(
                "request.role must be masterchain_config, validator_set_transition, or shard_accounts_dictionary",
            )
        }

    private data class TonFastpqTransitionCheck(
        val key: String,
        val operation: String,
        val oldValue: String,
        val newValue: String,
    )

    private fun tonTransitionCheck(transition: TonShardStateFastpqTransition): TonFastpqTransitionCheck =
        TonFastpqTransitionCheck(
            key = transition.key,
            operation = transition.operation,
            oldValue = transition.oldValue,
            newValue = transition.newValue,
        )

    private fun tonTransitionCheck(
        transition: TonSccpFullLightClientAuditFastpqTransition,
    ): TonFastpqTransitionCheck =
        TonFastpqTransitionCheck(
            key = transition.key,
            operation = transition.operation,
            oldValue = transition.oldValue,
            newValue = transition.newValue,
        )

    private fun tonShardStateExpectedTransitionChecks(
        statementBytes: ByteArray,
        witnessCommitmentBytes: ByteArray,
        verificationContextBytes: ByteArray,
    ): List<TonFastpqTransitionCheck> =
        listOf(
            TonFastpqTransitionCheck(
                key = SHARD_STATE_FASTPQ_STATEMENT_KEY_V1,
                operation = "meta_set",
                oldValue = "0x",
                newValue = "0x" + hexLower(statementBytes),
            ),
            TonFastpqTransitionCheck(
                key = SHARD_STATE_FASTPQ_WITNESS_KEY_V1,
                operation = "meta_set",
                oldValue = "0x",
                newValue = "0x" + hexLower(witnessCommitmentBytes),
            ),
            TonFastpqTransitionCheck(
                key = SHARD_STATE_FASTPQ_CONTEXT_KEY_V1,
                operation = "meta_set",
                oldValue = "0x",
                newValue = "0x" + hexLower(verificationContextBytes),
            ),
        )

    private fun tonAuditExpectedTransitionChecks(
        profile: TonFullLightClientAuditRoleProfile,
        statementBytes: ByteArray,
        verificationContextBytes: ByteArray,
        fullLightClientGateHash: String,
    ): List<TonFastpqTransitionCheck> =
        listOf(
            TonFastpqTransitionCheck(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = "0x",
                newValue = "0x" + hexLower(statementBytes),
            ),
            TonFastpqTransitionCheck(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = "0x",
                newValue = "0x" + hexLower(verificationContextBytes),
            ),
            TonFastpqTransitionCheck(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = "0x",
                newValue = fullLightClientGateHash,
            ),
        )

    private fun requireTonOpenVerifyRequestPayloadForWrapping(
        statementBytes: ByteArray,
        witnessCommitmentBytes: ByteArray?,
        verificationContextBytes: ByteArray,
        schemaDescriptor: ByteArray,
        publicInputColumns: List<List<String>>,
        fastpqFields: List<String>,
        transitionEntries: List<TonFastpqTransitionCheck>,
        expectedTransitionEntries: List<TonFastpqTransitionCheck>,
    ) {
        require(statementBytes.isNotEmpty()) { "request.statementBytes must not be empty" }
        if (witnessCommitmentBytes != null) {
            require(witnessCommitmentBytes.isNotEmpty()) {
                "request.witnessCommitmentBytes must not be empty"
            }
        }
        require(verificationContextBytes.isNotEmpty()) {
            "request.verificationContextBytes must not be empty"
        }
        require(schemaDescriptor.isNotEmpty()) { "request.schemaDescriptor must not be empty" }
        require(publicInputColumns.isNotEmpty()) { "request.publicInputColumns is required" }
        publicInputColumns.forEachIndexed { index, column ->
            require(column.isNotEmpty()) { "request.publicInputColumns[$index] must not be empty" }
            column.forEachIndexed { valueIndex, value ->
                normalizeNonEmpty(value, "request.publicInputColumns[$index][$valueIndex]")
            }
        }
        fastpqFields.forEachIndexed { index, field ->
            normalizeNonEmpty(field, "request.fastpqPublicInputs[$index]")
        }
        require(transitionEntries.isNotEmpty()) { "request.fastpqTransitions is required" }
        transitionEntries.forEachIndexed { index, transition ->
            normalizeNonEmpty(transition.key, "request.fastpqTransitions[$index].key")
            normalizeNonEmpty(transition.operation, "request.fastpqTransitions[$index].operation")
            normalizeNonEmpty(transition.oldValue, "request.fastpqTransitions[$index].oldValue")
            normalizeNonEmpty(transition.newValue, "request.fastpqTransitions[$index].newValue")
        }
        require(
            transitionEntries.sortedBy { it.key } == expectedTransitionEntries.sortedBy { it.key },
        ) {
            "request.fastpqTransitions must match the canonical TON source-state request"
        }
    }

    private fun wrapSourceStateVerificationProof(
        proofBytes: ByteArray,
        version: Int,
        proofFamily: String,
        circuitId: String,
        sourceDomain: Int,
    ): TonSccpSourceStateVerificationProof {
        require(version == 1) { "sourceStateProof.version must be 1" }
        require(proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "sourceStateProof.proofFamily must be stark-fri-v1"
        }
        require(sourceDomain == DOMAIN_TON) { "sourceStateProof.sourceDomain must be TON" }
        require(circuitId in SOURCE_STATE_VERIFICATION_CIRCUIT_IDS) {
            "sourceStateProof.circuitId must be a TON source-state verification circuit"
        }
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= SOURCE_STATE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $SOURCE_STATE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        return TonSccpSourceStateVerificationProof(
            version = version,
            proofFamily = proofFamily,
            circuitId = circuitId,
            proofBytes = proofBytes,
        )
    }

    @JvmStatic
    fun canonicalFullLightClientAuditStatementBytes(
        input: TonSccpFullLightClientAuditProofInput,
        role: TonSccpFullLightClientAuditRole,
    ): ByteArray {
        val value = normalizeFullLightClientAuditInput(input, role)
        val profile = auditRoleProfile(role)
        val shardState = value.shardState
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(profile.code)
        writeString(out, profile.circuitId, "circuitId")
        writeString(out, CONTRACT_PROOF_BACKEND_V1, "backend")
        writeI32Le(out, TON_MAINNET_GLOBAL_ID)
        writeU32Le(out, shardState.sourceDomain)
        writeU64Le(out, shardState.masterchainSeqno)
        writeI32Le(out, shardState.masterchainWorkchainId)
        writeU64Le(out, shardState.masterchainShard)
        out.write(hex32Bytes(shardState.masterchainBlockHash, "masterchainBlockHash"))
        out.write(hex32Bytes(shardState.masterchainFileHash, "masterchainFileHash"))
        out.write(hex32Bytes(shardState.validatorSetHash, "validatorSetHash"))
        out.write(hex32Bytes(shardState.masterchainConfigRoot, "masterchainConfigRoot"))
        out.write(hex32Bytes(shardState.masterchainConfigProofHash, "masterchainConfigProofHash"))
        writeI32Le(out, shardState.shardWorkchainId)
        writeU64Le(out, shardState.shardShard)
        writeU64Le(out, shardState.shardSeqno)
        out.write(hex32Bytes(shardState.shardBlockHash, "shardBlockHash"))
        out.write(hex32Bytes(shardState.shardFileHash, "shardFileHash"))
        out.write(hex32Bytes(shardState.shardStateRoot, "shardStateRoot"))
        out.write(hex32Bytes(shardState.shardStateDictionaryRoot, "shardStateDictionaryRoot"))
        out.write(hex32Bytes(shardState.transactionRoot, "transactionRoot"))
        writeU64Le(out, shardState.transactionLt)
        out.write(hex32Bytes(shardState.masterchainSignatureHash, "masterchainSignatureHash"))
        out.write(hex32Bytes(shardState.shardProofHash, "shardProofHash"))
        out.write(hex32Bytes(value.shardStateVerificationProofHash, "shardStateVerificationProofHash"))
        out.write(hex32Bytes(value.shardStateProofPublicInputsHash, "shardStateProofPublicInputsHash"))
        when (role) {
            TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG -> {
                out.write(hex32Bytes(value.validatorSetPayloadHash, "validatorSetPayloadHash"))
                out.write(hex32Bytes(value.configLeafHash, "configLeafHash"))
                out.write(hex32Bytes(value.configValueHash, "configValueHash"))
                out.write(hex32Bytes(shardState.configProofBocHash, "configProofBocHash"))
            }
            TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION -> {
                out.write(hex32Bytes(shardState.transitionChainHash, "transitionChainHash"))
                writeU32Le(out, shardState.validatorSetTransitionProofs.size)
                shardState.validatorSetTransitionProofs.forEach { transition ->
                    out.write(canonicalValidatorSetTransitionProofBytes(transition))
                }
            }
            TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY -> {
                out.write(hex32Bytes(shardState.shardStateProofBocHash, "shardStateProofBocHash"))
                out.write(hex32Bytes(shardState.shardAccountsProofBocHash, "shardAccountsProofBocHash"))
                writeU16Le(out, shardState.shardStateDictionaryKeyBitLen)
                writeVector(out, shardState.shardStateDictionaryKey)
                out.write(hex32Bytes(value.shardStateProofPublicInputsHash, "shardStateProofPublicInputsHash"))
            }
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun fullLightClientAuditStatementHash(
        input: TonSccpFullLightClientAuditProofInput,
        role: TonSccpFullLightClientAuditRole,
    ): String =
        hashHex(
            FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1,
            canonicalFullLightClientAuditStatementBytes(input, role),
        )

    @JvmStatic
    fun fullLightClientAuditPublicInputColumns(
        input: TonSccpFullLightClientAuditProofInput,
        role: TonSccpFullLightClientAuditRole,
    ): List<List<String>> {
        val value = normalizeFullLightClientAuditInput(input, role)
        val profile = auditRoleProfile(role)
        val shardState = value.shardState
        val statementHash = fullLightClientAuditStatementHash(input, role)
        requireFullLightClientAuditRequestHashSeparation(value, statementHash)
        val columns = mutableListOf(
            listOf("0x" + hexLower(sccpWordU8(profile.code))),
            listOf("0x" + hexLower(sccpWordU32Le(shardState.sourceDomain))),
            listOf("0x" + hexLower(sccpWordU64Le(shardState.masterchainSeqno))),
            listOf(shardState.masterchainBlockHash),
            listOf("0x" + hexLower(sccpWordU64Le(shardState.shardSeqno))),
            listOf(shardState.shardBlockHash),
            listOf(statementHash),
            listOf(value.sourceVerifierMaterialHash),
            listOf(value.sourceAdapterDeploymentHash),
            listOf(value.fullLightClientGateHash),
            listOf(value.verifierHash),
        )
        auditRoleColumns(value).forEach { columns.add(listOf(it)) }
        return columns
    }

    @JvmStatic
    fun fullLightClientAuditOpenVerifySchemaDescriptor(
        input: TonSccpFullLightClientAuditProofInput,
        role: TonSccpFullLightClientAuditRole,
    ): ByteArray {
        val value = normalizeFullLightClientAuditInput(input, role)
        requireFullLightClientAuditRequestHashSeparation(value)
        val profile = auditRoleProfile(role)
        val shardState = value.shardState
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(profile.code)
        writeString(out, profile.circuitId, "circuitId")
        writeString(out, FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1, "parameterSet")
        writeI32Le(out, TON_MAINNET_GLOBAL_ID)
        writeU32Le(out, shardState.sourceDomain)
        writeString(out, "verifier_id", "schemaField")
        writeString(out, profile.verifierId, "verifierId")
        writeString(out, "verifier_hash", "schemaField")
        out.write(hex32Bytes(value.verifierHash, "verifierHash"))
        writeString(out, "source_verifier_material_hash", "schemaField")
        out.write(hex32Bytes(value.sourceVerifierMaterialHash, "sourceVerifierMaterialHash"))
        writeString(out, "source_adapter_deployment_hash", "schemaField")
        out.write(hex32Bytes(value.sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"))
        writeString(out, "full_light_client_gate_hash", "schemaField")
        out.write(hex32Bytes(value.fullLightClientGateHash, "fullLightClientGateHash"))
        listOf(
            "role",
            "source_domain",
            "masterchain_seqno",
            "masterchain_block_hash",
            "shard_seqno",
            "shard_block_hash",
            "audit_statement_hash",
            "source_verifier_material_hash",
            "source_adapter_deployment_hash",
            "full_light_client_gate_hash",
            "verifier_hash",
        ).plus(profile.requiredInputNames).forEach { writeString(out, it, "requiredInput") }
        return out.toByteArray()
    }

    @JvmStatic
    fun buildFullLightClientAuditProofRequest(
        input: TonSccpFullLightClientAuditProofInput,
        role: TonSccpFullLightClientAuditRole,
    ): TonSccpFullLightClientAuditProofRequest {
        val value = normalizeFullLightClientAuditInput(input, role)
        val profile = auditRoleProfile(role)
        val shardState = value.shardState
        val statementBytes = canonicalFullLightClientAuditStatementBytes(input, role)
        val auditStatementHash = fullLightClientAuditStatementHash(input, role)
        val verificationContextBytes = canonicalFullLightClientAuditContextBytes(value, auditStatementHash)
        val transitions = listOf(
            TonSccpFullLightClientAuditFastpqTransition(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = "0x",
                newValue = "0x" + hexLower(statementBytes),
            ),
            TonSccpFullLightClientAuditFastpqTransition(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = "0x",
                newValue = "0x" + hexLower(verificationContextBytes),
            ),
            TonSccpFullLightClientAuditFastpqTransition(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = "0x",
                newValue = value.fullLightClientGateHash,
            ),
        ).sortedBy { it.key }
        return TonSccpFullLightClientAuditProofRequest(
            version = 1,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
            circuitId = profile.circuitId,
            parameterSet = FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1,
            role = profile.name,
            roleCode = profile.code,
            sourceDomain = DOMAIN_TON,
            masterchainSeqno = shardState.masterchainSeqno.toString(),
            shardSeqno = shardState.shardSeqno.toString(),
            verifierId = profile.verifierId,
            verifierHash = value.verifierHash,
            sourceStateVerifierId = shardState.sourceStateVerifierId,
            sourceStateVerifierHash = shardState.sourceStateVerifierHash,
            sourceVerifierMaterialHash = value.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = value.sourceAdapterDeploymentHash,
            fullLightClientGateHash = value.fullLightClientGateHash,
            shardStateProofPublicInputsHash = value.shardStateProofPublicInputsHash,
            shardStateVerificationProofHash = value.shardStateVerificationProofHash,
            auditStatementHash = auditStatementHash,
            statementBytes = statementBytes,
            verificationContextBytes = verificationContextBytes,
            schemaDescriptor = fullLightClientAuditOpenVerifySchemaDescriptor(input, role),
            publicInputColumns = fullLightClientAuditPublicInputColumns(input, role),
            fastpqPublicInputs = fullLightClientAuditFastpqPublicInputs(value, auditStatementHash),
            fastpqTransitions = transitions,
        )
    }

    @JvmStatic
    fun buildMasterchainConfigProofRequest(
        input: TonSccpFullLightClientAuditProofInput,
    ): TonSccpFullLightClientAuditProofRequest =
        buildFullLightClientAuditProofRequest(input, TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG)

    @JvmStatic
    fun buildValidatorSetTransitionProofRequest(
        input: TonSccpFullLightClientAuditProofInput,
    ): TonSccpFullLightClientAuditProofRequest =
        buildFullLightClientAuditProofRequest(input, TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION)

    @JvmStatic
    fun buildShardAccountsDictionaryProofRequest(
        input: TonSccpFullLightClientAuditProofInput,
    ): TonSccpFullLightClientAuditProofRequest =
        buildFullLightClientAuditProofRequest(input, TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY)

    @JvmStatic
    fun buildFullLightClientAuditProofRequests(
        input: TonSccpFullLightClientAuditProofInput,
    ): TonSccpFullLightClientAuditProofRequests =
        TonSccpFullLightClientAuditProofRequests(
            masterchainConfig = buildMasterchainConfigProofRequest(input),
            validatorSetTransition = buildValidatorSetTransitionProofRequest(input),
            shardAccountsDictionary = buildShardAccountsDictionaryProofRequest(input),
        )

    @JvmStatic
    fun submissionQueryId(publicInputs: TonSccpPublicInputsInput): String {
        val messageId = hex32Bytes(publicInputs.messageId, "messageId")
        var value = BigInteger.ZERO
        for (i in 0 until 8) {
            value = value.shiftLeft(8).or(BigInteger.valueOf((messageId[i].toInt() and 0xff).toLong()))
        }
        return value.toString()
    }

    private fun normalizeSubmissionDestinationBinding(
        binding: TonSccpSubmissionDestinationBindingInput,
        field: String,
    ): Pair<String, String> =
        Pair(
            normalizeNonEmpty(binding.key, "$field.key"),
            normalizeNonZeroHex32(binding.bindingHash, "$field.bindingHash"),
        )

    @JvmStatic
    fun canonicalSubmissionMetadataBytes(input: TonSccpSubmissionMetadataInput): ByteArray {
        val manifest = input.manifest
        require(manifest.version == 1) { "manifest.version must be 1" }
        require(manifest.localDomain == SccpSolana.DOMAIN_SORA) { "manifest.localDomain must be SORA" }
        require(manifest.counterpartyDomain == DOMAIN_TON) { "manifest.counterpartyDomain must be TON" }
        require(manifest.securityModel == "RecursiveZk") { "securityModel is unsupported" }
        require(manifest.anchorGovernance == "CryptographicProof") { "anchorGovernance is unsupported" }
        require(manifest.verifierTarget == "TonContract") { "verifierTarget is unsupported" }
        require(manifest.verifierBackendFamily == "TonContract") { "verifierBackendFamily is unsupported" }
        require(manifest.proofFamily == STARK_FRI_PROOF_FAMILY_V1) { "proofFamily must be stark-fri-v1" }
        require(manifest.verifierBackendKey == CONTRACT_PROOF_BACKEND_V1) {
            "verifierBackendKey must be ton-contract-v1"
        }
        require(input.publicInputs.targetDomain == DOMAIN_TON) { "publicInputs.targetDomain must be TON" }
        val resolvedBinding = input.destinationBinding ?: manifest.destinationBinding
        require(resolvedBinding != null) { "destinationBinding must be provided" }
        val destinationBinding = normalizeSubmissionDestinationBinding(resolvedBinding, "destinationBinding")
        if (input.destinationBinding != null && manifest.destinationBinding != null) {
            val explicitBinding = normalizeSubmissionDestinationBinding(
                input.destinationBinding,
                "destinationBinding",
            )
            val manifestBinding = normalizeSubmissionDestinationBinding(
                manifest.destinationBinding,
                "manifest.destinationBinding",
            )
            require(explicitBinding == manifestBinding) {
                "destinationBinding must match manifest.destinationBinding"
            }
        }
        if (input.destinationBindingHash != null) {
            val destinationBindingHash = normalizeNonZeroHex32(
                input.destinationBindingHash,
                "destinationBindingHash",
            )
            require(destinationBindingHash == destinationBinding.second) {
                "destinationBindingHash must match destinationBinding.bindingHash"
            }
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, manifest.localDomain)
        writeU32Le(out, manifest.counterpartyDomain)
        out.write(1)
        out.write(1)
        out.write(3)
        out.write(3)
        writeString(out, manifest.proofFamily, "proofFamily")
        writeString(out, manifest.verifierBackendKey, "verifierBackendKey")
        writeString(out, manifest.messageBackend, "messageBackend")
        writeString(out, manifest.registryBackend, "registryBackend")
        writeString(out, manifest.manifestSeed, "manifestSeed")
        writeString(out, destinationBinding.first, "destinationBinding.key")
        out.write(hex32Bytes(destinationBinding.second, "destinationBinding.bindingHash"))
        out.write(nonZeroHex32Bytes(input.statementHash, "statementHash"))
        out.write(canonicalPublicInputsBytes(input.publicInputs))
        return out.toByteArray()
    }

    @JvmStatic
    fun buildMessageBodyBoc(input: TonSccpMessageBodyInput): ByteArray {
        val proofResult = requireWrappedProofResultForSubmission(input.proofResult)
        require(input.publicInputs == proofResult.publicInputs) {
            "publicInputs must match proofResult.publicInputs"
        }
        require(input.proofBytes.contentEquals(proofResult.proofBytes)) {
            "proofBytes must match proofResult.proofBytes"
        }
        require(input.bundleBytes.contentEquals(proofResult.bundleBytes)) {
            "bundleBytes must match proofResult.bundleBytes"
        }
        require(input.statementHash == proofResult.proofContext.statementHash) {
            "statementHash must match proofResult.proofContext.statementHash"
        }
        require(input.destinationBindingHash == proofResult.proofContext.destinationBindingHash) {
            "destinationBindingHash must match proofResult.proofContext.destinationBindingHash"
        }
        require(input.publicInputs.targetDomain == DOMAIN_TON) {
            "publicInputs.targetDomain must be TON"
        }
        val publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs)
        val statementHash = nonZeroHex32Bytes(input.statementHash, "statementHash")
        val destinationBindingHash =
            nonZeroHex32Bytes(input.destinationBindingHash, "destinationBindingHash")
        val proofBytes = input.proofBytes
        val bundleBytes = requireNativeRecursivePayloadBytes(input.bundleBytes, "bundleBytes")
        val metadataBytes = input.metadataBytes
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        val rootData = ByteArrayOutputStream()
        writeU32Be(rootData, SUBMIT_OP_V1)
        writeU64Be(rootData, normalizeU64(input.queryId ?: submissionQueryId(input.publicInputs), "queryId"))
        writeU16Be(rootData, MESSAGE_SCHEMA_VERSION_V1)
        rootData.write(statementHash)
        rootData.write(destinationBindingHash)

        val cells = ArrayList<TonCell>()
        cells.add(TonCell(rootData.toByteArray(), mutableListOf()))
        val publicInputsRoot = pushSnakeCells(cells, publicInputsBytes)
        val proofRoot = pushSnakeCells(cells, proofBytes)
        val bundleRoot = pushSnakeCells(cells, bundleBytes)
        val metadataRoot = pushSnakeCells(cells, metadataBytes)
        cells[0].refs.addAll(listOf(publicInputsRoot, proofRoot, bundleRoot, metadataRoot))
        return encodeBocSingleRoot(cells, 0)
    }

    @JvmStatic
    fun buildSubmission(input: TonSccpMessageBodyInput): TonSccpSubmission {
        val messageBodyBoc = buildMessageBodyBoc(input)
        val messageBodyBocHex = "0x" + hexLower(messageBodyBoc)
        return TonSccpSubmission(
            version = 1,
            envelopeEncoding = MESSAGE_BODY_BOC_V1,
            submissionKind = "internal_message",
            verifierEntrypoint = "op::submit_sccp_message_proof",
            messageBodyBoc = messageBodyBoc,
            messageBodyBocHex = messageBodyBocHex,
            arguments = listOf(
                TonSccpSubmissionArgument("message_body_boc", "ton_boc", messageBodyBocHex),
            ),
            envelopeBytes = messageBodyBoc,
            envelopeHex = messageBodyBocHex,
        )
    }

    @JvmStatic
    fun buildProofRequest(input: TonSccpProofRequestInput): TonSccpProofRequest {
        require(input.sourceDomain == DOMAIN_TON) {
            "TON SCCP proof request sourceDomain must be TON"
        }
        require(input.backend == CONTRACT_PROOF_BACKEND_V1) {
            "TON SCCP proof request backend must be ton-contract-v1"
        }
        require(input.publicInputs.targetDomain == DOMAIN_TON) {
            "publicInputs.targetDomain must be TON"
        }
        val bundleBytes = requireNativeRecursivePayloadBytes(input.bundleBytes, "bundleBytes")
        val sourceProofBytes = requireOptionalSourceProofBytes(input.sourceProofBytes, "sourceProofBytes")
        requireSccpProofRequestBundleMatchesPublicInputs(
            publicInputs = input.publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
        )
        val publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs)
        val proofContext = normalizeProofContext(input.statementHash, input.destinationBindingHash)
        val sourceStateVerifierId = normalizeNonEmpty(input.sourceStateVerifierId, "sourceStateVerifierId")
        require(sourceStateVerifierId == MAINNET_SHARD_STATE_VERIFIER_ID_V1) {
            "sourceStateVerifierId must match TON shard-state verifier profile"
        }
        val sourceStateVerifierHashBytes =
            nonZeroHex32Bytes(input.sourceStateVerifierHash, "sourceStateVerifierHash")
        require(!sourceStateVerifierHashBytes.contentEquals(TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
            "sourceStateVerifierHash must not be the TON template verifier hash"
        }
        val sourceStateVerifierHash = "0x" + hexLower(sourceStateVerifierHashBytes)
        val deploymentBinding = SccpSolana.normalizeSourceAdapterDeploymentBinding(
            sourceDomain = input.sourceDomain,
            targetDomain = SccpSolana.DOMAIN_SORA,
            sourceAdapterDeploymentHash = input.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash = input.sourceAdapterDeploymentReceiptHash,
        )
        require(deploymentBinding.sourceAdapterDeploymentHash != SccpSolana.ZERO_HASH_V1) {
            "TON SCCP proof request requires non-zero source adapter deployment binding"
        }
        val deploymentBindingHash = SccpSolana.sourceAdapterDeploymentBindingHash(deploymentBinding)
        val preimage = ByteArrayOutputStream()
        preimage.write(publicInputsBytes)
        writeU32Le(preimage, bundleBytes.size)
        preimage.write(bundleBytes)
        writeU32Le(preimage, sourceProofBytes.size)
        preimage.write(sourceProofBytes)
        writeString(preimage, sourceStateVerifierId, "sourceStateVerifierId")
        preimage.write(sourceStateVerifierHashBytes)
        preimage.write(hex32Bytes(proofContext.statementHash, "statementHash"))
        preimage.write(hex32Bytes(proofContext.destinationBindingHash, "destinationBindingHash"))
        preimage.write(hex32Bytes(deploymentBindingHash, "sourceAdapterDeploymentBindingHash"))
        return TonSccpProofRequest(
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
            sourceStateVerifierId = sourceStateVerifierId,
            sourceStateVerifierHash = sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash = deploymentBindingHash,
            sourceAdapterDeploymentBinding = deploymentBinding,
            requestHash = hashHex("sccp:ton:proof-request:v1", preimage.toByteArray()),
        )
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: TonSccpProofRequest): TonSccpProofResult {
        require(request.backend == CONTRACT_PROOF_BACKEND_V1) {
            "TON SCCP proof request backend must be ton-contract-v1"
        }
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        requireProductionProofRequest(request)
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(request.requestHash, "requestHash"))
        envelopePayload.write(
            hex32Bytes(
                request.sourceAdapterDeploymentBindingHash,
                "sourceAdapterDeploymentBindingHash",
            ),
        )
        envelopePayload.write(proofBytes)
        return TonSccpProofResult(
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
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash = request.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding = request.sourceAdapterDeploymentBinding,
            requestHash = request.requestHash,
            envelopeHash = hashHex("sccp:ton:proof-envelope:v1", envelopePayload.toByteArray()),
        )
    }

    internal fun requireWrappedProofResultForSubmission(
        proofResult: TonSccpProofResult,
    ): TonSccpProofResult {
        require(proofResult.backend == CONTRACT_PROOF_BACKEND_V1) {
            "proofResult.backend must be ton-contract-v1"
        }
        val expectedProofContext = normalizeProofContext(
            proofResult.statementHash,
            proofResult.destinationBindingHash,
        )
        require(proofResult.proofContext == expectedProofContext) {
            "proofResult.proofContext must match statementHash and destinationBindingHash"
        }
        require(proofResult.publicInputs.targetDomain == DOMAIN_TON) {
            "proofResult.publicInputs.targetDomain must be TON"
        }
        val proofBytes = proofResult.proofBytes
        require(proofBytes.isNotEmpty()) { "proofResult.proofBytes must not be empty" }
        require(proofBytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofResult.proofBytes must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofResult.proofBytes must not be all zero" }
        require(proofResult.proofBase64 == Base64.getEncoder().encodeToString(proofBytes)) {
            "proofResult.proofBase64 must match proofResult.proofBytes"
        }
        val sourceStateVerifierId = normalizeNonEmpty(
            proofResult.sourceStateVerifierId,
            "proofResult.sourceStateVerifierId",
        )
        require(sourceStateVerifierId == MAINNET_SHARD_STATE_VERIFIER_ID_V1) {
            "proofResult.sourceStateVerifierId must match TON shard-state verifier profile"
        }
        val sourceStateVerifierHashBytes = nonZeroHex32Bytes(
            proofResult.sourceStateVerifierHash,
            "proofResult.sourceStateVerifierHash",
        )
        require(!sourceStateVerifierHashBytes.contentEquals(TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
            "proofResult.sourceStateVerifierHash must not be the TON template verifier hash"
        }
        val requestHash = normalizeHex32(proofResult.requestHash, "proofResult.requestHash")
        require(requestHash != SccpSolana.ZERO_HASH_V1) {
            "proofResult.requestHash must be non-zero"
        }
        val sourceAdapterDeploymentBindingHash = normalizeHex32(
            proofResult.sourceAdapterDeploymentBindingHash,
            "proofResult.sourceAdapterDeploymentBindingHash",
        )
        require(sourceAdapterDeploymentBindingHash != SccpSolana.ZERO_HASH_V1) {
            "proofResult.sourceAdapterDeploymentBindingHash must be non-zero"
        }
        val deploymentBinding = SccpSolana.normalizeSourceAdapterDeploymentBinding(
            sourceDomain = proofResult.sourceAdapterDeploymentBinding.sourceDomain,
            targetDomain = proofResult.sourceAdapterDeploymentBinding.targetDomain,
            sourceAdapterDeploymentHash =
                proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash =
                proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
        )
        require(deploymentBinding.sourceDomain == DOMAIN_TON) {
            "proofResult.sourceAdapterDeploymentBinding.sourceDomain must be TON"
        }
        require(deploymentBinding.targetDomain == SccpSolana.DOMAIN_SORA) {
            "proofResult.sourceAdapterDeploymentBinding.targetDomain must be SORA"
        }
        require(deploymentBinding.sourceAdapterDeploymentHash != SccpSolana.ZERO_HASH_V1) {
            "proofResult.sourceAdapterDeploymentBinding must be non-zero"
        }
        require(
            SccpSolana.sourceAdapterDeploymentBindingHash(deploymentBinding) ==
                sourceAdapterDeploymentBindingHash,
        ) {
            "proofResult.sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding"
        }
        val envelopeHash = normalizeHex32(proofResult.envelopeHash, "proofResult.envelopeHash")
        require(envelopeHash != SccpSolana.ZERO_HASH_V1) {
            "proofResult.envelopeHash must be non-zero"
        }
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(requestHash, "proofResult.requestHash"))
        envelopePayload.write(
            hex32Bytes(
                sourceAdapterDeploymentBindingHash,
                "proofResult.sourceAdapterDeploymentBindingHash",
            ),
        )
        envelopePayload.write(proofBytes)
        require(envelopeHash == hashHex("sccp:ton:proof-envelope:v1", envelopePayload.toByteArray())) {
            "proofResult.envelopeHash must match wrapped proof bytes"
        }
        val sourceProofBytes = proofResult.sourceProofBytes
        requireOptionalSourceProofBytes(sourceProofBytes, "proofResult.sourceProofBytes")
        requireNativeRecursivePayloadBytes(proofResult.bundleBytes, "proofResult.bundleBytes")
        val expectedRequest = buildProofRequest(
            TonSccpProofRequestInput(
                publicInputs = proofResult.publicInputs,
                bundleBytes = proofResult.bundleBytes,
                sourceProofBytes = sourceProofBytes,
                statementHash = proofResult.statementHash,
                destinationBindingHash = proofResult.destinationBindingHash,
                sourceStateVerifierId = sourceStateVerifierId,
                sourceStateVerifierHash = proofResult.sourceStateVerifierHash,
                sourceAdapterDeploymentHash = deploymentBinding.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash = deploymentBinding.sourceAdapterDeploymentReceiptHash,
                backend = proofResult.backend,
                sourceDomain = DOMAIN_TON,
            ),
        )
        require(expectedRequest.requestHash == requestHash) {
            "proofResult.requestHash must match bundleBytes and sourceProofBytes"
        }
        return proofResult
    }

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
        require(copy.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "$label must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(copy.isEmpty() || copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        return copy
    }

    private fun requireSccpProofRequestBundleMatchesPublicInputs(
        publicInputs: TonSccpPublicInputsInput,
        bundleBytes: ByteArray,
        sourceProofBytes: ByteArray,
    ): SccpBundleSummary {
        val summary = decodeCanonicalSccpMessageProofBundleSummary(bundleBytes, "bundleBytes")
        val messageId = normalizeHex32(publicInputs.messageId, "publicInputs.messageId")
        val payloadHash = normalizeHex32(publicInputs.payloadHash, "publicInputs.payloadHash")
        val commitmentRoot = normalizeHex32(publicInputs.commitmentRoot, "publicInputs.commitmentRoot")
        require(
            summary.targetDomain == publicInputs.targetDomain &&
                summary.messageId == messageId &&
                summary.payloadHash == payloadHash &&
                summary.commitmentRoot == commitmentRoot,
        ) {
            "bundleBytes must match publicInputs"
        }
        require(summary.sourceDomain == SccpSolana.DOMAIN_SORA || sourceProofBytes.isNotEmpty()) {
            "sourceProofBytes required for non-SORA source bundle"
        }
        return summary
    }

    private fun decodeCanonicalSccpMessageProofBundleSummary(
        bundleBytes: ByteArray,
        label: String,
    ): SccpBundleSummary {
        var offset = 0
        val version = readU8At(bundleBytes, offset, "$label.version")
        offset += 1
        require(version == 1) { "$label.version must be 1" }
        require(offset + 32 <= bundleBytes.size) { "$label.commitment_root is too short" }
        val commitmentRoot = "0x" + hexLower(bundleBytes.copyOfRange(offset, offset + 32))
        offset += 32
        val commitmentVec = readCanonicalSccpVec(bundleBytes, offset, "$label.commitment")
        offset = commitmentVec.nextOffset
        val merkleProofVec = readCanonicalSccpVec(bundleBytes, offset, "$label.merkle_proof")
        offset = merkleProofVec.nextOffset
        val payloadVec = readCanonicalSccpVec(bundleBytes, offset, "$label.payload")
        offset = payloadVec.nextOffset
        val finalityProofVec = readCanonicalSccpVec(bundleBytes, offset, "$label.finality_proof")
        offset = finalityProofVec.nextOffset
        requireExactPayloadEnd(offset, bundleBytes, label)

        val payload = decodeCanonicalSccpBundlePayloadSummary(payloadVec.bytes, "$label.payload")
        val expectedCommitmentBytes = canonicalSccpCommitmentBytes(
            payload.kind,
            payload.targetDomain,
            payload.messageId,
            payload.payloadHash,
        )
        require(commitmentVec.bytes.contentEquals(expectedCommitmentBytes)) {
            "$label.commitment must match payload"
        }
        val commitment = decodeCanonicalSccpBundleCommitmentSummary(commitmentVec.bytes, label)
        require(commitment.kindCode == sccpMessageKindCode(payload.kind)) {
            "$label.commitment kind must match payload"
        }
        val expectedRoot = merkleRootFromCanonicalCommitmentBytes(
            commitmentVec.bytes,
            merkleProofVec.bytes,
            "$label.merkle_proof",
        )
        require(commitmentRoot == expectedRoot) {
            "$label.commitment_root must match merkle proof"
        }
        return SccpBundleSummary(
            sourceDomain = payload.sourceDomain,
            targetDomain = commitment.targetDomain,
            messageId = commitment.messageId,
            payloadHash = commitment.payloadHash,
            commitmentRoot = commitmentRoot,
        )
    }

    private fun decodeCanonicalSccpBundlePayloadSummary(
        payloadBytes: ByteArray,
        label: String,
    ): SccpPayloadSummary {
        require(payloadBytes.size >= 2) { "$label is too short" }
        val discriminant = readU8At(payloadBytes, 0, "$label.kind")
        val body = payloadBytes.copyOfRange(1, payloadBytes.size)
        val version = readU8At(body, 0, "$label.version")
        require(version == 1) { "$label.version must be 1" }
        val cursor = Cursor(1)

        fun readDomain(field: String): Int {
            val domain = readU32LeAt(body, cursor.offset, "$label.$field")
            cursor.offset += 4
            requireSupportedSccpBundleDomain(domain, "$label.$field")
            return domain
        }

        fun readU64(field: String): BigInteger {
            val value = readU64LeAt(body, cursor.offset, "$label.$field")
            cursor.offset += 8
            return value
        }

        fun readCodec(field: String): Int {
            val codec = normalizeSccpCodecId(readU8At(body, cursor.offset, "$label.$field"), "$label.$field")
            cursor.offset += 1
            return codec
        }

        fun readCodecValue(codec: Int, field: String): ByteArray {
            val value = readCanonicalSccpVec(body, cursor.offset, "$label.$field")
            cursor.offset = value.nextOffset
            validateCanonicalSccpCodecBytes(codec, value.bytes, "$label.$field")
            return value.bytes
        }

        fun messageId(prefix: String): String = "0x" + hexLower(prefixedKeccakBytes(prefix, body))

        fun summary(kind: String, sourceDomain: Int, targetDomain: Int, prefix: String): SccpPayloadSummary =
            SccpPayloadSummary(
                kind = kind,
                sourceDomain = sourceDomain,
                targetDomain = targetDomain,
                messageId = messageId(prefix),
                payloadHash = "0x" + hexLower(prefixedHashBytes(SCCP_PAYLOAD_HASH_PREFIX_V1, payloadBytes)),
            )

        when (discriminant) {
            0 -> {
                val targetDomain = readDomain("target_domain")
                val sourceDomain = readDomain("home_domain")
                readU64("nonce")
                val assetIdCodec = readCodec("asset_id_codec")
                readCodecValue(assetIdCodec, "asset_id")
                readU8At(body, cursor.offset, "$label.decimals")
                cursor.offset += 1
                requireExactPayloadEnd(cursor.offset, body, label)
                return summary("AssetRegister", sourceDomain, targetDomain, SCCP_MSG_PREFIX_ASSET_REGISTER_V1)
            }

            1 -> {
                val sourceDomain = readDomain("source_domain")
                val targetDomain = readDomain("target_domain")
                require(sourceDomain != targetDomain) { "$label.target_domain must differ from source_domain" }
                readU64("nonce")
                val assetIdCodec = readCodec("asset_id_codec")
                readCodecValue(assetIdCodec, "asset_id")
                val routeIdCodec = readCodec("route_id_codec")
                readCodecValue(routeIdCodec, "route_id")
                requireExactPayloadEnd(cursor.offset, body, label)
                return summary("RouteActivate", sourceDomain, targetDomain, SCCP_MSG_PREFIX_ROUTE_ACTIVATE_V1)
            }

            2 -> {
                val sourceDomain = readDomain("source_domain")
                val targetDomain = readDomain("dest_domain")
                require(sourceDomain != targetDomain) { "$label.dest_domain must differ from source_domain" }
                readU64("nonce")
                readDomain("asset_home_domain")
                val assetIdCodec = readCodec("asset_id_codec")
                readCodecValue(assetIdCodec, "asset_id")
                val amount = readU128LeAt(body, cursor.offset, "$label.amount")
                cursor.offset += 16
                require(amount > BigInteger.ZERO) { "$label.amount must be greater than zero" }
                val senderCodec = readCodec("sender_codec")
                require(senderCodec == sccpCounterpartyAccountCodec(sourceDomain)) {
                    "$label.sender_codec must match source_domain"
                }
                readCodecValue(senderCodec, "sender")
                val recipientCodec = readCodec("recipient_codec")
                require(recipientCodec == sccpCounterpartyAccountCodec(targetDomain)) {
                    "$label.recipient_codec must match dest_domain"
                }
                readCodecValue(recipientCodec, "recipient")
                val routeIdCodec = readCodec("route_id_codec")
                readCodecValue(routeIdCodec, "route_id")
                requireExactPayloadEnd(cursor.offset, body, label)
                return summary("Transfer", sourceDomain, targetDomain, SCCP_MSG_PREFIX_TRANSFER_V1)
            }

            3 -> {
                val targetDomain = readDomain("target_domain")
                readU64("nonce")
                val assetId = readFixed(body, cursor, 32, "$label.sora_asset_id")
                require(assetId.any { it.toInt() != 0 }) { "$label.sora_asset_id must be non-zero" }
                readU8At(body, cursor.offset, "$label.decimals")
                cursor.offset += 1
                val name = readFixed(body, cursor, 32, "$label.name")
                require(fixedAsciiFieldIsNonEmpty(name)) { "$label.name must be non-empty" }
                val symbol = readFixed(body, cursor, 32, "$label.symbol")
                require(fixedAsciiFieldIsNonEmpty(symbol)) { "$label.symbol must be non-empty" }
                requireExactPayloadEnd(cursor.offset, body, label)
                return summary("TokenAdd", SccpSolana.DOMAIN_SORA, targetDomain, SCCP_MSG_PREFIX_TOKEN_ADD_V1)
            }

            4, 5 -> {
                val targetDomain = readDomain("target_domain")
                readU64("nonce")
                val assetId = readFixed(body, cursor, 32, "$label.sora_asset_id")
                require(assetId.any { it.toInt() != 0 }) { "$label.sora_asset_id must be non-zero" }
                requireExactPayloadEnd(cursor.offset, body, label)
                return if (discriminant == 4) {
                    summary("TokenPause", SccpSolana.DOMAIN_SORA, targetDomain, SCCP_MSG_PREFIX_TOKEN_PAUSE_V1)
                } else {
                    summary("TokenResume", SccpSolana.DOMAIN_SORA, targetDomain, SCCP_MSG_PREFIX_TOKEN_RESUME_V1)
                }
            }

            else -> throw IllegalArgumentException("$label contains unsupported SCCP payload kind")
        }
    }

    private fun decodeCanonicalSccpBundleCommitmentSummary(
        commitmentBytes: ByteArray,
        label: String,
    ): SccpCommitmentSummary {
        require(commitmentBytes.size == 70) { "$label.commitment must be 70 bytes" }
        val version = readU8At(commitmentBytes, 0, "$label.commitment.version")
        require(version == 1) { "$label.commitment.version must be 1" }
        return SccpCommitmentSummary(
            kindCode = readU8At(commitmentBytes, 1, "$label.commitment.kind"),
            targetDomain = readU32LeAt(commitmentBytes, 2, "$label.commitment.target_domain"),
            messageId = "0x" + hexLower(commitmentBytes.copyOfRange(6, 38)),
            payloadHash = "0x" + hexLower(commitmentBytes.copyOfRange(38, 70)),
        )
    }

    private fun merkleRootFromCanonicalCommitmentBytes(
        commitmentBytes: ByteArray,
        merkleProofBytes: ByteArray,
        label: String,
    ): String {
        var offset = 0
        val stepCount = readU32LeAt(merkleProofBytes, offset, "$label.steps")
        offset += 4
        var current = prefixedHashBytes(SCCP_HUB_LEAF_PREFIX_V1, commitmentBytes)
        for (index in 0 until stepCount) {
            require(offset + 33 <= merkleProofBytes.size) { "$label.steps[$index] is too short" }
            val sibling = merkleProofBytes.copyOfRange(offset, offset + 32)
            offset += 32
            val siblingIsLeft = readU8At(merkleProofBytes, offset, "$label.steps[$index].sibling_is_left")
            offset += 1
            require(siblingIsLeft == 0 || siblingIsLeft == 1) {
                "$label.steps[$index].sibling_is_left must be 0 or 1"
            }
            current = prefixedHashBytes(
                SCCP_HUB_NODE_PREFIX_V1,
                if (siblingIsLeft == 1) sibling + current else current + sibling,
            )
        }
        requireExactPayloadEnd(offset, merkleProofBytes, label)
        return "0x" + hexLower(current)
    }

    private fun canonicalSccpCommitmentBytes(
        kind: String,
        targetDomain: Int,
        messageId: String,
        payloadHash: String,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(sccpMessageKindCode(kind))
        writeU32Le(out, targetDomain)
        out.write(hex32Bytes(messageId, "commitment.messageId"))
        out.write(hex32Bytes(payloadHash, "commitment.payloadHash"))
        return out.toByteArray()
    }

    private fun sccpMessageKindCode(kind: String): Int =
        when (kind) {
            "Burn" -> 0
            "TokenAdd" -> 1
            "TokenPause" -> 2
            "TokenResume" -> 3
            "AssetRegister" -> 4
            "RouteActivate" -> 5
            "Transfer" -> 6
            else -> throw IllegalArgumentException("SCCP message kind is unsupported")
        }

    private fun requireSupportedSccpBundleDomain(domain: Int, label: String) {
        require(
            domain == SccpSolana.DOMAIN_SORA ||
                domain == SccpSourceProofs.DOMAIN_ETH ||
                domain == SccpSourceProofs.DOMAIN_BSC ||
                domain == SccpSolana.DOMAIN_SOLANA ||
                domain == DOMAIN_TON ||
                domain == SccpTron.DOMAIN_TRON,
        ) {
            "$label must be a supported SCCP domain"
        }
    }

    private fun normalizeSccpCodecId(value: Int, label: String): Int {
        require(
            value == CODEC_TEXT_UTF8 ||
                value == CODEC_EVM_HEX ||
                value == CODEC_SOLANA_BASE58 ||
                value == CODEC_TON_RAW ||
                value == CODEC_TRON_BASE58CHECK ||
                value == CODEC_SORA_ASSET_ID,
        ) {
            "$label codec is unsupported"
        }
        return value
    }

    private fun sccpCounterpartyAccountCodec(domain: Int): Int =
        when (domain) {
            SccpSolana.DOMAIN_SORA -> CODEC_TEXT_UTF8
            SccpSourceProofs.DOMAIN_ETH, SccpSourceProofs.DOMAIN_BSC -> CODEC_EVM_HEX
            SccpSolana.DOMAIN_SOLANA -> CODEC_SOLANA_BASE58
            DOMAIN_TON -> CODEC_TON_RAW
            SccpTron.DOMAIN_TRON -> CODEC_TRON_BASE58CHECK
            else -> throw IllegalArgumentException("SCCP domain must be supported")
        }

    private fun validateCanonicalSccpCodecBytes(codec: Int, raw: ByteArray, label: String) {
        when (codec) {
            CODEC_TEXT_UTF8 -> {
                require(decodeCanonicalUtf8Bytes(raw, label).isNotEmpty()) { "$label must not be empty" }
            }

            CODEC_EVM_HEX -> {
                validateCanonicalEvmHexAddress(decodeCanonicalUtf8Bytes(raw, label), label)
            }

            CODEC_SOLANA_BASE58 -> {
                decodeBase58Fixed(decodeCanonicalUtf8Bytes(raw, label), label, 32)
            }

            CODEC_TON_RAW -> {
                normalizeTonRawAddress(decodeCanonicalUtf8Bytes(raw, label), label)
            }

            CODEC_TRON_BASE58CHECK -> {
                tronBase58CheckPayload(decodeCanonicalUtf8Bytes(raw, label), label)
            }

            CODEC_SORA_ASSET_ID -> {
                require(raw.size == 32) { "$label must be 32 bytes" }
            }

            else -> throw IllegalArgumentException("$label codec is unsupported")
        }
    }

    private fun decodeCanonicalUtf8Bytes(raw: ByteArray, label: String): String {
        val text = raw.toString(Charsets.UTF_8)
        require(text.toByteArray(Charsets.UTF_8).contentEquals(raw)) { "$label must be canonical UTF-8" }
        return text
    }

    private fun validateCanonicalEvmHexAddress(text: String, label: String) {
        require(text.length == 42 && text.startsWith("0x") && text.drop(2).all { it.isDigit() || it in 'a'..'f' || it in 'A'..'F' }) {
            "$label must be a 0x-prefixed 20-byte EVM address"
        }
        val payload = text.drop(2)
        val checksum = keccak256(payload.lowercase().toByteArray(Charsets.UTF_8))
        payload.forEachIndexed { index, char ->
            if (char in '0'..'9') return@forEachIndexed
            val checksumByte = checksum[index / 2].toInt() and 0xff
            val checksumNibble = if (index % 2 == 0) checksumByte ushr 4 else checksumByte and 0x0f
            val shouldBeUppercase = checksumNibble >= 8
            require(if (shouldBeUppercase) char == char.uppercaseChar() else char == char.lowercaseChar()) {
                "$label must be a canonical EIP-55 EVM address"
            }
        }
    }

    private fun readCanonicalSccpVec(raw: ByteArray, offset: Int, label: String): ReadVec {
        val length = readU32LeAt(raw, offset, "$label.length")
        val start = offset + 4
        val end = start.toLong() + length.toLong()
        require(end <= raw.size.toLong()) { "$label is too short" }
        return ReadVec(raw.copyOfRange(start, end.toInt()), end.toInt())
    }

    private fun readFixed(raw: ByteArray, cursor: Cursor, length: Int, label: String): ByteArray {
        val end = cursor.offset + length
        require(end <= raw.size) { "$label is too short" }
        val out = raw.copyOfRange(cursor.offset, end)
        cursor.offset = end
        return out
    }

    private fun readU8At(raw: ByteArray, offset: Int, label: String): Int {
        require(offset + 1 <= raw.size) { "$label is too short" }
        return raw[offset].toInt() and 0xff
    }

    private fun readU32LeAt(raw: ByteArray, offset: Int, label: String): Int {
        require(offset + 4 <= raw.size) { "$label is too short" }
        val value = ((raw[offset].toLong() and 0xffL)) or
            ((raw[offset + 1].toLong() and 0xffL) shl 8) or
            ((raw[offset + 2].toLong() and 0xffL) shl 16) or
            ((raw[offset + 3].toLong() and 0xffL) shl 24)
        require(value <= Int.MAX_VALUE.toLong()) { "$label must fit platform size" }
        return value.toInt()
    }

    private fun readU64LeAt(raw: ByteArray, offset: Int, label: String): BigInteger {
        require(offset + 8 <= raw.size) { "$label is too short" }
        var value = BigInteger.ZERO
        for (index in 7 downTo 0) {
            value = value.shiftLeft(8).or(BigInteger.valueOf(raw[offset + index].toLong() and 0xffL))
        }
        return value
    }

    private fun readU128LeAt(raw: ByteArray, offset: Int, label: String): BigInteger {
        require(offset + 16 <= raw.size) { "$label is too short" }
        var value = BigInteger.ZERO
        for (index in 15 downTo 0) {
            value = value.shiftLeft(8).or(BigInteger.valueOf(raw[offset + index].toLong() and 0xffL))
        }
        return value
    }

    private fun requireExactPayloadEnd(offset: Int, raw: ByteArray, label: String) {
        require(offset == raw.size) { "$label must not contain trailing bytes" }
    }

    private fun fixedAsciiFieldIsNonEmpty(raw: ByteArray): Boolean {
        val end = raw.indexOf(0.toByte())
        val limit = if (end < 0) raw.size else end
        return raw.copyOfRange(0, limit).any { it.toInt() != 0 }
    }

    private fun decodeBase58Fixed(value: String, field: String, byteLength: Int): ByteArray {
        val raw = decodeBase58(value, field)
        require(raw.size == byteLength) { "$field must decode to $byteLength bytes" }
        return raw
    }

    private fun decodeBase58(value: String, field: String): ByteArray {
        require(value.trim() == value && value.isNotEmpty()) { "$field must be canonical base58" }
        var numeric = BigInteger.ZERO
        value.forEach { char ->
            val digit = BASE58_ALPHABET.indexOf(char)
            require(digit >= 0) { "$field must be canonical base58" }
            numeric = numeric.multiply(BigInteger.valueOf(58)).add(BigInteger.valueOf(digit.toLong()))
        }
        var encoded = if (numeric == BigInteger.ZERO) ByteArray(0) else numeric.toByteArray()
        if (encoded.isNotEmpty() && encoded[0].toInt() == 0) encoded = encoded.copyOfRange(1, encoded.size)
        val leadingZeroes = value.takeWhile { it == '1' }.length
        val decoded = ByteArray(leadingZeroes) + encoded
        require(encodeBase58(decoded) == value) { "$field must be canonical base58" }
        return decoded
    }

    private fun encodeBase58(bytes: ByteArray): String {
        var leadingZeroes = 0
        while (leadingZeroes < bytes.size && bytes[leadingZeroes].toInt() == 0) leadingZeroes += 1
        var numeric = BigInteger(1, bytes)
        val reversed = StringBuilder()
        while (numeric > BigInteger.ZERO) {
            val divRem = numeric.divideAndRemainder(BigInteger.valueOf(58))
            numeric = divRem[0]
            reversed.append(BASE58_ALPHABET[divRem[1].toInt()])
        }
        repeat(leadingZeroes) { reversed.append('1') }
        return reversed.reverse().toString()
    }

    private fun tronBase58CheckPayload(value: String, field: String): ByteArray {
        val raw = decodeBase58(value, field)
        require(raw.size == 25) { "$field must be a TRON Base58Check address" }
        val payload = raw.copyOfRange(0, 21)
        require((payload[0].toInt() and 0xff) == 0x41) { "$field must be a TRON mainnet address" }
        val checksum = sha256(sha256(payload)).copyOfRange(0, 4)
        require(raw.copyOfRange(21, 25).contentEquals(checksum)) {
            "$field must have a valid Base58Check checksum"
        }
        return payload
    }

    private fun prefixedKeccakBytes(prefix: String, payload: ByteArray): ByteArray {
        val prefixBytes = prefix.toByteArray(Charsets.UTF_8)
        return keccak256(prefixBytes + payload)
    }

    private fun keccak256(input: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(input, 0, input.size)
        val out = ByteArray(32)
        digest.doFinal(out, 0)
        return out
    }

    private fun requireCanonicalProofRequest(request: TonSccpProofRequest) {
        val expected = buildProofRequest(
            TonSccpProofRequestInput(
                publicInputs = request.publicInputs,
                bundleBytes = request.bundleBytes,
                sourceProofBytes = request.sourceProofBytes,
                statementHash = request.statementHash,
                destinationBindingHash = request.destinationBindingHash,
                sourceStateVerifierId = request.sourceStateVerifierId,
                sourceStateVerifierHash = request.sourceStateVerifierHash,
                sourceAdapterDeploymentHash =
                    request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash =
                    request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
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
                request.sourceStateVerifierId == expected.sourceStateVerifierId &&
                request.sourceStateVerifierHash == expected.sourceStateVerifierHash &&
                request.sourceAdapterDeploymentBindingHash ==
                    expected.sourceAdapterDeploymentBindingHash &&
                request.sourceAdapterDeploymentBinding == expected.sourceAdapterDeploymentBinding &&
                request.requestHash == expected.requestHash,
        ) { "proof request must be canonical" }
    }

    internal fun requireProductionProofRequest(request: TonSccpProofRequest) {
        requireCanonicalProofRequest(request)
        require(request.version == 1) {
            "proof request version must be 1"
        }
        require(request.sourceDomain == DOMAIN_TON) {
            "TON SCCP production proof sourceDomain must be TON"
        }
        require(request.targetDomain == DOMAIN_TON && request.publicInputs.targetDomain == DOMAIN_TON) {
            "TON SCCP production proofs must target TON public inputs"
        }
        require(request.backend == CONTRACT_PROOF_BACKEND_V1) {
            "TON SCCP proof request backend must be ton-contract-v1"
        }
        requireNativeRecursivePayloadBytes(request.bundleBytes, "TON SCCP proof request bundleBytes")
        requireOptionalSourceProofBytes(
            request.sourceProofBytes,
            "TON SCCP proof request sourceProofBytes",
        )
        require(request.sourceStateVerifierId == MAINNET_SHARD_STATE_VERIFIER_ID_V1) {
            "sourceStateVerifierId must match TON shard-state verifier profile"
        }
        val sourceStateVerifierHashBytes =
            nonZeroHex32Bytes(request.sourceStateVerifierHash, "sourceStateVerifierHash")
        require(!sourceStateVerifierHashBytes.contentEquals(TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
            "sourceStateVerifierHash must not be the TON template verifier hash"
        }
        val deploymentBinding = request.sourceAdapterDeploymentBinding
        require(deploymentBinding.sourceDomain == DOMAIN_TON) {
            "sourceAdapterDeploymentBinding.sourceDomain must be TON"
        }
        require(deploymentBinding.targetDomain == SccpSolana.DOMAIN_SORA) {
            "sourceAdapterDeploymentBinding.targetDomain must be SORA"
        }
        require(deploymentBinding.sourceAdapterDeploymentHash != SccpSolana.ZERO_HASH_V1) {
            "sourceAdapterDeploymentBinding must be non-zero"
        }
        require(
            SccpSolana.sourceAdapterDeploymentBindingHash(deploymentBinding) ==
                request.sourceAdapterDeploymentBindingHash,
        ) {
            "sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding"
        }
    }

    internal fun callbackRequestSnapshot(request: TonSccpProofRequest): TonSccpProofRequest =
        request.copy()

    private fun pushSnakeCells(cells: MutableList<TonCell>, bytes: ByteArray): Int {
        val start = cells.size
        if (bytes.isEmpty()) {
            require(cells.size + 1 <= MAX_BOC_CELLS) { "TON BOC contains too many cells" }
            cells.add(TonCell(ByteArray(0), mutableListOf()))
            return start
        }
        val chunkCount = (bytes.size + MAX_CELL_DATA_BYTES - 1) / MAX_CELL_DATA_BYTES
        require(cells.size + chunkCount <= MAX_BOC_CELLS) { "TON BOC contains too many cells" }
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
        require(cells.size <= MAX_BOC_CELLS) { "TON BOC contains too many cells" }
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

    private fun normalizeInclusionBranch(value: List<ByteArray>): List<ByteArray> =
        value.also {
            require(it.size <= MAX_SOURCE_MERKLE_BRANCH_NODES) {
                "inclusionBranch must contain at most $MAX_SOURCE_MERKLE_BRANCH_NODES entries"
            }
        }.mapIndexed { index, sibling ->
            require(sibling.size == 32) { "inclusionBranch[$index] must be 32 bytes" }
            sibling.copyOf()
        }

    private fun normalizeValidatorSet(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): Pair<List<ByteArray>, List<BigInteger>> {
        require(
            validatorPublicKeys.isNotEmpty() &&
                validatorPublicKeys.size <= MAX_VALIDATORS &&
                validatorPublicKeys.size == validatorWeights.size,
        ) {
            "TON validator public keys and weights must be same-length arrays"
        }
        val seen = HashSet<String>()
        val keys = validatorPublicKeys.mapIndexed { index, publicKey ->
            require(publicKey.size == 32) { "validatorPublicKeys[$index] must be 32 bytes" }
            require(publicKey.any { it.toInt() != 0 }) { "validatorPublicKeys[$index] must not be zero" }
            val encoded = hexLower(publicKey)
            require(seen.add(encoded)) { "TON validator public keys must be unique" }
            publicKey.copyOf()
        }
        val weights = validatorWeights.mapIndexed { index, weight ->
            val numeric = normalizeU64(weight, "validatorWeights[$index]")
            require(numeric != BigInteger.ZERO) { "validatorWeights[$index] must not be zero" }
            numeric
        }
        return Pair(keys, weights)
    }

    private fun canonicalValidatorSetBytesFromParts(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<BigInteger>,
    ): ByteArray {
        require(
            validatorPublicKeys.isNotEmpty() &&
                validatorPublicKeys.size <= MAX_VALIDATORS &&
                validatorPublicKeys.size == validatorWeights.size,
        ) {
            "TON validator public keys and weights must be same-length arrays"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, validatorPublicKeys.size)
        for (index in validatorPublicKeys.indices) {
            val publicKey = validatorPublicKeys[index]
            require(publicKey.size == 32) { "validatorPublicKeys[$index] must be 32 bytes" }
            require(publicKey.any { it.toInt() != 0 }) { "validatorPublicKeys[$index] must not be zero" }
            val weight = validatorWeights[index]
            require(weight != BigInteger.ZERO && weight <= MAX_U64) {
                "validatorWeights[$index] must fit u64 and be non-zero"
            }
            out.write(publicKey)
            writeU64Le(out, weight)
        }
        validateValidatorSetPayload(out.toByteArray())
        return out.toByteArray()
    }

    private fun validateValidatorSetPayload(payload: ByteArray) {
        require(payload.size >= 5 && payload[0].toInt() == 1) {
            "validatorSetPayload must use version 1"
        }
        val count =
            (payload[1].toInt() and 0xff) or
                ((payload[2].toInt() and 0xff) shl 8) or
                ((payload[3].toInt() and 0xff) shl 16) or
                ((payload[4].toInt() and 0xff) shl 24)
        require(count > 0 && count <= MAX_VALIDATORS && payload.size == 5 + count * 40) {
            "validatorSetPayload has invalid validator count or length"
        }
        val seen = HashSet<String>()
        var offset = 5
        for (index in 0 until count) {
            val publicKey = payload.copyOfRange(offset, offset + 32)
            offset += 32
            require(publicKey.any { it.toInt() != 0 }) { "validatorPublicKeys[$index] must not be zero" }
            require(seen.add(hexLower(publicKey))) {
                "TON validator public keys must be unique"
            }
            var weight = BigInteger.ZERO
            for (byteIndex in 0 until 8) {
                weight = weight.or(
                    BigInteger.valueOf((payload[offset + byteIndex].toInt() and 0xff).toLong())
                        .shiftLeft(byteIndex * 8),
                )
            }
            offset += 8
            require(weight != BigInteger.ZERO) { "validatorWeights[$index] must not be zero" }
        }
    }

    private fun signerIndicesFromBitmap(bitmap: ByteArray, rosterLength: Int): List<Int> {
        require(bitmap.size == (rosterLength + 7) / 8) {
            "signersBitmap length must match validatorPublicKeys"
        }
        val indices = ArrayList<Int>()
        for (byteIndex in bitmap.indices) {
            val value = bitmap[byteIndex].toInt() and 0xff
            for (bit in 0 until 8) {
                if (((value ushr bit) and 1) == 0) {
                    continue
                }
                val index = byteIndex * 8 + bit
                require(index < rosterLength) { "signersBitmap must not set padding bits" }
                indices.add(index)
            }
        }
        return indices
    }

    private fun writeVector(out: ByteArrayOutputStream, value: ByteArray) {
        writeU32Le(out, value.size)
        out.write(value)
    }

    private fun canonicalValidatorSignatureProofBytes(input: TonValidatorSignatureProofInput): ByteArray {
        val keysAndWeights = normalizeValidatorSet(input.validatorPublicKeys, input.validatorWeights)
        require(input.version == 1) { "TON validator signature proof version must be 1" }
        val totalWeight = normalizeU64(input.totalWeight, "totalWeight")
        val signedWeight = normalizeU64(input.signedWeight, "signedWeight")
        val computedTotalWeight = keysAndWeights.second.fold(BigInteger.ZERO) { sum, weight -> sum + weight }
        require(totalWeight == computedTotalWeight) { "totalWeight must match validatorWeights" }
        val signerIndices = signerIndicesFromBitmap(input.signersBitmap, keysAndWeights.first.size)
        require(signerIndices.isNotEmpty()) { "signersBitmap must select at least one validator" }
        require(input.signatures.size == signerIndices.size) { "signatures length must match signersBitmap" }
        val computedSignedWeight =
            signerIndices.fold(BigInteger.ZERO) { sum, index -> sum + keysAndWeights.second[index] }
        require(signedWeight == computedSignedWeight) { "signedWeight must match signersBitmap" }
        require(signedWeight * BigInteger.valueOf(3L) > totalWeight * BigInteger.valueOf(2L)) {
            "signedWeight must be greater than two thirds of totalWeight"
        }
        val out = ByteArrayOutputStream()
        out.write(input.version)
        writeU64Le(out, totalWeight)
        writeU64Le(out, signedWeight)
        out.write(nonZeroHex32Bytes(input.blockMessageHash, "blockMessageHash"))
        writeU32Le(out, keysAndWeights.first.size)
        keysAndWeights.first.forEach { writeVector(out, it) }
        writeU32Le(out, keysAndWeights.second.size)
        keysAndWeights.second.forEach { writeU64Le(out, it) }
        writeVector(out, input.signersBitmap)
        writeU32Le(out, input.signatures.size)
        input.signatures.forEachIndexed { index, signature ->
            require(signature.size == 64) { "signatures[$index] must be 64 bytes" }
            require(signature.any { it.toInt() != 0 }) { "signatures[$index] must not be all zero" }
            writeVector(out, signature)
        }
        return out.toByteArray()
    }

    private fun boundedBocHash(prefix: String, value: ByteArray, field: String): Pair<ByteArray, String> {
        val raw = value.copyOf()
        require(raw.isNotEmpty()) { "$field must not be empty" }
        require(raw.size <= MAX_BOC_BYTES) { "$field exceeds TON BoC proof byte limit" }
        return Pair(raw, hashHex(prefix, raw))
    }

    private fun normalizeValidatorSetTransitionForSourceState(
        input: TonValidatorSetTransitionProofInput,
    ): NormalizedTonValidatorSetTransitionProof {
        require(input.version == 1) { "TON validator-set transition proof version must be 1" }
        require(input.sourceDomain == DOMAIN_TON) { "sourceDomain must be TON" }
        require(input.masterchainWorkchainId == TON_MASTERCHAIN_WORKCHAIN_ID) {
            "masterchainWorkchainId must be TON masterchain"
        }
        val masterchainShard = normalizeU64(input.masterchainShard, "masterchainShard")
        require(masterchainShard == TON_MASTERCHAIN_SHARD) {
            "masterchainShard must be TON masterchain shard"
        }
        val transitionSignatureHash = normalizeHex32(
            input.transitionSignatureHash,
            "transitionSignatureHash",
        )
        val expectedTransitionSignatureHash = validatorSetTransitionSignatureHash(
            version = input.version,
            sourceDomain = input.sourceDomain,
            fromValidatorSetSeqno = input.fromValidatorSetSeqno,
            toValidatorSetSeqno = input.toValidatorSetSeqno,
            masterchainSeqno = input.masterchainSeqno,
            masterchainWorkchainId = input.masterchainWorkchainId,
            masterchainShard = input.masterchainShard,
            masterchainBlockHash = input.masterchainBlockHash,
            masterchainFileHash = input.masterchainFileHash,
            parentValidatorSetHash = input.parentValidatorSetHash,
            nextValidatorSetHash = input.nextValidatorSetHash,
            nextValidatorSetPayload = input.nextValidatorSetPayload,
            nextValidatorSetPayloadHash = input.nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash = input.nextValidatorSetConfigHash,
            transitionMessageHash = input.transitionMessageHash,
            validatorSignatureProof = input.validatorSignatureProof,
        )
        require(
            hex32Bytes(transitionSignatureHash, "transitionSignatureHash").contentEquals(
                hex32Bytes(expectedTransitionSignatureHash, "transitionSignatureHash"),
            ),
        ) {
            "transitionSignatureHash must match transition signature fields"
        }
        return NormalizedTonValidatorSetTransitionProof(
            version = input.version,
            sourceDomain = input.sourceDomain,
            fromValidatorSetSeqno = normalizeU64(
                input.fromValidatorSetSeqno,
                "fromValidatorSetSeqno",
            ),
            toValidatorSetSeqno = normalizeU64(input.toValidatorSetSeqno, "toValidatorSetSeqno"),
            masterchainSeqno = normalizeU64(input.masterchainSeqno, "masterchainSeqno"),
            masterchainWorkchainId = input.masterchainWorkchainId,
            masterchainShard = masterchainShard,
            masterchainBlockHash = normalizeNonZeroHex32(
                input.masterchainBlockHash,
                "masterchainBlockHash",
            ),
            masterchainFileHash = normalizeNonZeroHex32(
                input.masterchainFileHash,
                "masterchainFileHash",
            ),
            parentValidatorSetHash = normalizeHex32(
                input.parentValidatorSetHash,
                "parentValidatorSetHash",
            ),
            nextValidatorSetHash = normalizeHex32(input.nextValidatorSetHash, "nextValidatorSetHash"),
            nextValidatorSetPayload = input.nextValidatorSetPayload.copyOf(),
            nextValidatorSetPayloadHash = normalizeHex32(
                input.nextValidatorSetPayloadHash,
                "nextValidatorSetPayloadHash",
            ),
            nextValidatorSetConfigHash = normalizeHex32(
                input.nextValidatorSetConfigHash,
                "nextValidatorSetConfigHash",
            ),
            transitionMessageHash = normalizeHex32(
                input.transitionMessageHash,
                "transitionMessageHash",
            ),
            transitionSignatureHash = transitionSignatureHash,
            validatorSignatureProof = input.validatorSignatureProof,
        )
    }

    private fun canonicalValidatorSetTransitionProofBytes(
        transition: NormalizedTonValidatorSetTransitionProof,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(transition.version)
        writeU32Le(out, transition.sourceDomain)
        writeU64Le(out, transition.fromValidatorSetSeqno)
        writeU64Le(out, transition.toValidatorSetSeqno)
        writeU64Le(out, transition.masterchainSeqno)
        writeI32Le(out, transition.masterchainWorkchainId)
        writeU64Le(out, transition.masterchainShard)
        out.write(hex32Bytes(transition.masterchainBlockHash, "masterchainBlockHash"))
        out.write(hex32Bytes(transition.masterchainFileHash, "masterchainFileHash"))
        out.write(hex32Bytes(transition.parentValidatorSetHash, "parentValidatorSetHash"))
        out.write(hex32Bytes(transition.nextValidatorSetHash, "nextValidatorSetHash"))
        writeVector(out, transition.nextValidatorSetPayload)
        out.write(hex32Bytes(transition.nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"))
        out.write(hex32Bytes(transition.nextValidatorSetConfigHash, "nextValidatorSetConfigHash"))
        out.write(hex32Bytes(transition.transitionMessageHash, "transitionMessageHash"))
        out.write(hex32Bytes(transition.transitionSignatureHash, "transitionSignatureHash"))
        out.write(canonicalValidatorSignatureProofBytes(transition.validatorSignatureProof))
        return out.toByteArray()
    }

    private fun validatorSetTransitionChainHash(
        transitions: List<NormalizedTonValidatorSetTransitionProof>,
    ): String {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, transitions.size)
        transitions.forEach { transition ->
            out.write(canonicalValidatorSetTransitionProofBytes(transition))
        }
        return hashHex(VALIDATOR_SET_TRANSITION_CHAIN_PREFIX_V1, out.toByteArray())
    }

    private fun auditRoleProfile(role: TonSccpFullLightClientAuditRole): TonFullLightClientAuditRoleProfile =
        when (role) {
            TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG -> TonFullLightClientAuditRoleProfile(
                name = "masterchain_config",
                code = 1,
                circuitId = MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
                verifierId = SccpSourceProofs.TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1,
                requiredInputNames = listOf(
                    "masterchain_config_root",
                    "masterchain_config_proof_hash",
                    "validator_set_payload_hash",
                    "config_leaf_hash",
                    "config_value_hash",
                    "config_proof_boc_hash",
                ),
            )
            TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION -> TonFullLightClientAuditRoleProfile(
                name = "validator_set_transition",
                code = 2,
                circuitId = VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
                verifierId = SccpSourceProofs.TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1,
                requiredInputNames = listOf(
                    "source_trust_anchor_hash",
                    "validator_set_hash",
                    "validator_set_transition_chain_hash",
                    "masterchain_signature_hash",
                    "validator_set_transition_count",
                ),
            )
            TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY -> TonFullLightClientAuditRoleProfile(
                name = "shard_accounts_dictionary",
                code = 3,
                circuitId = SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
                verifierId = SccpSourceProofs.TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1,
                requiredInputNames = listOf(
                    "shard_state_root",
                    "shard_state_dictionary_root",
                    "transaction_root",
                    "shard_state_proof_boc_hash",
                    "shard_accounts_proof_boc_hash",
                    "shard_state_verification_proof_hash",
                ),
            )
        }

    private fun auditRoleVerifierHash(
        input: TonSccpFullLightClientAuditProofInput,
        role: TonSccpFullLightClientAuditRole,
    ): String =
        when (role) {
            TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG ->
                normalizeNonZeroHex32(input.tonMasterchainConfigVerifierHash, "tonMasterchainConfigVerifierHash")
            TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION ->
                normalizeNonZeroHex32(
                    input.tonValidatorSetTransitionVerifierHash,
                    "tonValidatorSetTransitionVerifierHash",
                )
            TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY ->
                normalizeNonZeroHex32(
                    input.tonShardAccountsDictionaryVerifierHash,
                    "tonShardAccountsDictionaryVerifierHash",
                )
        }

    private fun normalizeFullLightClientAuditInput(
        input: TonSccpFullLightClientAuditProofInput,
        role: TonSccpFullLightClientAuditRole,
    ): NormalizedTonFullLightClientAuditInput {
        val shardState = normalizeShardStateSourceStateInput(input.shardState)
        val sourceVerifierMaterialHash = SccpSourceProofs.sourceVerifierMaterialHash(
            sourceDomain = shardState.sourceDomain,
            sourceTrustAnchorHash = shardState.sourceTrustAnchorHash,
            consensusVerifierHash = shardState.consensusVerifierHash,
            messageInclusionVerifierHash = shardState.messageInclusionVerifierHash,
            finalityPolicyHash = shardState.finalityPolicyHash,
            sourceStateVerifierHash = shardState.sourceStateVerifierHash,
        )
        require(normalizeNonZeroHex32(input.sourceVerifierMaterialHash, "sourceVerifierMaterialHash") ==
            sourceVerifierMaterialHash) {
            "sourceVerifierMaterialHash must match TON shard-state verification context"
        }
        val sourceAdapterDeploymentHash = normalizeNonZeroHex32(
            input.sourceAdapterDeploymentHash,
            "sourceAdapterDeploymentHash",
        )
        val fullLightClientGateHash = normalizeNonZeroHex32(
            input.fullLightClientGateHash,
            "fullLightClientGateHash",
        )
        val auditRoleHashes = listOf(
            auditRoleVerifierHash(input, TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG),
            auditRoleVerifierHash(input, TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION),
            auditRoleVerifierHash(input, TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY),
        )
        requireFullLightClientAuditRoleSeparation(
            auditRoleHashes,
            listOf(
                shardState.sourceTrustAnchorHash,
                shardState.consensusVerifierHash,
                shardState.messageInclusionVerifierHash,
                shardState.finalityPolicyHash,
                shardState.sourceStateVerifierHash,
            ),
        )
        val shardStateProofPublicInputsHash = shardStateProofPublicInputsHash(input.shardState)
        input.shardStateProofPublicInputsHash?.let { supplied ->
            require(normalizeHex32(supplied, "shardStateProofPublicInputsHash") ==
                shardStateProofPublicInputsHash) {
                "shardStateProofPublicInputsHash must match TON shard-state inputs"
            }
        }
        val shardStateVerificationProofHash = shardStateVerificationProofHash(input.shardStateVerificationProof)
        input.shardStateVerificationProofHash?.let { supplied ->
            require(normalizeHex32(supplied, "shardStateVerificationProofHash") ==
                shardStateVerificationProofHash) {
                "shardStateVerificationProofHash must match shardStateVerificationProof"
            }
        }
        require(
            shardState.sourceTrustAnchorHash != shardState.validatorSetHash ||
                shardState.validatorSetTransitionProofs.isEmpty(),
        ) {
            "validatorSetTransitionProofs must be empty when validator set matches source trust anchor"
        }
        require(
            shardState.sourceTrustAnchorHash == shardState.validatorSetHash ||
                shardState.validatorSetTransitionProofs.isNotEmpty(),
        ) {
            "validatorSetTransitionProofs must connect source trust anchor to validatorSetHash"
        }
        val validatorSetPayloadHash = normalizeNonZeroHex32(
            input.validatorSetPayloadHash,
            "validatorSetPayloadHash",
        )
        val configLeafHash = normalizeNonZeroHex32(input.configLeafHash, "configLeafHash")
        val configValueHash = normalizeNonZeroHex32(input.configValueHash, "configValueHash")
        val expectedConfigProofHash = masterchainConfigProofHash(
            sourceDomain = shardState.sourceDomain,
            masterchainSeqno = shardState.masterchainSeqno.toString(),
            masterchainBlockHash = shardState.masterchainBlockHash,
            shardStateRoot = shardState.shardStateRoot,
            configRoot = shardState.masterchainConfigRoot,
            validatorSetHash = shardState.validatorSetHash,
            validatorSetPayloadHash = validatorSetPayloadHash,
            configLeafHash = configLeafHash,
            configLeafIndex = CURRENT_VALIDATOR_SET_CONFIG_PARAM.toString(),
            configValueHash = configValueHash,
            configDictionaryProofBoc = shardState.configDictionaryProofBoc,
            configInclusionBranch = emptyList(),
        )
        require(expectedConfigProofHash == shardState.masterchainConfigProofHash) {
            "masterchainConfigProofHash must match TON config proof fields"
        }
        return NormalizedTonFullLightClientAuditInput(
            role = role,
            shardState = shardState,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = sourceAdapterDeploymentHash,
            fullLightClientGateHash = fullLightClientGateHash,
            verifierHash = auditRoleHashes[auditRoleProfile(role).code - 1],
            shardStateProofPublicInputsHash = shardStateProofPublicInputsHash,
            shardStateVerificationProofHash = shardStateVerificationProofHash,
            validatorSetPayloadHash = validatorSetPayloadHash,
            configLeafHash = configLeafHash,
            configValueHash = configValueHash,
        )
    }

    private fun requireFullLightClientAuditRoleSeparation(
        auditRoleHashes: List<String>,
        existingHashes: List<String>,
    ) {
        val auditBytes = auditRoleHashes.map { hex32Bytes(it, "tonAuditVerifierHash") }
        for (index in auditBytes.indices) {
            for (otherIndex in index + 1 until auditBytes.size) {
                require(!auditBytes[index].contentEquals(auditBytes[otherIndex])) {
                    "TON full-light-client audit verifier hashes must be role-separated"
                }
            }
            require(TEMPLATE_SOURCE_MATERIAL_HASHES.none { auditBytes[index].contentEquals(it) }) {
                "TON full-light-client audit verifier hash must not reuse built-in template material"
            }
            for (existingHash in existingHashes) {
                val existingBytes = hex32Bytes(existingHash, "tonAuditExistingHash")
                require(existingBytes.all { it.toInt() == 0 } || !auditBytes[index].contentEquals(existingBytes)) {
                    "TON full-light-client audit verifier hash must not reuse existing source-adapter material"
                }
            }
        }
    }

    private fun requireFullLightClientAuditRequestHashSeparation(
        value: NormalizedTonFullLightClientAuditInput,
        statementHash: String? = null,
    ) {
        val verifierHash = hex32Bytes(value.verifierHash, "tonAuditVerifierHash")
        val requestHashes = mutableListOf(
            value.shardState.sourceStateVerifierHash,
            value.sourceVerifierMaterialHash,
            value.sourceAdapterDeploymentHash,
            value.fullLightClientGateHash,
            value.shardStateProofPublicInputsHash,
            value.shardStateVerificationProofHash,
            value.shardState.masterchainConfigProofHash,
            value.shardState.masterchainSignatureHash,
            value.shardState.shardProofHash,
            value.shardState.transitionChainHash,
        )
        requestHashes.addAll(auditRoleColumns(value))
        if (statementHash != null) {
            requestHashes.add(statementHash)
        }
        for (requestHash in requestHashes) {
            val requestBytes = hex32Bytes(requestHash, "tonAuditRequestHash")
            require(requestBytes.all { it.toInt() == 0 } || !verifierHash.contentEquals(requestBytes)) {
                "TON full-light-client audit verifier hash must not reuse request-bound hashes"
            }
        }
    }

    private fun auditRoleColumns(value: NormalizedTonFullLightClientAuditInput): List<String> {
        val shardState = value.shardState
        return when (value.role) {
            TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG -> listOf(
                shardState.masterchainConfigRoot,
                shardState.masterchainConfigProofHash,
                value.validatorSetPayloadHash,
                value.configLeafHash,
                value.configValueHash,
                shardState.configProofBocHash,
            )
            TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION -> listOf(
                shardState.sourceTrustAnchorHash,
                shardState.validatorSetHash,
                shardState.transitionChainHash,
                shardState.masterchainSignatureHash,
                "0x" + hexLower(sccpWordU64Le(BigInteger.valueOf(shardState.validatorSetTransitionProofs.size.toLong()))),
            )
            TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY -> listOf(
                shardState.shardStateRoot,
                shardState.shardStateDictionaryRoot,
                shardState.transactionRoot,
                shardState.shardStateProofBocHash,
                shardState.shardAccountsProofBocHash,
                value.shardStateVerificationProofHash,
            )
        }
    }

    private fun fullLightClientAuditFastpqPublicInputs(
        value: NormalizedTonFullLightClientAuditInput,
        statementHash: String,
    ): TonSccpFullLightClientAuditFastpqPublicInputs {
        val dsidPreimage = byteArrayOf(auditRoleProfile(value.role).code.toByte()) +
            hex32Bytes(statementHash, "auditStatementHash")
        val dsidHash = prefixedHashBytes(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1, dsidPreimage)
        val shardState = value.shardState
        val roots = when (value.role) {
            TonSccpFullLightClientAuditRole.MASTERCHAIN_CONFIG ->
                Triple(shardState.masterchainConfigRoot, shardState.validatorSetHash, shardState.masterchainConfigProofHash)
            TonSccpFullLightClientAuditRole.VALIDATOR_SET_TRANSITION ->
                Triple(shardState.sourceTrustAnchorHash, shardState.validatorSetHash, shardState.transitionChainHash)
            TonSccpFullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY ->
                Triple(shardState.shardStateRoot, shardState.transactionRoot, shardState.shardStateDictionaryRoot)
        }
        return TonSccpFullLightClientAuditFastpqPublicInputs(
            dsid = "0x" + hexLower(dsidHash.copyOfRange(0, 16)),
            slot = shardState.masterchainSeqno.toString(),
            oldRoot = roots.first,
            newRoot = roots.second,
            permRoot = roots.third,
            txSetHash = statementHash,
        )
    }

    private fun canonicalFullLightClientAuditContextBytes(
        value: NormalizedTonFullLightClientAuditInput,
        statementHash: String,
    ): ByteArray {
        requireFullLightClientAuditRequestHashSeparation(value, statementHash)
        val profile = auditRoleProfile(value.role)
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(profile.code)
        writeString(out, profile.circuitId, "circuitId")
        writeString(out, FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1, "parameterSet")
        writeString(out, profile.verifierId, "verifierId")
        out.write(hex32Bytes(value.verifierHash, "verifierHash"))
        out.write(hex32Bytes(value.sourceVerifierMaterialHash, "sourceVerifierMaterialHash"))
        out.write(hex32Bytes(value.sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"))
        out.write(hex32Bytes(value.fullLightClientGateHash, "fullLightClientGateHash"))
        out.write(hex32Bytes(value.shardStateProofPublicInputsHash, "shardStateProofPublicInputsHash"))
        out.write(hex32Bytes(statementHash, "auditStatementHash"))
        return out.toByteArray()
    }

    private fun fullLightClientAuditFastpqKey(
        prefix: String,
        profile: TonFullLightClientAuditRoleProfile,
    ): ByteArray =
        prefix.toByteArray(Charsets.UTF_8) +
            byteArrayOf(0) +
            profile.circuitId.toByteArray(Charsets.UTF_8)

    private fun normalizeShardStateSourceStateInput(
        input: TonShardStateProofRequestInput,
    ): NormalizedTonShardStateSourceStateInput {
        require(input.sourceDomain == DOMAIN_TON) { "sourceDomain must be TON" }
        require(input.masterchainWorkchainId == TON_MASTERCHAIN_WORKCHAIN_ID) {
            "masterchainWorkchainId must be TON masterchain"
        }
        val masterchainShard = normalizeU64(input.masterchainShard, "masterchainShard")
        require(masterchainShard == TON_MASTERCHAIN_SHARD) {
            "masterchainShard must be TON masterchain shard"
        }
        require(input.shardWorkchainId == TON_BASECHAIN_WORKCHAIN_ID) {
            "shardWorkchainId must be TON basechain"
        }
        val masterchainSeqno = normalizeU64(input.masterchainSeqno, "masterchainSeqno")
        val shardShard = normalizeU64(input.shardShard, "shardShard")
        require(shardShard != BigInteger.ZERO) { "shardShard must be non-zero" }
        val shardSeqno = normalizeU64(input.shardSeqno, "shardSeqno")
        require(shardSeqno != BigInteger.ZERO) { "shardSeqno must be non-zero" }
        val transactionLt = normalizeU64(input.transactionLt, "transactionLt")
        require(transactionLt != BigInteger.ZERO) { "transactionLt must be non-zero" }
        require(input.shardStateDictionaryKeyBitLen == SHARD_ACCOUNT_KEY_BITS) {
            "TON ShardAccounts key bit length must be 256"
        }
        val dictionaryKey = input.shardStateDictionaryKey.copyOf()
        require(hashmapKeyIsCanonical(dictionaryKey, input.shardStateDictionaryKeyBitLen)) {
            "shardStateDictionaryKey length is invalid"
        }
        val (shardStateProofBoc, shardStateProofBocHash) = boundedBocHash(
            SHARD_STATE_PROOF_BOC_PREFIX_V1,
            input.shardStateProofBoc,
            "shardStateProofBoc",
        )
        val (shardStateDictionaryProofBoc, shardAccountsProofBocHash) = boundedBocHash(
            SHARD_ACCOUNTS_PROOF_BOC_PREFIX_V1,
            input.shardStateDictionaryProofBoc,
            "shardStateDictionaryProofBoc",
        )
        val (configDictionaryProofBoc, configProofBocHash) = boundedBocHash(
            CONFIG_PROOF_BOC_PREFIX_V1,
            input.configDictionaryProofBoc,
            "configDictionaryProofBoc",
        )
        val shardStateRoot = normalizeNonZeroHex32(input.shardStateRoot, "shardStateRoot")
        val transactionRoot = normalizeNonZeroHex32(input.transactionRoot, "transactionRoot")
        val dictionaryRoot = normalizeNonZeroHex32(
            input.shardStateDictionaryRoot,
            "shardStateDictionaryRoot",
        )
        require(shardStateProofRootHash(shardStateProofBoc) == shardStateRoot) {
            "shardStateProofBoc root must match shardStateRoot"
        }
        val opening = shardStateAccountsOpening(shardStateProofBoc)
        require(opening.accountsRootHash == dictionaryRoot) {
            "shardStateProofBoc accounts root must match shardStateDictionaryRoot"
        }
        require(opening.globalId == TON_MAINNET_GLOBAL_ID) {
            "shardStateProofBoc ShardStateUnsplit global_id must be TON mainnet"
        }
        require(opening.workchainId == TON_BASECHAIN_WORKCHAIN_ID) {
            "shardStateProofBoc ShardIdent workchain_id must be TON basechain"
        }
        require(opening.workchainId == input.shardWorkchainId) {
            "shardStateProofBoc ShardIdent workchain_id must match shardWorkchainId"
        }
        require(BigInteger.valueOf(Integer.toUnsignedLong(opening.seqNo)) == shardSeqno) {
            "shardStateProofBoc ShardStateUnsplit seq_no must match shardSeqno"
        }
        require(opening.shardId == shardShard) {
            "shardStateProofBoc ShardIdent shard must match shardShard"
        }
        require(opening.seqNo != 0 && opening.genUtime != 0 && opening.genLt != 0L) {
            "shardStateProofBoc ShardStateUnsplit metadata must be non-zero"
        }
        require(BigInteger.valueOf(Integer.toUnsignedLong(opening.minRefMcSeqno)) <= masterchainSeqno) {
            "shardStateProofBoc ShardStateUnsplit min_ref_mc_seqno exceeds masterchainSeqno"
        }
        require(
            shardStateAccountKeyMatchesShardPrefix(
                dictionaryKey,
                input.shardStateDictionaryKeyBitLen,
                opening,
            ),
        ) {
            "shardStateDictionaryKey must match shardStateProofBoc ShardIdent prefix"
        }
        require(hashmapEProofRootHash(shardStateDictionaryProofBoc) == dictionaryRoot) {
            "shardStateDictionaryProofBoc root must match shardStateDictionaryRoot"
        }
        val selectedTransaction = shardAccountsLastTransaction(
            shardStateDictionaryProofBoc,
            dictionaryKey,
            input.shardStateDictionaryKeyBitLen,
        )
        require(selectedTransaction != null && selectedTransaction.hash == transactionRoot) {
            "shardStateDictionaryProofBoc ShardAccount last transaction hash must match transactionRoot"
        }
        require(selectedTransaction.lt == transactionLt) {
            "shardStateDictionaryProofBoc ShardAccount last transaction lt must match transactionLt"
        }
        val transitions = input.validatorSetTransitionProofs.map {
            normalizeValidatorSetTransitionForSourceState(it)
        }
        val sourceStateVerifierId = normalizeNonEmpty(input.sourceStateVerifierId, "sourceStateVerifierId")
        require(sourceStateVerifierId == MAINNET_SHARD_STATE_VERIFIER_ID_V1) {
            "sourceStateVerifierId must match TON shard-state verifier profile"
        }
        val sourceStateVerifierHashBytes =
            nonZeroHex32Bytes(input.sourceStateVerifierHash, "sourceStateVerifierHash")
        require(!sourceStateVerifierHashBytes.contentEquals(TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
            "sourceStateVerifierHash must not be the TON template verifier hash"
        }
        val sourceStateVerifierHash = "0x" + hexLower(sourceStateVerifierHashBytes)
        return NormalizedTonShardStateSourceStateInput(
            version = 1,
            sourceDomain = input.sourceDomain,
            masterchainSeqno = masterchainSeqno,
            masterchainWorkchainId = input.masterchainWorkchainId,
            masterchainShard = masterchainShard,
            masterchainBlockHash = normalizeNonZeroHex32(
                input.masterchainBlockHash,
                "masterchainBlockHash",
            ),
            masterchainFileHash = normalizeNonZeroHex32(
                input.masterchainFileHash,
                "masterchainFileHash",
            ),
            validatorSetHash = normalizeHex32(input.validatorSetHash, "validatorSetHash"),
            masterchainConfigRoot = normalizeHex32(
                input.masterchainConfigRoot,
                "masterchainConfigRoot",
            ),
            masterchainConfigProofHash = normalizeHex32(
                input.masterchainConfigProofHash,
                "masterchainConfigProofHash",
            ),
            shardWorkchainId = input.shardWorkchainId,
            shardShard = shardShard,
            shardSeqno = shardSeqno,
            shardBlockHash = normalizeHex32(input.shardBlockHash, "shardBlockHash"),
            shardFileHash = normalizeNonZeroHex32(input.shardFileHash, "shardFileHash"),
            shardStateRoot = shardStateRoot,
            transactionRoot = transactionRoot,
            transactionLt = transactionLt,
            shardStateDictionaryRoot = dictionaryRoot,
            shardStateDictionaryKeyBitLen = input.shardStateDictionaryKeyBitLen,
            shardStateDictionaryKey = dictionaryKey,
            masterchainSignatureHash = normalizeHex32(
                input.masterchainSignatureHash,
                "masterchainSignatureHash",
            ),
            shardProofHash = normalizeHex32(input.shardProofHash, "shardProofHash"),
            shardStateProofBoc = shardStateProofBoc,
            shardStateDictionaryProofBoc = shardStateDictionaryProofBoc,
            configDictionaryProofBoc = configDictionaryProofBoc,
            shardStateProofBocHash = shardStateProofBocHash,
            shardAccountsProofBocHash = shardAccountsProofBocHash,
            configProofBocHash = configProofBocHash,
            validatorSetTransitionProofs = transitions,
            transitionChainHash = validatorSetTransitionChainHash(transitions),
            sourceStateVerifierId = sourceStateVerifierId,
            sourceStateVerifierHash = sourceStateVerifierHash,
            sourceTrustAnchorId = normalizeNonEmpty(input.sourceTrustAnchorId, "sourceTrustAnchorId"),
            sourceTrustAnchorHash = normalizeNonZeroHex32(
                input.sourceTrustAnchorHash,
                "sourceTrustAnchorHash",
            ),
            consensusVerifierId = normalizeNonEmpty(input.consensusVerifierId, "consensusVerifierId"),
            consensusVerifierHash = normalizeNonZeroHex32(
                input.consensusVerifierHash,
                "consensusVerifierHash",
            ),
            messageInclusionVerifierId = normalizeNonEmpty(
                input.messageInclusionVerifierId,
                "messageInclusionVerifierId",
            ),
            messageInclusionVerifierHash = normalizeNonZeroHex32(
                input.messageInclusionVerifierHash,
                "messageInclusionVerifierHash",
            ),
            finalityPolicyId = normalizeNonEmpty(input.finalityPolicyId, "finalityPolicyId"),
            finalityPolicyHash = normalizeNonZeroHex32(input.finalityPolicyHash, "finalityPolicyHash"),
        )
    }

    private fun parseBocCompleteOrdinary(input: ByteArray): ParsedBoc {
        val boc = input.copyOf()
        require(boc.size >= BOC_MAGIC.size + 2 && boc.size <= MAX_BOC_BYTES) {
            "TON BoC length is invalid"
        }
        for (index in BOC_MAGIC.indices) {
            require(boc[index] == BOC_MAGIC[index]) { "TON BoC magic is invalid" }
        }
        val cursor = Cursor(BOC_MAGIC.size)
        val flagsSize = boc[cursor.offset].toInt() and 0xff
        cursor.offset += 1
        val hasIndex = (flagsSize and 0x80) != 0
        val hasCrc32c = (flagsSize and 0x40) != 0
        val hasCacheBits = (flagsSize and 0x20) != 0
        val flags = (flagsSize ushr 3) and 0x03
        val sizeBytes = flagsSize and 0x07
        val offsetBytes = boc[cursor.offset].toInt() and 0xff
        cursor.offset += 1
        require(!hasCacheBits && flags == 0 && sizeBytes in 1..4 && offsetBytes in 1..8) {
            "TON BoC header flags are unsupported"
        }
        val cellsCount = readSizedUInt(boc, cursor, sizeBytes)
        val rootsCount = readSizedUInt(boc, cursor, sizeBytes)
        val absentCount = readSizedUInt(boc, cursor, sizeBytes)
        val totalCellsSize = readSizedUInt(boc, cursor, offsetBytes)
        require(
            cellsCount > 0 &&
                cellsCount <= MAX_BOC_CELLS &&
                rootsCount > 0 &&
                rootsCount <= cellsCount &&
                absentCount == 0 &&
                rootsCount + absentCount <= cellsCount,
        ) {
            "TON BoC counts are invalid"
        }
        val roots = ArrayList<Int>(rootsCount)
        repeat(rootsCount) {
            val root = readSizedUInt(boc, cursor, sizeBytes)
            require(root < cellsCount) { "TON BoC root index is invalid" }
            roots.add(root)
        }
        if (hasIndex) {
            var previous = 0
            for (index in 0 until cellsCount) {
                val cellOffset = readSizedUInt(boc, cursor, offsetBytes)
                require(cellOffset >= previous && cellOffset <= totalCellsSize) {
                    "TON BoC index is invalid"
                }
                require(index + 1 != cellsCount || cellOffset == totalCellsSize) {
                    "TON BoC index is invalid"
                }
                previous = cellOffset
            }
        }
        require(totalCellsSize <= boc.size - cursor.offset) {
            "TON BoC cell data length is invalid"
        }
        val cellDataEnd = cursor.offset + totalCellsSize
        val expectedEnd = cellDataEnd + if (hasCrc32c) 4 else 0
        require(expectedEnd == boc.size) { "TON BoC cell data length is invalid" }
        if (hasCrc32c) {
            val expectedCrc = crc32c(boc, cellDataEnd)
            require(
                (boc[cellDataEnd].toInt() and 0xff) == (expectedCrc and 0xff) &&
                    (boc[cellDataEnd + 1].toInt() and 0xff) == ((expectedCrc ushr 8) and 0xff) &&
                    (boc[cellDataEnd + 2].toInt() and 0xff) == ((expectedCrc ushr 16) and 0xff) &&
                    (boc[cellDataEnd + 3].toInt() and 0xff) == ((expectedCrc ushr 24) and 0xff),
            ) {
                "TON BoC CRC32C is invalid"
            }
        }
        val cellData = boc.copyOfRange(cursor.offset, cellDataEnd)
        val cellCursor = Cursor(0)
        val cells = ArrayList<BocCell>(cellsCount)
        for (cellIndex in 0 until cellsCount) {
            require(cellCursor.offset + 2 <= cellData.size) { "TON BoC cell is truncated" }
            val descriptor = cellData[cellCursor.offset].toInt() and 0xff
            cellCursor.offset += 1
            val dataDescriptor = cellData[cellCursor.offset].toInt() and 0xff
            cellCursor.offset += 1
            val refsCount = descriptor and 0x07
            val exotic = (descriptor and 0x08) != 0
            val hasHashes = (descriptor and 0x10) != 0
            val level = (descriptor ushr 5) and 0x07
            val dataBytes = (dataDescriptor + 1) / 2
            require(
                refsCount <= MAX_REFS &&
                    !hasHashes &&
                    dataBytes <= MAX_CELL_SERIALIZED_DATA_BYTES &&
                    cellCursor.offset + dataBytes <= cellData.size,
            ) {
                "TON BoC cell descriptor is unsupported"
            }
            val data = cellData.copyOfRange(cellCursor.offset, cellCursor.offset + dataBytes)
            require(cellDataPaddingIsValid(dataDescriptor, data)) {
                "TON BoC cell data padding is invalid"
            }
            cellCursor.offset += dataBytes
            val refs = ArrayList<Int>(refsCount)
            repeat(refsCount) {
                val refIndex = readSizedUInt(cellData, cellCursor, sizeBytes)
                require(refIndex < cellsCount && refIndex > cellIndex) {
                    "TON BoC cell refs must be forward internal refs"
                }
                refs.add(refIndex)
            }
            cells.add(BocCell((descriptor and 0xef).toByte(), dataDescriptor.toByte(), data, refs, level, exotic))
        }
        require(cellCursor.offset == cellData.size) { "TON BoC has trailing cell data" }
        return ParsedBoc(roots, cells)
    }

    private fun bocChildForHashLevel(
        kind: BocCellKind,
        computed: BocComputedCell,
        level: Int,
    ): Pair<ByteArray, Int> {
        val childLevel = if (kind == BocCellKind.MERKLE_PROOF || kind == BocCellKind.MERKLE_UPDATE) {
            level + 1
        } else {
            level
        }
        val index = Math.min(childLevel, 3)
        return Pair(computed.hashes[index], computed.depths[index])
    }

    private fun bocCellHashes(cells: List<BocCell>): List<BocComputedCell> {
        val computed = MutableList(cells.size) {
            BocComputedCell(0, MutableList(4) { ByteArray(32) }, MutableList(4) { 0 })
        }
        for (index in cells.size - 1 downTo 0) {
            val cell = cells[index]
            val kind = bocCellKind(cell)
            val pruned = if (kind == BocCellKind.PRUNED_BRANCH) parsePrunedBranch(cell) else null
            val mask = when (kind) {
                BocCellKind.ORDINARY -> {
                    var value = 0
                    for (ref in cell.refs) {
                        require(ref >= 0 && ref < computed.size) { "TON BoC cell refs are invalid" }
                        value = value or computed[ref].mask
                    }
                    value
                }
                BocCellKind.PRUNED_BRANCH -> pruned!!.mask
                BocCellKind.MERKLE_PROOF -> {
                    require(
                        cellSerializedBitLenIsByteAligned(cell.dataDescriptor.toInt() and 0xff, cell.data) &&
                            cell.data.size == 35 &&
                            cell.refs.size == 1,
                    ) {
                        "TON BoC Merkle proof cell is invalid"
                    }
                    val child = childHashDepthForLevel(computed[cell.refs[0]], 0)
                    val proofHash = cell.data.copyOfRange(1, 33)
                    val proofDepth = ((cell.data[33].toInt() and 0xff) shl 8) or
                        (cell.data[34].toInt() and 0xff)
                    require(proofHash.contentEquals(child.first) && proofDepth == child.second) {
                        "TON BoC Merkle proof cell is invalid"
                    }
                    levelMaskValue(computed[cell.refs[0]].mask ushr 1)
                }
                BocCellKind.MERKLE_UPDATE -> {
                    require(
                        cellSerializedBitLenIsByteAligned(cell.dataDescriptor.toInt() and 0xff, cell.data) &&
                            cell.data.size == 69 &&
                            cell.refs.size == 2,
                    ) {
                        "TON BoC Merkle update cell is invalid"
                    }
                    for ((refPos, hashOffset, depthOffset) in listOf(Triple(0, 1, 65), Triple(1, 33, 67))) {
                        val child = childHashDepthForLevel(computed[cell.refs[refPos]], 0)
                        val proofHash = cell.data.copyOfRange(hashOffset, hashOffset + 32)
                        val proofDepth = ((cell.data[depthOffset].toInt() and 0xff) shl 8) or
                            (cell.data[depthOffset + 1].toInt() and 0xff)
                        require(proofHash.contentEquals(child.first) && proofDepth == child.second) {
                            "TON BoC Merkle update cell is invalid"
                        }
                    }
                    levelMaskValue((computed[cell.refs[0]].mask or computed[cell.refs[1]].mask) ushr 1)
                }
            }
            require(cell.level == mask) { "TON BoC cell level mask is invalid" }

            val totalHashCount = levelMaskHashIndex(mask) + 1
            val hashCount = if (kind == BocCellKind.PRUNED_BRANCH) 1 else totalHashCount
            val hashOffset = totalHashCount - hashCount
            val computedHashes = ArrayList<ByteArray>(hashCount)
            val computedDepths = ArrayList<Int>(hashCount)
            var hashIndex = 0
            for (levelIndex in 0..levelMaskLevel(mask)) {
                if (!levelMaskIsSignificant(mask, levelIndex)) continue
                if (hashIndex < hashOffset) {
                    hashIndex += 1
                    continue
                }
                val currentData =
                    if (hashIndex == hashOffset) {
                        require(levelIndex == 0 || kind == BocCellKind.PRUNED_BRANCH) {
                            "TON BoC cell hash level is invalid"
                        }
                        cell.data
                    } else {
                        computedHashes[hashIndex - hashOffset - 1]
                    }
                var currentDepth = 0
                for (ref in cell.refs) {
                    val child = bocChildForHashLevel(kind, computed[ref], levelIndex)
                    currentDepth = Math.max(currentDepth, child.second)
                }
                if (cell.refs.isNotEmpty()) currentDepth += 1
                require(currentDepth <= 0xffff) { "TON BoC cell depth is invalid" }

                val appliedMask = levelMaskApply(mask, levelIndex)
                val descriptor = cell.refs.size or
                    (if (kind == BocCellKind.ORDINARY) 0 else 0x08) or
                    (appliedMask shl 5)
                val representation = ByteArrayOutputStream()
                representation.write(descriptor)
                representation.write(cell.dataDescriptor.toInt() and 0xff)
                representation.write(currentData)
                for (ref in cell.refs) {
                    val child = bocChildForHashLevel(kind, computed[ref], levelIndex)
                    writeU16Be(representation, child.second)
                }
                for (ref in cell.refs) {
                    val child = bocChildForHashLevel(kind, computed[ref], levelIndex)
                    representation.write(child.first)
                }
                computedHashes.add(sha256(representation.toByteArray()))
                computedDepths.add(currentDepth)
                hashIndex += 1
            }

            val resolvedHashes = MutableList(4) { ByteArray(32) }
            val resolvedDepths = MutableList(4) { 0 }
            for (resolvedLevel in 0 until 4) {
                val resolvedHashIndex = levelMaskHashIndex(levelMaskApply(mask, resolvedLevel))
                if (pruned != null) {
                    val thisHashIndex = levelMaskHashIndex(mask)
                    if (resolvedHashIndex != thisHashIndex) {
                        resolvedHashes[resolvedLevel] = pruned.hashes[resolvedHashIndex]
                        resolvedDepths[resolvedLevel] = pruned.depths[resolvedHashIndex]
                    } else {
                        resolvedHashes[resolvedLevel] = computedHashes[0]
                        resolvedDepths[resolvedLevel] = computedDepths[0]
                    }
                } else {
                    resolvedHashes[resolvedLevel] = computedHashes[resolvedHashIndex]
                    resolvedDepths[resolvedLevel] = computedDepths[resolvedHashIndex]
                }
            }
            computed[index] = BocComputedCell(mask, resolvedHashes, resolvedDepths)
        }
        return computed
    }

    private fun bocProofRootAndChildIndex(
        parsed: ParsedBoc,
        computed: List<BocComputedCell>,
    ): Pair<ByteArray, Int> {
        require(parsed.roots.size == 1) { "TON BoC must contain exactly one root" }
        val rootIndex = parsed.roots[0]
        require(rootIndex in parsed.cells.indices && rootIndex in computed.indices) {
            "TON BoC root index is invalid"
        }
        val root = parsed.cells[rootIndex]
        return when (bocCellKind(root)) {
            BocCellKind.ORDINARY -> Pair(computed[rootIndex].hashes[3], rootIndex)
            BocCellKind.MERKLE_PROOF -> {
                require(root.refs.size == 1 && root.data.size >= 33) {
                    "TON BoC Merkle proof cell is invalid"
                }
                Pair(root.data.copyOfRange(1, 33), root.refs[0])
            }
            BocCellKind.PRUNED_BRANCH,
            BocCellKind.MERKLE_UPDATE,
            -> throw IllegalArgumentException("TON shard-state proof root is pruned or unsupported")
        }
    }

    private fun shardStateAccountKeyMatchesShardPrefix(
        key: ByteArray,
        keyBitLen: Int,
        opening: ShardStateAccountsOpening,
    ): Boolean {
        if (keyBitLen != SHARD_ACCOUNT_KEY_BITS) return false
        for (bitIndex in 0 until opening.shardPfxBits) {
            if (hashmapKeyBit(key, keyBitLen, bitIndex) != opening.shardPrefixBits[bitIndex]) {
                return false
            }
        }
        return true
    }

    private fun shardIdFromPrefixBits(shardPfxBits: Int, shardPrefixBits: List<Boolean>): BigInteger {
        require(shardPfxBits in 0..60) { "TON ShardIdent prefix length is invalid" }
        var shardId = BigInteger.ZERO
        for (bitIndex in 0 until shardPfxBits) {
            if (shardPrefixBits[bitIndex]) {
                shardId = shardId.setBit(63 - bitIndex)
            }
        }
        return shardId.setBit(63 - shardPfxBits)
    }

    private fun shardStateUnsplitAccountsOpeningFromCell(
        cells: List<BocCell>,
        computed: List<BocComputedCell>,
        cellIndex: Int,
    ): ShardStateAccountsOpening {
        require(cellIndex in cells.indices) { "TON ShardStateUnsplit cell index is invalid" }
        val cell = cells[cellIndex]
        require(bocCellKind(cell) == BocCellKind.ORDINARY) {
            "TON ShardStateUnsplit root must be ordinary"
        }
        val reader = BocBitReader(cell)
        require(reader.readUInt(32) == SHARD_STATE_UNSPLIT_TAG) {
            "TON ShardStateUnsplit tag is invalid"
        }
        val globalId = reader.readUInt(32)
        require(reader.readUInt(2) == 0) { "TON ShardIdent tag is invalid" }
        val shardPfxBits = reader.readUInt(6)
        require(shardPfxBits <= 60) { "TON ShardIdent prefix length is invalid" }
        val workchainId = reader.readUInt(32)
        val shardPrefixBits = List(64) { reader.readBit() }
        val seqNo = reader.readUInt(32)
        reader.readUInt(32)
        val genUtime = reader.readUInt(32)
        val genLt = reader.readUInt64(64)
        val minRefMcSeqno = reader.readUInt(32)
        val outMsgQueueInfoRef = reader.readRef()
        require(outMsgQueueInfoRef in computed.indices) {
            "TON ShardStateUnsplit out_msg_queue_info ref is invalid"
        }
        reader.readBit()
        val accountsRef = reader.readRef()
        require(accountsRef in computed.indices) {
            "TON ShardStateUnsplit accounts ref is invalid"
        }
        val trailingFieldsRef = reader.readRef()
        require(trailingFieldsRef in computed.indices) {
            "TON ShardStateUnsplit trailing fields ref is invalid"
        }
        if (reader.readBit()) {
            require(workchainId != TON_BASECHAIN_WORKCHAIN_ID) {
                "TON basechain ShardStateUnsplit custom must be absent"
            }
            val customRef = reader.readRef()
            require(customRef in computed.indices) { "TON ShardStateUnsplit custom ref is invalid" }
        }
        require(reader.isExhausted()) { "TON ShardStateUnsplit has trailing data" }
        return ShardStateAccountsOpening(
            accountsRootHash = "0x" + hexLower(computed[accountsRef].hashes[3]),
            globalId = globalId,
            workchainId = workchainId,
            seqNo = seqNo,
            genUtime = genUtime,
            genLt = genLt,
            minRefMcSeqno = minRefMcSeqno,
            shardPfxBits = shardPfxBits,
            shardPrefixBits = shardPrefixBits,
            shardId = shardIdFromPrefixBits(shardPfxBits, shardPrefixBits),
        )
    }

    private fun readSizedUInt(bytes: ByteArray, cursor: Cursor, size: Int): Int {
        require(size in 1..8 && cursor.offset + size <= bytes.size) { "TON BoC is truncated" }
        var value = 0L
        for (index in 0 until size) {
            value = (value shl 8) or ((bytes[cursor.offset + index].toInt() and 0xff).toLong())
        }
        cursor.offset += size
        require(value <= Int.MAX_VALUE) { "TON sized integer overflows" }
        return value.toInt()
    }

    private fun crc32c(bytes: ByteArray, end: Int): Int {
        var crc = -1
        for (index in 0 until end) {
            crc = crc xor (bytes[index].toInt() and 0xff)
            repeat(8) {
                val mask = -(crc and 1)
                crc = (crc ushr 1) xor (CRC32C_REFLECTED_POLY and mask)
            }
        }
        return crc.inv()
    }

    private fun cellDataPaddingIsValid(dataDescriptor: Int, data: ByteArray): Boolean =
        (dataDescriptor and 1) == 0 || (data.isNotEmpty() && data[data.size - 1].toInt() != 0)

    private fun cellSerializedBitLenIsByteAligned(dataDescriptor: Int, data: ByteArray): Boolean =
        (dataDescriptor and 1) == 0 && dataDescriptor / 2 == data.size

    private fun cellSerializedBitLen(dataDescriptor: Int, data: ByteArray): Int {
        if ((dataDescriptor and 1) == 0) {
            val byteLen = dataDescriptor / 2
            require(byteLen == data.size) { "TON BoC cell data length is invalid" }
            return byteLen * 8
        }
        val fullBytes = (dataDescriptor + 1) / 2
        val floorBytes = dataDescriptor / 2
        require(fullBytes == data.size && floorBytes + 1 == fullBytes && data.isNotEmpty()) {
            "TON BoC cell data length is invalid"
        }
        val last = data[data.size - 1].toInt() and 0xff
        require(last != 0) { "TON BoC cell data padding is invalid" }
        return floorBytes * 8 + (7 - Integer.numberOfTrailingZeros(last))
    }

    private fun hashmapUIntLenBits(maxValue: Int): Int {
        var bits = 0
        var value = maxValue
        while (value > 0) {
            bits += 1
            value = value ushr 1
        }
        return bits
    }

    private fun hashmapKeyIsCanonical(key: ByteArray, keyBitLen: Int): Boolean {
        if (keyBitLen < 0 || keyBitLen > 0xffff) return false
        val expectedBytes = (keyBitLen + 7) / 8
        if (key.size != expectedBytes) return false
        val unused = expectedBytes * 8 - keyBitLen
        return unused == 0 || ((key[key.size - 1].toInt() and ((1 shl unused) - 1)) == 0)
    }

    private fun hashmapKeyBit(key: ByteArray, keyBitLen: Int, bitIndex: Int): Boolean {
        require(bitIndex < keyBitLen) { "TON HashmapE key bit is out of range" }
        return (((key[bitIndex / 8].toInt() and 0xff) ushr (7 - (bitIndex % 8))) and 1) != 0
    }

    private fun hashmapUnwrapMerkleProofCell(cells: List<BocCell>, cellIndex: Int): Int? {
        require(cellIndex >= 0 && cellIndex < cells.size) { "TON HashmapE cell index is invalid" }
        val cell = cells[cellIndex]
        return when (bocCellKind(cell)) {
            BocCellKind.ORDINARY -> cellIndex
            BocCellKind.MERKLE_PROOF -> {
                require(cell.refs.size == 1) { "TON BoC Merkle proof cell is invalid" }
                cell.refs[0]
            }
            BocCellKind.PRUNED_BRANCH,
            BocCellKind.MERKLE_UPDATE,
            -> null
        }
    }

    private fun hashmapReadLabel(
        reader: BocBitReader,
        key: ByteArray,
        keyBitLen: Int,
        keyOffset: Int,
        maxLen: Int,
    ): Int? {
        if (!reader.readBit()) {
            var labelLen = 0
            while (reader.readBit()) {
                labelLen += 1
                if (labelLen > maxLen) return null
            }
            for (offset in 0 until labelLen) {
                if (reader.readBit() != hashmapKeyBit(key, keyBitLen, keyOffset + offset)) return null
            }
            return labelLen
        }
        if (!reader.readBit()) {
            val labelLen = reader.readUInt(hashmapUIntLenBits(maxLen))
            if (labelLen > maxLen) return null
            for (offset in 0 until labelLen) {
                if (reader.readBit() != hashmapKeyBit(key, keyBitLen, keyOffset + offset)) return null
            }
            return labelLen
        }
        val labelBit = reader.readBit()
        val labelLen = reader.readUInt(hashmapUIntLenBits(maxLen))
        if (labelLen > maxLen) return null
        for (offset in 0 until labelLen) {
            if (labelBit != hashmapKeyBit(key, keyBitLen, keyOffset + offset)) return null
        }
        return labelLen
    }

    private fun hashmapReadLabelBits(reader: BocBitReader, maxLen: Int): MutableList<Boolean>? {
        val bits = ArrayList<Boolean>()
        if (!reader.readBit()) {
            var labelLen = 0
            while (reader.readBit()) {
                labelLen += 1
                if (labelLen > maxLen) return null
            }
            repeat(labelLen) { bits.add(reader.readBit()) }
            return bits
        }
        if (!reader.readBit()) {
            val labelLen = reader.readUInt(hashmapUIntLenBits(maxLen))
            if (labelLen > maxLen) return null
            repeat(labelLen) { bits.add(reader.readBit()) }
            return bits
        }
        val labelBit = reader.readBit()
        val labelLen = reader.readUInt(hashmapUIntLenBits(maxLen))
        if (labelLen > maxLen) return null
        repeat(labelLen) { bits.add(labelBit) }
        return bits
    }

    private fun hashmapCellRefValueHash(
        cells: List<BocCell>,
        computed: List<BocComputedCell>,
        rootIndex: Int,
        key: ByteArray,
        keyBitLen: Int,
    ): String? {
        var cellIndex = hashmapUnwrapMerkleProofCell(cells, rootIndex) ?: return null
        var keyOffset = 0
        var remaining = keyBitLen
        for (step in 0..cells.size) {
            cellIndex = hashmapUnwrapMerkleProofCell(cells, cellIndex) ?: return null
            val reader = BocBitReader(cells[cellIndex])
            val labelLen = hashmapReadLabel(reader, key, keyBitLen, keyOffset, remaining) ?: return null
            keyOffset += labelLen
            remaining -= labelLen
            if (remaining == 0) {
                if (reader.remainingBits() != 0 || reader.remainingRefs() != 1) return null
                val valueRef = reader.readRef()
                if (bocCellKind(cells[valueRef]) == BocCellKind.PRUNED_BRANCH) return null
                return "0x" + hexLower(computed[valueRef].hashes[3])
            }
            if (reader.remainingBits() != 0 || reader.remainingRefs() != 2) return null
            val nextBit = hashmapKeyBit(key, keyBitLen, keyOffset)
            keyOffset += 1
            remaining -= 1
            val leftRef = reader.readRef()
            val rightRef = reader.readRef()
            cellIndex = if (nextBit) rightRef else leftRef
        }
        return null
    }

    private fun hashmapCellRefValueIndex(
        cells: List<BocCell>,
        rootIndex: Int,
        key: ByteArray,
        keyBitLen: Int,
    ): Int? {
        var cellIndex = hashmapUnwrapMerkleProofCell(cells, rootIndex) ?: return null
        var keyOffset = 0
        var remaining = keyBitLen
        for (step in 0..cells.size) {
            cellIndex = hashmapUnwrapMerkleProofCell(cells, cellIndex) ?: return null
            val reader = BocBitReader(cells[cellIndex])
            val labelLen = hashmapReadLabel(reader, key, keyBitLen, keyOffset, remaining) ?: return null
            keyOffset += labelLen
            remaining -= labelLen
            if (remaining == 0) {
                if (reader.remainingBits() != 0 || reader.remainingRefs() != 1) return null
                val valueRef = reader.readRef()
                if (valueRef !in cells.indices || bocCellKind(cells[valueRef]) == BocCellKind.PRUNED_BRANCH) {
                    return null
                }
                return valueRef
            }
            if (reader.remainingBits() != 0 || reader.remainingRefs() != 2) return null
            val nextBit = hashmapKeyBit(key, keyBitLen, keyOffset)
            keyOffset += 1
            remaining -= 1
            val leftRef = reader.readRef()
            val rightRef = reader.readRef()
            cellIndex = if (nextBit) rightRef else leftRef
        }
        return null
    }

    private fun bitsToU16(bits: List<Boolean>): Int? {
        if (bits.size > 16) return null
        var value = 0
        for (bit in bits) {
            value = (value shl 1) or (if (bit) 1 else 0)
        }
        return value
    }

    private fun readEd25519SigPubkey(reader: BocBitReader): ByteArray? {
        if (reader.readUInt(32) != ED25519_PUBKEY_CONSTRUCTOR) return null
        return reader.readBytes(32)
    }

    private fun readValidatorDescr(reader: BocBitReader): Pair<ByteArray, BigInteger>? {
        val constructor = reader.readUInt(8)
        if (constructor != VALIDATOR_CONSTRUCTOR && constructor != VALIDATOR_ADDR_CONSTRUCTOR) {
            return null
        }
        val publicKey = readEd25519SigPubkey(reader) ?: return null
        val weight = reader.readUIntBigInteger(64)
        if (weight == BigInteger.ZERO) return null
        if (constructor == VALIDATOR_ADDR_CONSTRUCTOR) {
            reader.skipBits(256)
        }
        return Pair(publicKey, weight)
    }

    private fun collectValidatorDescrsFromReader(
        cells: List<BocCell>,
        reader: BocBitReader,
        remaining: Int,
        prefix: MutableList<Boolean>,
        out: MutableList<TonValidatorDescr>,
        budget: IntArray,
    ): Boolean {
        if (budget[0] <= 0 || out.size > MAX_VALIDATORS) return false
        budget[0] -= 1
        val labelBits = hashmapReadLabelBits(reader, remaining) ?: return false
        prefix.addAll(labelBits)
        val nextRemaining = remaining - labelBits.size
        if (nextRemaining == 0) {
            val key = bitsToU16(prefix)
            val validator = readValidatorDescr(reader)
            repeat(labelBits.size) { prefix.removeAt(prefix.lastIndex) }
            if (key == null || validator == null || !reader.isExhausted()) {
                return false
            }
            out.add(TonValidatorDescr(key, validator.first, validator.second))
            return true
        }
        if (reader.remainingBits() != 0 || reader.remainingRefs() != 2) {
            repeat(labelBits.size) { prefix.removeAt(prefix.lastIndex) }
            return false
        }
        val leftRef = reader.readRef()
        val rightRef = reader.readRef()
        prefix.add(false)
        val leftOk = collectValidatorDescrsFromCell(cells, leftRef, nextRemaining - 1, prefix, out, budget)
        prefix.removeAt(prefix.lastIndex)
        if (!leftOk) {
            repeat(labelBits.size) { prefix.removeAt(prefix.lastIndex) }
            return false
        }
        prefix.add(true)
        val rightOk = collectValidatorDescrsFromCell(cells, rightRef, nextRemaining - 1, prefix, out, budget)
        prefix.removeAt(prefix.lastIndex)
        repeat(labelBits.size) { prefix.removeAt(prefix.lastIndex) }
        return rightOk
    }

    private fun collectValidatorDescrsFromCell(
        cells: List<BocCell>,
        cellIndex: Int,
        remaining: Int,
        prefix: MutableList<Boolean>,
        out: MutableList<TonValidatorDescr>,
        budget: IntArray,
    ): Boolean {
        if (cellIndex !in cells.indices || bocCellKind(cells[cellIndex]) != BocCellKind.ORDINARY) {
            return false
        }
        return collectValidatorDescrsFromReader(
            cells,
            BocBitReader(cells[cellIndex]),
            remaining,
            prefix,
            out,
            budget,
        )
    }

    private fun validatorSetPayloadFromCell(cells: List<BocCell>, cellIndex: Int): ByteArray {
        require(cellIndex in cells.indices) { "TON ValidatorSet cell index is invalid" }
        val cell = cells[cellIndex]
        require(bocCellKind(cell) == BocCellKind.ORDINARY) { "TON ValidatorSet cell must be ordinary" }
        val reader = BocBitReader(cell)
        val constructor = reader.readUInt(8)
        require(constructor == VALIDATORS_CONSTRUCTOR || constructor == VALIDATORS_EXT_CONSTRUCTOR) {
            "TON ValidatorSet constructor is unsupported"
        }
        val utimeSince = reader.readUIntBigInteger(32)
        val utimeUntil = reader.readUIntBigInteger(32)
        require(utimeUntil > utimeSince) { "TON ValidatorSet validity interval is invalid" }
        val total = reader.readUInt(16)
        val main = reader.readUInt(16)
        require(total > 0 && total <= MAX_VALIDATORS && main > 0 && main <= total) {
            "TON ValidatorSet counts are invalid"
        }
        val declaredTotalWeight =
            if (constructor == VALIDATORS_EXT_CONSTRUCTOR) reader.readUIntBigInteger(64) else null
        val entries = ArrayList<TonValidatorDescr>(total)
        val budget = intArrayOf(cells.size + 1)
        val ok =
            if (constructor == VALIDATORS_EXT_CONSTRUCTOR) {
                val hasRoot = reader.readBit()
                require(hasRoot && reader.remainingBits() == 0 && reader.remainingRefs() == 1) {
                    "TON ValidatorSet dictionary root is invalid"
                }
                collectValidatorDescrsFromCell(
                    cells,
                    reader.readRef(),
                    VALIDATOR_SET_KEY_BITS,
                    ArrayList(),
                    entries,
                    budget,
                )
            } else {
                collectValidatorDescrsFromReader(
                    cells,
                    reader,
                    VALIDATOR_SET_KEY_BITS,
                    ArrayList(),
                    entries,
                    budget,
                )
            }
        require(ok) { "TON ValidatorSet dictionary is invalid" }
        require(entries.size == total && entries.size <= MAX_VALIDATORS) {
            "TON ValidatorSet validator count is invalid"
        }
        entries.sortBy { it.key }
        for (index in 1 until entries.size) {
            require(entries[index - 1].key < entries[index].key) {
                "TON ValidatorSet dictionary keys must be unique and ordered"
            }
        }
        val totalWeight = entries.fold(BigInteger.ZERO) { sum, entry -> sum.add(entry.weight) }
        if (declaredTotalWeight != null) {
            require(declaredTotalWeight != BigInteger.ZERO && declaredTotalWeight == totalWeight) {
                "TON ValidatorSet total weight is invalid"
            }
        }
        return canonicalValidatorSetBytesFromParts(
            entries.map { it.publicKey },
            entries.map { it.weight },
        )
    }

    private fun skipVarUInt(reader: BocBitReader, lengthBits: Int) {
        reader.skipBits(reader.readUInt(lengthBits) * 8)
    }

    private fun skipCurrencyCollection(reader: BocBitReader) {
        skipVarUInt(reader, 4)
        if (reader.readBit()) reader.readRef()
    }

    private fun skipDepthBalanceInfo(reader: BocBitReader) {
        val splitDepth = reader.readUInt(5)
        require(splitDepth <= 30) { "TON DepthBalanceInfo split depth is invalid" }
        skipCurrencyCollection(reader)
    }

    private fun readShardAccountLastTransaction(
        computed: List<BocComputedCell>,
        reader: BocBitReader,
    ): ShardAccountLastTransaction {
        skipDepthBalanceInfo(reader)
        val accountRef = reader.readRef()
        require(accountRef in computed.indices) { "TON ShardAccount account ref is invalid" }
        val lastTransactionHash = reader.readBytes(32)
        val lastTransactionLt = reader.readUIntBigInteger(64)
        require(lastTransactionLt != BigInteger.ZERO) {
            "TON ShardAccount last transaction lt must be non-zero"
        }
        require(reader.isExhausted()) { "TON ShardAccount has trailing data" }
        return ShardAccountLastTransaction("0x" + hexLower(lastTransactionHash), lastTransactionLt)
    }

    private fun hashmapShardAccountsLastTransaction(
        cells: List<BocCell>,
        computed: List<BocComputedCell>,
        rootIndex: Int,
        key: ByteArray,
        keyBitLen: Int,
    ): ShardAccountLastTransaction? {
        var cellIndex = hashmapUnwrapMerkleProofCell(cells, rootIndex) ?: return null
        var keyOffset = 0
        var remaining = keyBitLen
        for (step in 0..cells.size) {
            cellIndex = hashmapUnwrapMerkleProofCell(cells, cellIndex) ?: return null
            val reader = BocBitReader(cells[cellIndex])
            val labelLen = hashmapReadLabel(reader, key, keyBitLen, keyOffset, remaining) ?: return null
            keyOffset += labelLen
            remaining -= labelLen
            if (remaining == 0) {
                return readShardAccountLastTransaction(computed, reader)
            }
            val nextBit = hashmapKeyBit(key, keyBitLen, keyOffset)
            keyOffset += 1
            remaining -= 1
            val leftRef = reader.readRef()
            val rightRef = reader.readRef()
            skipDepthBalanceInfo(reader)
            if (!reader.isExhausted()) return null
            cellIndex = if (nextBit) rightRef else leftRef
        }
        return null
    }

    private fun levelMaskValue(mask: Int): Int = mask and 0x07

    private fun levelMaskLevel(mask: Int): Int {
        var value = levelMaskValue(mask)
        var level = 0
        while (value != 0) {
            level += 1
            value = value ushr 1
        }
        return level
    }

    private fun levelMaskHashIndex(mask: Int): Int {
        var value = levelMaskValue(mask)
        var count = 0
        while (value != 0) {
            count += value and 1
            value = value ushr 1
        }
        return count
    }

    private fun levelMaskApply(mask: Int, level: Int): Int =
        if (level == 0) 0 else levelMaskValue(mask) and ((1 shl level) - 1)

    private fun levelMaskIsSignificant(mask: Int, level: Int): Boolean =
        level == 0 || (((levelMaskValue(mask) ushr (level - 1)) and 1) != 0)

    private fun childHashDepthForLevel(computed: BocComputedCell, level: Int): Pair<ByteArray, Int> {
        val index = Math.min(level, 3)
        return Pair(computed.hashes[index], computed.depths[index])
    }

    private fun bocCellKind(cell: BocCell): BocCellKind {
        if (!cell.exotic) return BocCellKind.ORDINARY
        return when (if (cell.data.isEmpty()) -1 else cell.data[0].toInt() and 0xff) {
            1 -> BocCellKind.PRUNED_BRANCH
            3 -> BocCellKind.MERKLE_PROOF
            4 -> BocCellKind.MERKLE_UPDATE
            else -> throw IllegalArgumentException("TON BoC exotic cell type is unsupported")
        }
    }

    private fun parsePrunedBranch(cell: BocCell): BocPrunedBranch {
        val dataDescriptor = cell.dataDescriptor.toInt() and 0xff
        require(
            cellSerializedBitLenIsByteAligned(dataDescriptor, cell.data) &&
                cell.refs.isEmpty() &&
                cell.data.size >= 2 &&
                (cell.data[0].toInt() and 0xff) == 1,
        ) {
            "TON BoC pruned branch cell is invalid"
        }
        if (cell.data.size == 35) {
            val hash = cell.data.copyOfRange(1, 33)
            val depth = ((cell.data[33].toInt() and 0xff) shl 8) or (cell.data[34].toInt() and 0xff)
            return BocPrunedBranch(1, arrayListOf(hash), arrayListOf(depth))
        }
        val mask = levelMaskValue(cell.data[1].toInt() and 0xff)
        val level = levelMaskLevel(mask)
        require(level in 1..3 && cell.data.size == 2 + level * 34) {
            "TON BoC pruned branch cell is invalid"
        }
        val hashes = ArrayList<ByteArray>(level)
        for (index in 0 until level) {
            val start = 2 + index * 32
            hashes.add(cell.data.copyOfRange(start, start + 32))
        }
        val depths = ArrayList<Int>(level)
        val depthsStart = 2 + level * 32
        for (index in 0 until level) {
            val start = depthsStart + index * 2
            depths.add(((cell.data[start].toInt() and 0xff) shl 8) or (cell.data[start + 1].toInt() and 0xff))
        }
        return BocPrunedBranch(mask, hashes, depths)
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
        return "0x" + hexLower(prefixedHashBytes(prefix, payload))
    }

    private fun prefixedHashBytes(prefix: String, payload: ByteArray): ByteArray {
        val prefixBytes = prefix.toByteArray(Charsets.UTF_8)
        val preimage = ByteArray(prefixBytes.size + payload.size)
        System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.size)
        System.arraycopy(payload, 0, preimage, prefixBytes.size, payload.size)
        return Blake2b.digest256(preimage)
    }

    private fun sha256(input: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(input)

    private fun hex32Bytes(value: String, field: String): ByteArray {
        require(value.trim() == value) { "$field must be canonical hex" }
        var body = value
        if (body.startsWith("0x", ignoreCase = true)) body = body.substring(2)
        require(body.none { it.isWhitespace() }) { "$field must be canonical hex" }
        require(body.length == 64) { "$field must be 32 bytes" }
        val out = ByteArray(32)
        for (i in out.indices) {
            out[i] = body.substring(i * 2, i * 2 + 2).toIntOrNull(16)?.toByte()
                ?: throw IllegalArgumentException("$field must be canonical hex")
        }
        return out
    }

    private fun nonZeroHex32Bytes(value: String, field: String): ByteArray {
        val bytes = hex32Bytes(value, field)
        require(bytes.any { it.toInt() != 0 }) { "$field must not be zero" }
        return bytes
    }

    private fun requireHashRolesDistinct(context: String, fields: List<Pair<String, ByteArray>>) {
        val seen = mutableMapOf<String, String>()
        for ((field, bytes) in fields) {
            val encoded = hexLower(bytes)
            val previous = seen[encoded]
            require(previous == null) { "$context must be distinct: $field matches $previous" }
            seen[encoded] = field
        }
    }

    private fun normalizeHex32(value: String, field: String): String = "0x" + hexLower(hex32Bytes(value, field))

    private fun normalizeNonZeroHex32(value: String, field: String): String =
        "0x" + hexLower(nonZeroHex32Bytes(value, field))

    private fun normalizeNonEmpty(value: String, field: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$field must be non-empty" }
        return trimmed
    }

    private fun normalizePositiveDecimalText(value: String, field: String): String {
        require(value.trim() == value && value.matches(Regex("[0-9]+")) && !value.startsWith("0")) {
            "$field must be a positive decimal"
        }
        return value
    }

    private fun normalizeTonActiveAccountStatus(value: String, field: String): String {
        require(value == "active") { "$field must be active" }
        return value
    }

    private fun normalizeTonRawAddress(value: String, field: String): String {
        require(value.trim() == value) { "$field must not contain whitespace" }
        val parts = value.split(":")
        require(parts.size == 2) { "$field must be workchain:account_hex" }
        val workchain = parts[0]
        val accountHex = parts[1]
        val digits = if (workchain.startsWith("-")) workchain.substring(1) else workchain
        require(
            digits.isNotEmpty() &&
                digits.all { it in '0'..'9' } &&
                !workchain.startsWith("+") &&
                !(workchain.startsWith("-") && digits == "0") &&
                !(digits.length > 1 && digits.startsWith("0")),
        ) {
            "$field workchain must be canonical i32"
        }
        val workchainId = workchain.toIntOrNull()
            ?: throw IllegalArgumentException("$field workchain must be canonical i32")
        require(workchainId == TON_BASECHAIN_WORKCHAIN_ID) { "$field workchain must be basechain 0" }
        require(accountHex.length == 64) { "$field account must be 32 bytes" }
        require(accountHex.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field account must be lowercase canonical hex"
        }
        require(hex32Bytes(accountHex, "$field account").any { it.toInt() != 0 }) {
            "$field account must not be zero"
        }
        return value
    }

    private fun normalizeProofContext(
        statementHash: String,
        destinationBindingHash: String,
    ): TonSccpProofContext =
        TonSccpProofContext(
            version = 1,
            statementHash = normalizeNonZeroHex32(statementHash, "statementHash"),
            destinationBindingHash =
                normalizeNonZeroHex32(destinationBindingHash, "destinationBindingHash"),
        )

    private fun writeString(out: ByteArrayOutputStream, value: String, field: String) {
        val bytes = normalizeNonEmpty(value, field).toByteArray(Charsets.UTF_8)
        writeU32Le(out, bytes.size)
        out.write(bytes)
    }

    private fun normalizeU64(value: String, field: String): BigInteger {
        require(value.trim() == value) { "$field must be a canonical unsigned integer" }
        require(value.matches(Regex("[0-9]+"))) { "$field must be a canonical unsigned integer" }
        require(value == "0" || !value.startsWith("0")) { "$field must be a canonical unsigned integer" }
        val numeric = BigInteger(value)
        require(numeric <= MAX_U64) { "$field must fit u64" }
        return numeric
    }

    private fun writeU16Be(out: ByteArrayOutputStream, value: Int) {
        out.write((value ushr 8) and 0xff)
        out.write(value and 0xff)
    }

    private fun writeU16Le(out: ByteArrayOutputStream, value: Int) {
        require(value in 0..0xffff) { "u16 value must fit u16" }
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
    }

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0) { "u32 value must not be negative" }
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun writeI32Le(out: ByteArrayOutputStream, value: Int) {
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

    private fun currentValidatorSetConfigKey(): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32Be(out, CURRENT_VALIDATOR_SET_CONFIG_PARAM)
        return out.toByteArray()
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        for (i in 0 until 8) {
            out.write(working.and(BigInteger.valueOf(0xffL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun sccpWordU32Le(value: Int): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32Le(out, value)
        return out.toByteArray().copyOf(32)
    }

    private fun sccpWordU8(value: Int): ByteArray {
        require(value in 0..0xff) { "u8 value out of range" }
        val out = ByteArray(32)
        out[0] = value.toByte()
        return out
    }

    private fun sccpWordI32Le(value: Int): ByteArray {
        val out = ByteArrayOutputStream()
        writeI32Le(out, value)
        return out.toByteArray().copyOf(32)
    }

    private fun sccpWordU64Le(value: BigInteger): ByteArray {
        val out = ByteArrayOutputStream()
        writeU64Le(out, value)
        return out.toByteArray().copyOf(32)
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

    private data class NormalizedTonValidatorSetTransitionProof(
        val version: Int,
        val sourceDomain: Int,
        val fromValidatorSetSeqno: BigInteger,
        val toValidatorSetSeqno: BigInteger,
        val masterchainSeqno: BigInteger,
        val masterchainWorkchainId: Int,
        val masterchainShard: BigInteger,
        val masterchainBlockHash: String,
        val masterchainFileHash: String,
        val parentValidatorSetHash: String,
        val nextValidatorSetHash: String,
        val nextValidatorSetPayload: ByteArray,
        val nextValidatorSetPayloadHash: String,
        val nextValidatorSetConfigHash: String,
        val transitionMessageHash: String,
        val transitionSignatureHash: String,
        val validatorSignatureProof: TonValidatorSignatureProofInput,
    )

    private data class NormalizedTonShardStateSourceStateInput(
        val version: Int,
        val sourceDomain: Int,
        val masterchainSeqno: BigInteger,
        val masterchainWorkchainId: Int,
        val masterchainShard: BigInteger,
        val masterchainBlockHash: String,
        val masterchainFileHash: String,
        val validatorSetHash: String,
        val masterchainConfigRoot: String,
        val masterchainConfigProofHash: String,
        val shardWorkchainId: Int,
        val shardShard: BigInteger,
        val shardSeqno: BigInteger,
        val shardBlockHash: String,
        val shardFileHash: String,
        val shardStateRoot: String,
        val transactionRoot: String,
        val transactionLt: BigInteger,
        val shardStateDictionaryRoot: String,
        val shardStateDictionaryKeyBitLen: Int,
        val shardStateDictionaryKey: ByteArray,
        val masterchainSignatureHash: String,
        val shardProofHash: String,
        val shardStateProofBoc: ByteArray,
        val shardStateDictionaryProofBoc: ByteArray,
        val configDictionaryProofBoc: ByteArray,
        val shardStateProofBocHash: String,
        val shardAccountsProofBocHash: String,
        val configProofBocHash: String,
        val validatorSetTransitionProofs: List<NormalizedTonValidatorSetTransitionProof>,
        val transitionChainHash: String,
        val sourceStateVerifierId: String,
        val sourceStateVerifierHash: String,
        val sourceTrustAnchorId: String,
        val sourceTrustAnchorHash: String,
        val consensusVerifierId: String,
        val consensusVerifierHash: String,
        val messageInclusionVerifierId: String,
        val messageInclusionVerifierHash: String,
        val finalityPolicyId: String,
        val finalityPolicyHash: String,
    )

    private data class TonFullLightClientAuditRoleProfile(
        val name: String,
        val code: Int,
        val circuitId: String,
        val verifierId: String,
        val requiredInputNames: List<String>,
    )

    private data class NormalizedTonFullLightClientAuditInput(
        val role: TonSccpFullLightClientAuditRole,
        val shardState: NormalizedTonShardStateSourceStateInput,
        val sourceVerifierMaterialHash: String,
        val sourceAdapterDeploymentHash: String,
        val fullLightClientGateHash: String,
        val verifierHash: String,
        val shardStateProofPublicInputsHash: String,
        val shardStateVerificationProofHash: String,
        val validatorSetPayloadHash: String,
        val configLeafHash: String,
        val configValueHash: String,
    )

    private data class ReadVec(val bytes: ByteArray, val nextOffset: Int)

    private data class SccpPayloadSummary(
        val kind: String,
        val sourceDomain: Int,
        val targetDomain: Int,
        val messageId: String,
        val payloadHash: String,
    )

    private data class SccpCommitmentSummary(
        val kindCode: Int,
        val targetDomain: Int,
        val messageId: String,
        val payloadHash: String,
    )

    private data class SccpBundleSummary(
        val sourceDomain: Int,
        val targetDomain: Int,
        val messageId: String,
        val payloadHash: String,
        val commitmentRoot: String,
    )

    private data class TonCell(val data: ByteArray, val refs: MutableList<Int>)

    private data class BocCell(
        val descriptor: Byte,
        val dataDescriptor: Byte,
        val data: ByteArray,
        val refs: List<Int>,
        val level: Int,
        val exotic: Boolean,
    )

    private enum class BocCellKind {
        ORDINARY,
        PRUNED_BRANCH,
        MERKLE_PROOF,
        MERKLE_UPDATE,
    }

    private data class BocPrunedBranch(
        val mask: Int,
        val hashes: List<ByteArray>,
        val depths: List<Int>,
    )

    private data class BocComputedCell(
        val mask: Int,
        val hashes: List<ByteArray>,
        val depths: List<Int>,
    )

    private data class ShardStateAccountsOpening(
        val accountsRootHash: String,
        val globalId: Int,
        val workchainId: Int,
        val seqNo: Int,
        val genUtime: Int,
        val genLt: Long,
        val minRefMcSeqno: Int,
        val shardPfxBits: Int,
        val shardPrefixBits: List<Boolean>,
        val shardId: BigInteger,
    )

    private data class TonValidatorDescr(
        val key: Int,
        val publicKey: ByteArray,
        val weight: BigInteger,
    )

    private class BocBitReader(private val cell: BocCell) {
        private val bitLen = SccpTon.cellSerializedBitLen(cell.dataDescriptor.toInt() and 0xff, cell.data)
        private var bitOffset = 0
        private var refOffset = 0

        fun readBit(): Boolean {
            require(bitOffset < bitLen) { "TON HashmapE cell bits are truncated" }
            val bit = (((cell.data[bitOffset / 8].toInt() and 0xff) ushr (7 - (bitOffset % 8))) and 1) != 0
            bitOffset += 1
            return bit
        }

        fun readUInt(bits: Int): Int {
            var value = 0
            repeat(bits) {
                value = (value shl 1) or (if (readBit()) 1 else 0)
            }
            return value
        }

        fun readUInt64(bits: Int): Long {
            var value = 0L
            repeat(bits) {
                value = (value shl 1) or (if (readBit()) 1L else 0L)
            }
            return value
        }

        fun readUIntBigInteger(bits: Int): BigInteger {
            var value = BigInteger.ZERO
            repeat(bits) {
                value = value.shiftLeft(1)
                if (readBit()) {
                    value = value.or(BigInteger.ONE)
                }
            }
            return value
        }

        fun readBytes(byteLength: Int): ByteArray =
            ByteArray(byteLength) { readUInt(8).toByte() }

        fun skipBits(bits: Int) {
            require(bits >= 0 && bitOffset + bits <= bitLen) { "TON BoC cell bits are truncated" }
            bitOffset += bits
        }

        fun readRef(): Int {
            require(refOffset < cell.refs.size) { "TON HashmapE cell refs are truncated" }
            val ref = cell.refs[refOffset]
            refOffset += 1
            return ref
        }

        fun remainingBits(): Int = bitLen - bitOffset

        fun remainingRefs(): Int = cell.refs.size - refOffset

        fun isExhausted(): Boolean = remainingBits() == 0 && remainingRefs() == 0
    }

    private data class ParsedBoc(val roots: List<Int>, val cells: List<BocCell>)

    private data class Cursor(var offset: Int)
}

/** TON validator signature proof transcript material used by validator-set transitions. */
data class TonValidatorSignatureProofInput(
    val version: Int = 1,
    val totalWeight: String,
    val signedWeight: String,
    val blockMessageHash: String,
    val validatorPublicKeys: List<ByteArray>,
    val validatorWeights: List<String>,
    val signersBitmap: ByteArray,
    val signatures: List<ByteArray>,
)

/** TON validator-set transition material used by shard-state source proofs. */
data class TonValidatorSetTransitionProofInput(
    val version: Int = 1,
    val sourceDomain: Int = SccpTon.DOMAIN_TON,
    val fromValidatorSetSeqno: String,
    val toValidatorSetSeqno: String,
    val masterchainSeqno: String,
    val masterchainWorkchainId: Int = -1,
    val masterchainShard: String = "9223372036854775808",
    val masterchainBlockHash: String,
    val masterchainFileHash: String,
    val parentValidatorSetHash: String,
    val nextValidatorSetHash: String,
    val nextValidatorSetPayload: ByteArray,
    val nextValidatorSetPayloadHash: String,
    val nextValidatorSetConfigHash: String,
    val transitionMessageHash: String,
    val transitionSignatureHash: String,
    val validatorSignatureProof: TonValidatorSignatureProofInput,
)

/** Witness material for a TON shard-state OpenVerify source-state proof request. */
data class TonShardStateProofRequestInput(
    val sourceDomain: Int = SccpTon.DOMAIN_TON,
    val masterchainSeqno: String,
    val masterchainWorkchainId: Int = -1,
    val masterchainShard: String = "9223372036854775808",
    val masterchainBlockHash: String,
    val masterchainFileHash: String,
    val validatorSetHash: String,
    val masterchainConfigRoot: String,
    val masterchainConfigProofHash: String,
    val shardWorkchainId: Int = 0,
    val shardShard: String,
    val shardSeqno: String,
    val shardBlockHash: String,
    val shardFileHash: String,
    val shardStateRoot: String,
    val transactionRoot: String,
    val transactionLt: String,
    val shardStateDictionaryRoot: String,
    val shardStateDictionaryKeyBitLen: Int,
    val shardStateDictionaryKey: ByteArray,
    val masterchainSignatureHash: String,
    val shardProofHash: String,
    val shardStateProofBoc: ByteArray,
    val shardStateDictionaryProofBoc: ByteArray,
    val configDictionaryProofBoc: ByteArray,
    val validatorSetTransitionProofs: List<TonValidatorSetTransitionProofInput> = emptyList(),
    val sourceStateVerifierId: String = SccpTon.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
    val sourceStateVerifierHash: String,
    val sourceTrustAnchorId: String = "sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1",
    val sourceTrustAnchorHash: String,
    val consensusVerifierId: String = "sccp:ton:consensus-verifier:masterchain-block-proof:v1",
    val consensusVerifierHash: String,
    val messageInclusionVerifierId: String =
        "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1",
    val messageInclusionVerifierHash: String,
    val finalityPolicyId: String = "sccp:ton:finality-policy:masterchain-finality:v1",
    val finalityPolicyHash: String,
)

/** FastPQ public input tuple used by the TON shard-state OpenVerify request. */
data class TonShardStateFastpqPublicInputs(
    val dsid: String,
    val slot: String,
    val oldRoot: String,
    val newRoot: String,
    val permRoot: String,
    val txSetHash: String,
)

/** FastPQ metadata transition emitted by the TON shard-state OpenVerify request. */
data class TonShardStateFastpqTransition(
    val key: String,
    val operation: String,
    val oldValue: String,
    val newValue: String,
)

/** Request bytes and metadata for a user-side TON shard-state source-state proof. */
class TonShardStateProofRequest(
    val version: Int,
    val proofFamily: String,
    val circuitId: String,
    val parameterSet: String,
    val sourceDomain: Int,
    val masterchainSeqno: String,
    val shardSeqno: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val shardStateProofPublicInputsHash: String,
    statementBytes: ByteArray,
    witnessCommitmentBytes: ByteArray,
    verificationContextBytes: ByteArray,
    schemaDescriptor: ByteArray,
    publicInputColumns: List<List<String>>,
    val fastpqPublicInputs: TonShardStateFastpqPublicInputs,
    fastpqTransitions: List<TonShardStateFastpqTransition>,
) {
    private val statementBytesStorage = statementBytes.copyOf()
    private val witnessCommitmentBytesStorage = witnessCommitmentBytes.copyOf()
    private val verificationContextBytesStorage = verificationContextBytes.copyOf()
    private val schemaDescriptorStorage = schemaDescriptor.copyOf()
    private val publicInputColumnsStorage = publicInputColumns.map { it.toList() }
    private val fastpqTransitionsStorage = fastpqTransitions.toList()

    val statementBytes: ByteArray
        get() = statementBytesStorage.copyOf()

    val witnessCommitmentBytes: ByteArray
        get() = witnessCommitmentBytesStorage.copyOf()

    val verificationContextBytes: ByteArray
        get() = verificationContextBytesStorage.copyOf()

    val schemaDescriptor: ByteArray
        get() = schemaDescriptorStorage.copyOf()

    val publicInputColumns: List<List<String>>
        get() = publicInputColumnsStorage.map { it.toList() }

    val fastpqTransitions: List<TonShardStateFastpqTransition>
        get() = fastpqTransitionsStorage.toList()
}

/** Source-state verification proof capsule generated by a user-side TON prover. */
class TonSccpSourceStateVerificationProof(
    val version: Int = 1,
    val proofFamily: String = "stark-fri-v1",
    val circuitId: String = SccpTon.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
    proofBytes: ByteArray,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val proofBase64: String
        get() = Base64.getEncoder().encodeToString(proofBytesStorage)

    fun copy(
        version: Int = this.version,
        proofFamily: String = this.proofFamily,
        circuitId: String = this.circuitId,
        proofBytes: ByteArray = this.proofBytes,
    ): TonSccpSourceStateVerificationProof =
        TonSccpSourceStateVerificationProof(version, proofFamily, circuitId, proofBytes)

    operator fun component1(): Int = version
    operator fun component2(): String = proofFamily
    operator fun component3(): String = circuitId
    operator fun component4(): ByteArray = proofBytes

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is TonSccpSourceStateVerificationProof &&
            version == other.version &&
            proofFamily == other.proofFamily &&
            circuitId == other.circuitId &&
            proofBytesStorage.contentEquals(other.proofBytesStorage)

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + proofFamily.hashCode()
        result = 31 * result + circuitId.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        return result
    }

    override fun toString(): String =
        "TonSccpSourceStateVerificationProof(version=$version, " +
            "proofFamily=$proofFamily, circuitId=$circuitId, " +
            "proofBytes=${proofBytesStorage.size} bytes)"
}

/** TON full light-client audit role proven by a user-side prover. */
enum class TonSccpFullLightClientAuditRole {
    /** Verifies the masterchain config dictionary and validator-set payload binding. */
    MASTERCHAIN_CONFIG,

    /** Verifies validator-set transition proofs from the trusted anchor to the active set. */
    VALIDATOR_SET_TRANSITION,

    /** Verifies the shard-state ShardAccounts dictionary opening used by SCCP messages. */
    SHARD_ACCOUNTS_DICTIONARY,
}

/** Input required to build TON full light-client audit proof requests on UI/mobile clients. */
data class TonSccpFullLightClientAuditProofInput(
    val shardState: TonShardStateProofRequestInput,
    val shardStateVerificationProof: TonSccpSourceStateVerificationProof,
    val validatorSetPayloadHash: String,
    val configLeafHash: String,
    val configValueHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterDeploymentHash: String,
    val fullLightClientGateHash: String,
    val tonMasterchainConfigVerifierHash: String,
    val tonValidatorSetTransitionVerifierHash: String,
    val tonShardAccountsDictionaryVerifierHash: String,
    val shardStateProofPublicInputsHash: String? = null,
    val shardStateVerificationProofHash: String? = null,
)

/** FastPQ public inputs bound to a TON full light-client audit role proof request. */
data class TonSccpFullLightClientAuditFastpqPublicInputs(
    val dsid: String,
    val slot: String,
    val oldRoot: String,
    val newRoot: String,
    val permRoot: String,
    val txSetHash: String,
)

/** One FastPQ transition supplied to a TON full light-client audit role prover. */
data class TonSccpFullLightClientAuditFastpqTransition(
    val key: String,
    val operation: String,
    val oldValue: String,
    val newValue: String,
)

/** OpenVerify request for one TON full light-client audit role proof. */
class TonSccpFullLightClientAuditProofRequest(
    val version: Int,
    val proofFamily: String,
    val circuitId: String,
    val parameterSet: String,
    val role: String,
    val roleCode: Int,
    val sourceDomain: Int,
    val masterchainSeqno: String,
    val shardSeqno: String,
    val verifierId: String,
    val verifierHash: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterDeploymentHash: String,
    val fullLightClientGateHash: String,
    val shardStateProofPublicInputsHash: String,
    val shardStateVerificationProofHash: String,
    val auditStatementHash: String,
    statementBytes: ByteArray,
    verificationContextBytes: ByteArray,
    schemaDescriptor: ByteArray,
    publicInputColumns: List<List<String>>,
    val fastpqPublicInputs: TonSccpFullLightClientAuditFastpqPublicInputs,
    fastpqTransitions: List<TonSccpFullLightClientAuditFastpqTransition>,
) {
    private val statementBytesStorage = statementBytes.copyOf()
    private val verificationContextBytesStorage = verificationContextBytes.copyOf()
    private val schemaDescriptorStorage = schemaDescriptor.copyOf()
    private val publicInputColumnsStorage = publicInputColumns.map { it.toList() }
    private val fastpqTransitionsStorage = fastpqTransitions.toList()

    val statementBytes: ByteArray
        get() = statementBytesStorage.copyOf()

    val verificationContextBytes: ByteArray
        get() = verificationContextBytesStorage.copyOf()

    val schemaDescriptor: ByteArray
        get() = schemaDescriptorStorage.copyOf()

    val publicInputColumns: List<List<String>>
        get() = publicInputColumnsStorage.map { it.toList() }

    val fastpqTransitions: List<TonSccpFullLightClientAuditFastpqTransition>
        get() = fastpqTransitionsStorage.toList()
}

/** Role-separated TON full light-client audit proof requests. */
data class TonSccpFullLightClientAuditProofRequests(
    val masterchainConfig: TonSccpFullLightClientAuditProofRequest,
    val validatorSetTransition: TonSccpFullLightClientAuditProofRequest,
    val shardAccountsDictionary: TonSccpFullLightClientAuditProofRequest,
)

/** Local proof engine for nested TON shard-state source-state requests. */
fun interface TonSccpShardStateProofEngine {
    fun prove(request: TonShardStateProofRequest): ByteArray
}

/** Local proof engine for TON full-light source-state audit role requests. */
fun interface TonSccpFullLightClientAuditProofEngine {
    fun prove(request: TonSccpFullLightClientAuditProofRequest): ByteArray
}

/** Role-separated TON full-light audit proof capsules. */
data class TonSccpFullLightClientAuditProofs(
    val masterchainConfig: TonSccpSourceStateVerificationProof,
    val validatorSetTransition: TonSccpSourceStateVerificationProof,
    val shardAccountsDictionary: TonSccpSourceStateVerificationProof,
)

/** Source-state proof wrapper for UI and mobile TON proof engines. */
class TonSccpSourceStateProver(
    private val shardStateProofEngine: TonSccpShardStateProofEngine? = null,
    private val fullLightClientAuditProofEngine: TonSccpFullLightClientAuditProofEngine? = null,
) {
    fun proveShardState(
        input: TonShardStateProofRequestInput,
    ): TonSccpSourceStateVerificationProof =
        proveShardState(SccpTon.buildShardStateProofRequest(input))

    fun proveShardState(
        request: TonShardStateProofRequest,
    ): TonSccpSourceStateVerificationProof {
        val engine = shardStateProofEngine
            ?: throw IllegalStateException("TON SCCP source-state prover is not linked")
        SccpTon.requireSourceStateProofRequestForProverCallback(request)
        return SccpTon.wrapSourceStateVerificationProof(
            engine.prove(callbackRequestSnapshot(request)),
            request,
        )
    }

    fun proveFullLightClientAudit(
        input: TonSccpFullLightClientAuditProofInput,
    ): TonSccpFullLightClientAuditProofs {
        val requests = SccpTon.buildFullLightClientAuditProofRequests(input)
        return TonSccpFullLightClientAuditProofs(
            masterchainConfig = proveFullLightClientAudit(requests.masterchainConfig),
            validatorSetTransition = proveFullLightClientAudit(requests.validatorSetTransition),
            shardAccountsDictionary = proveFullLightClientAudit(requests.shardAccountsDictionary),
        )
    }

    fun proveFullLightClientAudit(
        request: TonSccpFullLightClientAuditProofRequest,
    ): TonSccpSourceStateVerificationProof {
        val engine = fullLightClientAuditProofEngine
            ?: throw IllegalStateException("TON SCCP source-state prover is not linked")
        SccpTon.requireSourceStateProofRequestForProverCallback(request)
        return SccpTon.wrapSourceStateVerificationProof(
            engine.prove(callbackRequestSnapshot(request)),
            request,
        )
    }

    private fun callbackRequestSnapshot(request: TonShardStateProofRequest): TonShardStateProofRequest =
        TonShardStateProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            sourceDomain = request.sourceDomain,
            masterchainSeqno = request.masterchainSeqno,
            shardSeqno = request.shardSeqno,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            shardStateProofPublicInputsHash = request.shardStateProofPublicInputsHash,
            statementBytes = request.statementBytes,
            witnessCommitmentBytes = request.witnessCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = request.fastpqTransitions,
        )

    private fun callbackRequestSnapshot(
        request: TonSccpFullLightClientAuditProofRequest,
    ): TonSccpFullLightClientAuditProofRequest =
        TonSccpFullLightClientAuditProofRequest(
            version = request.version,
            proofFamily = request.proofFamily,
            circuitId = request.circuitId,
            parameterSet = request.parameterSet,
            role = request.role,
            roleCode = request.roleCode,
            sourceDomain = request.sourceDomain,
            masterchainSeqno = request.masterchainSeqno,
            shardSeqno = request.shardSeqno,
            verifierId = request.verifierId,
            verifierHash = request.verifierHash,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            sourceVerifierMaterialHash = request.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = request.sourceAdapterDeploymentHash,
            fullLightClientGateHash = request.fullLightClientGateHash,
            shardStateProofPublicInputsHash = request.shardStateProofPublicInputsHash,
            shardStateVerificationProofHash = request.shardStateVerificationProofHash,
            auditStatementHash = request.auditStatementHash,
            statementBytes = request.statementBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = request.fastpqTransitions,
        )
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

/** Governed SORA -> TON destination binding carried in submission metadata. */
data class TonSccpSubmissionDestinationBindingInput(
    val key: String,
    val bindingHash: String,
)

/** SCCP manifest fields used to derive TON submission metadata. */
data class TonSccpSubmissionManifestInput(
    val version: Int = 1,
    val localDomain: Int = SccpSolana.DOMAIN_SORA,
    val counterpartyDomain: Int = SccpTon.DOMAIN_TON,
    val securityModel: String = "RecursiveZk",
    val anchorGovernance: String = "CryptographicProof",
    val verifierTarget: String = "TonContract",
    val verifierBackendFamily: String = "TonContract",
    val proofFamily: String = SccpTon.STARK_FRI_PROOF_FAMILY_V1,
    val verifierBackendKey: String = SccpTon.CONTRACT_PROOF_BACKEND_V1,
    val messageBackend: String,
    val registryBackend: String,
    val manifestSeed: String,
    val destinationBinding: TonSccpSubmissionDestinationBindingInput? = null,
)

/** Inputs for canonical TON submission metadata included in message-body BOCs. */
data class TonSccpSubmissionMetadataInput(
    val manifest: TonSccpSubmissionManifestInput,
    val destinationBinding: TonSccpSubmissionDestinationBindingInput? = null,
    val destinationBindingHash: String? = null,
    val publicInputs: TonSccpPublicInputsInput,
    val statementHash: String,
)

/** Inputs for a TON internal message body carrying an SCCP proof submission. */
class TonSccpMessageBodyInput private constructor(
    val proofResult: TonSccpProofResult,
    val publicInputs: TonSccpPublicInputsInput,
    proofBytes: ByteArray,
    bundleBytes: ByteArray,
    val statementHash: String,
    val destinationBindingHash: String,
    metadataBytes: ByteArray = ByteArray(0),
    val queryId: String? = null,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val metadataBytesStorage: ByteArray = metadataBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val metadataBytes: ByteArray
        get() = metadataBytesStorage.copyOf()

    private data class CheckedMessageBodyInput(
        val proofResult: TonSccpProofResult,
        val publicInputs: TonSccpPublicInputsInput,
        val proofBytes: ByteArray,
        val bundleBytes: ByteArray,
        val statementHash: String,
        val destinationBindingHash: String,
    )

    constructor(
        proofResult: TonSccpProofResult,
        bundleBytes: ByteArray,
        metadataBytes: ByteArray = ByteArray(0),
        queryId: String? = null,
    ) : this(
        checkedInput = checkedMessageBodyInput(proofResult, bundleBytes),
        metadataBytes = metadataBytes,
        queryId = queryId,
    )

    private constructor(
        checkedInput: CheckedMessageBodyInput,
        metadataBytes: ByteArray = ByteArray(0),
        queryId: String? = null,
    ) : this(
        proofResult = checkedInput.proofResult,
        publicInputs = checkedInput.publicInputs,
        proofBytes = checkedInput.proofBytes,
        bundleBytes = checkedInput.bundleBytes,
        statementHash = checkedInput.statementHash,
        destinationBindingHash = checkedInput.destinationBindingHash,
        metadataBytes = metadataBytes,
        queryId = queryId,
    )

    private companion object {
        fun checkedMessageBodyInput(
            proofResult: TonSccpProofResult,
            bundleBytes: ByteArray,
        ): CheckedMessageBodyInput {
            val checkedProofResult = SccpTon.requireWrappedProofResultForSubmission(proofResult)
            return CheckedMessageBodyInput(
                proofResult = checkedProofResult,
                publicInputs = checkedProofResult.publicInputs,
                proofBytes = checkedProofResult.proofBytes,
                bundleBytes = requireBundleMatchesProofResult(bundleBytes, checkedProofResult),
                statementHash = checkedProofResult.proofContext.statementHash,
                destinationBindingHash = checkedProofResult.proofContext.destinationBindingHash,
            )
        }

        fun requireBundleMatchesProofResult(
            bundleBytes: ByteArray,
            proofResult: TonSccpProofResult,
        ): ByteArray {
            require(bundleBytes.contentEquals(proofResult.bundleBytes)) {
                "bundleBytes must match proofResult.bundleBytes"
            }
            return bundleBytes
        }
    }

    fun copy(
        proofResult: TonSccpProofResult = this.proofResult,
        bundleBytes: ByteArray = this.bundleBytes,
        metadataBytes: ByteArray = this.metadataBytes,
        queryId: String? = this.queryId,
    ): TonSccpMessageBodyInput =
        TonSccpMessageBodyInput(
            proofResult = proofResult,
            bundleBytes = bundleBytes,
            metadataBytes = metadataBytes,
            queryId = queryId,
        )

    operator fun component1(): TonSccpPublicInputsInput = publicInputs
    operator fun component2(): ByteArray = proofBytes
    operator fun component3(): ByteArray = bundleBytes
    operator fun component4(): String = statementHash
    operator fun component5(): String = destinationBindingHash
    operator fun component6(): ByteArray = metadataBytes
    operator fun component7(): String? = queryId

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is TonSccpMessageBodyInput &&
            proofResult == other.proofResult &&
            publicInputs == other.publicInputs &&
            proofBytesStorage.contentEquals(other.proofBytesStorage) &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            metadataBytesStorage.contentEquals(other.metadataBytesStorage) &&
            queryId == other.queryId

    override fun hashCode(): Int {
        var result = proofResult.hashCode()
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + metadataBytesStorage.contentHashCode()
        result = 31 * result + (queryId?.hashCode() ?: 0)
        return result
    }

    override fun toString(): String =
        "TonSccpMessageBodyInput(publicInputs=$publicInputs, " +
            "proofBytes=${proofBytesStorage.size} bytes, " +
            "bundleBytes=${bundleBytesStorage.size} bytes, statementHash=$statementHash, " +
            "destinationBindingHash=$destinationBindingHash, " +
            "metadataBytes=${metadataBytesStorage.size} bytes, queryId=$queryId)"
}

/** One TON SCCP submission argument in Rust template order. */
data class TonSccpSubmissionArgument(
    val key: String,
    val encoding: String,
    val bytesHex: String,
)

/** Prebuilt TON SCCP submission envelope for wallet or liteserver broadcasting. */
class TonSccpSubmission(
    val version: Int,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    messageBodyBoc: ByteArray,
    val messageBodyBocHex: String,
    arguments: List<TonSccpSubmissionArgument>,
    envelopeBytes: ByteArray,
    val envelopeHex: String,
) {
    private val messageBodyBocStorage: ByteArray = messageBodyBoc.copyOf()
    private val envelopeBytesStorage: ByteArray = envelopeBytes.copyOf()

    val messageBodyBoc: ByteArray
        get() = messageBodyBocStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = envelopeBytesStorage.copyOf()

    val arguments: List<TonSccpSubmissionArgument> = arguments.toList()

    constructor(
        envelopeEncoding: String,
        messageBodyBoc: ByteArray,
        messageBodyBocHex: String,
    ) : this(
        1,
        envelopeEncoding,
        "internal_message",
        "op::submit_sccp_message_proof",
        messageBodyBoc,
        messageBodyBocHex,
        listOf(
            TonSccpSubmissionArgument("message_body_boc", "ton_boc", messageBodyBocHex),
        ),
        messageBodyBoc,
        messageBodyBocHex,
    )

    fun copy(
        version: Int = this.version,
        envelopeEncoding: String = this.envelopeEncoding,
        submissionKind: String = this.submissionKind,
        verifierEntrypoint: String = this.verifierEntrypoint,
        messageBodyBoc: ByteArray = this.messageBodyBoc,
        messageBodyBocHex: String = this.messageBodyBocHex,
        arguments: List<TonSccpSubmissionArgument> = this.arguments,
        envelopeBytes: ByteArray = this.envelopeBytes,
        envelopeHex: String = this.envelopeHex,
    ): TonSccpSubmission =
        TonSccpSubmission(
            version,
            envelopeEncoding,
            submissionKind,
            verifierEntrypoint,
            messageBodyBoc,
            messageBodyBocHex,
            arguments,
            envelopeBytes,
            envelopeHex,
        )

    operator fun component1(): String = envelopeEncoding
    operator fun component2(): ByteArray = messageBodyBoc
    operator fun component3(): String = messageBodyBocHex

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is TonSccpSubmission &&
            version == other.version &&
            envelopeEncoding == other.envelopeEncoding &&
            submissionKind == other.submissionKind &&
            verifierEntrypoint == other.verifierEntrypoint &&
            messageBodyBocStorage.contentEquals(other.messageBodyBocStorage) &&
            messageBodyBocHex == other.messageBodyBocHex &&
            arguments == other.arguments &&
            envelopeBytesStorage.contentEquals(other.envelopeBytesStorage) &&
            envelopeHex == other.envelopeHex

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + envelopeEncoding.hashCode()
        result = 31 * result + submissionKind.hashCode()
        result = 31 * result + verifierEntrypoint.hashCode()
        result = 31 * result + messageBodyBocStorage.contentHashCode()
        result = 31 * result + messageBodyBocHex.hashCode()
        result = 31 * result + arguments.hashCode()
        result = 31 * result + envelopeBytesStorage.contentHashCode()
        result = 31 * result + envelopeHex.hashCode()
        return result
    }

    override fun toString(): String =
        "TonSccpSubmission(version=$version, envelopeEncoding=$envelopeEncoding, " +
            "submissionKind=$submissionKind, verifierEntrypoint=$verifierEntrypoint, " +
            "messageBodyBoc=${messageBodyBocStorage.size} bytes, " +
            "messageBodyBocHex=$messageBodyBocHex, arguments=$arguments, " +
            "envelopeBytes=${envelopeBytesStorage.size} bytes, envelopeHex=$envelopeHex)"
}

/** Inputs used to build a local TON SCCP proof request. */
data class TonSccpProofRequestInput(
    val publicInputs: TonSccpPublicInputsInput,
    val bundleBytes: ByteArray,
    val sourceProofBytes: ByteArray = ByteArray(0),
    val statementHash: String,
    val destinationBindingHash: String,
    val sourceStateVerifierId: String = SccpTon.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
    val sourceStateVerifierHash: String = SccpSolana.ZERO_HASH_V1,
    val sourceAdapterDeploymentHash: String = SccpSolana.ZERO_HASH_V1,
    val sourceAdapterDeploymentReceiptHash: String = SccpSolana.ZERO_HASH_V1,
    val backend: String = SccpTon.CONTRACT_PROOF_BACKEND_V1,
    val sourceDomain: Int = SccpTon.DOMAIN_TON,
) {
    constructor(
        publicInputs: TonSccpPublicInputsInput,
        bundleBytes: ByteArray,
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String,
        destinationBindingHash: String,
        sourceStateVerifierId: String = SccpTon.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        sourceStateVerifierHash: String = SccpSolana.ZERO_HASH_V1,
        sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding,
        backend: String = SccpTon.CONTRACT_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpTon.DOMAIN_TON,
    ) : this(
        publicInputs = publicInputs,
        bundleBytes = bundleBytes,
        sourceProofBytes = sourceProofBytes,
        statementHash = statementHash,
        destinationBindingHash = destinationBindingHash,
        sourceStateVerifierId = sourceStateVerifierId,
        sourceStateVerifierHash = sourceStateVerifierHash,
        sourceAdapterDeploymentHash =
            checkedTonProofRequestDeploymentBinding(
                sourceAdapterDeploymentBinding,
                sourceDomain,
            ).sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash =
            checkedTonProofRequestDeploymentBinding(
                sourceAdapterDeploymentBinding,
                sourceDomain,
            ).sourceAdapterDeploymentReceiptHash,
        backend = backend,
        sourceDomain = sourceDomain,
    )
}

private fun checkedTonProofRequestDeploymentBinding(
    sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding,
    sourceDomain: Int,
): TonSccpSourceAdapterDeploymentBinding {
    val deploymentBinding = SccpSolana.normalizeSourceAdapterDeploymentBinding(
        sourceDomain = sourceAdapterDeploymentBinding.sourceDomain,
        targetDomain = sourceAdapterDeploymentBinding.targetDomain,
        sourceAdapterDeploymentHash = sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash =
            sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
    )
    require(deploymentBinding.sourceDomain == sourceDomain) {
        "sourceAdapterDeploymentBinding.sourceDomain must match sourceDomain"
    }
    require(deploymentBinding.sourceDomain == SccpTon.DOMAIN_TON) {
        "sourceAdapterDeploymentBinding.sourceDomain must be TON"
    }
    require(deploymentBinding.targetDomain == SccpSolana.DOMAIN_SORA) {
        "sourceAdapterDeploymentBinding.targetDomain must be SORA"
    }
    require(deploymentBinding.sourceAdapterDeploymentHash != SccpSolana.ZERO_HASH_V1) {
        "sourceAdapterDeploymentBinding must be non-zero"
    }
    return deploymentBinding
}

/** Statement and verifier deployment context proved by the local TON SCCP prover. */
data class TonSccpProofContext(
    val version: Int,
    val statementHash: String,
    val destinationBindingHash: String,
)

/** Source-adapter deployment binding carried by local TON SCCP proof requests. */
typealias TonSccpSourceAdapterDeploymentBinding = SolanaSccpSourceAdapterDeploymentBinding

/** Request passed to a linked local TON SCCP prover. */
class TonSccpProofRequest(
    val version: Int,
    val backend: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: TonSccpPublicInputsInput,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    sourceProofBytes: ByteArray,
    val proofContext: TonSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val sourceAdapterDeploymentBindingHash: String,
    val sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding,
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
        publicInputs: TonSccpPublicInputsInput = this.publicInputs,
        publicInputsBytes: ByteArray = this.publicInputsBytes,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: TonSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        sourceStateVerifierId: String = this.sourceStateVerifierId,
        sourceStateVerifierHash: String = this.sourceStateVerifierHash,
        sourceAdapterDeploymentBindingHash: String = this.sourceAdapterDeploymentBindingHash,
        sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding =
            this.sourceAdapterDeploymentBinding,
        requestHash: String = this.requestHash,
    ): TonSccpProofRequest =
        TonSccpProofRequest(
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
            sourceStateVerifierId,
            sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding,
            requestHash,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): Int = sourceDomain
    operator fun component4(): Int = targetDomain
    operator fun component5(): TonSccpPublicInputsInput = publicInputs
    operator fun component6(): ByteArray = publicInputsBytes
    operator fun component7(): ByteArray = bundleBytes
    operator fun component8(): ByteArray = sourceProofBytes
    operator fun component9(): TonSccpProofContext = proofContext
    operator fun component10(): String = statementHash
    operator fun component11(): String = destinationBindingHash
    operator fun component12(): String = sourceStateVerifierId
    operator fun component13(): String = sourceStateVerifierHash
    operator fun component14(): String = sourceAdapterDeploymentBindingHash
    operator fun component15(): TonSccpSourceAdapterDeploymentBinding = sourceAdapterDeploymentBinding
    operator fun component16(): String = requestHash

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is TonSccpProofRequest &&
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
            sourceStateVerifierId == other.sourceStateVerifierId &&
            sourceStateVerifierHash == other.sourceStateVerifierHash &&
            sourceAdapterDeploymentBindingHash == other.sourceAdapterDeploymentBindingHash &&
            sourceAdapterDeploymentBinding == other.sourceAdapterDeploymentBinding &&
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
        result = 31 * result + sourceStateVerifierId.hashCode()
        result = 31 * result + sourceStateVerifierHash.hashCode()
        result = 31 * result + sourceAdapterDeploymentBindingHash.hashCode()
        result = 31 * result + sourceAdapterDeploymentBinding.hashCode()
        result = 31 * result + requestHash.hashCode()
        return result
    }

    override fun toString(): String =
        "TonSccpProofRequest(version=$version, backend=$backend, sourceDomain=$sourceDomain, " +
            "targetDomain=$targetDomain, publicInputs=$publicInputs, " +
            "publicInputsBytes=${publicInputsBytesStorage.size} bytes, " +
            "bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, proofContext=$proofContext, " +
            "statementHash=$statementHash, destinationBindingHash=$destinationBindingHash, " +
            "sourceStateVerifierId=$sourceStateVerifierId, " +
            "sourceStateVerifierHash=$sourceStateVerifierHash, " +
            "sourceAdapterDeploymentBindingHash=$sourceAdapterDeploymentBindingHash, " +
            "sourceAdapterDeploymentBinding=$sourceAdapterDeploymentBinding, requestHash=$requestHash)"
}

/** TON live-account evidence collected by UI code before route canary submission. */
data class TonSccpRouteCanaryEvidenceInput(
    val routeAllowlistHash: String,
    val destinationBindingHash: String,
    val expectedDestinationBindingHash: String? = null,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val verifierContractAddress: String,
    val verifierCodeHash: String,
    val accountStatus: String = "active",
    val accountStateHash: String,
    val lastTransactionLt: String,
    val lastTransactionHash: String,
    val verifierCodeBocRootHash: String,
)

/** Proof bytes returned by a linked local TON SCCP prover. */
class TonSccpProofResult(
    val version: Int,
    val backend: String,
    proofBytes: ByteArray,
    val proofBase64: String,
    val publicInputs: TonSccpPublicInputsInput,
    bundleBytes: ByteArray = ByteArray(0),
    sourceProofBytes: ByteArray = ByteArray(0),
    val proofContext: TonSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val sourceAdapterDeploymentBindingHash: String,
    val sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding,
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
        publicInputs: TonSccpPublicInputsInput = this.publicInputs,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: TonSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        sourceStateVerifierId: String = this.sourceStateVerifierId,
        sourceStateVerifierHash: String = this.sourceStateVerifierHash,
        sourceAdapterDeploymentBindingHash: String = this.sourceAdapterDeploymentBindingHash,
        sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding =
            this.sourceAdapterDeploymentBinding,
        requestHash: String = this.requestHash,
        envelopeHash: String = this.envelopeHash,
    ): TonSccpProofResult =
        TonSccpProofResult(
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
            sourceStateVerifierId,
            sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding,
            requestHash,
            envelopeHash,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): ByteArray = proofBytes
    operator fun component4(): String = proofBase64
    operator fun component5(): TonSccpPublicInputsInput = publicInputs
    operator fun component6(): ByteArray = bundleBytes
    operator fun component7(): ByteArray = sourceProofBytes
    operator fun component8(): TonSccpProofContext = proofContext
    operator fun component9(): String = statementHash
    operator fun component10(): String = destinationBindingHash
    operator fun component11(): String = sourceStateVerifierId
    operator fun component12(): String = sourceStateVerifierHash
    operator fun component13(): String = sourceAdapterDeploymentBindingHash
    operator fun component14(): TonSccpSourceAdapterDeploymentBinding = sourceAdapterDeploymentBinding
    operator fun component15(): String = requestHash
    operator fun component16(): String = envelopeHash

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is TonSccpProofResult &&
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
            sourceStateVerifierId == other.sourceStateVerifierId &&
            sourceStateVerifierHash == other.sourceStateVerifierHash &&
            sourceAdapterDeploymentBindingHash == other.sourceAdapterDeploymentBindingHash &&
            sourceAdapterDeploymentBinding == other.sourceAdapterDeploymentBinding &&
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
        result = 31 * result + sourceStateVerifierId.hashCode()
        result = 31 * result + sourceStateVerifierHash.hashCode()
        result = 31 * result + sourceAdapterDeploymentBindingHash.hashCode()
        result = 31 * result + sourceAdapterDeploymentBinding.hashCode()
        result = 31 * result + requestHash.hashCode()
        result = 31 * result + envelopeHash.hashCode()
        return result
    }

    override fun toString(): String =
        "TonSccpProofResult(version=$version, backend=$backend, " +
            "proofBytes=${proofBytesStorage.size} bytes, proofBase64=$proofBase64, " +
            "publicInputs=$publicInputs, bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, proofContext=$proofContext, " +
            "statementHash=$statementHash, destinationBindingHash=$destinationBindingHash, " +
            "sourceStateVerifierId=$sourceStateVerifierId, " +
            "sourceStateVerifierHash=$sourceStateVerifierHash, " +
            "sourceAdapterDeploymentBindingHash=$sourceAdapterDeploymentBindingHash, " +
            "sourceAdapterDeploymentBinding=$sourceAdapterDeploymentBinding, " +
            "requestHash=$requestHash, envelopeHash=$envelopeHash)"
}

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
        SccpTon.buildProofRequest(witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input)

    fun prove(input: TonSccpProofRequestInput): TonSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine ?: throw IllegalStateException("TON SCCP local prover is not linked")
        SccpTon.requireProductionProofRequest(request)
        return SccpTon.wrapProofResult(engine.prove(SccpTon.callbackRequestSnapshot(request)), request)
    }

    private fun witnessProviderInputSnapshot(input: TonSccpProofRequestInput): TonSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )
}
