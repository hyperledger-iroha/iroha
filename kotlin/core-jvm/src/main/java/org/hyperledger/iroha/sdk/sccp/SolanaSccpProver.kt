package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.crypto.Blake3

/** Solana SCCP proof request helpers for local-first UI proof generation. */
object SccpSolana {
    const val DOMAIN_SORA: Int = 0
    const val DOMAIN_SOLANA: Int = 3
    const val RECURSIVE_PROOF_BACKEND_V1: String = "sccp-solana-recursive-mainnet-v1"
    const val ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1: String = "sccp-solana-accounts-lt-hash-v1"
    const val TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1: String = "sccp-solana-tower-replay-v1"
    const val FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1: String =
        "sccp-solana-full-accountsdb-lattice-v1"
    const val BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1: String =
        "sccp-solana-bank-fork-choice-v1"
    const val UPGRADEABLE_LOADER_ID: String = "BPFLoaderUpgradeab1e11111111111111111111111"
    const val MAINNET_GENESIS_HASH: String = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"
    const val MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1: String =
        "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1"
    const val SOURCE_STATE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val SOURCE_STATE_MAX_PROOF_LABEL_BYTES: Int = 128
    const val NATIVE_RECURSIVE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1: String =
        "0x6b4e4106bbb6b343ae1a4a36c9c68756d4454d2167c9b8b2ee3225e39fb0a48b"
    private val TEMPLATE_SOURCE_MATERIAL_HASHES_V1: Set<String> = setOf(
        "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
        "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
        "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
        TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
        "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56",
    )
    const val MAINNET_TOWER_REPLAY_VERIFIER_ID_V1: String =
        "sccp:sol:light-client:tower-replay-mainnet-beta:v1"
    const val MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1: String =
        "sccp:sol:light-client:full-accountsdb-lattice-mainnet-beta:v1"
    const val MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1: String =
        "sccp:sol:light-client:bank-fork-choice-mainnet-beta:v1"
    const val MAINNET_SLOTS_PER_EPOCH: Long = 432_000L
    const val TOWER_LOCKOUT_CONFIRMATION_DEPTH: Long = 32L
    const val TOWER_VOTE_STACK_DEPTH: Long = TOWER_LOCKOUT_CONFIRMATION_DEPTH - 1L
    const val TOWER_WARMUP_COOLDOWN_RATE_BPS: Long = 900L
    const val MAX_VALIDATORS: Int = 8_192
    const val MAX_SOURCE_MERKLE_BRANCH_NODES: Int = 64
    const val VOTE_PROGRAM_ID: String =
        "0x0761481d357474bb7c4d7624ebd3bdb3d8355e73d11043fc0da3538000000000"
    const val STAKE_PROGRAM_ID: String =
        "0x06a1d8179137542a983437bdfe2a7ab2557f535c8a78722b68a49dc000000000"
    const val SYSVAR_PROGRAM_ID: String =
        "0x06a7d5171875f729c73d93408f216120067ed88c76e08c287fc1946000000000"
    const val STAKE_HISTORY_SYSVAR_ID: String =
        "0x06a7d517193584d0feed9bb3431d13206be544281b57b8566cc5375ff4000000"
    const val BORSH_INSTRUCTION_V1: String = "borsh_instruction_v1"
    const val ZERO_HASH_V1: String = "0x0000000000000000000000000000000000000000000000000000000000000000"

    private const val SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1: String = "submit_sccp_message_proof"
    private const val ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX_V1: String =
        "sccp:solana:accounts-lt-proof-public-inputs:v1"
    private const val ACCOUNTS_LT_HASH_OPENED_CONTRIBUTIONS_PREFIX_V1: String =
        "sccp:solana:accounts-lt-opened-contributions:v1"
    private const val MAINNET_GENESIS_HASH_PREFIX_V1: String =
        "sccp:solana:mainnet-genesis:v1"
    private const val BANK_HASH_HARD_FORK_DATA_PREFIX_V1: String =
        "sccp:solana:bank-hash-hard-fork-data:v1"
    private const val ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX_V1: String =
        "sccp:solana:accounts-lt:fastpq:dsid:v1"
    private const val ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1: String = "fastpq-lane-balanced"
    private const val ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY_V1: String =
        "sccp:solana:accounts-lt:v1:statement"
    private const val ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY_V1: String =
        "sccp:solana:accounts-lt:v1:accounts"
    private const val ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY_V1: String =
        "sccp:solana:accounts-lt:v1:opened-contributions"
    private const val ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY_V1: String =
        "sccp:solana:accounts-lt:v1:residual"
    private const val ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY_V1: String =
        "sccp:solana:accounts-lt:v1:context"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1: String =
        "sccp:solana:full-light-client-audit:fastpq:dsid:v1"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1: String =
        "fastpq-lane-balanced"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1: String =
        "sccp:solana:full-light-client-audit:v1:statement"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1: String =
        "sccp:solana:full-light-client-audit:v1:context"
    private const val FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1: String =
        "sccp:solana:full-light-client-audit:v1:gate"
    private const val FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1: String =
        "sccp:solana:full-light-client-audit:statement:v1"
    private const val SOLANA_SOURCE_CHAIN_KEY_V1: String = "sol"
    private const val SOLANA_SOURCE_PROOF_PLAN_CODE_V1: Int = 3
    private const val SOLANA_FINALITY_MODEL_CODE_V1: Int = 3
    private const val SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG: Int = 2
    private const val SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG: Int = 3
    private const val SOLANA_PROGRAMDATA_METADATA_LEN: Int = 45
    private const val SOLANA_ROUTE_CANARY_LIVE_PROGRAM_PREFIX_V1: String =
        "iroha:sccp:solana-route-canary-live-program:v1"
    private const val TRANSACTION_SIGNATURE_BYTES: Int = 64
    private const val PROGRAM_ID_BYTES: Int = 32
    private const val STAKE_STATE_V2_STAKE_ACCOUNT_DATA_LEN: Int = 200
    private const val VOTE_STATE_ACCOUNT_DATA_LEN: Int = 3_762
    private const val MAX_ACCOUNT_RAW_DATA_BYTES: Int = 65_536
    private const val ACCOUNTS_LT_HASH_BYTES: Int = 2_048
    private const val LT_HASH_ELEMENTS: Int = 1_024
    private const val OPENED_LT_HASH_ROLE_VOTE: Int = 1
    private const val OPENED_LT_HASH_ROLE_STAKE: Int = 2
    private const val OPENED_LT_HASH_ROLE_STAKE_HISTORY_SYSVAR: Int = 3
    private const val MAX_BANK_HARD_FORK_HASH_DATA_BYTES: Int = 1_024
    private const val BLS_PUBLIC_KEY_COMPRESSED_LEN: Int = 48
    private const val VOTE_STATE_V1_14_11_DISCRIMINANT: Int = 1
    private const val VOTE_STATE_V3_DISCRIMINANT: Int = 2
    private const val VOTE_STATE_V4_DISCRIMINANT: Int = 3
    private const val VOTE_STATE_PRIOR_VOTERS: Int = 32
    private const val VOTE_STATE_V4_AUTHORIZED_VOTERS: Int = 4
    private const val VOTE_STATE_MAX_EPOCH_CREDITS: Int = 64
    private const val STAKE_STATE_V2_STAKE_DISCRIMINANT: Int = 2
    private const val STAKE_STATE_V2_STAKER_OFFSET: Int = 12
    private const val STAKE_STATE_V2_WITHDRAWER_OFFSET: Int = 44
    private const val STAKE_STATE_V2_VOTER_PUBKEY_OFFSET: Int = 124
    private const val STAKE_STATE_V2_DELEGATED_STAKE_OFFSET: Int = 156
    private const val STAKE_STATE_V2_ACTIVATION_EPOCH_OFFSET: Int = 164
    private const val STAKE_STATE_V2_DEACTIVATION_EPOCH_OFFSET: Int = 172
    private const val STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_OFFSET: Int = 180
    private const val STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_BYTES: Int = 8
    private val STAKE_STATE_V2_LEGACY_WARMUP_COOLDOWN_RATE_BYTES: ByteArray =
        byteArrayOf(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0.toByte(), 0x3f)
    private val STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES: ByteArray =
        byteArrayOf(0x0a, 0xd7.toByte(), 0xa3.toByte(), 0x70, 0x3d, 0x0a, 0xb7.toByte(), 0x3f)
    private const val STAKE_STATE_V2_CREDITS_OBSERVED_OFFSET: Int = 188
    private const val STAKE_STATE_V2_FLAG_OFFSET: Int = 196
    private const val STAKE_STATE_V2_KNOWN_FLAGS_MASK: Int = 0b0000_0001
    private const val BASE58_ALPHABET: String =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    private val BASE_58: BigInteger = BigInteger.valueOf(58L)
    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val WARMUP_COOLDOWN_RATE_BPS: BigInteger = BigInteger.valueOf(TOWER_WARMUP_COOLDOWN_RATE_BPS)
    private val BASIS_POINTS_PER_UNIT: BigInteger = BigInteger.valueOf(10_000L)
    private val SOLANA_BPF_ELF_MAGIC: ByteArray = byteArrayOf(0x7f, 0x45, 0x4c, 0x46)
    private val BASE58_INDEX: IntArray = IntArray(128) { -1 }.also { index ->
        BASE58_ALPHABET.forEachIndexed { position, symbol -> index[symbol.code] = position }
    }

    @JvmStatic
    fun canonicalRouteCanaryEvidenceBytes(input: SolanaSccpRouteCanaryEvidenceInput): ByteArray {
        val routeAllowlistHash = nonZeroHex32Bytes(input.routeAllowlistHash, "routeAllowlistHash")
        val destinationBindingHash = nonZeroHex32Bytes(input.destinationBindingHash, "destinationBindingHash")
        val canonicalSolanaDestinationBindingHash = SccpSourceProofs.destinationBindingHash(DOMAIN_SOLANA)
        val expectedDestinationBindingHash = normalizeNonZeroHex32(
            input.expectedDestinationBindingHash ?: canonicalSolanaDestinationBindingHash,
            "expectedDestinationBindingHash",
        )
        require(expectedDestinationBindingHash == canonicalSolanaDestinationBindingHash) {
            "expectedDestinationBindingHash must match canonical Solana destination binding"
        }
        require("0x" + hexLower(destinationBindingHash) == canonicalSolanaDestinationBindingHash) {
            "destinationBindingHash must match canonical Solana destination binding"
        }
        val sourceVerifierMaterialHash =
            nonZeroHex32Bytes(input.sourceVerifierMaterialHash, "sourceVerifierMaterialHash")
        val sourceAdapterEngineDeploymentHash = nonZeroHex32Bytes(
            input.sourceAdapterEngineDeploymentHash,
            "sourceAdapterEngineDeploymentHash",
        )
        requireHashRolesDistinct(
            "Solana route canary governed hashes",
            listOf(
                "routeAllowlistHash" to routeAllowlistHash,
                "destinationBindingHash" to destinationBindingHash,
                "sourceVerifierMaterialHash" to sourceVerifierMaterialHash,
                "sourceAdapterEngineDeploymentHash" to sourceAdapterEngineDeploymentHash,
            ),
        )
        val evidence = normalizeRouteCanaryProgramDataEvidence(input)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, DOMAIN_SORA)
        writeU32Le(out, DOMAIN_SOLANA)
        out.write(routeAllowlistHash)
        out.write(destinationBindingHash)
        out.write(sourceVerifierMaterialHash)
        out.write(sourceAdapterEngineDeploymentHash)
        out.write(evidence.verifierProgram)
        out.write(hex32Bytes(evidence.verifierCodeHash, "verifierCodeHash"))
        writeVec(out, evidence.rpcCommitment.toByteArray(StandardCharsets.UTF_8))
        writeVec(out, evidence.programOwner.toByteArray(StandardCharsets.UTF_8))
        writeVec(out, evidence.programdataOwner.toByteArray(StandardCharsets.UTF_8))
        out.write(1)
        writeVec(out, evidence.programAccountData)
        out.write(evidence.programdataAddress)
        writeU64Le(out, evidence.programdataSlot)
        writeU64Le(out, evidence.expectedProgramdataSlot)
        writeU64Le(out, evidence.programAccountContextSlot)
        writeU64Le(out, evidence.programdataAccountContextSlot)
        writeVec(out, evidence.programdataMetadata)
        writeVec(out, evidence.programdataExecutable)
        return out.toByteArray()
    }

    @JvmStatic
    fun routeCanaryEvidenceHash(input: SolanaSccpRouteCanaryEvidenceInput): String =
        "0x" + hexLower(
            hashBytes(
                SOLANA_ROUTE_CANARY_LIVE_PROGRAM_PREFIX_V1,
                canonicalRouteCanaryEvidenceBytes(input),
            ),
        )

    @JvmStatic
    fun normalizeWitness(input: SolanaSccpWitnessInput): SolanaSccpWitness {
        require(input.targetDomain == DOMAIN_SORA) { "targetDomain must be SORA" }
        val finalizedSlot = normalizeU64(input.finalizedSlot, "finalizedSlot")
        val parentSlot = normalizeU64(input.parentSlot, "parentSlot")
        require(parentSlot.add(BigInteger.ONE) == finalizedSlot) {
            "parentSlot must be the direct parent of finalizedSlot"
        }
        val bankSignatureCount = normalizeU64(input.bankSignatureCount, "bankSignatureCount")
        require(bankSignatureCount != BigInteger.ZERO) { "bankSignatureCount must be nonzero" }
        val parentBankHash = normalizeNonZeroHex32(input.parentBankHash, "parentBankHash")
        val bankHash = normalizeNonZeroHex32(input.bankHash, "bankHash")
        val blockhashBytes = solanaHash32Bytes(input.blockhash, "blockhash")
        val transactionStatusRoot = normalizeNonZeroHex32(input.transactionStatusRoot, "transactionStatusRoot")
        val sourceEventDigest = normalizeNonZeroHex32(input.sourceEventDigest, "sourceEventDigest")
        val transactionSignature =
            normalizeSolanaBase58Fixed(input.transactionSignature, "transactionSignature", TRANSACTION_SIGNATURE_BYTES)
        val emitterProgramId =
            normalizeSolanaBase58Fixed(input.emitterProgramId, "emitterProgramId", PROGRAM_ID_BYTES)
        val inclusionBranch = normalizeInclusionBranch(input.inclusionBranch)
        if (inclusionBranch.isNotEmpty()) {
            val derivedTransactionStatusRoot = transactionStatusRootFromBranch(
                sourceEventDigest,
                transactionSignature,
                emitterProgramId,
                inclusionBranch,
            )
            require(derivedTransactionStatusRoot == transactionStatusRoot) {
                "transactionStatusRoot must match inclusionBranch"
            }
        }
        val messageProofHash = normalizeMessageProofHash(
            input.messageProofHash,
            sourceEventDigest,
            transactionStatusRoot,
            transactionSignature,
            emitterProgramId,
            inclusionBranch,
        )
        val accountInclusionRoot = normalizeNonZeroHex32(input.accountInclusionRoot, "accountInclusionRoot")
        val accountsLtHashChecksum = normalizeNonZeroHex32(input.accountsLtHashChecksum, "accountsLtHashChecksum")
        val accountsLtHashChecksumBytes = hex32Bytes(accountsLtHashChecksum, "accountsLtHashChecksum")
        require(input.bankHashHardForkData.size <= MAX_BANK_HARD_FORK_HASH_DATA_BYTES) {
            "bankHashHardForkData is too large"
        }
        if (input.accountsLtHash != null) {
            val expectedBankHash = "0x" + hexLower(
                agaveBankHashBytes(
                    hex32Bytes(parentBankHash, "parentBankHash"),
                    bankSignatureCount,
                    blockhashBytes,
                    input.accountsLtHash,
                    input.bankHashHardForkData,
                ),
            )
            require(bankHash == expectedBankHash) { "bankHash must match Agave bank hash inputs" }
            require(Blake3.hash(input.accountsLtHash).contentEquals(accountsLtHashChecksumBytes)) {
                "accountsLtHashChecksum must match accountsLtHash"
            }
        }
        val accountsLtHashProofPublicInputsHash = accountsLtHashProofPublicInputsHash(
            finalizedSlot = finalizedSlot.toString(),
            parentSlot = parentSlot.toString(),
            bankSignatureCount = bankSignatureCount.toString(),
            parentBankHash = parentBankHash,
            bankHash = bankHash,
            blockhash = "0x" + hexLower(blockhashBytes),
            bankHashHardForkData = input.bankHashHardForkData,
            transactionStatusRoot = transactionStatusRoot,
            accountInclusionRoot = accountInclusionRoot,
            accountsLtHashChecksum = accountsLtHashChecksum,
            accountsLtHash = input.accountsLtHash,
        )
        if (input.accountsLtHashProofPublicInputsHash != null) {
            require(
                normalizeHex32(
                    input.accountsLtHashProofPublicInputsHash,
                    "accountsLtHashProofPublicInputsHash",
                ) == accountsLtHashProofPublicInputsHash,
            ) {
                "accountsLtHashProofPublicInputsHash must match bank-state inputs"
            }
        }
        val deploymentBinding = normalizeSourceAdapterDeploymentBinding(
            sourceDomain = DOMAIN_SOLANA,
            targetDomain = input.targetDomain,
            sourceAdapterDeploymentHash = input.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash = input.sourceAdapterDeploymentReceiptHash,
        )
        val sourceStateVerifierId = normalizeNonEmpty(input.sourceStateVerifierId, "sourceStateVerifierId")
        val sourceStateVerifierHash = normalizeHex32(input.sourceStateVerifierHash, "sourceStateVerifierHash")
        require(
            sourceStateVerifierHash == ZERO_HASH_V1 ||
                sourceStateVerifierId == MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
        ) {
            "sourceStateVerifierId must match Solana AccountsDB verifier profile"
        }
        return SolanaSccpWitness(
            version = 1,
            sourceDomain = DOMAIN_SOLANA,
            targetDomain = input.targetDomain,
            mainnetGenesisHash = normalizeNonEmpty(input.mainnetGenesisHash, "mainnetGenesisHash"),
            finalizedSlot = finalizedSlot.toString(),
            parentSlot = parentSlot.toString(),
            bankSignatureCount = bankSignatureCount.toString(),
            parentBankHash = parentBankHash,
            blockhash = "0x" + hexLower(blockhashBytes),
            bankHash = bankHash,
            transactionStatusRoot = transactionStatusRoot,
            messageProofHash = messageProofHash,
            accountInclusionRoot = accountInclusionRoot,
            accountsLtHashChecksum = accountsLtHashChecksum,
            accountsLtHashProofPublicInputsHash = accountsLtHashProofPublicInputsHash,
            bankHashHardForkData = input.bankHashHardForkData.copyOf(),
            accountsLtHash = input.accountsLtHash?.copyOf(),
            transactionSignature = transactionSignature,
            emitterProgramId = emitterProgramId,
            messageId = normalizeHex32(input.messageId, "messageId"),
            payloadHash = normalizeHex32(input.payloadHash, "payloadHash"),
            commitmentRoot = normalizeHex32(input.commitmentRoot, "commitmentRoot"),
            sourceEventDigest = sourceEventDigest,
            sourceStateVerifierId = sourceStateVerifierId,
            sourceStateVerifierHash = sourceStateVerifierHash,
            sourceAdapterDeploymentHash = deploymentBinding.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash = deploymentBinding.sourceAdapterDeploymentReceiptHash,
            inclusionBranch = inclusionBranch,
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
        writeU64Le(out, normalizeU64(witness.parentSlot, "parentSlot"))
        writeU64Le(out, normalizeU64(witness.bankSignatureCount, "bankSignatureCount"))
        out.write(solanaHash32Bytes(witness.blockhash, "blockhash"))
        writeString(out, witness.transactionSignature, "transactionSignature")
        writeString(out, witness.emitterProgramId, "emitterProgramId")
        out.write(hex32Bytes(witness.parentBankHash, "parentBankHash"))
        out.write(hex32Bytes(witness.bankHash, "bankHash"))
        out.write(hex32Bytes(witness.transactionStatusRoot, "transactionStatusRoot"))
        out.write(hex32Bytes(witness.messageProofHash, "messageProofHash"))
        out.write(hex32Bytes(witness.accountInclusionRoot, "accountInclusionRoot"))
        out.write(hex32Bytes(witness.accountsLtHashChecksum, "accountsLtHashChecksum"))
        out.write(hex32Bytes(witness.accountsLtHashProofPublicInputsHash, "accountsLtHashProofPublicInputsHash"))
        writeVec(out, witness.bankHashHardForkData)
        writeVec(out, witness.accountsLtHash ?: ByteArray(0))
        out.write(hex32Bytes(witness.messageId, "messageId"))
        out.write(hex32Bytes(witness.payloadHash, "payloadHash"))
        out.write(hex32Bytes(witness.commitmentRoot, "commitmentRoot"))
        out.write(hex32Bytes(witness.sourceEventDigest, "sourceEventDigest"))
        writeString(out, witness.sourceStateVerifierId, "sourceStateVerifierId")
        out.write(hex32Bytes(witness.sourceStateVerifierHash, "sourceStateVerifierHash"))
        out.write(hex32Bytes(witness.sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"))
        out.write(hex32Bytes(witness.sourceAdapterDeploymentReceiptHash, "sourceAdapterDeploymentReceiptHash"))
        writeU32Le(out, witness.inclusionBranch.size)
        witness.inclusionBranch.forEachIndexed { index, sibling ->
            require(sibling.size == 32) { "inclusionBranch[$index] must be 32 bytes" }
            out.write(sibling)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun canonicalMessageProofBytes(
        sourceEventDigest: String,
        transactionStatusRoot: String,
        transactionSignature: String,
        emitterProgramId: String,
        inclusionBranch: List<ByteArray>,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        val sourceEventDigestBytes = hex32Bytes(sourceEventDigest, "sourceEventDigest")
        require(sourceEventDigestBytes.any { it.toInt() != 0 }) {
            "sourceEventDigest must not be zero"
        }
        val transactionStatusRootBytes = hex32Bytes(transactionStatusRoot, "transactionStatusRoot")
        require(transactionStatusRootBytes.any { it.toInt() != 0 }) {
            "transactionStatusRoot must not be zero"
        }
        out.write(sourceEventDigestBytes)
        out.write(transactionStatusRootBytes)
        writeVec(out, decodeSolanaBase58Fixed(transactionSignature, "transactionSignature", TRANSACTION_SIGNATURE_BYTES))
        writeVec(out, decodeSolanaBase58Fixed(emitterProgramId, "emitterProgramId", PROGRAM_ID_BYTES))
        val normalizedInclusionBranch = normalizeInclusionBranch(inclusionBranch)
        require(normalizedInclusionBranch.isNotEmpty()) { "inclusionBranch must not be empty" }
        writeU32Le(out, normalizedInclusionBranch.size)
        normalizedInclusionBranch.forEach { sibling ->
            out.write(sibling)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun messageProofHash(
        sourceEventDigest: String,
        transactionStatusRoot: String,
        transactionSignature: String,
        emitterProgramId: String,
        inclusionBranch: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:solana:message-proof:v1",
            canonicalMessageProofBytes(
                sourceEventDigest,
                transactionStatusRoot,
                transactionSignature,
                emitterProgramId,
                inclusionBranch,
            ),
        )

    @JvmStatic
    fun canonicalTransactionStatusLeafBytes(
        sourceEventDigest: String,
        transactionSignature: String,
        emitterProgramId: String,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"))
        writeVec(out, decodeSolanaBase58Fixed(transactionSignature, "transactionSignature", TRANSACTION_SIGNATURE_BYTES))
        writeVec(out, decodeSolanaBase58Fixed(emitterProgramId, "emitterProgramId", PROGRAM_ID_BYTES))
        return out.toByteArray()
    }

    @JvmStatic
    fun transactionStatusLeafHash(
        sourceEventDigest: String,
        transactionSignature: String,
        emitterProgramId: String,
    ): String =
        hashHex(
            "sccp:solana:transaction-status-leaf:v1",
            canonicalTransactionStatusLeafBytes(sourceEventDigest, transactionSignature, emitterProgramId),
        )

    @JvmStatic
    fun transactionStatusRootFromBranch(
        sourceEventDigest: String,
        transactionSignature: String,
        emitterProgramId: String,
        inclusionBranch: List<ByteArray>,
    ): String {
        val normalizedInclusionBranch = normalizeInclusionBranch(inclusionBranch)
        require(normalizedInclusionBranch.isNotEmpty()) { "inclusionBranch must not be empty" }
        var current = hex32Bytes(
            transactionStatusLeafHash(sourceEventDigest, transactionSignature, emitterProgramId),
            "transactionStatusLeafHash",
        )
        normalizedInclusionBranch.forEach { sibling ->
            current = sourceMerkleNodeHash(current, sibling)
        }
        return "0x" + hexLower(current)
    }

    @JvmStatic
    fun mainnetEpochForSlot(slot: String): String =
        normalizeU64(slot, "slot")
            .divide(BigInteger.valueOf(MAINNET_SLOTS_PER_EPOCH))
            .toString()

    @JvmStatic
    fun canonicalEpochStakeRootBytes(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorStakes: List<String>,
    ): ByteArray {
        val rosterBytes = canonicalVoteRosterBytes(validatorPublicKeys, validatorStakes)
        val rosterHash = hashBytes("sccp:solana:vote-roster:v1", rosterBytes)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, normalizeU64(epoch, "epoch"))
        out.write(rosterHash)
        out.write(rosterBytes)
        return out.toByteArray()
    }

    @JvmStatic
    fun epochStakeRoot(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorStakes: List<String>,
    ): String =
        hashHex(
            "sccp:solana:epoch-stake-root:v1",
            canonicalEpochStakeRootBytes(epoch, validatorPublicKeys, validatorStakes),
        )

    @JvmStatic
    fun canonicalStakeActivationBytes(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorStakes: List<String>,
        validatorActivationEpochs: List<String>,
        validatorDeactivationEpochs: List<String>,
    ): ByteArray {
        require(validatorActivationEpochs.size == validatorPublicKeys.size) {
            "validatorActivationEpochs must match validatorPublicKeys"
        }
        require(validatorDeactivationEpochs.size == validatorPublicKeys.size) {
            "validatorDeactivationEpochs must match validatorPublicKeys"
        }
        val resolvedEpoch = normalizeU64(epoch, "epoch")
        val rosterBytes = canonicalVoteRosterBytes(validatorPublicKeys, validatorStakes)
        val rosterHash = hashBytes("sccp:solana:vote-roster:v1", rosterBytes)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, resolvedEpoch)
        out.write(rosterHash)
        writeU32Le(out, validatorPublicKeys.size)
        validatorPublicKeys.forEachIndexed { index, publicKey ->
            val activationEpoch = normalizeU64(validatorActivationEpochs[index], "validatorActivationEpochs[$index]")
            val deactivationEpoch = normalizeU64(validatorDeactivationEpochs[index], "validatorDeactivationEpochs[$index]")
            require(activationEpoch < resolvedEpoch) { "validatorActivationEpochs[$index] must be active at epoch" }
            require(deactivationEpoch > activationEpoch) {
                "validatorDeactivationEpochs[$index] must be greater than activation epoch"
            }
            writeVec(out, publicKey)
            writeU64Le(out, normalizeU64(validatorStakes[index], "validatorStakes[$index]"))
            writeU64Le(out, activationEpoch)
            writeU64Le(out, deactivationEpoch)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun stakeActivationHash(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorStakes: List<String>,
        validatorActivationEpochs: List<String>,
        validatorDeactivationEpochs: List<String>,
    ): String =
        hashHex(
            "sccp:solana:stake-activation:v1",
            canonicalStakeActivationBytes(
                epoch,
                validatorPublicKeys,
                validatorStakes,
                validatorActivationEpochs,
                validatorDeactivationEpochs,
            ),
        )

    @JvmStatic
    fun canonicalAccountOpeningBytes(
        address: ByteArray,
        owner: ByteArray,
        lamports: String,
        rentEpoch: String,
        executable: Boolean,
        dataHash: String,
    ): ByteArray {
        require(address.size == 32 && address.any { it.toInt() != 0 }) {
            "address must be a non-zero 32-byte Solana account id"
        }
        require(owner.size == 32 && owner.any { it.toInt() != 0 }) {
            "owner must be a non-zero 32-byte Solana program id"
        }
        val normalizedLamports = normalizeU64(lamports, "lamports")
        require(normalizedLamports > BigInteger.ZERO) { "lamports must be greater than zero" }
        val normalizedRentEpoch = normalizeU64(rentEpoch, "rentEpoch")
        val dataHashBytes = hex32Bytes(dataHash, "dataHash")
        require(dataHashBytes.any { it.toInt() != 0 }) { "dataHash must not be zero" }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeVec(out, address)
        writeVec(out, owner)
        writeU64Le(out, normalizedLamports)
        writeU64Le(out, normalizedRentEpoch)
        out.write(if (executable) 1 else 0)
        out.write(dataHashBytes)
        return out.toByteArray()
    }

    @JvmStatic
    fun accountOpeningHash(
        address: ByteArray,
        owner: ByteArray,
        lamports: String,
        rentEpoch: String,
        executable: Boolean,
        dataHash: String,
    ): String =
        hashHex(
            "sccp:solana:account-opening:v1",
            canonicalAccountOpeningBytes(address, owner, lamports, rentEpoch, executable, dataHash),
        )

    @JvmStatic
    fun accountRawDataHash(rawData: ByteArray): String {
        require(rawData.isNotEmpty() && rawData.size <= MAX_ACCOUNT_RAW_DATA_BYTES) {
            "rawData must be between 1 and 65536 bytes"
        }
        return hashHex("sccp:solana:account-raw-data:v1", rawData)
    }

    @JvmStatic
    fun accountsLtHashChecksum(accountsLtHash: ByteArray): String {
        require(accountsLtHash.size == ACCOUNTS_LT_HASH_BYTES) { "accountsLtHash must be 2048 bytes" }
        return "0x" + hexLower(Blake3.hash(accountsLtHash))
    }

    @JvmStatic
    fun accountLtHash(
        opening: SolanaSccpAccountOpeningInput,
        rawData: ByteArray,
    ): ByteArray {
        require(opening.address.size == 32) { "address must be 32 bytes" }
        require(opening.owner.size == 32) { "owner must be 32 bytes" }
        require(rawData.size <= MAX_ACCOUNT_RAW_DATA_BYTES) { "rawData must be at most 65536 bytes" }
        val lamports = normalizeU64(opening.lamports, "lamports")
        if (lamports == BigInteger.ZERO) {
            return ByteArray(ACCOUNTS_LT_HASH_BYTES)
        }
        val preimage = ByteArrayOutputStream()
        writeU64Le(preimage, lamports)
        preimage.write(rawData)
        preimage.write(if (opening.executable) 1 else 0)
        preimage.write(opening.owner)
        preimage.write(opening.address)
        return Blake3.derive(preimage.toByteArray(), ACCOUNTS_LT_HASH_BYTES)
    }

    @JvmStatic
    fun accountsLtHashFromOpenings(
        openings: List<SolanaSccpAccountOpeningInput>,
        rawDataValues: List<ByteArray>,
    ): ByteArray {
        require(openings.size == rawDataValues.size) {
            "openings and rawDataValues must have matching lengths"
        }
        val out = ByteArray(ACCOUNTS_LT_HASH_BYTES)
        openings.indices.forEach { index ->
            addAccountsLtHashContribution(out, accountLtHash(openings[index], rawDataValues[index]))
        }
        return out
    }

    @JvmStatic
    fun openedAccountsLtHashResidual(input: SolanaSccpOpenedAccountsLtHashContributionsInput): ByteArray =
        normalizeOpenedAccountsLtHashContributions(input).residualAccountsLtHash.copyOf()

    @JvmStatic
    fun openedAccountsLtHashResidualChecksum(
        input: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): String = "0x" + hexLower(normalizeOpenedAccountsLtHashContributions(input).residualAccountsLtHashChecksum)

    @JvmStatic
    fun canonicalOpenedAccountsLtHashContributionsBytes(
        input: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): ByteArray {
        val normalized = normalizeOpenedAccountsLtHashContributions(input)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalized.sourceDomain)
        writeU64Le(out, normalized.finalizedSlot)
        out.write(normalized.accountInclusionRoot)
        out.write(normalized.accountsLtHashChecksum)
        out.write(normalized.openedAccountsLtHashChecksum)
        out.write(normalized.residualAccountsLtHashChecksum)
        writeVec(out, normalized.openedAccountsLtHash)
        writeVec(out, normalized.residualAccountsLtHash)
        writeU32Le(out, normalized.rows.size)
        normalized.rows.forEach { row ->
            out.write(row.role)
            out.write(row.address)
            out.write(row.accountHash)
            out.write(row.rawDataHash)
            writeVec(out, row.accountLtHash)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun openedAccountsLtHashContributionsHash(
        input: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): String =
        hashHex(
            ACCOUNTS_LT_HASH_OPENED_CONTRIBUTIONS_PREFIX_V1,
            canonicalOpenedAccountsLtHashContributionsBytes(input),
        )

    @JvmStatic
    fun canonicalAccountsLtHashCommitmentBytes(
        witnessInput: SolanaSccpWitnessInput,
        openedInput: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): ByteArray {
        val normalized = normalizeAccountsLtHashProofRequest(witnessInput, openedInput)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalized.witness.sourceDomain)
        writeU64Le(out, normalizeU64(normalized.witness.finalizedSlot, "finalizedSlot"))
        out.write(hex32Bytes(normalized.witness.accountsLtHashChecksum, "accountsLtHashChecksum"))
        out.write(hex32Bytes(normalized.openedContributionsHash, "openedAccountsLtHashContributionsHash"))
        out.write(hex32Bytes(normalized.residualChecksum, "openedAccountsLtHashResidualChecksum"))
        writeVec(out, normalized.accountsLtHash)
        return out.toByteArray()
    }

    @JvmStatic
    fun canonicalAccountsLtHashVerificationContextBytes(
        witnessInput: SolanaSccpWitnessInput,
        openedInput: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): ByteArray {
        val normalized = normalizeAccountsLtHashProofRequest(witnessInput, openedInput)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeString(out, ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1, "circuitId")
        writeString(out, ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1, "parameterSet")
        writeString(out, normalized.witness.sourceStateVerifierId, "sourceStateVerifierId")
        out.write(hex32Bytes(normalized.witness.sourceStateVerifierHash, "sourceStateVerifierHash"))
        out.write(
            hex32Bytes(
                normalized.witness.accountsLtHashProofPublicInputsHash,
                "accountsLtHashProofPublicInputsHash",
            ),
        )
        out.write(hex32Bytes(normalized.openedContributionsHash, "openedAccountsLtHashContributionsHash"))
        out.write(hex32Bytes(normalized.residualChecksum, "openedAccountsLtHashResidualChecksum"))
        return out.toByteArray()
    }

    @JvmStatic
    fun accountsLtHashPublicInputColumns(
        witnessInput: SolanaSccpWitnessInput,
        openedInput: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): List<List<String>> {
        val normalized = normalizeAccountsLtHashProofRequest(witnessInput, openedInput)
        val witness = normalized.witness
        return listOf(
            listOf("0x" + hexLower(sccpWordU32Le(witness.sourceDomain))),
            listOf(solanaMainnetGenesisHashPublicInput()),
            listOf("0x" + hexLower(sccpWordU64Le(normalizeU64(witness.finalizedSlot, "finalizedSlot")))),
            listOf("0x" + hexLower(sccpWordU64Le(normalizeU64(witness.parentSlot, "parentSlot")))),
            listOf("0x" + hexLower(sccpWordU64Le(normalizeU64(witness.bankSignatureCount, "bankSignatureCount")))),
            listOf(witness.parentBankHash),
            listOf(witness.bankHash),
            listOf("0x" + hexLower(solanaHash32Bytes(witness.blockhash, "blockhash"))),
            listOf(witness.transactionStatusRoot),
            listOf(witness.accountInclusionRoot),
            listOf(witness.accountsLtHashChecksum),
            listOf(witness.accountsLtHashProofPublicInputsHash),
            listOf(normalized.openedContributionsHash),
            listOf(normalized.residualChecksum),
        )
    }

    @JvmStatic
    fun accountsLtHashOpenVerifySchemaDescriptor(
        witnessInput: SolanaSccpWitnessInput,
        openedInput: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): ByteArray {
        val normalized = normalizeAccountsLtHashProofRequest(witnessInput, openedInput)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeString(out, ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1, "circuitId")
        writeString(out, ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1, "parameterSet")
        writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash")
        writeU32Le(out, normalized.witness.sourceDomain)
        writeString(out, "source_state_verifier_id", "schemaField")
        writeString(out, normalized.witness.sourceStateVerifierId, "sourceStateVerifierId")
        writeString(out, "source_state_verifier_hash", "schemaField")
        out.write(hex32Bytes(normalized.witness.sourceStateVerifierHash, "sourceStateVerifierHash"))
        listOf(
            "source_domain",
            "mainnet_genesis_hash",
            "finalized_slot",
            "parent_slot",
            "bank_signature_count",
            "parent_bank_hash",
            "bank_hash",
            "blockhash",
            "transaction_status_root",
            "account_inclusion_root",
            "accounts_lt_hash_checksum",
            "accounts_lt_hash_proof_public_inputs_hash",
            "opened_accounts_lt_hash_contributions_hash",
            "opened_accounts_lt_hash_residual_checksum",
        ).forEach { writeString(out, it, "requiredInput") }
        return out.toByteArray()
    }

    @JvmStatic
    fun buildAccountsLtHashProofRequest(
        witnessInput: SolanaSccpWitnessInput,
        openedInput: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): SolanaSccpAccountsLtHashProofRequest {
        val normalized = normalizeAccountsLtHashProofRequest(witnessInput, openedInput)
        val witness = normalized.witness
        val statementBytes = canonicalAccountsLtHashProofPublicInputsBytes(
            witness.finalizedSlot,
            witness.parentSlot,
            witness.bankSignatureCount,
            witness.parentBankHash,
            witness.bankHash,
            witness.blockhash,
            witness.bankHashHardForkData,
            witness.transactionStatusRoot,
            witness.accountInclusionRoot,
            witness.accountsLtHashChecksum,
            witness.sourceDomain,
            witness.accountsLtHash,
        )
        val accountCommitmentBytes = canonicalAccountsLtHashCommitmentBytes(witnessInput, openedInput)
        val verificationContextBytes = canonicalAccountsLtHashVerificationContextBytes(witnessInput, openedInput)
        val publicInputsHashBytes = hex32Bytes(
            witness.accountsLtHashProofPublicInputsHash,
            "accountsLtHashProofPublicInputsHash",
        )
        val dsidHash = hashBytes(ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX_V1, publicInputsHashBytes)
        val transitions = listOf(
            SolanaSccpAccountsLtHashFastpqTransition(
                ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY_V1,
                "meta_set",
                ByteArray(0),
                statementBytes,
            ),
            SolanaSccpAccountsLtHashFastpqTransition(
                ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY_V1,
                "meta_set",
                ByteArray(0),
                accountCommitmentBytes,
            ),
            SolanaSccpAccountsLtHashFastpqTransition(
                ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY_V1,
                "meta_set",
                ByteArray(0),
                hex32Bytes(normalized.openedContributionsHash, "openedAccountsLtHashContributionsHash"),
            ),
            SolanaSccpAccountsLtHashFastpqTransition(
                ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY_V1,
                "meta_set",
                ByteArray(0),
                hex32Bytes(normalized.residualChecksum, "openedAccountsLtHashResidualChecksum"),
            ),
            SolanaSccpAccountsLtHashFastpqTransition(
                ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY_V1,
                "meta_set",
                ByteArray(0),
                verificationContextBytes,
            ),
        )
        return SolanaSccpAccountsLtHashProofRequest(
            version = 1,
            proofFamily = "stark-fri-v1",
            circuitId = ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
            parameterSet = ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1,
            sourceDomain = witness.sourceDomain,
            finalizedSlot = witness.finalizedSlot,
            parentSlot = witness.parentSlot,
            sourceStateVerifierId = witness.sourceStateVerifierId,
            sourceStateVerifierHash = witness.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash = witness.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = normalized.openedContributionsHash,
            openedAccountsLtHashResidualChecksum = normalized.residualChecksum,
            statementBytes = statementBytes,
            accountCommitmentBytes = accountCommitmentBytes,
            verificationContextBytes = verificationContextBytes,
            schemaDescriptor = accountsLtHashOpenVerifySchemaDescriptor(witnessInput, openedInput),
            publicInputColumns = accountsLtHashPublicInputColumns(witnessInput, openedInput),
            fastpqPublicInputs = SolanaSccpAccountsLtHashFastpqPublicInputs(
                dsid = "0x" + hexLower(dsidHash.copyOfRange(0, 16)),
                slot = witness.finalizedSlot,
                oldRoot = witness.parentBankHash,
                newRoot = witness.bankHash,
                permRoot = witness.accountInclusionRoot,
                txSetHash = witness.accountsLtHashProofPublicInputsHash,
            ),
            fastpqTransitions = transitions,
        )
    }

    @JvmStatic
    fun canonicalAccountInclusionLeafBytes(
        finalizedSlot: String,
        address: ByteArray,
        owner: ByteArray,
        lamports: String,
        rentEpoch: String,
        executable: Boolean,
        dataHash: String,
        rawDataHash: String,
    ): ByteArray {
        require(address.size == 32 && address.any { it.toInt() != 0 }) {
            "address must be a non-zero 32-byte Solana account id"
        }
        val slot = normalizeU64(finalizedSlot, "finalizedSlot")
        val rawHashBytes = hex32Bytes(rawDataHash, "rawDataHash")
        require(rawHashBytes.any { it.toInt() != 0 }) { "rawDataHash must not be zero" }
        val openingHash = hex32Bytes(
            accountOpeningHash(address, owner, lamports, rentEpoch, executable, dataHash),
            "openingHash",
        )
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, slot)
        writeVec(out, address)
        out.write(openingHash)
        out.write(rawHashBytes)
        return out.toByteArray()
    }

    @JvmStatic
    fun accountInclusionLeafHash(
        finalizedSlot: String,
        address: ByteArray,
        owner: ByteArray,
        lamports: String,
        rentEpoch: String,
        executable: Boolean,
        dataHash: String,
        rawData: ByteArray,
    ): String =
        hashHex(
            "sccp:solana:account-inclusion-leaf:v1",
            canonicalAccountInclusionLeafBytes(
                finalizedSlot,
                address,
                owner,
                lamports,
                rentEpoch,
                executable,
                dataHash,
                accountRawDataHash(rawData),
            ),
        )

    @JvmStatic
    fun canonicalAccountInclusionNodeBytes(left: String, right: String): ByteArray {
        val leftBytes = hex32Bytes(left, "left")
        val rightBytes = hex32Bytes(right, "right")
        require(leftBytes.any { it.toInt() != 0 }) { "left must not be zero" }
        require(rightBytes.any { it.toInt() != 0 }) { "right must not be zero" }
        val first: ByteArray
        val second: ByteArray
        if (compareLexicographically(leftBytes, rightBytes) <= 0) {
            first = leftBytes
            second = rightBytes
        } else {
            first = rightBytes
            second = leftBytes
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(first)
        out.write(second)
        return out.toByteArray()
    }

    @JvmStatic
    fun accountInclusionNodeHash(left: String, right: String): String =
        hashHex(
            "sccp:solana:account-inclusion-node:v1",
            canonicalAccountInclusionNodeBytes(left, right),
        )

    @JvmStatic
    fun accountInclusionRootFromBranch(leaf: String, siblings: List<String>): String {
        require(siblings.size <= MAX_SOURCE_MERKLE_BRANCH_NODES) {
            "siblings must contain at most $MAX_SOURCE_MERKLE_BRANCH_NODES entries"
        }
        var current = normalizeNonZeroHex32(leaf, "leaf")
        siblings.forEachIndexed { index, sibling ->
            current = accountInclusionNodeHash(current, normalizeHex32(sibling, "siblings[$index]"))
        }
        return current
    }

    @JvmStatic
    fun accountInclusionRootAndBranches(leaves: List<String>): SolanaSccpAccountInclusionWitness {
        require(leaves.isNotEmpty()) { "leaves must be non-empty" }
        var level = leaves.mapIndexed { index, leaf ->
            val hash = hex32Bytes(leaf, "leaves[$index]")
            require(hash.any { it.toInt() != 0 }) { "leaves[$index] must not be zero" }
            AccountInclusionLevelNode(hash, listOf(index))
        }.sortedWith { left, right -> compareLexicographically(left.hash, right.hash) }
        for (index in 1 until level.size) {
            require(!level[index - 1].hash.contentEquals(level[index].hash)) {
                "leaves must be unique"
            }
        }
        val branches = MutableList(leaves.size) { mutableListOf<String>() }
        while (level.size > 1) {
            val next = mutableListOf<AccountInclusionLevelNode>()
            var index = 0
            while (index < level.size) {
                if (index + 1 >= level.size) {
                    next.add(level[index])
                    index += 2
                    continue
                }
                val left = level[index]
                val right = level[index + 1]
                val leftHex = "0x" + hexLower(left.hash)
                val rightHex = "0x" + hexLower(right.hash)
                left.indexes.forEach { branches[it].add(rightHex) }
                right.indexes.forEach { branches[it].add(leftHex) }
                next.add(
                    AccountInclusionLevelNode(
                        hex32Bytes(accountInclusionNodeHash(leftHex, rightHex), "parent"),
                        left.indexes + right.indexes,
                    ),
                )
                index += 2
            }
            level = next
        }
        return SolanaSccpAccountInclusionWitness(
            root = "0x" + hexLower(level[0].hash),
            branches = branches.map { it.toList() },
        )
    }

    private fun requireUniqueOpenedAccountAddresses(openings: List<SolanaSccpAccountOpeningInput>) {
        val seenAddresses = mutableSetOf<String>()
        openings.forEach { opening ->
            require(opening.address.size == 32) { "address must be 32 bytes" }
            require(seenAddresses.add(hexLower(opening.address))) {
                "opened account addresses must be unique"
            }
        }
    }

    @JvmStatic
    fun openedAccountInclusionWitness(
        input: SolanaSccpOpenedAccountInclusionWitnessInput,
    ): SolanaSccpOpenedAccountInclusionWitness {
        require(input.validatorVoteAccountOpenings.size == input.validatorVoteAccountRawData.size) {
            "validatorVoteAccountOpenings and validatorVoteAccountRawData must have matching lengths"
        }
        require(input.validatorVoteAccountOpenings.size <= MAX_VALIDATORS) {
            "validatorVoteAccountOpenings must contain at most $MAX_VALIDATORS entries"
        }
        require(input.validatorStakeAccountOpenings.size == input.validatorStakeAccountRawData.size) {
            "validatorStakeAccountOpenings and validatorStakeAccountRawData must have matching lengths"
        }
        require(input.validatorStakeAccountOpenings.size <= MAX_VALIDATORS) {
            "validatorStakeAccountOpenings must contain at most $MAX_VALIDATORS entries"
        }
        requireUniqueOpenedAccountAddresses(
            input.validatorVoteAccountOpenings +
                input.validatorStakeAccountOpenings +
                listOf(input.stakeHistorySysvarOpening),
        )
        fun leaf(opening: SolanaSccpAccountOpeningInput, rawData: ByteArray): String =
            accountInclusionLeafHash(
                input.finalizedSlot,
                opening.address,
                opening.owner,
                opening.lamports,
                opening.rentEpoch,
                opening.executable,
                opening.dataHash,
                rawData,
            )
        val voteLeaves = input.validatorVoteAccountOpenings.mapIndexed { index, opening ->
            leaf(opening, input.validatorVoteAccountRawData[index])
        }
        val stakeLeaves = input.validatorStakeAccountOpenings.mapIndexed { index, opening ->
            leaf(opening, input.validatorStakeAccountRawData[index])
        }
        val stakeHistoryLeaf = leaf(input.stakeHistorySysvarOpening, input.stakeHistorySysvarRawData)
        val witness = accountInclusionRootAndBranches(voteLeaves + stakeLeaves + stakeHistoryLeaf)
        if (input.expectedAccountInclusionRoot != null) {
            require(normalizeNonZeroHex32(input.expectedAccountInclusionRoot, "accountInclusionRoot") == witness.root) {
                "accountInclusionRoot must match opened account inclusion witness"
            }
        }
        val voteBranches = witness.branches.take(voteLeaves.size)
        val stakeBranches = witness.branches.drop(voteLeaves.size).take(stakeLeaves.size)
        return SolanaSccpOpenedAccountInclusionWitness(
            root = witness.root,
            branches = witness.branches,
            validatorVoteAccountBranches = voteBranches,
            validatorStakeAccountBranches = stakeBranches,
            stakeHistorySysvarBranch = witness.branches.last(),
        )
    }

    @JvmStatic
    fun openedAccountInclusionWitness(
        input: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): SolanaSccpOpenedAccountInclusionWitness =
        openedAccountInclusionWitness(
            SolanaSccpOpenedAccountInclusionWitnessInput(
                finalizedSlot = input.finalizedSlot,
                validatorVoteAccountOpenings = input.validatorVoteAccountOpenings,
                validatorVoteAccountRawData = input.validatorVoteAccountRawData,
                validatorStakeAccountOpenings = input.validatorStakeAccountOpenings,
                validatorStakeAccountRawData = input.validatorStakeAccountRawData,
                stakeHistorySysvarOpening = input.stakeHistorySysvarOpening,
                stakeHistorySysvarRawData = input.stakeHistorySysvarRawData,
                expectedAccountInclusionRoot = input.accountInclusionRoot,
            ),
        )

    @JvmStatic
    fun canonicalVoteAccountDataBytes(
        nodePubkey: ByteArray,
        authorizedVoter: ByteArray,
        authorizedWithdrawer: ByteArray,
        inflationRewardsCollector: ByteArray,
        blockRevenueCollector: ByteArray,
        inflationRewardsCommissionBps: String,
        blockRevenueCommissionBps: String,
        pendingDelegatorRewards: String,
        blsPubkeyCompressed: ByteArray,
        rootSlot: String,
        towerVoteSlots: List<String>,
    ): ByteArray {
        listOf(
            "nodePubkey" to nodePubkey,
            "authorizedVoter" to authorizedVoter,
            "authorizedWithdrawer" to authorizedWithdrawer,
            "inflationRewardsCollector" to inflationRewardsCollector,
            "blockRevenueCollector" to blockRevenueCollector,
        ).forEach { (label, bytes) ->
            require(bytes.size == 32 && bytes.any { it.toInt() != 0 }) {
                "$label must be a non-zero 32-byte Solana public key"
            }
        }
        val normalizedInflationRewardsCommissionBps =
            normalizeU64(inflationRewardsCommissionBps, "inflationRewardsCommissionBps")
        val normalizedBlockRevenueCommissionBps =
            normalizeU64(blockRevenueCommissionBps, "blockRevenueCommissionBps")
        val normalizedPendingDelegatorRewards =
            normalizeU64(pendingDelegatorRewards, "pendingDelegatorRewards")
        require(normalizedInflationRewardsCommissionBps <= BASIS_POINTS_PER_UNIT) {
            "inflationRewardsCommissionBps must be at most 10000"
        }
        require(normalizedBlockRevenueCommissionBps <= BASIS_POINTS_PER_UNIT) {
            "blockRevenueCommissionBps must be at most 10000"
        }
        require(blsPubkeyCompressed.isEmpty() || blsPubkeyCompressed.size == BLS_PUBLIC_KEY_COMPRESSED_LEN) {
            "blsPubkeyCompressed must be empty or 48 bytes"
        }
        require(blsPubkeyCompressed.isEmpty() || blsPubkeyCompressed.any { it.toInt() != 0 }) {
            "blsPubkeyCompressed must be empty or non-zero 48 bytes"
        }
        val normalizedRootSlot = normalizeU64(rootSlot, "rootSlot")
        require(towerVoteSlots.size == TOWER_VOTE_STACK_DEPTH.toInt()) {
            "towerVoteSlots must contain 31 active post-root slots"
        }
        val slots = towerVoteSlots.mapIndexed { index, slot -> normalizeU64(slot, "towerVoteSlots[$index]") }
        var previousSlot = normalizedRootSlot
        slots.forEachIndexed { index, slot ->
            require(slot > previousSlot) { "towerVoteSlots[$index] must be greater than the previous slot" }
            previousSlot = slot
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeVec(out, nodePubkey)
        writeVec(out, authorizedVoter)
        writeVec(out, authorizedWithdrawer)
        writeVec(out, inflationRewardsCollector)
        writeVec(out, blockRevenueCollector)
        writeU16Le(out, normalizedInflationRewardsCommissionBps.toInt())
        writeU16Le(out, normalizedBlockRevenueCommissionBps.toInt())
        writeU64Le(out, normalizedPendingDelegatorRewards)
        writeVec(out, blsPubkeyCompressed)
        writeU64Le(out, normalizedRootSlot)
        writeU32Le(out, slots.size)
        slots.forEach { slot -> writeU64Le(out, slot) }
        return out.toByteArray()
    }

    @JvmStatic
    fun voteAccountDataHash(
        nodePubkey: ByteArray,
        authorizedVoter: ByteArray,
        authorizedWithdrawer: ByteArray,
        inflationRewardsCollector: ByteArray,
        blockRevenueCollector: ByteArray,
        inflationRewardsCommissionBps: String,
        blockRevenueCommissionBps: String,
        pendingDelegatorRewards: String,
        blsPubkeyCompressed: ByteArray,
        rootSlot: String,
        towerVoteSlots: List<String>,
    ): String =
        hashHex(
            "sccp:solana:vote-account-data:v1",
            canonicalVoteAccountDataBytes(
                nodePubkey,
                authorizedVoter,
                authorizedWithdrawer,
                inflationRewardsCollector,
                blockRevenueCollector,
                inflationRewardsCommissionBps,
                blockRevenueCommissionBps,
                pendingDelegatorRewards,
                blsPubkeyCompressed,
                rootSlot,
                towerVoteSlots,
            ),
        )

    @JvmStatic
    fun voteAccountDataFromRawVoteState(
        rawData: ByteArray,
        epoch: String,
        voteAccountAddress: ByteArray,
    ): SolanaSccpParsedVoteStateAccountData {
        require(rawData.size == VOTE_STATE_ACCOUNT_DATA_LEN) {
            "rawData must be a 3762-byte Solana VoteState account"
        }
        require(voteAccountAddress.size == 32 && voteAccountAddress.any { it.toInt() != 0 }) {
            "voteAccountAddress must be a non-zero 32-byte Solana public key"
        }
        val signedEpoch = normalizeU64(epoch, "epoch")
        var cursor = 0

        fun readU8(field: String): Int {
            require(cursor + 1 <= rawData.size) { "$field is too short" }
            val value = rawData[cursor].toInt() and 0xFF
            cursor += 1
            return value
        }

        fun readU32(field: String): Int {
            require(cursor + 4 <= rawData.size) { "$field is too short" }
            val value = readU32Le(rawData, cursor)
            cursor += 4
            return value
        }

        fun readU16(field: String): Int {
            require(cursor + 2 <= rawData.size) { "$field is too short" }
            val value = readU16Le(rawData, cursor)
            cursor += 2
            return value
        }

        fun readU64(field: String): BigInteger {
            require(cursor + 8 <= rawData.size) { "$field is too short" }
            val value = readU64Le(rawData, cursor)
            cursor += 8
            return value
        }

        fun readPubkey(field: String): ByteArray {
            require(cursor + 32 <= rawData.size) { "$field is too short" }
            val value = rawData.copyOfRange(cursor, cursor + 32)
            cursor += 32
            return value
        }

        val variant = readU32("voteStateVariant")
        val hasLatency = when (variant) {
            VOTE_STATE_V1_14_11_DISCRIMINANT -> false
            VOTE_STATE_V3_DISCRIMINANT -> true
            VOTE_STATE_V4_DISCRIMINANT -> true
            else -> throw IllegalArgumentException("rawData must contain VoteStateVersions::V1_14_11, ::V3, or ::V4")
        }
        val nodePubkey = readPubkey("nodePubkey")
        val authorizedWithdrawer = readPubkey("authorizedWithdrawer")
        val inflationRewardsCollector: ByteArray
        val blockRevenueCollector: ByteArray
        val inflationRewardsCommissionBps: String
        val blockRevenueCommissionBps: String
        val pendingDelegatorRewards: String
        val blsPubkeyCompressed: ByteArray
        if (variant == VOTE_STATE_V4_DISCRIMINANT) {
            inflationRewardsCollector = readPubkey("inflationRewardsCollector")
            blockRevenueCollector = readPubkey("blockRevenueCollector")
            val inflationRewardsCommissionBpsValue = readU16("inflationRewardsCommissionBps")
            val blockRevenueCommissionBpsValue = readU16("blockRevenueCommissionBps")
            require(inflationRewardsCommissionBpsValue <= BASIS_POINTS_PER_UNIT.toInt()) {
                "inflationRewardsCommissionBps must be at most 10000"
            }
            require(blockRevenueCommissionBpsValue <= BASIS_POINTS_PER_UNIT.toInt()) {
                "blockRevenueCommissionBps must be at most 10000"
            }
            inflationRewardsCommissionBps = inflationRewardsCommissionBpsValue.toString()
            blockRevenueCommissionBps = blockRevenueCommissionBpsValue.toString()
            pendingDelegatorRewards = readU64("pendingDelegatorRewards").toString()
            when (readU8("blsPubkeyCompressed")) {
                0 -> blsPubkeyCompressed = ByteArray(0)
                1 -> {
                    require(cursor + BLS_PUBLIC_KEY_COMPRESSED_LEN <= rawData.size) {
                        "blsPubkeyCompressed is too short"
                    }
                    blsPubkeyCompressed = rawData.copyOfRange(cursor, cursor + BLS_PUBLIC_KEY_COMPRESSED_LEN)
                    cursor += BLS_PUBLIC_KEY_COMPRESSED_LEN
                }
                else -> throw IllegalArgumentException("blsPubkeyCompressed option discriminator must be 0 or 1")
            }
        } else {
            val commission = readU8("commission")
            inflationRewardsCollector = voteAccountAddress.copyOf()
            blockRevenueCollector = nodePubkey.copyOf()
            inflationRewardsCommissionBps = (commission * 100).toString()
            blockRevenueCommissionBps = BASIS_POINTS_PER_UNIT.toString()
            pendingDelegatorRewards = "0"
            blsPubkeyCompressed = ByteArray(0)
        }
        require(readU64("towerVoteSlots") == BigInteger.valueOf(TOWER_VOTE_STACK_DEPTH)) {
            "towerVoteSlots must contain 31 active post-root slots"
        }
        val towerVoteSlots = mutableListOf<String>()
        val towerVoteSlotValues = mutableListOf<BigInteger>()
        val depth = TOWER_VOTE_STACK_DEPTH.toInt()
        for (index in 0 until depth) {
            if (hasLatency) {
                readU8("towerVoteSlots[$index].latency")
            }
            val slot = readU64("towerVoteSlots[$index].slot")
            val confirmationCount = readU32("towerVoteSlots[$index].confirmationCount")
            require(confirmationCount == depth - index) {
                "towerVoteSlots[$index] has an invalid Tower confirmation count"
            }
            towerVoteSlots.add(slot.toString())
            towerVoteSlotValues.add(slot)
        }
        require(readU8("rootSlot") == 1) { "rawData must contain a rooted vote state" }
        val rootSlot = readU64("rootSlot")
        var previousTowerSlot = rootSlot
        towerVoteSlotValues.forEachIndexed { index, slot ->
            require(slot > previousTowerSlot) {
                "towerVoteSlots[$index] must be greater than the previous slot"
            }
            previousTowerSlot = slot
        }
        val authorizedVoterCount = readU64("authorizedVoters")
        val authorizedVoterLimit =
            if (variant == VOTE_STATE_V4_DISCRIMINANT) {
                VOTE_STATE_V4_AUTHORIZED_VOTERS
            } else {
                VOTE_STATE_PRIOR_VOTERS
            }
        require(
            authorizedVoterCount > BigInteger.ZERO &&
                authorizedVoterCount <= BigInteger.valueOf(authorizedVoterLimit.toLong())
        ) {
            if (variant == VOTE_STATE_V4_DISCRIMINANT) {
                "authorizedVoters must contain 1..4 entries for VoteStateV4"
            } else {
                "authorizedVoters must contain 1..32 entries"
            }
        }
        var previousAuthorizedEpoch: BigInteger? = null
        var authorizedVoter: ByteArray? = null
        for (index in 0 until authorizedVoterCount.toInt()) {
            val authorizedEpoch = readU64("authorizedVoters[$index].epoch")
            previousAuthorizedEpoch?.let { previousEpoch ->
                require(previousEpoch < authorizedEpoch) {
                    "authorizedVoters must be sorted by strictly increasing epoch"
                }
            }
            val voter = readPubkey("authorizedVoters[$index].authorizedVoter")
            require(voter.any { it.toInt() != 0 }) {
                "authorizedVoters[$index].authorizedVoter must be non-zero"
            }
            if (authorizedEpoch <= signedEpoch) {
                authorizedVoter = voter
            }
            previousAuthorizedEpoch = authorizedEpoch
        }
        val selectedAuthorizedVoter = authorizedVoter
            ?: throw IllegalArgumentException("authorizedVoters must include an entry at or before epoch")
        if (variant != VOTE_STATE_V4_DISCRIMINANT) {
            for (index in 0 until VOTE_STATE_PRIOR_VOTERS) {
                val priorVoter = readPubkey("priorVoters[$index].pubkey")
                val fromEpoch = readU64("priorVoters[$index].fromEpoch")
                val untilEpoch = readU64("priorVoters[$index].untilEpoch")
                if (!priorVoter.any { it.toInt() != 0 }) {
                    require(fromEpoch == BigInteger.ZERO && untilEpoch == BigInteger.ZERO) {
                        "priorVoters[$index] zero pubkey must have zero epoch bounds"
                    }
                } else {
                    require(fromEpoch < untilEpoch) {
                        "priorVoters[$index] must have increasing epoch bounds"
                    }
                }
            }
            val priorVotersIndex = readU64("priorVoters.index")
            val priorVotersIsEmpty = readU8("priorVoters.isEmpty")
            require(
                priorVotersIndex < BigInteger.valueOf(VOTE_STATE_PRIOR_VOTERS.toLong()) &&
                    (priorVotersIsEmpty == 0 || priorVotersIsEmpty == 1)
            ) {
                "priorVoters must have a valid cursor and boolean empty flag"
            }
        }
        val epochCreditCount = readU64("epochCredits")
        require(epochCreditCount <= BigInteger.valueOf(VOTE_STATE_MAX_EPOCH_CREDITS.toLong())) {
            "epochCredits exceeds Solana history bound"
        }
        var previousEpochCreditEpoch: BigInteger? = null
        var previousEpochCreditTotal: BigInteger? = null
        for (index in 0 until epochCreditCount.toInt()) {
            val creditEpoch = readU64("epochCredits[$index].epoch")
            val credits = readU64("epochCredits[$index].credits")
            val previousCredits = readU64("epochCredits[$index].previousCredits")
            require(creditEpoch <= signedEpoch) { "epochCredits must be sorted and monotonic" }
            previousEpochCreditEpoch?.let { previousEpoch ->
                require(previousEpoch < creditEpoch) { "epochCredits must be sorted and monotonic" }
            }
            require(previousCredits <= credits) { "epochCredits must be sorted and monotonic" }
            previousEpochCreditTotal?.let { previousTotal ->
                require(previousTotal <= previousCredits) { "epochCredits must be sorted and monotonic" }
            }
            previousEpochCreditEpoch = creditEpoch
            previousEpochCreditTotal = credits
        }
        val lastTimestampSlot = readU64("lastTimestamp.slot")
        val lastTimestamp = readU64("lastTimestamp.timestamp")
        val lastTowerVoteSlot = towerVoteSlotValues[towerVoteSlotValues.size - 1]
        require(
            if (lastTimestampSlot == BigInteger.ZERO) {
                lastTimestamp == BigInteger.ZERO
            } else {
                lastTimestampSlot <= lastTowerVoteSlot &&
                    lastTimestamp <= BigInteger.valueOf(Long.MAX_VALUE)
            }
        ) {
            "lastTimestamp must be default or within the Tower vote stack"
        }
        for (index in cursor until rawData.size) {
            require(rawData[index].toInt() == 0) { "rawData padding must be zero" }
        }
        val parsed = SolanaSccpParsedVoteStateAccountData(
            nodePubkey = nodePubkey,
            authorizedVoter = selectedAuthorizedVoter,
            authorizedWithdrawer = authorizedWithdrawer,
            inflationRewardsCollector = inflationRewardsCollector,
            blockRevenueCollector = blockRevenueCollector,
            inflationRewardsCommissionBps = inflationRewardsCommissionBps,
            blockRevenueCommissionBps = blockRevenueCommissionBps,
            pendingDelegatorRewards = pendingDelegatorRewards,
            blsPubkeyCompressed = blsPubkeyCompressed,
            rootSlot = rootSlot.toString(),
            towerVoteSlots = towerVoteSlots,
        )
        canonicalVoteAccountDataBytes(
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
        )
        return parsed
    }

    @JvmStatic
    fun voteAccountDataHashFromRawVoteState(
        rawData: ByteArray,
        epoch: String,
        voteAccountAddress: ByteArray,
    ): String {
        val parsed = voteAccountDataFromRawVoteState(rawData, epoch, voteAccountAddress)
        return voteAccountDataHash(
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
        )
    }

    @JvmStatic
    fun voteAccountDataFromRawVoteStateV1OrV3(
        rawData: ByteArray,
        epoch: String,
        voteAccountAddress: ByteArray,
    ): SolanaSccpParsedVoteStateV1OrV3AccountData =
        voteAccountDataFromRawVoteState(rawData, epoch, voteAccountAddress)

    @JvmStatic
    fun voteAccountDataHashFromRawVoteStateV1OrV3(
        rawData: ByteArray,
        epoch: String,
        voteAccountAddress: ByteArray,
    ): String = voteAccountDataHashFromRawVoteState(rawData, epoch, voteAccountAddress)

    @JvmStatic
    fun canonicalStakeAccountDataBytes(
        staker: ByteArray,
        withdrawer: ByteArray,
        voterPubkey: ByteArray,
        delegatedStake: String,
        activationEpoch: String,
        deactivationEpoch: String,
        creditsObserved: String = "0",
        stakeFlags: String = "0",
        warmupCooldownRateBytes: ByteArray = STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES.copyOf(),
    ): ByteArray {
        listOf(
            "staker" to staker,
            "withdrawer" to withdrawer,
            "voterPubkey" to voterPubkey,
        ).forEach { (label, bytes) ->
            require(bytes.size == 32 && bytes.any { it.toInt() != 0 }) {
                "$label must be a non-zero 32-byte Solana public key"
            }
        }
        val normalizedDelegatedStake = normalizeU64(delegatedStake, "delegatedStake")
        require(normalizedDelegatedStake > BigInteger.ZERO) { "delegatedStake must be greater than zero" }
        val normalizedActivationEpoch = normalizeU64(activationEpoch, "activationEpoch")
        val normalizedDeactivationEpoch = normalizeU64(deactivationEpoch, "deactivationEpoch")
        require(normalizedDeactivationEpoch > normalizedActivationEpoch) {
            "deactivationEpoch must be greater than activationEpoch"
        }
        require(warmupCooldownRateBytes.size == STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_BYTES) {
            "warmupCooldownRateBytes must be 8 bytes"
        }
        require(
            warmupCooldownRateBytes.contentEquals(STAKE_STATE_V2_LEGACY_WARMUP_COOLDOWN_RATE_BYTES) ||
                warmupCooldownRateBytes.contentEquals(STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES),
        ) {
            "warmupCooldownRateBytes must be Solana 0.25 or 0.09 f64 bytes"
        }
        val normalizedCreditsObserved = normalizeU64(creditsObserved, "creditsObserved")
        val normalizedStakeFlags = normalizeU64(stakeFlags, "stakeFlags")
        require(
            normalizedStakeFlags <= BigInteger.valueOf(255L) &&
                (normalizedStakeFlags.toInt() and STAKE_STATE_V2_KNOWN_FLAGS_MASK.inv()) == 0,
        ) { "stakeFlags contains reserved StakeFlags bits" }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeVec(out, staker)
        writeVec(out, withdrawer)
        writeVec(out, voterPubkey)
        writeU64Le(out, normalizedDelegatedStake)
        writeU64Le(out, normalizedActivationEpoch)
        writeU64Le(out, normalizedDeactivationEpoch)
        writeVec(out, warmupCooldownRateBytes)
        writeU64Le(out, normalizedCreditsObserved)
        out.write(normalizedStakeFlags.toInt())
        return out.toByteArray()
    }

    @JvmStatic
    fun stakeAccountDataHash(
        staker: ByteArray,
        withdrawer: ByteArray,
        voterPubkey: ByteArray,
        delegatedStake: String,
        activationEpoch: String,
        deactivationEpoch: String,
        creditsObserved: String = "0",
        stakeFlags: String = "0",
        warmupCooldownRateBytes: ByteArray = STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES.copyOf(),
    ): String =
        hashHex(
            "sccp:solana:stake-account-data:v1",
            canonicalStakeAccountDataBytes(
                staker,
                withdrawer,
                voterPubkey,
                delegatedStake,
                activationEpoch,
                deactivationEpoch,
                creditsObserved,
                stakeFlags,
                warmupCooldownRateBytes,
            ),
        )

    @JvmStatic
    fun stakeAccountDataFromRawStakeStateV2(rawData: ByteArray): SolanaSccpParsedStakeStateV2StakeAccountData {
        require(rawData.size == STAKE_STATE_V2_STAKE_ACCOUNT_DATA_LEN) {
            "rawData must be a 200-byte Solana StakeStateV2 account"
        }
        require(readU32Le(rawData, 0) == STAKE_STATE_V2_STAKE_DISCRIMINANT) {
            "rawData must contain StakeStateV2::Stake"
        }
        require(rawData.copyOfRange(STAKE_STATE_V2_FLAG_OFFSET + 1, rawData.size).all { it.toInt() == 0 }) {
            "rawData must not contain non-zero stake account padding"
        }
        val stakeFlags = rawData[STAKE_STATE_V2_FLAG_OFFSET].toInt() and 0xff
        require((stakeFlags and STAKE_STATE_V2_KNOWN_FLAGS_MASK.inv()) == 0) {
            "rawData contains reserved StakeFlags bits"
        }
        val parsed = SolanaSccpParsedStakeStateV2StakeAccountData(
            staker = rawData.copyOfRange(STAKE_STATE_V2_STAKER_OFFSET, STAKE_STATE_V2_STAKER_OFFSET + 32),
            withdrawer = rawData.copyOfRange(STAKE_STATE_V2_WITHDRAWER_OFFSET, STAKE_STATE_V2_WITHDRAWER_OFFSET + 32),
            voterPubkey = rawData.copyOfRange(
                STAKE_STATE_V2_VOTER_PUBKEY_OFFSET,
                STAKE_STATE_V2_VOTER_PUBKEY_OFFSET + 32,
            ),
            delegatedStake = readU64Le(rawData, STAKE_STATE_V2_DELEGATED_STAKE_OFFSET).toString(),
            activationEpoch = readU64Le(rawData, STAKE_STATE_V2_ACTIVATION_EPOCH_OFFSET).toString(),
            deactivationEpoch = readU64Le(rawData, STAKE_STATE_V2_DEACTIVATION_EPOCH_OFFSET).toString(),
            warmupCooldownRateBytes = rawData.copyOfRange(
                STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_OFFSET,
                STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_OFFSET + STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_BYTES,
            ),
            creditsObserved = readU64Le(rawData, STAKE_STATE_V2_CREDITS_OBSERVED_OFFSET).toString(),
            stakeFlags = stakeFlags.toString(),
        )
        canonicalStakeAccountDataBytes(
            parsed.staker,
            parsed.withdrawer,
            parsed.voterPubkey,
            parsed.delegatedStake,
            parsed.activationEpoch,
            parsed.deactivationEpoch,
            parsed.creditsObserved,
            parsed.stakeFlags,
            parsed.warmupCooldownRateBytes,
        )
        return parsed
    }

    @JvmStatic
    fun stakeAccountDataHashFromRawStakeStateV2(rawData: ByteArray): String {
        val parsed = stakeAccountDataFromRawStakeStateV2(rawData)
        return stakeAccountDataHash(
            parsed.staker,
            parsed.withdrawer,
            parsed.voterPubkey,
            parsed.delegatedStake,
            parsed.activationEpoch,
            parsed.deactivationEpoch,
            parsed.creditsObserved,
            parsed.stakeFlags,
            parsed.warmupCooldownRateBytes,
        )
    }

    @JvmStatic
    fun canonicalStakeAccountStateBytes(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorStakes: List<String>,
        validatorActivationEpochs: List<String>,
        validatorDeactivationEpochs: List<String>,
        validatorVoteAccountAddresses: List<ByteArray>,
        validatorStakeAccountAddresses: List<ByteArray>,
        validatorVoteAccountHashes: List<ByteArray>,
        validatorStakeAccountHashes: List<ByteArray>,
    ): ByteArray {
        val resolvedEpoch = normalizeU64(epoch, "epoch")
        val activationBytes = canonicalStakeActivationBytes(
            epoch,
            validatorPublicKeys,
            validatorStakes,
            validatorActivationEpochs,
            validatorDeactivationEpochs,
        )
        val stakeActivationHash = hashBytes("sccp:solana:stake-activation:v1", activationBytes)
        val voteAccounts = normalizeFixed32List(
            validatorVoteAccountAddresses,
            validatorPublicKeys.size,
            "validatorVoteAccountAddresses",
        )
        val stakeAccounts = normalizeFixed32List(
            validatorStakeAccountAddresses,
            validatorPublicKeys.size,
            "validatorStakeAccountAddresses",
        )
        val voteAccountHashes = normalizeFixed32List(
            validatorVoteAccountHashes,
            validatorPublicKeys.size,
            "validatorVoteAccountHashes",
            unique = false,
        )
        val stakeAccountHashes = normalizeFixed32List(
            validatorStakeAccountHashes,
            validatorPublicKeys.size,
            "validatorStakeAccountHashes",
            unique = false,
        )
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, resolvedEpoch)
        out.write(stakeActivationHash)
        writeU32Le(out, validatorPublicKeys.size)
        val stakeAccountKeys = stakeAccounts.map { hexLower(it) }.toSet()
        validatorPublicKeys.forEachIndexed { index, publicKey ->
            require(!voteAccounts[index].contentEquals(stakeAccounts[index])) {
                "validatorStakeAccountAddresses[$index] must differ from vote account"
            }
            require(!stakeAccountKeys.contains(hexLower(voteAccounts[index]))) {
                "validatorVoteAccountAddresses[$index] must not overlap stake accounts"
            }
            writeVec(out, publicKey)
            writeU64Le(out, normalizeU64(validatorStakes[index], "validatorStakes[$index]"))
            writeU64Le(out, normalizeU64(validatorActivationEpochs[index], "validatorActivationEpochs[$index]"))
            writeU64Le(out, normalizeU64(validatorDeactivationEpochs[index], "validatorDeactivationEpochs[$index]"))
            writeVec(out, voteAccounts[index])
            writeVec(out, stakeAccounts[index])
            out.write(voteAccountHashes[index])
            out.write(stakeAccountHashes[index])
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun stakeAccountStateHash(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorStakes: List<String>,
        validatorActivationEpochs: List<String>,
        validatorDeactivationEpochs: List<String>,
        validatorVoteAccountAddresses: List<ByteArray>,
        validatorStakeAccountAddresses: List<ByteArray>,
        validatorVoteAccountHashes: List<ByteArray>,
        validatorStakeAccountHashes: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:solana:stake-account-state:v1",
            canonicalStakeAccountStateBytes(
                epoch,
                validatorPublicKeys,
                validatorStakes,
                validatorActivationEpochs,
                validatorDeactivationEpochs,
                validatorVoteAccountAddresses,
                validatorStakeAccountAddresses,
                validatorVoteAccountHashes,
                validatorStakeAccountHashes,
            ),
        )

    private data class NormalizedStakeHistoryEntry(
        val epoch: BigInteger,
        val effective: BigInteger,
        val activating: BigInteger,
        val deactivating: BigInteger,
    )

    private data class StakeActivationStatus(
        val effective: BigInteger,
        val activating: BigInteger,
        val deactivating: BigInteger,
    )

    private fun stakeHistoryEntryForEpoch(
        stakeHistoryEntries: List<NormalizedStakeHistoryEntry>,
        epoch: BigInteger,
    ): NormalizedStakeHistoryEntry? =
        stakeHistoryEntries.firstOrNull { it.epoch == epoch }

    private fun stakeChangeAllowance(
        accountPortion: BigInteger,
        clusterPortion: BigInteger,
        clusterEffective: BigInteger,
    ): BigInteger {
        if (accountPortion == BigInteger.ZERO || clusterPortion == BigInteger.ZERO || clusterEffective == BigInteger.ZERO) {
            return BigInteger.ZERO
        }
        val numerator = accountPortion.multiply(clusterEffective).multiply(WARMUP_COOLDOWN_RATE_BPS)
        val denominator = clusterPortion.multiply(BASIS_POINTS_PER_UNIT)
        val delta = numerator.divide(denominator)
        return if (delta < accountPortion) delta else accountPortion
    }

    private fun stakeAndActivatingV2(
        targetEpoch: BigInteger,
        delegatedStake: BigInteger,
        activationEpoch: BigInteger,
        deactivationEpoch: BigInteger,
        stakeHistoryEntries: List<NormalizedStakeHistoryEntry>,
    ): Pair<BigInteger, BigInteger> {
        if (activationEpoch == MAX_U64) {
            return Pair(delegatedStake, BigInteger.ZERO)
        }
        if (activationEpoch == deactivationEpoch) {
            return Pair(BigInteger.ZERO, BigInteger.ZERO)
        }
        if (targetEpoch == activationEpoch) {
            return Pair(BigInteger.ZERO, delegatedStake)
        }
        if (targetEpoch < activationEpoch) {
            return Pair(BigInteger.ZERO, BigInteger.ZERO)
        }
        var previousClusterStake = stakeHistoryEntryForEpoch(stakeHistoryEntries, activationEpoch)
            ?: return Pair(delegatedStake, BigInteger.ZERO)

        var previousEpoch = activationEpoch
        var activatedStakeAmount = BigInteger.ZERO
        while (true) {
            val currentEpoch = previousEpoch.add(BigInteger.ONE)
            if (previousClusterStake.activating == BigInteger.ZERO) {
                break
            }
            val remainingActivatingStake = delegatedStake.subtract(activatedStakeAmount)
            val newlyEffectiveStake = stakeChangeAllowance(
                remainingActivatingStake,
                previousClusterStake.activating,
                previousClusterStake.effective,
            ).max(BigInteger.ONE)
            activatedStakeAmount = activatedStakeAmount.add(newlyEffectiveStake).min(delegatedStake)
            if (activatedStakeAmount >= delegatedStake) {
                activatedStakeAmount = delegatedStake
                break
            }
            if (currentEpoch >= targetEpoch || currentEpoch >= deactivationEpoch) {
                break
            }
            val currentClusterStake = stakeHistoryEntryForEpoch(stakeHistoryEntries, currentEpoch) ?: break
            previousEpoch = currentEpoch
            previousClusterStake = currentClusterStake
        }

        return Pair(activatedStakeAmount, delegatedStake.subtract(activatedStakeAmount))
    }

    private fun delegationStakeStatusV2(
        targetEpoch: BigInteger,
        delegatedStake: BigInteger,
        activationEpoch: BigInteger,
        deactivationEpoch: BigInteger,
        stakeHistoryEntries: List<NormalizedStakeHistoryEntry>,
    ): StakeActivationStatus {
        val (effectiveStake, activatingStake) = stakeAndActivatingV2(
            targetEpoch,
            delegatedStake,
            activationEpoch,
            deactivationEpoch,
            stakeHistoryEntries,
        )
        if (targetEpoch < deactivationEpoch) {
            return StakeActivationStatus(effectiveStake, activatingStake, BigInteger.ZERO)
        }
        if (targetEpoch == deactivationEpoch) {
            return StakeActivationStatus(effectiveStake, BigInteger.ZERO, effectiveStake)
        }
        var previousClusterStake = stakeHistoryEntryForEpoch(stakeHistoryEntries, deactivationEpoch)
            ?: return StakeActivationStatus(BigInteger.ZERO, BigInteger.ZERO, BigInteger.ZERO)

        var previousEpoch = deactivationEpoch
        var remainingDeactivatingStake = effectiveStake
        while (true) {
            val currentEpoch = previousEpoch.add(BigInteger.ONE)
            if (previousClusterStake.deactivating == BigInteger.ZERO) {
                break
            }
            val newlyDeactivatedStake = stakeChangeAllowance(
                remainingDeactivatingStake,
                previousClusterStake.deactivating,
                previousClusterStake.effective,
            ).max(BigInteger.ONE)
            remainingDeactivatingStake = remainingDeactivatingStake.subtract(newlyDeactivatedStake)
                .max(BigInteger.ZERO)
            if (remainingDeactivatingStake == BigInteger.ZERO) {
                break
            }
            if (currentEpoch >= targetEpoch) {
                break
            }
            val currentClusterStake = stakeHistoryEntryForEpoch(stakeHistoryEntries, currentEpoch) ?: break
            previousEpoch = currentEpoch
            previousClusterStake = currentClusterStake
        }

        return StakeActivationStatus(
            remainingDeactivatingStake,
            BigInteger.ZERO,
            remainingDeactivatingStake,
        )
    }

    @JvmStatic
    fun canonicalStakeHistorySysvarDataBytes(
        stakeHistoryEntries: List<SolanaSccpStakeHistoryEntry>,
    ): ByteArray {
        require(stakeHistoryEntries.isNotEmpty()) { "stakeHistoryEntries must be non-empty" }
        require(stakeHistoryEntries.size <= 512) { "stakeHistoryEntries must not exceed 512 entries" }
        var previousEpoch: BigInteger? = null
        val normalizedStakeHistoryEntries = stakeHistoryEntries.mapIndexed { index, entry ->
            val normalized = NormalizedStakeHistoryEntry(
                epoch = normalizeU64(entry.epoch, "stakeHistoryEntries[$index].epoch"),
                effective = normalizeU64(entry.effective, "stakeHistoryEntries[$index].effective"),
                activating = normalizeU64(entry.activating, "stakeHistoryEntries[$index].activating"),
                deactivating = normalizeU64(entry.deactivating, "stakeHistoryEntries[$index].deactivating"),
            )
            previousEpoch?.let { priorEpoch ->
                require(priorEpoch < normalized.epoch) {
                    "stakeHistoryEntries must be sorted by strictly increasing epoch"
                }
            }
            previousEpoch = normalized.epoch
            normalized
        }

        val out = ByteArrayOutputStream()
        writeU64Le(out, BigInteger.valueOf(normalizedStakeHistoryEntries.size.toLong()))
        normalizedStakeHistoryEntries.asReversed().forEach { entry ->
            writeU64Le(out, entry.epoch)
            writeU64Le(out, entry.effective)
            writeU64Le(out, entry.activating)
            writeU64Le(out, entry.deactivating)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun stakeHistorySysvarDataHash(
        stakeHistoryEntries: List<SolanaSccpStakeHistoryEntry>,
    ): String =
        hashHex(
            "sccp:solana:stake-history-sysvar-data:v1",
            canonicalStakeHistorySysvarDataBytes(stakeHistoryEntries),
        )

    @JvmStatic
    fun stakeHistorySysvarDataHashFromRawData(rawData: ByteArray): String {
        require(rawData.size >= 8 && (rawData.size - 8) % 32 == 0) {
            "rawData must be Solana StakeHistory sysvar bincode Vec bytes"
        }
        val entryCount = readU64Le(rawData, 0)
        require(entryCount > BigInteger.ZERO && entryCount <= BigInteger.valueOf(512)) {
            "rawData must contain 1..512 StakeHistory sysvar entries"
        }
        require(rawData.size == 8 + entryCount.toInt() * 32) {
            "rawData must contain 1..512 StakeHistory sysvar entries"
        }
        var offset = 8
        var previousEpoch: BigInteger? = null
        for (index in 0 until entryCount.toInt()) {
            val epoch = readU64Le(rawData, offset)
            offset += 32
            previousEpoch?.let {
                require(it > epoch) { "rawData StakeHistory entries must be newest-first" }
            }
            previousEpoch = epoch
        }
        return hashHex("sccp:solana:stake-history-sysvar-data:v1", rawData)
    }

    @JvmStatic
    fun canonicalStakeHistoryBytes(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorEffectiveStakes: List<String>,
        validatorDelegatedStakes: List<String>,
        validatorActivationEpochs: List<String>,
        validatorDeactivationEpochs: List<String>,
        validatorVoteAccountAddresses: List<ByteArray>,
        validatorStakeAccountAddresses: List<ByteArray>,
        validatorVoteAccountHashes: List<ByteArray>,
        validatorStakeAccountHashes: List<ByteArray>,
        stakeHistoryEntries: List<SolanaSccpStakeHistoryEntry>,
    ): ByteArray {
        require(validatorEffectiveStakes.size == validatorPublicKeys.size) {
            "validatorEffectiveStakes must match validatorPublicKeys"
        }
        require(validatorDelegatedStakes.size == validatorPublicKeys.size) {
            "validatorDelegatedStakes must match validatorPublicKeys"
        }
        require(validatorActivationEpochs.size == validatorPublicKeys.size) {
            "validatorActivationEpochs must match validatorPublicKeys"
        }
        require(validatorDeactivationEpochs.size == validatorPublicKeys.size) {
            "validatorDeactivationEpochs must match validatorPublicKeys"
        }
        require(stakeHistoryEntries.isNotEmpty()) { "stakeHistoryEntries must be non-empty" }
        require(stakeHistoryEntries.size <= 512) { "stakeHistoryEntries must not exceed 512 entries" }
        val resolvedEpoch = normalizeU64(epoch, "epoch")
        val effectiveStakes = validatorEffectiveStakes.mapIndexed { index, stake ->
            normalizeU64(stake, "validatorEffectiveStakes[$index]")
        }
        val delegatedStakes = validatorDelegatedStakes.mapIndexed { index, stake ->
            normalizeU64(stake, "validatorDelegatedStakes[$index]")
        }
        val activationEpochs = validatorActivationEpochs.mapIndexed { index, value ->
            normalizeU64(value, "validatorActivationEpochs[$index]")
        }
        val deactivationEpochs = validatorDeactivationEpochs.mapIndexed { index, value ->
            normalizeU64(value, "validatorDeactivationEpochs[$index]")
        }
        var previousEpoch: BigInteger? = null
        var signedEpochEntry: NormalizedStakeHistoryEntry? = null
        val normalizedStakeHistoryEntries = stakeHistoryEntries.mapIndexed { index, entry ->
            val normalized = NormalizedStakeHistoryEntry(
                epoch = normalizeU64(entry.epoch, "stakeHistoryEntries[$index].epoch"),
                effective = normalizeU64(entry.effective, "stakeHistoryEntries[$index].effective"),
                activating = normalizeU64(entry.activating, "stakeHistoryEntries[$index].activating"),
                deactivating = normalizeU64(entry.deactivating, "stakeHistoryEntries[$index].deactivating"),
            )
            require(normalized.epoch <= resolvedEpoch) { "stakeHistoryEntries[$index].epoch must not exceed epoch" }
            previousEpoch?.let { priorEpoch ->
                require(priorEpoch < normalized.epoch) {
                    "stakeHistoryEntries must be sorted by strictly increasing epoch"
                }
            }
            previousEpoch = normalized.epoch
            if (normalized.epoch == resolvedEpoch) {
                signedEpochEntry = normalized
            }
            normalized
        }
        val signedEntry = signedEpochEntry ?: throw IllegalArgumentException("stakeHistoryEntries must include epoch")
        var totalEffectiveStake = BigInteger.ZERO
        var totalDelegatedStake = BigInteger.ZERO
        var totalActivatingStake = BigInteger.ZERO
        var totalDeactivatingStake = BigInteger.ZERO
        validatorPublicKeys.forEachIndexed { index, _ ->
            val effectiveStake = effectiveStakes[index]
            val delegatedStake = delegatedStakes[index]
            val activationEpoch = activationEpochs[index]
            val deactivationEpoch = deactivationEpochs[index]
            require(delegatedStake > BigInteger.ZERO) {
                "validatorDelegatedStakes[$index] must be greater than zero"
            }
            require(deactivationEpoch > activationEpoch) {
                "validatorDeactivationEpochs[$index] must be greater than activation epoch"
            }
            val status = delegationStakeStatusV2(
                resolvedEpoch,
                delegatedStake,
                activationEpoch,
                deactivationEpoch,
                normalizedStakeHistoryEntries,
            )
            require(status.effective > BigInteger.ZERO && status.effective == effectiveStake) {
                "validatorEffectiveStakes[$index] must equal replayed StakeHistory effective stake"
            }
            totalEffectiveStake = totalEffectiveStake.add(status.effective)
            totalDelegatedStake = totalDelegatedStake.add(delegatedStake)
            totalActivatingStake = totalActivatingStake.add(status.activating)
            totalDeactivatingStake = totalDeactivatingStake.add(status.deactivating)
        }
        require(totalEffectiveStake > BigInteger.ZERO && totalDelegatedStake >= totalEffectiveStake) {
            "replayed StakeHistory effective stake must be non-zero and not exceed delegated stake"
        }
        require(signedEntry.effective == totalEffectiveStake) {
            "signed epoch StakeHistory effective stake must equal replayed validator effective stake"
        }
        require(signedEntry.activating >= totalActivatingStake) {
            "signed epoch StakeHistory activating stake must cover replayed validators"
        }
        require(signedEntry.deactivating >= totalDeactivatingStake) {
            "signed epoch StakeHistory deactivating stake must cover replayed validators"
        }
        val stakeAccountStateHash = hashBytes(
            "sccp:solana:stake-account-state:v1",
            canonicalStakeAccountStateBytes(
                epoch,
                validatorPublicKeys,
                validatorDelegatedStakes,
                validatorActivationEpochs,
                validatorDeactivationEpochs,
                validatorVoteAccountAddresses,
                validatorStakeAccountAddresses,
                validatorVoteAccountHashes,
                validatorStakeAccountHashes,
            ),
        )

        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, resolvedEpoch)
        out.write(stakeAccountStateHash)
        writeU32Le(out, validatorPublicKeys.size)
        validatorPublicKeys.forEachIndexed { index, publicKey ->
            writeVec(out, publicKey)
            writeU64Le(out, effectiveStakes[index])
            writeU64Le(out, delegatedStakes[index])
            writeU64Le(out, activationEpochs[index])
            writeU64Le(out, deactivationEpochs[index])
        }
        writeU32Le(out, normalizedStakeHistoryEntries.size)
        normalizedStakeHistoryEntries.forEach { entry ->
            writeU64Le(out, entry.epoch)
            writeU64Le(out, entry.effective)
            writeU64Le(out, entry.activating)
            writeU64Le(out, entry.deactivating)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun stakeHistoryHash(
        epoch: String,
        validatorPublicKeys: List<ByteArray>,
        validatorEffectiveStakes: List<String>,
        validatorDelegatedStakes: List<String>,
        validatorActivationEpochs: List<String>,
        validatorDeactivationEpochs: List<String>,
        validatorVoteAccountAddresses: List<ByteArray>,
        validatorStakeAccountAddresses: List<ByteArray>,
        validatorVoteAccountHashes: List<ByteArray>,
        validatorStakeAccountHashes: List<ByteArray>,
        stakeHistoryEntries: List<SolanaSccpStakeHistoryEntry>,
    ): String =
        hashHex(
            "sccp:solana:stake-history:v1",
            canonicalStakeHistoryBytes(
                epoch,
                validatorPublicKeys,
                validatorEffectiveStakes,
                validatorDelegatedStakes,
                validatorActivationEpochs,
                validatorDeactivationEpochs,
                validatorVoteAccountAddresses,
                validatorStakeAccountAddresses,
                validatorVoteAccountHashes,
                validatorStakeAccountHashes,
                stakeHistoryEntries,
            ),
        )

    @JvmStatic
    fun canonicalTowerLockoutBytes(
        finalizedSlot: String,
        rootedSlot: String,
        parentSlot: String,
        parentBankHash: String,
        epoch: String? = null,
    ): ByteArray {
        val finalized = normalizeU64(finalizedSlot, "finalizedSlot")
        val resolvedEpoch = epoch?.let { normalizeU64(it, "epoch") }
            ?: normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch")
        require(resolvedEpoch == normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch")) {
            "epoch must match Solana mainnet finalizedSlot"
        }
        val rooted = normalizeU64(rootedSlot, "rootedSlot")
        val parent = normalizeU64(parentSlot, "parentSlot")
        require(rooted <= parent) { "rootedSlot must be less than or equal to parentSlot" }
        require(parent.add(BigInteger.ONE) == finalized) {
            "parentSlot must be the direct parent of finalizedSlot"
        }
        require(finalized.subtract(rooted) >= BigInteger.valueOf(TOWER_VOTE_STACK_DEPTH)) {
            "rootedSlot must satisfy the Solana Tower lockout depth"
        }
        val parentBankHashBytes = hex32Bytes(parentBankHash, "parentBankHash")
        require(parentBankHashBytes.any { it.toInt() != 0 }) { "parentBankHash must not be zero" }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, resolvedEpoch)
        writeU64Le(out, BigInteger.valueOf(TOWER_LOCKOUT_CONFIRMATION_DEPTH))
        writeU64Le(out, finalized)
        writeU64Le(out, rooted)
        writeU64Le(out, parent)
        out.write(parentBankHashBytes)
        return out.toByteArray()
    }

    @JvmStatic
    fun towerLockoutHash(
        finalizedSlot: String,
        rootedSlot: String,
        parentSlot: String,
        parentBankHash: String,
        epoch: String? = null,
    ): String =
        hashHex(
            "sccp:solana:tower-lockout:v1",
            canonicalTowerLockoutBytes(finalizedSlot, rootedSlot, parentSlot, parentBankHash, epoch),
        )

    @JvmStatic
    fun canonicalTowerReplayBytes(
        finalizedSlot: String,
        rootedSlot: String,
        parentSlot: String,
        bankForkHash: String,
        towerVoteSlots: List<String>,
        epoch: String? = null,
    ): ByteArray {
        val finalized = normalizeU64(finalizedSlot, "finalizedSlot")
        val resolvedEpoch = epoch?.let { normalizeU64(it, "epoch") }
            ?: normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch")
        require(resolvedEpoch == normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch")) {
            "epoch must match Solana mainnet finalizedSlot"
        }
        val rooted = normalizeU64(rootedSlot, "rootedSlot")
        val parent = normalizeU64(parentSlot, "parentSlot")
        require(parent.add(BigInteger.ONE) == finalized) {
            "parentSlot must be the direct parent of finalizedSlot"
        }
        require(rooted < finalized) { "rootedSlot must be less than finalizedSlot" }
        require(finalized.subtract(rooted) >= BigInteger.valueOf(TOWER_VOTE_STACK_DEPTH)) {
            "rootedSlot must satisfy the Solana Tower lockout depth"
        }
        val bankForkHashBytes = hex32Bytes(bankForkHash, "bankForkHash")
        require(bankForkHashBytes.any { it.toInt() != 0 }) { "bankForkHash must not be zero" }

        val depth = TOWER_VOTE_STACK_DEPTH.toInt()
        require(towerVoteSlots.size == depth) { "towerVoteSlots must contain 31 active post-root slots" }
        val votes = towerVoteSlots.mapIndexed { index, slot ->
            normalizeU64(slot, "towerVoteSlots[$index]")
        }
        require(votes.first() > rooted) { "towerVoteSlots[0] must be greater than rootedSlot" }
        require(votes.last() == finalized) { "last towerVoteSlots entry must equal finalizedSlot" }
        require(votes[depth - 2] == parent) { "penultimate towerVoteSlots entry must equal parentSlot" }
        for (index in 1 until votes.size) {
            require(votes[index - 1] < votes[index]) { "towerVoteSlots must be strictly increasing" }
        }
        votes.forEachIndexed { index, voteSlot ->
            require(voteSlot <= finalized) { "towerVoteSlots[$index] must not exceed finalizedSlot" }
            val confirmationCount = depth - index
            val lockout = BigInteger.ONE.shiftLeft(confirmationCount)
            require(voteSlot.add(lockout) > finalized) {
                "towerVoteSlots[$index] does not satisfy its Tower lockout"
            }
        }

        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, resolvedEpoch)
        writeU64Le(out, BigInteger.valueOf(TOWER_LOCKOUT_CONFIRMATION_DEPTH))
        writeU64Le(out, finalized)
        writeU64Le(out, rooted)
        writeU64Le(out, parent)
        out.write(bankForkHashBytes)
        writeU32Le(out, votes.size)
        votes.forEachIndexed { index, voteSlot ->
            writeU64Le(out, voteSlot)
            writeU64Le(out, BigInteger.valueOf((depth - index).toLong()))
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun towerReplayHash(
        finalizedSlot: String,
        rootedSlot: String,
        parentSlot: String,
        bankForkHash: String,
        towerVoteSlots: List<String>,
        epoch: String? = null,
    ): String =
        hashHex(
            "sccp:solana:tower-replay:v1",
            canonicalTowerReplayBytes(
                finalizedSlot,
                rootedSlot,
                parentSlot,
                bankForkHash,
                towerVoteSlots,
                epoch,
            ),
        )

    @JvmStatic
    fun agaveBankHash(
        parentBankHash: String,
        bankSignatureCount: String,
        blockhash: String,
        accountsLtHash: ByteArray,
        bankHashHardForkData: ByteArray = ByteArray(0),
    ): String {
        val parentBankHashBytes = hex32Bytes(parentBankHash, "parentBankHash")
        require(parentBankHashBytes.any { it.toInt() != 0 }) { "parentBankHash must not be zero" }
        val signatureCount = normalizeU64(bankSignatureCount, "bankSignatureCount")
        require(signatureCount != BigInteger.ZERO) { "bankSignatureCount must be nonzero" }
        val blockhashBytes = hex32Bytes(blockhash, "blockhash")
        require(blockhashBytes.any { it.toInt() != 0 }) { "blockhash must not be zero" }
        return "0x" + hexLower(
            agaveBankHashBytes(
                parentBankHashBytes,
                signatureCount,
                blockhashBytes,
                accountsLtHash,
                bankHashHardForkData,
            ),
        )
    }

    @JvmStatic
    fun canonicalBankForkBytes(
        finalizedSlot: String,
        parentSlot: String,
        bankSignatureCount: String,
        parentBankHash: String,
        bankHash: String,
        blockhash: String,
        accountsLtHash: ByteArray? = null,
        bankHashHardForkData: ByteArray = ByteArray(0),
        transactionStatusRoot: String,
        accountInclusionRoot: String,
        accountsLtHashChecksum: String,
        epoch: String? = null,
    ): ByteArray {
        val finalized = normalizeU64(finalizedSlot, "finalizedSlot")
        val resolvedEpoch = epoch?.let { normalizeU64(it, "epoch") }
            ?: normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch")
        require(resolvedEpoch == normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch")) {
            "epoch must match Solana mainnet finalizedSlot"
        }
        val parent = normalizeU64(parentSlot, "parentSlot")
        require(parent.add(BigInteger.ONE) == finalized) {
            "parentSlot must be the direct parent of finalizedSlot"
        }
        val signatureCount = normalizeU64(bankSignatureCount, "bankSignatureCount")
        require(signatureCount != BigInteger.ZERO) { "bankSignatureCount must be nonzero" }
        val parentBankHashBytes = hex32Bytes(parentBankHash, "parentBankHash")
        require(parentBankHashBytes.any { it.toInt() != 0 }) { "parentBankHash must not be zero" }
        val bankHashBytes = hex32Bytes(bankHash, "bankHash")
        require(bankHashBytes.any { it.toInt() != 0 }) { "bankHash must not be zero" }
        require(!parentBankHashBytes.contentEquals(bankHashBytes)) {
            "parentBankHash must differ from bankHash"
        }
        val blockhashBytes = hex32Bytes(blockhash, "blockhash")
        require(blockhashBytes.any { it.toInt() != 0 }) { "blockhash must not be zero" }
        val transactionStatusRootBytes = hex32Bytes(transactionStatusRoot, "transactionStatusRoot")
        require(transactionStatusRootBytes.any { it.toInt() != 0 }) {
            "transactionStatusRoot must not be zero"
        }
        val accountInclusionRootBytes = hex32Bytes(accountInclusionRoot, "accountInclusionRoot")
        require(accountInclusionRootBytes.any { it.toInt() != 0 }) {
            "accountInclusionRoot must not be zero"
        }
        val accountsLtHashChecksumBytes = hex32Bytes(accountsLtHashChecksum, "accountsLtHashChecksum")
        require(accountsLtHashChecksumBytes.any { it.toInt() != 0 }) {
            "accountsLtHashChecksum must not be zero"
        }
        require(bankHashHardForkData.size <= MAX_BANK_HARD_FORK_HASH_DATA_BYTES) {
            "bankHashHardForkData is too large"
        }
        if (accountsLtHash != null) {
            val expectedBankHash = agaveBankHashBytes(
                parentBankHashBytes,
                signatureCount,
                blockhashBytes,
                accountsLtHash,
                bankHashHardForkData,
            )
            require(bankHashBytes.contentEquals(expectedBankHash)) {
                "bankHash must match Agave bank hash inputs"
            }
            require(Blake3.hash(accountsLtHash).contentEquals(accountsLtHashChecksumBytes)) {
                "accountsLtHashChecksum must match accountsLtHash"
            }
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU64Le(out, resolvedEpoch)
        writeU64Le(out, finalized)
        writeU64Le(out, parent)
        writeU64Le(out, signatureCount)
        out.write(parentBankHashBytes)
        out.write(bankHashBytes)
        out.write(blockhashBytes)
        out.write(transactionStatusRootBytes)
        out.write(accountInclusionRootBytes)
        out.write(accountsLtHashChecksumBytes)
        writeVec(out, bankHashHardForkData)
        return out.toByteArray()
    }

    @JvmStatic
    fun bankForkHash(
        finalizedSlot: String,
        parentSlot: String,
        bankSignatureCount: String,
        parentBankHash: String,
        bankHash: String,
        blockhash: String,
        accountsLtHash: ByteArray? = null,
        bankHashHardForkData: ByteArray = ByteArray(0),
        transactionStatusRoot: String,
        accountInclusionRoot: String,
        accountsLtHashChecksum: String,
        epoch: String? = null,
    ): String =
        hashHex(
            "sccp:solana:bank-fork:v1",
            canonicalBankForkBytes(
                finalizedSlot,
                parentSlot,
                bankSignatureCount,
                parentBankHash,
                bankHash,
                blockhash,
                accountsLtHash,
                bankHashHardForkData,
                transactionStatusRoot,
                accountInclusionRoot,
                accountsLtHashChecksum,
                epoch,
            ),
        )

    @JvmStatic
    fun canonicalAccountsLtHashProofPublicInputsBytes(
        finalizedSlot: String,
        parentSlot: String,
        bankSignatureCount: String,
        parentBankHash: String,
        bankHash: String,
        blockhash: String,
        bankHashHardForkData: ByteArray = ByteArray(0),
        transactionStatusRoot: String,
        accountInclusionRoot: String,
        accountsLtHashChecksum: String,
        sourceDomain: Int = DOMAIN_SOLANA,
        accountsLtHash: ByteArray? = null,
    ): ByteArray {
        require(sourceDomain == DOMAIN_SOLANA) { "sourceDomain must be Solana" }
        val blockhashBytes = solanaHash32Bytes(blockhash, "blockhash")
        val blockhashHex = "0x" + hexLower(blockhashBytes)
        val bankForkHashBytes = hex32Bytes(
            bankForkHash(
                finalizedSlot,
                parentSlot,
                bankSignatureCount,
                parentBankHash,
                bankHash,
                blockhashHex,
                accountsLtHash,
                bankHashHardForkData,
                transactionStatusRoot,
                accountInclusionRoot,
                accountsLtHashChecksum,
                null,
            ),
            "bankForkHash",
        )
        val finalized = normalizeU64(finalizedSlot, "finalizedSlot")
        val epoch = normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch")
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, sourceDomain)
        writeString(out, RECURSIVE_PROOF_BACKEND_V1, "backend")
        writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash")
        writeU64Le(out, epoch)
        writeU64Le(out, finalized)
        writeU64Le(out, normalizeU64(parentSlot, "parentSlot"))
        writeU64Le(out, normalizeU64(bankSignatureCount, "bankSignatureCount"))
        out.write(hex32Bytes(parentBankHash, "parentBankHash"))
        out.write(hex32Bytes(bankHash, "bankHash"))
        out.write(blockhashBytes)
        out.write(hex32Bytes(transactionStatusRoot, "transactionStatusRoot"))
        out.write(hex32Bytes(accountInclusionRoot, "accountInclusionRoot"))
        out.write(hex32Bytes(accountsLtHashChecksum, "accountsLtHashChecksum"))
        writeVec(out, bankHashHardForkData)
        out.write(bankForkHashBytes)
        return out.toByteArray()
    }

    @JvmStatic
    fun accountsLtHashProofPublicInputsHash(
        finalizedSlot: String,
        parentSlot: String,
        bankSignatureCount: String,
        parentBankHash: String,
        bankHash: String,
        blockhash: String,
        bankHashHardForkData: ByteArray = ByteArray(0),
        transactionStatusRoot: String,
        accountInclusionRoot: String,
        accountsLtHashChecksum: String,
        sourceDomain: Int = DOMAIN_SOLANA,
        accountsLtHash: ByteArray? = null,
    ): String =
        hashHex(
            ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX_V1,
            canonicalAccountsLtHashProofPublicInputsBytes(
                finalizedSlot,
                parentSlot,
                bankSignatureCount,
                parentBankHash,
                bankHash,
                blockhash,
                bankHashHardForkData,
                transactionStatusRoot,
                accountInclusionRoot,
                accountsLtHashChecksum,
                sourceDomain,
                accountsLtHash,
            ),
        )

    @JvmStatic
    fun canonicalSourceStateVerificationProofBytes(
        proof: SolanaSccpSourceStateVerificationProof,
    ): ByteArray {
        requireSourceStateProofLabel(proof.proofFamily, "proofFamily")
        requireSourceStateProofLabel(proof.circuitId, "circuitId")
        require(proof.version == 1) { "sourceStateProof.version must be 1" }
        require(proof.proofFamily == "stark-fri-v1") {
            "sourceStateProof.proofFamily must be stark-fri-v1"
        }
        require(
            proof.circuitId == ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1 ||
                proof.circuitId == TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1 ||
                proof.circuitId == FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1 ||
                proof.circuitId == BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
        ) {
            "sourceStateProof.circuitId must be a Solana source-state verification circuit"
        }
        val proofFamily = normalizeNonEmpty(proof.proofFamily, "proofFamily")
        val circuitId = normalizeNonEmpty(proof.circuitId, "circuitId")
        val proofBytes = proof.proofBytes
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= SOURCE_STATE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $SOURCE_STATE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        val out = ByteArrayOutputStream()
        out.write(proof.version)
        writeString(out, proofFamily, "proofFamily")
        writeString(out, circuitId, "circuitId")
        writeVec(out, proofBytes)
        return out.toByteArray()
    }

    @JvmStatic
    fun accountsLtHashProofHash(proof: SolanaSccpSourceStateVerificationProof): String {
        require(proof.circuitId == ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1) {
            "accountsLtHashProof.circuitId must be the Solana AccountsLtHash circuit"
        }
        return hashHex(
            "sccp:solana:accounts-lt-proof:v1",
            canonicalSourceStateVerificationProofBytes(proof),
        )
    }

    @JvmStatic
    fun wrapSourceStateVerificationProof(
        proofBytes: ByteArray,
        request: SolanaSccpAccountsLtHashProofRequest,
    ): SolanaSccpSourceStateVerificationProof {
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
        request: SolanaSccpFullLightClientAuditProofRequest,
    ): SolanaSccpSourceStateVerificationProof {
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
    fun requireCanonicalSourceStateProofRequest(
        request: SolanaSccpAccountsLtHashProofRequest,
    ) {
        requireSourceStateProofRequestForWrapping(request)
    }

    @JvmStatic
    fun requireCanonicalSourceStateProofRequest(
        request: SolanaSccpFullLightClientAuditProofRequest,
    ) {
        requireSourceStateProofRequestForWrapping(request)
    }

    private fun requireSourceStateProofRequestForWrapping(
        request: SolanaSccpAccountsLtHashProofRequest,
    ) {
        require(request.version == 1) { "Solana source-state proof request.version must be 1" }
        require(request.proofFamily == "stark-fri-v1") {
            "Solana source-state proof request.proofFamily must be stark-fri-v1"
        }
        require(request.circuitId == ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1) {
            "request.circuitId must be the Solana AccountsLtHash OpenVerify circuit"
        }
        require(request.parameterSet == ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1) {
            "request.parameterSet must be fastpq-lane-balanced"
        }
        require(request.sourceDomain == DOMAIN_SOLANA) {
            "Solana source-state proof request.sourceDomain must be Solana"
        }
        val finalizedSlot = normalizeU64(request.finalizedSlot, "request.finalizedSlot")
        val parentSlot = normalizeU64(request.parentSlot, "request.parentSlot")
        require(parentSlot.add(BigInteger.ONE) == finalizedSlot) {
            "request.parentSlot must be the direct parent of finalizedSlot"
        }
        require(request.sourceStateVerifierId == MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1) {
            "request.sourceStateVerifierId must match Solana AccountsDB verifier profile"
        }
        val sourceStateVerifierHash =
            normalizeNonZeroHex32(request.sourceStateVerifierHash, "request.sourceStateVerifierHash")
        require(sourceStateVerifierHash != TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1) {
            "request.sourceStateVerifierHash must not be the Solana template verifier hash"
        }
        val accountsLtHashProofPublicInputsHash = normalizeNonZeroHex32(
            request.accountsLtHashProofPublicInputsHash,
            "request.accountsLtHashProofPublicInputsHash",
        )
        normalizeNonZeroHex32(
            request.openedAccountsLtHashContributionsHash,
            "request.openedAccountsLtHashContributionsHash",
        )
        normalizeNonZeroHex32(
            request.openedAccountsLtHashResidualChecksum,
            "request.openedAccountsLtHashResidualChecksum",
        )
        requireSolanaOpenVerifyRequestPayloadForWrapping(
            statementBytes = request.statementBytes,
            accountCommitmentBytes = request.accountCommitmentBytes,
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
            transitionEntries = request.fastpqTransitions.map {
                SourceStateTransitionCheck(it.key, it.operation, it.oldValue, it.newValue)
            },
            expectedTransitions = listOf(
                SourceStateTransitionCheck(
                    ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY_V1,
                    "meta_set",
                    ByteArray(0),
                    request.statementBytes,
                ),
                SourceStateTransitionCheck(
                    ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY_V1,
                    "meta_set",
                    ByteArray(0),
                    request.accountCommitmentBytes,
                ),
                SourceStateTransitionCheck(
                    ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY_V1,
                    "meta_set",
                    ByteArray(0),
                    hex32Bytes(
                        request.openedAccountsLtHashContributionsHash,
                        "request.openedAccountsLtHashContributionsHash",
                    ),
                ),
                SourceStateTransitionCheck(
                    ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY_V1,
                    "meta_set",
                    ByteArray(0),
                    hex32Bytes(
                        request.openedAccountsLtHashResidualChecksum,
                        "request.openedAccountsLtHashResidualChecksum",
                    ),
                ),
                SourceStateTransitionCheck(
                    ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY_V1,
                    "meta_set",
                    ByteArray(0),
                    request.verificationContextBytes,
                ),
            ),
        )
        require(
            accountsLtHashProofPublicInputsHash == hashHex(
                ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX_V1,
                request.statementBytes,
            ),
        ) { "request.accountsLtHashProofPublicInputsHash must match request.statementBytes" }
        val expectedDsid = "0x" + hexLower(
            hashBytes(
                ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX_V1,
                hex32Bytes(accountsLtHashProofPublicInputsHash, "request.accountsLtHashProofPublicInputsHash"),
            ).copyOfRange(0, 16),
        )
        require(
            normalizeHexBytes(request.fastpqPublicInputs.dsid, "request.fastpqPublicInputs.dsid", 16) == expectedDsid,
        ) { "request.fastpqPublicInputs.dsid must match request.statementBytes" }
        require(
            normalizeNonZeroHex32(
                request.fastpqPublicInputs.txSetHash,
                "request.fastpqPublicInputs.txSetHash",
            ) == accountsLtHashProofPublicInputsHash,
        ) { "request.fastpqPublicInputs.txSetHash must match request.statementBytes" }
        requireSolanaSourceStatePublicInputBindingForWrapping(request)
    }

    private fun requireSourceStateProofRequestForWrapping(
        request: SolanaSccpFullLightClientAuditProofRequest,
    ) {
        require(request.version == 1) { "Solana source-state proof request.version must be 1" }
        require(request.proofFamily == "stark-fri-v1") {
            "Solana source-state proof request.proofFamily must be stark-fri-v1"
        }
        require(request.parameterSet == FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1) {
            "request.parameterSet must be fastpq-lane-balanced"
        }
        require(request.sourceDomain == DOMAIN_SOLANA) {
            "Solana source-state proof request.sourceDomain must be Solana"
        }
        val profile = auditRoleProfileForRequest(request.role)
        require(request.roleCode == profile.code) { "request.roleCode must match request.role" }
        require(request.circuitId == profile.circuitId) {
            "request.circuitId must match request.role"
        }
        require(request.verifierId == profile.verifierId) {
            "request.verifierId must match request.role"
        }
        normalizeU64(request.finalizedSlot, "request.finalizedSlot")
        val verifierHash = normalizeNonZeroHex32(request.verifierHash, "request.verifierHash")
        require(request.sourceStateVerifierId == MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1) {
            "request.sourceStateVerifierId must match Solana AccountsDB verifier profile"
        }
        val sourceStateVerifierHash =
            normalizeNonZeroHex32(request.sourceStateVerifierHash, "request.sourceStateVerifierHash")
        require(sourceStateVerifierHash != TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1) {
            "request.sourceStateVerifierHash must not be the Solana template verifier hash"
        }
        var auditStatementHash = ""
        val roleSeparatedRequestHashes = mutableListOf(sourceStateVerifierHash)
        for ((field, hash) in listOf(
            "request.sourceVerifierMaterialHash" to request.sourceVerifierMaterialHash,
            "request.sourceAdapterDeploymentHash" to request.sourceAdapterDeploymentHash,
            "request.fullLightClientGateHash" to request.fullLightClientGateHash,
            "request.finalityContextHash" to request.finalityContextHash,
            "request.voteMessageHash" to request.voteMessageHash,
            "request.accountsLtHashProofHash" to request.accountsLtHashProofHash,
            "request.auditStatementHash" to request.auditStatementHash,
        )) {
            val normalizedHash = normalizeNonZeroHex32(hash, field)
            roleSeparatedRequestHashes.add(normalizedHash)
            if (field == "request.auditStatementHash") {
                auditStatementHash = normalizedHash
            }
        }
        require(!roleSeparatedRequestHashes.contains(verifierHash)) {
            "request.verifierHash must be role-separated from Solana full-light audit request hashes"
        }
        requireSolanaOpenVerifyRequestPayloadForWrapping(
            statementBytes = request.statementBytes,
            accountCommitmentBytes = null,
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
            transitionEntries = request.fastpqTransitions.map {
                SourceStateTransitionCheck(it.key, it.operation, it.oldValue, it.newValue)
            },
            expectedTransitions = listOf(
                SourceStateTransitionCheck(
                    "0x" + hexLower(
                        fullLightClientAuditFastpqKey(
                            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1,
                            profile,
                        ),
                    ),
                    "meta_set",
                    ByteArray(0),
                    request.statementBytes,
                ),
                SourceStateTransitionCheck(
                    "0x" + hexLower(
                        fullLightClientAuditFastpqKey(
                            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1,
                            profile,
                        ),
                    ),
                    "meta_set",
                    ByteArray(0),
                    request.verificationContextBytes,
                ),
                SourceStateTransitionCheck(
                    "0x" + hexLower(
                        fullLightClientAuditFastpqKey(
                            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1,
                            profile,
                        ),
                    ),
                    "meta_set",
                    ByteArray(0),
                    hex32Bytes(request.fullLightClientGateHash, "request.fullLightClientGateHash"),
                ),
            ),
        )
        require(
            auditStatementHash == hashHex(
                FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1,
                request.statementBytes,
            ),
        ) { "request.auditStatementHash must match request.statementBytes" }
        val dsidPreimage = byteArrayOf(profile.code.toByte()) +
            hex32Bytes(auditStatementHash, "request.auditStatementHash")
        val expectedDsid = "0x" + hexLower(
            hashBytes(
                FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1,
                dsidPreimage,
            ).copyOfRange(0, 16),
        )
        require(
            normalizeHexBytes(request.fastpqPublicInputs.dsid, "request.fastpqPublicInputs.dsid", 16) == expectedDsid,
        ) { "request.fastpqPublicInputs.dsid must match request.statementBytes" }
        require(
            normalizeNonZeroHex32(
                request.fastpqPublicInputs.txSetHash,
                "request.fastpqPublicInputs.txSetHash",
            ) == auditStatementHash,
        ) { "request.fastpqPublicInputs.txSetHash must match request.statementBytes" }
        requireSolanaSourceStatePublicInputBindingForWrapping(request)
    }

    private fun requireSolanaSourceStatePublicInputBindingForWrapping(
        request: SolanaSccpAccountsLtHashProofRequest,
    ) {
        val publicInputColumns = request.publicInputColumns
        require(publicInputColumns.isNotEmpty()) { "request.publicInputColumns is required" }
        val sourceDomainColumn = "0x" + hexLower(sccpWordU32Le(DOMAIN_SOLANA))
        val mainnetGenesisColumn = solanaMainnetGenesisHashPublicInput()
        require(request.circuitId == ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1) {
            "request.circuitId must be the Solana AccountsLtHash OpenVerify circuit"
        }
        requirePublicInputColumn(publicInputColumns, 0, sourceDomainColumn, "source_domain")
        requirePublicInputColumn(publicInputColumns, 1, mainnetGenesisColumn, "mainnet_genesis_hash")
        requirePublicInputColumn(
            publicInputColumns,
            2,
            "0x" + hexLower(sccpWordU64Le(normalizeU64(request.finalizedSlot, "request.finalizedSlot"))),
            "finalized_slot",
        )
        requirePublicInputColumn(
            publicInputColumns,
            3,
            "0x" + hexLower(sccpWordU64Le(normalizeU64(request.parentSlot, "request.parentSlot"))),
            "parent_slot",
        )
        requirePublicInputColumn(
            publicInputColumns,
            11,
            normalizeNonZeroHex32(
                request.accountsLtHashProofPublicInputsHash,
                "request.accountsLtHashProofPublicInputsHash",
            ),
            "accounts_lt_hash_proof_public_inputs_hash",
        )
        requirePublicInputColumn(
            publicInputColumns,
            12,
            normalizeNonZeroHex32(
                request.openedAccountsLtHashContributionsHash,
                "request.openedAccountsLtHashContributionsHash",
            ),
            "opened_accounts_lt_hash_contributions_hash",
        )
        requirePublicInputColumn(
            publicInputColumns,
            13,
            normalizeNonZeroHex32(
                request.openedAccountsLtHashResidualChecksum,
                "request.openedAccountsLtHashResidualChecksum",
            ),
            "opened_accounts_lt_hash_residual_checksum",
        )
    }

    private fun requireSolanaSourceStatePublicInputBindingForWrapping(
        request: SolanaSccpFullLightClientAuditProofRequest,
    ) {
        val publicInputColumns = request.publicInputColumns
        require(publicInputColumns.isNotEmpty()) { "request.publicInputColumns is required" }
        val sourceDomainColumn = "0x" + hexLower(sccpWordU32Le(DOMAIN_SOLANA))
        val mainnetGenesisColumn = solanaMainnetGenesisHashPublicInput()
        val profile = auditRoleProfileForRequest(request.role)
        requirePublicInputColumn(
            publicInputColumns,
            0,
            "0x" + hexLower(sccpWordU8(profile.code)),
            "role",
        )
        requirePublicInputColumn(publicInputColumns, 1, sourceDomainColumn, "source_domain")
        requirePublicInputColumn(publicInputColumns, 2, mainnetGenesisColumn, "mainnet_genesis_hash")
        requirePublicInputColumn(
            publicInputColumns,
            3,
            "0x" + hexLower(sccpWordU64Le(normalizeU64(request.finalizedSlot, "request.finalizedSlot"))),
            "finalized_slot",
        )
        for ((index, hash, fieldName) in listOf(
            Triple(4, normalizeNonZeroHex32(request.finalityContextHash, "request.finalityContextHash"), "finality_context_hash"),
            Triple(5, normalizeNonZeroHex32(request.auditStatementHash, "request.auditStatementHash"), "audit_statement_hash"),
            Triple(
                6,
                normalizeNonZeroHex32(request.sourceVerifierMaterialHash, "request.sourceVerifierMaterialHash"),
                "source_verifier_material_hash",
            ),
            Triple(
                7,
                normalizeNonZeroHex32(request.sourceAdapterDeploymentHash, "request.sourceAdapterDeploymentHash"),
                "source_adapter_deployment_hash",
            ),
            Triple(8, normalizeNonZeroHex32(request.fullLightClientGateHash, "request.fullLightClientGateHash"), "full_light_client_gate_hash"),
            Triple(9, normalizeNonZeroHex32(request.verifierHash, "request.verifierHash"), "verifier_hash"),
            Triple(13, normalizeNonZeroHex32(request.voteMessageHash, "request.voteMessageHash"), "vote_message_hash"),
            Triple(14, normalizeNonZeroHex32(request.accountsLtHashProofHash, "request.accountsLtHashProofHash"), "accounts_lt_hash_proof_hash"),
        )) {
            requirePublicInputColumn(publicInputColumns, index, hash, fieldName)
        }
    }

    private fun requirePublicInputColumn(
        publicInputColumns: List<List<String>>,
        index: Int,
        expected: String,
        fieldName: String,
    ) {
        require(
            index < publicInputColumns.size &&
                publicInputColumns[index].size == 1 &&
                normalizeNonEmpty(
                    publicInputColumns[index][0],
                    "request.publicInputColumns[$index][0]",
                ) == expected,
        ) {
            "request.publicInputColumns must bind $fieldName"
        }
    }

    private fun auditRoleProfileForRequest(role: String): FullLightClientAuditRoleProfile =
        when (role) {
            "towerReplay", "tower_replay" -> auditRoleProfile(SolanaSccpFullLightClientAuditRole.TOWER_REPLAY)
            "fullAccountsdbLattice", "full_accountsdb_lattice" ->
                auditRoleProfile(SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE)
            "bankForkChoice", "bank_fork_choice" ->
                auditRoleProfile(SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE)
            else -> throw IllegalArgumentException(
                "request.role must be tower_replay, full_accountsdb_lattice, or bank_fork_choice",
            )
        }

    private data class SourceStateTransitionCheck(
        val key: String,
        val operation: String,
        val oldValue: ByteArray,
        val newValue: ByteArray,
    )

    private fun requireSolanaOpenVerifyRequestPayloadForWrapping(
        statementBytes: ByteArray,
        accountCommitmentBytes: ByteArray?,
        verificationContextBytes: ByteArray,
        schemaDescriptor: ByteArray,
        publicInputColumns: List<List<String>>,
        fastpqFields: List<String>,
        transitionEntries: List<SourceStateTransitionCheck>,
        expectedTransitions: List<SourceStateTransitionCheck>,
    ) {
        require(statementBytes.isNotEmpty()) { "request.statementBytes must not be empty" }
        if (accountCommitmentBytes != null) {
            require(accountCommitmentBytes.isNotEmpty()) {
                "request.accountCommitmentBytes must not be empty"
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
            require(transition.newValue.isNotEmpty()) {
                "request.fastpqTransitions[$index].newValue must not be empty"
            }
        }
        val actual = transitionEntries.sortedBy { it.key }
        val expected = expectedTransitions.sortedBy { it.key }
        require(actual.size == expected.size && actual.zip(expected).all { (left, right) ->
            left.key == right.key &&
                left.operation == right.operation &&
                left.oldValue.contentEquals(right.oldValue) &&
                left.newValue.contentEquals(right.newValue)
        }) {
            "request.fastpqTransitions must match the canonical Solana source-state request"
        }
    }

    private fun wrapSourceStateVerificationProof(
        proofBytes: ByteArray,
        version: Int,
        proofFamily: String,
        circuitId: String,
        sourceDomain: Int,
    ): SolanaSccpSourceStateVerificationProof {
        require(version == 1) { "sourceStateProof.version must be 1" }
        require(proofFamily == "stark-fri-v1") { "sourceStateProof.proofFamily must be stark-fri-v1" }
        requireSourceStateProofLabel(proofFamily, "sourceStateProof.proofFamily")
        requireSourceStateProofLabel(circuitId, "sourceStateProof.circuitId")
        require(sourceDomain == DOMAIN_SOLANA) { "sourceStateProof.sourceDomain must be Solana" }
        require(
            circuitId == ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1 ||
                circuitId == TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1 ||
                circuitId == FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1 ||
                circuitId == BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
        ) {
            "sourceStateProof.circuitId must be a Solana source-state verification circuit"
        }
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= SOURCE_STATE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $SOURCE_STATE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        return SolanaSccpSourceStateVerificationProof(
            version = version,
            proofFamily = proofFamily,
            circuitId = circuitId,
            proofBytes = proofBytes,
        )
    }

    @JvmStatic
    fun canonicalFullLightClientAuditFinalityContextBytes(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): ByteArray {
        val context = normalizeFullLightClientAuditInput(
            input,
            SolanaSccpFullLightClientAuditRole.TOWER_REPLAY,
        ).context
        val out = ByteArrayOutputStream()
        out.write(context.version)
        writeU64Le(out, context.epoch)
        writeU64Le(out, context.rootedSlot)
        writeU64Le(out, context.parentSlot)
        writeU32Le(out, context.towerVoteSlots.size)
        context.towerVoteSlots.forEach { writeU64Le(out, it) }
        out.write(hex32Bytes(context.parentBankHash, "parentBankHash"))
        writeU64Le(out, context.bankSignatureCount)
        writeVec(out, context.bankHashHardForkData)
        out.write(hex32Bytes(context.epochStakeRoot, "epochStakeRoot"))
        out.write(hex32Bytes(context.stakeActivationHash, "stakeActivationHash"))
        out.write(hex32Bytes(context.stakeAccountStateHash, "stakeAccountStateHash"))
        out.write(hex32Bytes(context.stakeHistoryHash, "stakeHistoryHash"))
        out.write(hex32Bytes(context.stakeHistorySysvarAccountHash, "stakeHistorySysvarAccountHash"))
        out.write(hex32Bytes(context.accountInclusionRoot, "accountInclusionRoot"))
        out.write(hex32Bytes(context.accountsLtHashChecksum, "accountsLtHashChecksum"))
        out.write(
            hex32Bytes(
                context.accountsLtHashProofPublicInputsHash,
                "accountsLtHashProofPublicInputsHash",
            ),
        )
        out.write(hex32Bytes(context.towerLockoutHash, "towerLockoutHash"))
        out.write(hex32Bytes(context.towerReplayHash, "towerReplayHash"))
        out.write(hex32Bytes(context.bankForkHash, "bankForkHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun fullLightClientAuditFinalityContextHash(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): String =
        hashHex(
            "sccp:solana:finality-context:v1",
            canonicalFullLightClientAuditFinalityContextBytes(input),
        )

    @JvmStatic
    fun canonicalFullLightClientAuditVoteMessageBytes(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): ByteArray {
        val value = normalizeFullLightClientAuditInput(
            input,
            SolanaSccpFullLightClientAuditRole.TOWER_REPLAY,
        )
        val witness = value.witness
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, DOMAIN_SOLANA)
        writeU64Le(out, normalizeU64(witness.finalizedSlot, "finalizedSlot"))
        out.write(hex32Bytes(witness.blockhash, "blockhash"))
        out.write(hex32Bytes(witness.bankHash, "bankHash"))
        out.write(hex32Bytes(witness.transactionStatusRoot, "transactionStatusRoot"))
        out.write(hex32Bytes(witness.messageProofHash, "messageProofHash"))
        out.write(hex32Bytes(value.finalityContextHash, "finalityContextHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun fullLightClientAuditVoteMessageHash(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): String =
        hashHex(
            "sccp:solana:finalized-vote:v1",
            canonicalFullLightClientAuditVoteMessageBytes(input),
        )

    @JvmStatic
    fun canonicalFullLightClientAuditStatementBytes(
        input: SolanaSccpFullLightClientAuditProofInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): ByteArray {
        val value = normalizeFullLightClientAuditInput(input, role)
        val profile = auditRoleProfile(role)
        val witness = value.witness
        val context = value.context
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(profile.code)
        writeString(out, profile.circuitId, "circuitId")
        writeString(out, RECURSIVE_PROOF_BACKEND_V1, "backend")
        writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash")
        writeU32Le(out, DOMAIN_SOLANA)
        writeU64Le(out, context.epoch)
        writeU64Le(out, normalizeU64(witness.finalizedSlot, "finalizedSlot"))
        writeU64Le(out, context.rootedSlot)
        writeU64Le(out, context.parentSlot)
        out.write(hex32Bytes(value.finalityContextHash, "finalityContextHash"))
        out.write(hex32Bytes(value.voteMessageHash, "voteMessageHash"))
        out.write(hex32Bytes(value.accountsLtHashProofHash, "accountsLtHashProofHash"))
        when (role) {
            SolanaSccpFullLightClientAuditRole.TOWER_REPLAY -> {
                out.write(hex32Bytes(context.towerLockoutHash, "towerLockoutHash"))
                out.write(hex32Bytes(context.towerReplayHash, "towerReplayHash"))
                out.write(hex32Bytes(context.bankForkHash, "bankForkHash"))
                out.write(hex32Bytes(context.epochStakeRoot, "epochStakeRoot"))
                out.write(hex32Bytes(context.stakeActivationHash, "stakeActivationHash"))
                out.write(hex32Bytes(context.stakeAccountStateHash, "stakeAccountStateHash"))
                out.write(hex32Bytes(context.stakeHistoryHash, "stakeHistoryHash"))
                out.write(hex32Bytes(context.stakeHistorySysvarAccountHash, "stakeHistorySysvarAccountHash"))
                out.write(hex32Bytes(context.accountInclusionRoot, "accountInclusionRoot"))
                writeU32Le(out, context.towerVoteSlots.size)
                context.towerVoteSlots.forEach { writeU64Le(out, it) }
            }
            SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE -> {
                out.write(hex32Bytes(context.accountInclusionRoot, "accountInclusionRoot"))
                out.write(hex32Bytes(context.accountsLtHashChecksum, "accountsLtHashChecksum"))
                out.write(
                    hex32Bytes(
                        context.accountsLtHashProofPublicInputsHash,
                        "accountsLtHashProofPublicInputsHash",
                    ),
                )
                out.write(
                    hex32Bytes(
                        value.openedAccountsLtHashContributionsHash,
                        "openedAccountsLtHashContributionsHash",
                    ),
                )
                out.write(
                    hex32Bytes(
                        value.openedAccountsLtHashResidualChecksum,
                        "openedAccountsLtHashResidualChecksum",
                    ),
                )
                out.write(hex32Bytes(value.accountsLtHashProofHash, "accountsLtHashProofHash"))
            }
            SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE -> {
                out.write(hex32Bytes(context.parentBankHash, "parentBankHash"))
                out.write(hex32Bytes(witness.bankHash, "bankHash"))
                out.write(hex32Bytes(witness.blockhash, "blockhash"))
                out.write(hex32Bytes(witness.transactionStatusRoot, "transactionStatusRoot"))
                out.write(hex32Bytes(context.accountInclusionRoot, "accountInclusionRoot"))
                out.write(hex32Bytes(context.accountsLtHashChecksum, "accountsLtHashChecksum"))
                writeU64Le(out, context.bankSignatureCount)
                writeVec(out, context.bankHashHardForkData)
                out.write(hex32Bytes(context.bankForkHash, "bankForkHash"))
                out.write(hex32Bytes(context.towerReplayHash, "towerReplayHash"))
            }
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun fullLightClientAuditStatementHash(
        input: SolanaSccpFullLightClientAuditProofInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): String =
        hashHex(
            FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1,
            canonicalFullLightClientAuditStatementBytes(input, role),
        )

    @JvmStatic
    fun fullLightClientAuditPublicInputColumns(
        input: SolanaSccpFullLightClientAuditProofInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): List<List<String>> {
        val value = normalizeFullLightClientAuditInput(input, role)
        val profile = auditRoleProfile(role)
        val columns = mutableListOf(
            listOf("0x" + hexLower(sccpWordU8(profile.code))),
            listOf("0x" + hexLower(sccpWordU32Le(DOMAIN_SOLANA))),
            listOf(solanaMainnetGenesisHashPublicInput()),
            listOf("0x" + hexLower(sccpWordU64Le(normalizeU64(value.witness.finalizedSlot, "finalizedSlot")))),
            listOf(value.finalityContextHash),
            listOf(fullLightClientAuditStatementHash(input, role)),
            listOf(value.sourceVerifierMaterialHash),
            listOf(value.sourceAdapterDeploymentHash),
            listOf(value.fullLightClientGateHash),
            listOf(value.verifierHash),
            listOf("0x" + hexLower(sccpWordU64Le(value.context.epoch))),
            listOf("0x" + hexLower(sccpWordU64Le(value.context.rootedSlot))),
            listOf("0x" + hexLower(sccpWordU64Le(value.context.parentSlot))),
            listOf(value.voteMessageHash),
            listOf(value.accountsLtHashProofHash),
        )
        auditRoleColumns(value, role).forEach { columns.add(listOf(it)) }
        return columns
    }

    @JvmStatic
    fun fullLightClientAuditOpenVerifySchemaDescriptor(
        input: SolanaSccpFullLightClientAuditProofInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): ByteArray {
        val value = normalizeFullLightClientAuditInput(input, role)
        val profile = auditRoleProfile(role)
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(profile.code)
        writeString(out, profile.circuitId, "circuitId")
        writeString(out, FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1, "parameterSet")
        writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash")
        writeU32Le(out, DOMAIN_SOLANA)
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
            "mainnet_genesis_hash",
            "finalized_slot",
            "finality_context_hash",
            "audit_statement_hash",
            "source_verifier_material_hash",
            "source_adapter_deployment_hash",
            "full_light_client_gate_hash",
            "verifier_hash",
            "epoch",
            "rooted_slot",
            "parent_slot",
            "vote_message_hash",
            "accounts_lt_hash_proof_hash",
        ).plus(profile.requiredInputNames).forEach { writeString(out, it, "requiredInput") }
        return out.toByteArray()
    }

    @JvmStatic
    fun buildFullLightClientAuditProofRequest(
        input: SolanaSccpFullLightClientAuditProofInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): SolanaSccpFullLightClientAuditProofRequest {
        val value = normalizeFullLightClientAuditInput(input, role)
        val profile = auditRoleProfile(role)
        val statementBytes = canonicalFullLightClientAuditStatementBytes(input, role)
        val auditStatementHash = fullLightClientAuditStatementHash(input, role)
        requireFullLightClientAuditRoleRequestHashSeparation(value, auditStatementHash)
        val verificationContextBytes = canonicalFullLightClientAuditContextBytes(value, auditStatementHash)
        val transitions = listOf(
            SolanaSccpFullLightClientAuditFastpqTransition(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = ByteArray(0),
                newValue = statementBytes,
            ),
            SolanaSccpFullLightClientAuditFastpqTransition(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = ByteArray(0),
                newValue = verificationContextBytes,
            ),
            SolanaSccpFullLightClientAuditFastpqTransition(
                key = "0x" + hexLower(
                    fullLightClientAuditFastpqKey(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1, profile),
                ),
                operation = "meta_set",
                oldValue = ByteArray(0),
                newValue = hex32Bytes(value.fullLightClientGateHash, "fullLightClientGateHash"),
            ),
        ).sortedBy { it.key }
        return SolanaSccpFullLightClientAuditProofRequest(
            version = 1,
            proofFamily = "stark-fri-v1",
            circuitId = profile.circuitId,
            parameterSet = FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1,
            role = profile.name,
            roleCode = profile.code,
            sourceDomain = DOMAIN_SOLANA,
            finalizedSlot = value.witness.finalizedSlot,
            verifierId = profile.verifierId,
            verifierHash = value.verifierHash,
            sourceStateVerifierId = value.witness.sourceStateVerifierId,
            sourceStateVerifierHash = value.witness.sourceStateVerifierHash,
            sourceVerifierMaterialHash = value.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = value.sourceAdapterDeploymentHash,
            fullLightClientGateHash = value.fullLightClientGateHash,
            finalityContextHash = value.finalityContextHash,
            voteMessageHash = value.voteMessageHash,
            accountsLtHashProofHash = value.accountsLtHashProofHash,
            auditStatementHash = auditStatementHash,
            statementBytes = statementBytes,
            verificationContextBytes = verificationContextBytes,
            schemaDescriptor = fullLightClientAuditOpenVerifySchemaDescriptor(input, role),
            publicInputColumns = fullLightClientAuditPublicInputColumns(input, role),
            fastpqPublicInputs = fullLightClientAuditFastpqPublicInputs(value, auditStatementHash),
            fastpqTransitions = transitions,
        )
    }

    private fun requireFullLightClientAuditRoleRequestHashSeparation(
        value: NormalizedFullLightClientAuditInput,
        auditStatementHash: String,
    ) {
        val requestHashes = listOf(
            value.witness.sourceStateVerifierHash,
            value.sourceVerifierMaterialHash,
            value.sourceAdapterDeploymentHash,
            value.fullLightClientGateHash,
            value.finalityContextHash,
            value.voteMessageHash,
            value.accountsLtHashProofHash,
            auditStatementHash,
        )
        require(value.verifierHash !in requestHashes) {
            "verifierHash must be role-separated from Solana full-light audit request hashes"
        }
    }

    @JvmStatic
    fun buildTowerReplayProofRequest(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): SolanaSccpFullLightClientAuditProofRequest =
        buildFullLightClientAuditProofRequest(input, SolanaSccpFullLightClientAuditRole.TOWER_REPLAY)

    @JvmStatic
    fun buildFullAccountsdbLatticeProofRequest(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): SolanaSccpFullLightClientAuditProofRequest =
        buildFullLightClientAuditProofRequest(input, SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE)

    @JvmStatic
    fun buildBankForkChoiceProofRequest(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): SolanaSccpFullLightClientAuditProofRequest =
        buildFullLightClientAuditProofRequest(input, SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE)

    @JvmStatic
    fun buildFullLightClientAuditProofRequests(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): SolanaSccpFullLightClientAuditProofRequests =
        SolanaSccpFullLightClientAuditProofRequests(
            towerReplay = buildTowerReplayProofRequest(input),
            fullAccountsdbLattice = buildFullAccountsdbLatticeProofRequest(input),
            bankForkChoice = buildBankForkChoiceProofRequest(input),
        )

    @JvmStatic
    fun buildProofRequest(input: SolanaSccpWitnessInput): SolanaSccpProofRequest {
        val witness = normalizeWitness(input)
        val witnessHash = hashHex("sccp:solana:witness:v1", canonicalWitnessBytes(witness))
        val proofContext = normalizeProofContext(input.statementHash, input.destinationBindingHash)
        val proofContextHash = proofContextHash(proofContext)
        val deploymentBinding = normalizeSourceAdapterDeploymentBinding(
            sourceDomain = witness.sourceDomain,
            targetDomain = witness.targetDomain,
            sourceAdapterDeploymentHash = witness.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash = witness.sourceAdapterDeploymentReceiptHash,
        )
        require(deploymentBinding.sourceAdapterDeploymentHash != ZERO_HASH_V1) {
            "Solana SCCP proof request requires non-zero source adapter deployment binding"
        }
        val deploymentBindingHash = sourceAdapterDeploymentBindingHash(deploymentBinding)
        return SolanaSccpProofRequest(
            version = 1,
            backend = RECURSIVE_PROOF_BACKEND_V1,
            sourceDomain = DOMAIN_SOLANA,
            targetDomain = witness.targetDomain,
            mainnetGenesisHash = witness.mainnetGenesisHash,
            witnessHash = witnessHash,
            proofContextHash = proofContextHash,
            sourceAdapterDeploymentBindingHash = deploymentBindingHash,
            sourceStateVerifierId = witness.sourceStateVerifierId,
            sourceStateVerifierHash = witness.sourceStateVerifierHash,
            publicInputs = SolanaSccpPublicInputs(
                messageId = witness.messageId,
                payloadHash = witness.payloadHash,
                commitmentRoot = witness.commitmentRoot,
                finalizedSlot = witness.finalizedSlot,
                parentSlot = witness.parentSlot,
                bankSignatureCount = witness.bankSignatureCount,
                parentBankHash = witness.parentBankHash,
                blockhash = witness.blockhash,
                bankHash = witness.bankHash,
                transactionStatusRoot = witness.transactionStatusRoot,
                messageProofHash = witness.messageProofHash,
                accountInclusionRoot = witness.accountInclusionRoot,
                accountsLtHashChecksum = witness.accountsLtHashChecksum,
                accountsLtHashProofPublicInputsHash = witness.accountsLtHashProofPublicInputsHash,
                sourceEventDigest = witness.sourceEventDigest,
                sourceStateVerifierId = witness.sourceStateVerifierId,
                sourceStateVerifierHash = witness.sourceStateVerifierHash,
                statementHash = proofContext.statementHash,
                destinationBindingHash = proofContext.destinationBindingHash,
                sourceAdapterDeploymentHash = deploymentBinding.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash = deploymentBinding.sourceAdapterDeploymentReceiptHash,
                sourceAdapterDeploymentBindingHash = deploymentBindingHash,
            ),
            witness = witness,
            proofContext = proofContext,
            sourceAdapterDeploymentBinding = deploymentBinding,
        )
    }

    @JvmStatic
    fun normalizeSourceAdapterDeploymentBinding(
        sourceDomain: Int = DOMAIN_SOLANA,
        targetDomain: Int = DOMAIN_SORA,
        sourceAdapterDeploymentHash: String = ZERO_HASH_V1,
        sourceAdapterDeploymentReceiptHash: String = ZERO_HASH_V1,
    ): SolanaSccpSourceAdapterDeploymentBinding {
        val deploymentHash = normalizeHex32(sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash")
        val receiptHash = normalizeHex32(
            sourceAdapterDeploymentReceiptHash,
            "sourceAdapterDeploymentReceiptHash",
        )
        require(
            sourceDomain == SccpSourceProofs.DOMAIN_ETH ||
                sourceDomain == SccpSourceProofs.DOMAIN_BSC ||
                sourceDomain == SccpSourceProofs.DOMAIN_SOL ||
                sourceDomain == SccpSourceProofs.DOMAIN_TON ||
                sourceDomain == SccpSourceProofs.DOMAIN_TRON,
        ) {
            "sourceAdapterDeploymentBinding.sourceDomain must be a launch-scope remote domain"
        }
        require(targetDomain == DOMAIN_SORA) {
            "sourceAdapterDeploymentBinding.targetDomain must be SORA"
        }
        val deploymentIsZero = deploymentHash == ZERO_HASH_V1
        val receiptIsZero = receiptHash == ZERO_HASH_V1
        require(deploymentIsZero == receiptIsZero) {
            "sourceAdapterDeploymentHash and sourceAdapterDeploymentReceiptHash must both be zero or both be non-zero"
        }
        require(deploymentIsZero || deploymentHash != receiptHash) {
            "sourceAdapterDeploymentHash must differ from sourceAdapterDeploymentReceiptHash"
        }
        return SolanaSccpSourceAdapterDeploymentBinding(
            version = 1,
            sourceDomain = sourceDomain,
            targetDomain = targetDomain,
            sourceAdapterDeploymentHash = deploymentHash,
            sourceAdapterDeploymentReceiptHash = receiptHash,
        )
    }

    @JvmStatic
    fun canonicalSourceAdapterDeploymentBindingBytes(
        binding: SolanaSccpSourceAdapterDeploymentBinding,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(binding.version)
        writeU32Le(out, binding.sourceDomain)
        writeU32Le(out, binding.targetDomain)
        out.write(hex32Bytes(binding.sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"))
        out.write(hex32Bytes(binding.sourceAdapterDeploymentReceiptHash, "sourceAdapterDeploymentReceiptHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun sourceAdapterDeploymentBindingHash(
        binding: SolanaSccpSourceAdapterDeploymentBinding,
    ): String =
        hashHex(
            "sccp:source-adapter-deployment-binding:v1",
            canonicalSourceAdapterDeploymentBindingBytes(binding),
        )

    @JvmStatic
    fun normalizeProofContext(
        statementHash: String,
        destinationBindingHash: String,
    ): SolanaSccpProofContext =
        SolanaSccpProofContext(
            version = 1,
            statementHash = normalizeNonZeroHex32(statementHash, "statementHash"),
            destinationBindingHash = normalizeNonZeroHex32(destinationBindingHash, "destinationBindingHash"),
        )

    @JvmStatic
    fun canonicalProofContextBytes(context: SolanaSccpProofContext): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(context.version)
        out.write(hex32Bytes(context.statementHash, "statementHash"))
        out.write(hex32Bytes(context.destinationBindingHash, "destinationBindingHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun proofContextHash(context: SolanaSccpProofContext): String =
        hashHex("sccp:solana:proof-context:v1", canonicalProofContextBytes(context))

    @JvmStatic
    fun canonicalSubmissionPublicInputsBytes(input: SolanaSccpSubmissionPublicInputs): ByteArray {
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
    fun buildSubmission(input: SolanaSccpSubmissionInput): SolanaSccpSubmission {
        val bundleBytes = requireNativeRecursivePayloadBytes(input.bundleBytes, "bundleBytes")
        require(input.publicInputs.version == 1) { "publicInputs.version must be 1" }
        require(input.publicInputs.targetDomain == DOMAIN_SOLANA) {
            "publicInputs.targetDomain must be Solana"
        }
        val proofResult = input.proofResult
            ?: throw IllegalArgumentException("proofResult must be a wrapped Solana SCCP proof result")
        val checkedProofResult = requireWrappedProofResultForSubmission(proofResult, input.publicInputs)
        val proofBytes = checkedProofResult.proofBytes
        require(input.proofBytes.contentEquals(proofBytes)) {
            "proofBytes must match proofResult.proofBytes"
        }
        val publicInputsBytes = canonicalSubmissionPublicInputsBytes(input.publicInputs)
        val proofContext = checkedProofResult.proofContext
        val proofContextStatementHash = normalizeHex32(
            proofContext.statementHash,
            "proofResult.proofContext.statementHash",
        )
        val proofContextDestinationBindingHash = normalizeHex32(
            proofContext.destinationBindingHash,
            "proofResult.proofContext.destinationBindingHash",
        )
        require(normalizeHex32(input.statementHash, "statementHash") == proofContextStatementHash) {
            "statementHash must match proofResult.proofContext"
        }
        require(normalizeHex32(input.destinationBindingHash, "destinationBindingHash") == proofContextDestinationBindingHash) {
            "destinationBindingHash must match proofResult.proofContext"
        }
        require(proofContextDestinationBindingHash == SccpSourceProofs.destinationBindingHash(DOMAIN_SOLANA)) {
            "destinationBindingHash must match canonical Solana destination binding"
        }
        val expectedProofContextHash = proofContextHash(proofContext)
        input.proofContextHash?.let { supplied ->
            require(normalizeHex32(supplied, "proofContextHash") == expectedProofContextHash) {
                "proofContextHash must match statementHash and destinationBindingHash"
            }
        }
        val statementHashBytes = hex32Bytes(proofContextStatementHash, "statementHash")
        val destinationBindingHashBytes = hex32Bytes(
            proofContextDestinationBindingHash,
            "destinationBindingHash",
        )
        val proofContextHashBytes = hex32Bytes(expectedProofContextHash, "proofContextHash")
        val argumentPairs = listOf(
            "proof_bytes" to proofBytes,
            "public_inputs" to publicInputsBytes,
            "bundle_bytes" to bundleBytes,
            "statement_hash" to statementHashBytes,
            "destination_binding_hash" to destinationBindingHashBytes,
            "proof_context_hash" to proofContextHashBytes,
        )
        val instructionData = encodeInstructionData(argumentPairs.map { it.second })
        val arguments = argumentPairs.map { (key, bytes) ->
            SolanaSccpSubmissionArgument(key, "raw_bytes", "0x" + hexLower(bytes))
        }
        return SolanaSccpSubmission(
            version = 1,
            envelopeEncoding = BORSH_INSTRUCTION_V1,
            submissionKind = "program_instruction",
            verifierEntrypoint = SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
            proofBytes = proofBytes,
            publicInputs = input.publicInputs,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            statementHash = proofContextStatementHash,
            destinationBindingHash = proofContextDestinationBindingHash,
            proofContextHash = expectedProofContextHash,
            arguments = arguments,
            instructionData = instructionData,
            instructionDataHex = "0x" + hexLower(instructionData),
            envelopeBytes = instructionData.copyOf(),
            envelopeHex = "0x" + hexLower(instructionData),
        )
    }

    private fun requireNativeRecursivePayloadBytes(bytes: ByteArray, label: String): ByteArray {
        require(bytes.isNotEmpty()) { "$label must not be empty" }
        require(bytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "$label must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(bytes.any { it.toInt() != 0 }) { "$label must not be all zero" }
        return bytes.copyOf()
    }

    @JvmStatic
    fun wrapProofResult(
        proofBytes: ByteArray,
        request: SolanaSccpProofRequest,
    ): SolanaSccpProofResult {
        require(request.backend == RECURSIVE_PROOF_BACKEND_V1) {
            "Solana SCCP proof request backend must be sccp-solana-recursive-mainnet-v1"
        }
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofBytes must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        requireCanonicalProofRequest(request)
        requireProductionProofRequest(request)
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(request.witnessHash, "witnessHash"))
        envelopePayload.write(hex32Bytes(request.proofContextHash, "proofContextHash"))
        envelopePayload.write(
            hex32Bytes(
                request.sourceAdapterDeploymentBindingHash,
                "sourceAdapterDeploymentBindingHash",
            ),
        )
        envelopePayload.write(proofBytes)
        return SolanaSccpProofResult(
            version = 1,
            backend = request.backend,
            proofBytes = proofBytes.copyOf(),
            proofBase64 = Base64.getEncoder().encodeToString(proofBytes),
            publicInputs = request.publicInputs,
            witnessHash = request.witnessHash,
            proofContextHash = request.proofContextHash,
            sourceAdapterDeploymentBindingHash = request.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            proofContext = request.proofContext,
            sourceAdapterDeploymentBinding = request.sourceAdapterDeploymentBinding,
            envelopeHash = hashHex("sccp:solana:proof-envelope:v1", envelopePayload.toByteArray()),
        )
    }

    private fun requireProofResultSourcePublicInputShape(publicInputs: SolanaSccpPublicInputs) {
        val finalizedSlot = normalizeU64(publicInputs.finalizedSlot, "proofResult.publicInputs.finalizedSlot")
        val parentSlot = normalizeU64(publicInputs.parentSlot, "proofResult.publicInputs.parentSlot")
        require(parentSlot.add(BigInteger.ONE) == finalizedSlot) {
            "proofResult.publicInputs.parentSlot must be the direct parent of finalizedSlot"
        }
        val bankSignatureCount = normalizeU64(
            publicInputs.bankSignatureCount,
            "proofResult.publicInputs.bankSignatureCount",
        )
        require(bankSignatureCount != BigInteger.ZERO) {
            "proofResult.publicInputs.bankSignatureCount must be nonzero"
        }
        normalizeNonZeroHex32(publicInputs.parentBankHash, "proofResult.publicInputs.parentBankHash")
        normalizeNonZeroHex32(publicInputs.blockhash, "proofResult.publicInputs.blockhash")
        normalizeNonZeroHex32(publicInputs.bankHash, "proofResult.publicInputs.bankHash")
        normalizeNonZeroHex32(
            publicInputs.transactionStatusRoot,
            "proofResult.publicInputs.transactionStatusRoot",
        )
        normalizeNonZeroHex32(publicInputs.messageProofHash, "proofResult.publicInputs.messageProofHash")
        normalizeNonZeroHex32(publicInputs.accountInclusionRoot, "proofResult.publicInputs.accountInclusionRoot")
        normalizeNonZeroHex32(publicInputs.accountsLtHashChecksum, "proofResult.publicInputs.accountsLtHashChecksum")
        normalizeNonZeroHex32(
            publicInputs.accountsLtHashProofPublicInputsHash,
            "proofResult.publicInputs.accountsLtHashProofPublicInputsHash",
        )
        normalizeNonZeroHex32(publicInputs.sourceEventDigest, "proofResult.publicInputs.sourceEventDigest")
    }

    internal fun requireWrappedProofResultForSubmission(
        proofResult: SolanaSccpProofResult,
        publicInputs: SolanaSccpSubmissionPublicInputs,
    ): SolanaSccpProofResult {
        require(proofResult.version == 1) { "proofResult.version must be 1" }
        require(proofResult.backend == RECURSIVE_PROOF_BACKEND_V1) {
            "proofResult.backend must be sccp-solana-recursive-mainnet-v1"
        }
        require(proofResult.proofContext.version == 1) {
            "proofResult.proofContext.version must be 1"
        }
        val expectedProofContextHash = proofContextHash(proofResult.proofContext)
        require(
            normalizeHex32(
                proofResult.proofContextHash,
                "proofResult.proofContextHash",
            ) == expectedProofContextHash,
        ) { "proofResult.proofContextHash must match statementHash and destinationBindingHash" }
        val proofBytes = proofResult.proofBytes
        require(proofBytes.isNotEmpty()) { "proofResult.proofBytes must not be empty" }
        require(proofBytes.size <= NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofResult.proofBytes must be at most $NATIVE_RECURSIVE_MAX_PROOF_BYTES bytes"
        }
        require(proofBytes.any { it.toInt() != 0 }) { "proofResult.proofBytes must not be all zero" }
        require(proofResult.proofBase64 == Base64.getEncoder().encodeToString(proofBytes)) {
            "proofResult.proofBase64 must match proofResult.proofBytes"
        }
        val envelopeHash = normalizeHex32(proofResult.envelopeHash, "proofResult.envelopeHash")
        require(envelopeHash != ZERO_HASH_V1) {
            "proofResult.envelopeHash must be non-zero"
        }
        val normalizedSourceAdapterDeploymentBindingHash = normalizeHex32(
            proofResult.sourceAdapterDeploymentBindingHash,
            "proofResult.sourceAdapterDeploymentBindingHash",
        )
        require(normalizedSourceAdapterDeploymentBindingHash != ZERO_HASH_V1) {
            "proofResult.sourceAdapterDeploymentBindingHash must be non-zero"
        }
        val deploymentBinding = proofResult.sourceAdapterDeploymentBinding
        require(deploymentBinding.version == 1) {
            "proofResult.sourceAdapterDeploymentBinding.version must be 1"
        }
        require(deploymentBinding.sourceDomain == DOMAIN_SOLANA && deploymentBinding.targetDomain == DOMAIN_SORA) {
            "proofResult.sourceAdapterDeploymentBinding must be Solana -> SORA"
        }
        val deploymentHash = normalizeHex32(
            deploymentBinding.sourceAdapterDeploymentHash,
            "proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash",
        )
        val deploymentReceiptHash = normalizeHex32(
            deploymentBinding.sourceAdapterDeploymentReceiptHash,
            "proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash",
        )
        require(deploymentHash != ZERO_HASH_V1 && deploymentReceiptHash != ZERO_HASH_V1) {
            "proofResult.sourceAdapterDeploymentBinding deployment hashes must be non-zero"
        }
        val expectedSourceAdapterDeploymentBindingHash =
            sourceAdapterDeploymentBindingHash(deploymentBinding)
        require(normalizedSourceAdapterDeploymentBindingHash == expectedSourceAdapterDeploymentBindingHash) {
            "proofResult.sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding"
        }
        val envelopePayload = ByteArrayOutputStream()
        val witnessHash = normalizeNonZeroHex32(proofResult.witnessHash, "proofResult.witnessHash")
        envelopePayload.write(hex32Bytes(witnessHash, "proofResult.witnessHash"))
        envelopePayload.write(hex32Bytes(expectedProofContextHash, "proofResult.proofContextHash"))
        envelopePayload.write(
            hex32Bytes(
                normalizedSourceAdapterDeploymentBindingHash,
                "proofResult.sourceAdapterDeploymentBindingHash",
            ),
        )
        envelopePayload.write(proofBytes)
        require(envelopeHash == hashHex("sccp:solana:proof-envelope:v1", envelopePayload.toByteArray())) {
            "proofResult.envelopeHash must match wrapped proof bytes"
        }
        require(proofResult.sourceStateVerifierId == MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1) {
            "proofResult.sourceStateVerifierId must match Solana AccountsDB verifier profile"
        }
        val sourceStateVerifierHash = normalizeHex32(
            proofResult.sourceStateVerifierHash,
            "proofResult.sourceStateVerifierHash",
        )
        require(sourceStateVerifierHash != ZERO_HASH_V1) {
            "proofResult.sourceStateVerifierHash must be non-zero"
        }
        require(sourceStateVerifierHash != TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1) {
            "proofResult.sourceStateVerifierHash must not be the Solana template verifier hash"
        }
        require(proofResult.publicInputs.sourceStateVerifierId == proofResult.sourceStateVerifierId) {
            "proofResult.publicInputs.sourceStateVerifierId must match proofResult.sourceStateVerifierId"
        }
        require(
            normalizeHex32(
                proofResult.publicInputs.sourceStateVerifierHash,
                "proofResult.publicInputs.sourceStateVerifierHash",
            ) == sourceStateVerifierHash,
        ) {
            "proofResult.publicInputs.sourceStateVerifierHash must match proofResult.sourceStateVerifierHash"
        }
        requireProofResultSourcePublicInputShape(proofResult.publicInputs)
        val proofContextStatementHash = normalizeHex32(
            proofResult.proofContext.statementHash,
            "proofResult.proofContext.statementHash",
        )
        val proofContextDestinationBindingHash = normalizeHex32(
            proofResult.proofContext.destinationBindingHash,
            "proofResult.proofContext.destinationBindingHash",
        )
        require(
            normalizeHex32(
                proofResult.publicInputs.statementHash,
                "proofResult.publicInputs.statementHash",
            ) == proofContextStatementHash,
        ) { "proofResult.publicInputs.statementHash must match proofContext" }
        require(
            normalizeHex32(
                proofResult.publicInputs.destinationBindingHash,
                "proofResult.publicInputs.destinationBindingHash",
            ) == proofContextDestinationBindingHash,
        ) { "proofResult.publicInputs.destinationBindingHash must match proofContext" }
        require(
            normalizeHex32(
                proofResult.publicInputs.sourceAdapterDeploymentHash,
                "proofResult.publicInputs.sourceAdapterDeploymentHash",
            ) == deploymentHash,
        ) {
            "proofResult.publicInputs.sourceAdapterDeploymentHash must match sourceAdapterDeploymentBinding"
        }
        require(
            normalizeHex32(
                proofResult.publicInputs.sourceAdapterDeploymentReceiptHash,
                "proofResult.publicInputs.sourceAdapterDeploymentReceiptHash",
            ) == deploymentReceiptHash,
        ) {
            "proofResult.publicInputs.sourceAdapterDeploymentReceiptHash must match sourceAdapterDeploymentBinding"
        }
        require(
            normalizeHex32(
                proofResult.publicInputs.sourceAdapterDeploymentBindingHash,
                "proofResult.publicInputs.sourceAdapterDeploymentBindingHash",
            ) == expectedSourceAdapterDeploymentBindingHash,
        ) {
            "proofResult.publicInputs.sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding"
        }
        require(
            normalizeHex32(proofResult.publicInputs.messageId, "proofResult.publicInputs.messageId") ==
                normalizeHex32(publicInputs.messageId, "publicInputs.messageId"),
        ) { "proofResult.publicInputs.messageId must match publicInputs.messageId" }
        require(
            normalizeHex32(proofResult.publicInputs.payloadHash, "proofResult.publicInputs.payloadHash") ==
                normalizeHex32(publicInputs.payloadHash, "publicInputs.payloadHash"),
        ) { "proofResult.publicInputs.payloadHash must match publicInputs.payloadHash" }
        require(
            normalizeHex32(proofResult.publicInputs.commitmentRoot, "proofResult.publicInputs.commitmentRoot") ==
                normalizeHex32(publicInputs.commitmentRoot, "publicInputs.commitmentRoot"),
        ) { "proofResult.publicInputs.commitmentRoot must match publicInputs.commitmentRoot" }
        require(
            normalizeU64(proofResult.publicInputs.finalizedSlot, "proofResult.publicInputs.finalizedSlot") ==
                normalizeU64(publicInputs.finalityHeight, "publicInputs.finalityHeight"),
        ) { "proofResult.publicInputs.finalizedSlot must match publicInputs.finalityHeight" }
        require(
            normalizeHex32(proofResult.publicInputs.bankHash, "proofResult.publicInputs.bankHash") ==
                normalizeHex32(publicInputs.finalityBlockHash, "publicInputs.finalityBlockHash"),
        ) { "proofResult.publicInputs.bankHash must match publicInputs.finalityBlockHash" }
        return proofResult
    }

    private fun requireCanonicalProofRequest(request: SolanaSccpProofRequest) {
        val witness = request.witness
        val expected = buildProofRequest(
            SolanaSccpWitnessInput(
                targetDomain = witness.targetDomain,
                mainnetGenesisHash = witness.mainnetGenesisHash,
                finalizedSlot = witness.finalizedSlot,
                parentSlot = witness.parentSlot,
                bankSignatureCount = witness.bankSignatureCount,
                parentBankHash = witness.parentBankHash,
                blockhash = witness.blockhash,
                bankHash = witness.bankHash,
                transactionStatusRoot = witness.transactionStatusRoot,
                messageProofHash = witness.messageProofHash,
                accountInclusionRoot = witness.accountInclusionRoot,
                accountsLtHashChecksum = witness.accountsLtHashChecksum,
                accountsLtHashProofPublicInputsHash = witness.accountsLtHashProofPublicInputsHash,
                bankHashHardForkData = witness.bankHashHardForkData,
                accountsLtHash = witness.accountsLtHash,
                transactionSignature = witness.transactionSignature,
                emitterProgramId = witness.emitterProgramId,
                messageId = witness.messageId,
                payloadHash = witness.payloadHash,
                commitmentRoot = witness.commitmentRoot,
                sourceEventDigest = witness.sourceEventDigest,
                sourceStateVerifierId = witness.sourceStateVerifierId,
                sourceStateVerifierHash = witness.sourceStateVerifierHash,
                statementHash = request.proofContext.statementHash,
                destinationBindingHash = request.proofContext.destinationBindingHash,
                inclusionBranch = witness.inclusionBranch,
                sourceAdapterDeploymentHash = witness.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash = witness.sourceAdapterDeploymentReceiptHash,
            ),
        )
        require(
            request.version == expected.version &&
                request.backend == expected.backend &&
                request.sourceDomain == expected.sourceDomain &&
                request.targetDomain == expected.targetDomain &&
                request.mainnetGenesisHash == expected.mainnetGenesisHash &&
                request.witnessHash == expected.witnessHash &&
                request.proofContextHash == expected.proofContextHash &&
                request.sourceAdapterDeploymentBindingHash ==
                    expected.sourceAdapterDeploymentBindingHash &&
                request.sourceStateVerifierId == expected.sourceStateVerifierId &&
                request.sourceStateVerifierHash == expected.sourceStateVerifierHash &&
                request.publicInputs == expected.publicInputs &&
                canonicalWitnessBytes(request.witness).contentEquals(
                    canonicalWitnessBytes(expected.witness),
                ) &&
                request.proofContext == expected.proofContext &&
                request.sourceAdapterDeploymentBinding == expected.sourceAdapterDeploymentBinding,
        ) { "proof request must be canonical" }
    }

    internal fun requireProductionProofRequest(request: SolanaSccpProofRequest) {
        require(request.sourceDomain == DOMAIN_SOLANA && request.targetDomain == DOMAIN_SORA) {
            "Solana SCCP production proofs must target SORA"
        }
        require(request.mainnetGenesisHash == MAINNET_GENESIS_HASH && request.witness.mainnetGenesisHash == MAINNET_GENESIS_HASH) {
            "mainnetGenesisHash must match Solana mainnet-beta"
        }
        require(request.sourceStateVerifierId == MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1) {
            "sourceStateVerifierId must match Solana AccountsDB verifier profile"
        }
        val sourceStateVerifierHash = normalizeHex32(request.sourceStateVerifierHash, "sourceStateVerifierHash")
        require(sourceStateVerifierHash != ZERO_HASH_V1) {
            "sourceStateVerifierHash must not be zero for Solana production proofs"
        }
        require(sourceStateVerifierHash != TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1) {
            "sourceStateVerifierHash must not be the Solana template verifier hash"
        }
        require(request.witness.inclusionBranch.isNotEmpty()) {
            "inclusionBranch must not be empty for Solana production proofs"
        }
        val accountsLtHash = request.witness.accountsLtHash
        require(accountsLtHash != null) {
            "accountsLtHash must be present for Solana production proofs"
        }
        require(accountsLtHash.size == ACCOUNTS_LT_HASH_BYTES) { "accountsLtHash must be 2048 bytes" }
        require(accountsLtHash.any { it.toInt() != 0 }) { "accountsLtHash must not be zero" }
        require(
            normalizeHex32(
                request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
                "sourceAdapterDeploymentHash",
            ) != ZERO_HASH_V1,
        ) { "sourceAdapterDeploymentHash must not be zero for Solana production proofs" }
        require(
            normalizeHex32(
                request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
                "sourceAdapterDeploymentReceiptHash",
            ) != ZERO_HASH_V1,
        ) { "sourceAdapterDeploymentReceiptHash must not be zero for Solana production proofs" }
    }

    private data class FullLightClientAuditRoleProfile(
        val name: String,
        val code: Int,
        val circuitId: String,
        val verifierId: String,
        val requiredInputNames: List<String>,
    )

    private data class NormalizedFullLightClientAuditContext(
        val version: Int,
        val epoch: BigInteger,
        val rootedSlot: BigInteger,
        val parentSlot: BigInteger,
        val towerVoteSlots: List<BigInteger>,
        val parentBankHash: String,
        val bankSignatureCount: BigInteger,
        val bankHashHardForkData: ByteArray,
        val epochStakeRoot: String,
        val stakeActivationHash: String,
        val stakeAccountStateHash: String,
        val stakeHistoryHash: String,
        val stakeHistorySysvarAccountHash: String,
        val accountInclusionRoot: String,
        val accountsLtHashChecksum: String,
        val accountsLtHashProofPublicInputsHash: String,
        val towerLockoutHash: String,
        val towerReplayHash: String,
        val bankForkHash: String,
    )

    private data class NormalizedFullLightClientAuditInput(
        val role: SolanaSccpFullLightClientAuditRole,
        val witness: SolanaSccpWitness,
        val context: NormalizedFullLightClientAuditContext,
        val sourceVerifierMaterialHash: String,
        val sourceAdapterDeploymentHash: String,
        val fullLightClientGateHash: String,
        val verifierHash: String,
        val finalityContextHash: String,
        val voteMessageHash: String,
        val accountsLtHashProofHash: String,
        val openedAccountsLtHashContributionsHash: String,
        val openedAccountsLtHashResidualChecksum: String,
    )

    private fun auditRoleProfile(role: SolanaSccpFullLightClientAuditRole): FullLightClientAuditRoleProfile =
        when (role) {
            SolanaSccpFullLightClientAuditRole.TOWER_REPLAY -> FullLightClientAuditRoleProfile(
                name = "tower_replay",
                code = 1,
                circuitId = TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
                verifierId = MAINNET_TOWER_REPLAY_VERIFIER_ID_V1,
                requiredInputNames = listOf(
                    "tower_lockout_hash",
                    "tower_replay_hash",
                    "bank_fork_hash",
                    "epoch_stake_root",
                    "stake_activation_hash",
                    "stake_account_state_hash",
                    "stake_history_hash",
                    "stake_history_sysvar_account_hash",
                    "account_inclusion_root",
                ),
            )
            SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE -> FullLightClientAuditRoleProfile(
                name = "full_accountsdb_lattice",
                code = 2,
                circuitId = FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
                verifierId = MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1,
                requiredInputNames = listOf(
                    "account_inclusion_root",
                    "accounts_lt_hash_checksum",
                    "accounts_lt_hash_proof_public_inputs_hash",
                    "opened_accounts_lt_hash_contributions_hash",
                    "opened_accounts_lt_hash_residual_checksum",
                    "accounts_lt_hash_proof_hash",
                ),
            )
            SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE -> FullLightClientAuditRoleProfile(
                name = "bank_fork_choice",
                code = 3,
                circuitId = BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
                verifierId = MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1,
                requiredInputNames = listOf(
                    "parent_bank_hash",
                    "bank_hash",
                    "blockhash",
                    "transaction_status_root",
                    "account_inclusion_root",
                    "accounts_lt_hash_checksum",
                    "bank_signature_count",
                    "bank_hash_hard_fork_data_hash",
                    "bank_fork_hash",
                    "tower_replay_hash",
                ),
            )
        }

    private fun roleVerifierHash(
        input: SolanaSccpFullLightClientAuditProofInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): String =
        when (role) {
            SolanaSccpFullLightClientAuditRole.TOWER_REPLAY ->
                normalizeNonZeroHex32(input.solanaTowerReplayVerifierHash, "solanaTowerReplayVerifierHash")
            SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE ->
                normalizeNonZeroHex32(
                    input.solanaFullAccountsdbLatticeVerifierHash,
                    "solanaFullAccountsdbLatticeVerifierHash",
                )
            SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE ->
                normalizeNonZeroHex32(input.solanaBankForkChoiceVerifierHash, "solanaBankForkChoiceVerifierHash")
        }

    private fun requireFullLightClientAuditRoleSeparation(
        input: SolanaSccpFullLightClientAuditProofInput,
        witness: SolanaSccpWitness,
    ) {
        val auditHashes = listOf(
            roleVerifierHash(input, SolanaSccpFullLightClientAuditRole.TOWER_REPLAY),
            roleVerifierHash(input, SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE),
            roleVerifierHash(input, SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE),
        )
        require(auditHashes.toSet().size == auditHashes.size) {
            "Solana full-light-client audit verifier hashes must be role-separated"
        }
        require(auditHashes.all { it !in TEMPLATE_SOURCE_MATERIAL_HASHES_V1 }) {
            "Solana full-light-client audit verifier hashes must not reuse built-in template material"
        }
        val existingHashes = listOf(
            normalizeNonZeroHex32(input.sourceTrustAnchorHash, "sourceTrustAnchorHash"),
            normalizeNonZeroHex32(input.consensusVerifierHash, "consensusVerifierHash"),
            normalizeNonZeroHex32(input.messageInclusionVerifierHash, "messageInclusionVerifierHash"),
            normalizeNonZeroHex32(input.finalityPolicyHash, "finalityPolicyHash"),
            witness.sourceStateVerifierHash,
            normalizeNonZeroHex32(
                input.adapterVerifierVkHash
                    ?: SccpSourceProofs.sourceAdapterVerifierVkHash(DOMAIN_SOLANA),
                "adapterVerifierVkHash",
            ),
            witness.sourceAdapterDeploymentReceiptHash,
        )
        require(auditHashes.all { auditHash ->
            existingHashes.all { existingHash ->
                existingHash == ZERO_HASH_V1 || existingHash != auditHash
            }
        }) {
            "Solana full-light-client audit verifier hashes must not reuse existing source-adapter material"
        }
    }

    private fun fullLightClientGateHashFromBoundHashes(
        sourceVerifierMaterialHash: String,
        sourceAdapterDeploymentHash: String,
        towerReplayVerifierHash: String,
        fullAccountsdbLatticeVerifierHash: String,
        bankForkChoiceVerifierHash: String,
    ): String {
        val verifierHashes = listOf(
            MAINNET_TOWER_REPLAY_VERIFIER_ID_V1 to
                hex32Bytes(normalizeNonZeroHex32(towerReplayVerifierHash, "solanaTowerReplayVerifierHash"), "solanaTowerReplayVerifierHash"),
            MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1 to
                hex32Bytes(
                    normalizeNonZeroHex32(
                        fullAccountsdbLatticeVerifierHash,
                        "solanaFullAccountsdbLatticeVerifierHash",
                    ),
                    "solanaFullAccountsdbLatticeVerifierHash",
                ),
            MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1 to
                hex32Bytes(normalizeNonZeroHex32(bankForkChoiceVerifierHash, "solanaBankForkChoiceVerifierHash"), "solanaBankForkChoiceVerifierHash"),
        )
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, DOMAIN_SOLANA)
        writeU32Le(out, DOMAIN_SORA)
        writeVec(out, SOLANA_SOURCE_CHAIN_KEY_V1.toByteArray(StandardCharsets.UTF_8))
        out.write(SOLANA_SOURCE_PROOF_PLAN_CODE_V1)
        out.write(SOLANA_FINALITY_MODEL_CODE_V1)
        writeVec(out, MAINNET_GENESIS_HASH.toByteArray(StandardCharsets.UTF_8))
        out.write(hex32Bytes(normalizeNonZeroHex32(sourceVerifierMaterialHash, "sourceVerifierMaterialHash"), "sourceVerifierMaterialHash"))
        out.write(hex32Bytes(normalizeNonZeroHex32(sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"), "sourceAdapterDeploymentHash"))
        verifierHashes.forEach { (verifierId, verifierHash) ->
            writeVec(out, verifierId.toByteArray(StandardCharsets.UTF_8))
            out.write(verifierHash)
        }
        return hashHex("sccp:solana:full-light-client-gate:v1", out.toByteArray())
    }

    private fun normalizeFullLightClientAuditInput(
        input: SolanaSccpFullLightClientAuditProofInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): NormalizedFullLightClientAuditInput {
        val witness = normalizeWitness(
            witnessInputWithOpenedAccountsLtHash(input.witnessInput, input.openedAccounts),
        )
        require(witness.sourceDomain == DOMAIN_SOLANA && witness.targetDomain == DOMAIN_SORA) {
            "Solana audit requests require a Solana -> SORA witness"
        }
        require(witness.mainnetGenesisHash == MAINNET_GENESIS_HASH) {
            "mainnetGenesisHash must match Solana mainnet-beta"
        }
        require(witness.sourceStateVerifierHash != ZERO_HASH_V1) {
            "sourceStateVerifierHash must not be zero"
        }
        require(witness.sourceStateVerifierHash != TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1) {
            "sourceStateVerifierHash must not be the Solana template verifier hash"
        }
        requireFullLightClientAuditRoleSeparation(input, witness)
        val sourceAdapterDeploymentReceiptHash =
            normalizeNonZeroHex32(input.sourceAdapterDeploymentReceiptHash, "sourceAdapterDeploymentReceiptHash")
        require(witness.sourceAdapterDeploymentReceiptHash == sourceAdapterDeploymentReceiptHash) {
            "sourceAdapterDeploymentReceiptHash must match witness"
        }
        val sourceVerifierMaterialHash = SccpSourceProofs.sourceVerifierMaterialHash(
            sourceDomain = DOMAIN_SOLANA,
            sourceTrustAnchorHash = input.sourceTrustAnchorHash,
            consensusVerifierHash = input.consensusVerifierHash,
            messageInclusionVerifierHash = input.messageInclusionVerifierHash,
            finalityPolicyHash = input.finalityPolicyHash,
            sourceStateVerifierHash = witness.sourceStateVerifierHash,
        )
        input.sourceVerifierMaterialHash?.let { supplied ->
            require(normalizeHex32(supplied, "sourceVerifierMaterialHash") == sourceVerifierMaterialHash) {
                "sourceVerifierMaterialHash must match sourceVerifierMaterial"
            }
        }
        val sourceAdapterDeploymentHash = SccpSourceProofs.sourceAdapterEngineDeploymentHash(
            sourceDomain = DOMAIN_SOLANA,
            sourceTrustAnchorHash = input.sourceTrustAnchorHash,
            consensusVerifierHash = input.consensusVerifierHash,
            messageInclusionVerifierHash = input.messageInclusionVerifierHash,
            finalityPolicyHash = input.finalityPolicyHash,
            deploymentReceiptHash = sourceAdapterDeploymentReceiptHash,
            adapterVerifierVkHash = input.adapterVerifierVkHash,
            sourceStateVerifierHash = witness.sourceStateVerifierHash,
            solanaTowerReplayVerifierHash = input.solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = input.solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash = input.solanaBankForkChoiceVerifierHash,
        )
        input.sourceAdapterDeploymentHash?.let { supplied ->
            require(normalizeHex32(supplied, "sourceAdapterDeploymentHash") == sourceAdapterDeploymentHash) {
                "sourceAdapterDeploymentHash must match sourceAdapterDeployment"
            }
        }
        val fullLightClientGateHash = SccpSourceProofs.solanaFullLightClientGateHash(
            sourceDomain = DOMAIN_SOLANA,
            sourceTrustAnchorHash = input.sourceTrustAnchorHash,
            consensusVerifierHash = input.consensusVerifierHash,
            messageInclusionVerifierHash = input.messageInclusionVerifierHash,
            finalityPolicyHash = input.finalityPolicyHash,
            deploymentReceiptHash = sourceAdapterDeploymentReceiptHash,
            solanaTowerReplayVerifierHash = input.solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = input.solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash = input.solanaBankForkChoiceVerifierHash,
            adapterVerifierVkHash = input.adapterVerifierVkHash,
            sourceStateVerifierHash = witness.sourceStateVerifierHash,
        )
        input.fullLightClientGateHash?.let { supplied ->
            require(normalizeHex32(supplied, "fullLightClientGateHash") == fullLightClientGateHash) {
                "fullLightClientGateHash must match the bound Solana full-light-client audit verifier hashes"
            }
        }
        require(witness.sourceAdapterDeploymentHash == sourceAdapterDeploymentHash) {
            "sourceAdapterDeploymentHash must match witness"
        }
        require(fullLightClientGateHash != ZERO_HASH_V1) {
            "fullLightClientGateHash must match the bound Solana full-light-client audit verifier hashes"
        }
        val context = normalizeFullLightClientAuditContext(input, witness)
        val finalityContextHash = hashHex(
            "sccp:solana:finality-context:v1",
            canonicalFinalityContextBytes(context),
        )
        input.finalityContextHash?.let { supplied ->
            require(normalizeHex32(supplied, "finalityContextHash") == finalityContextHash) {
                "finalityContextHash must match finality context fields"
            }
        }
        val voteMessageHash = hashHex(
            "sccp:solana:finalized-vote:v1",
            canonicalVoteMessageBytes(witness, finalityContextHash),
        )
        input.voteMessageHash?.let { supplied ->
            require(normalizeHex32(supplied, "voteMessageHash") == voteMessageHash) {
                "voteMessageHash must match finality context and message proof"
            }
        }
        val accountsLtHashProofHash = accountsLtHashProofHash(input.accountsLtHashProof)
        input.accountsLtHashProofHash?.let { supplied ->
            require(normalizeHex32(supplied, "accountsLtHashProofHash") == accountsLtHashProofHash) {
                "accountsLtHashProofHash must match accountsLtHashProof"
            }
        }
        val openedContributionsHash = openedAccountsLtHashContributionsHash(input.openedAccounts)
        val residualChecksum = openedAccountsLtHashResidualChecksum(input.openedAccounts)
        input.openedAccountsLtHashContributionsHash?.let { supplied ->
            require(normalizeHex32(supplied, "openedAccountsLtHashContributionsHash") == openedContributionsHash) {
                "openedAccountsLtHashContributionsHash must match opened AccountsLtHash inputs"
            }
        }
        input.openedAccountsLtHashResidualChecksum?.let { supplied ->
            require(normalizeHex32(supplied, "openedAccountsLtHashResidualChecksum") == residualChecksum) {
                "openedAccountsLtHashResidualChecksum must match opened AccountsLtHash inputs"
            }
        }
        return NormalizedFullLightClientAuditInput(
            role = role,
            witness = witness,
            context = context,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = sourceAdapterDeploymentHash,
            fullLightClientGateHash = fullLightClientGateHash,
            verifierHash = roleVerifierHash(input, role),
            finalityContextHash = finalityContextHash,
            voteMessageHash = voteMessageHash,
            accountsLtHashProofHash = accountsLtHashProofHash,
            openedAccountsLtHashContributionsHash = openedContributionsHash,
            openedAccountsLtHashResidualChecksum = residualChecksum,
        )
    }

    private fun normalizeFullLightClientAuditContext(
        input: SolanaSccpFullLightClientAuditProofInput,
        witness: SolanaSccpWitness,
    ): NormalizedFullLightClientAuditContext {
        val finalized = normalizeU64(witness.finalizedSlot, "finalizedSlot")
        val epoch = input.epoch?.let { normalizeU64(it, "epoch") }
            ?: normalizeU64(mainnetEpochForSlot(witness.finalizedSlot), "epoch")
        require(epoch == normalizeU64(mainnetEpochForSlot(witness.finalizedSlot), "epoch")) {
            "epoch must match Solana mainnet finalizedSlot"
        }
        val rooted = normalizeU64(input.rootedSlot, "rootedSlot")
        val parent = normalizeU64(witness.parentSlot, "parentSlot")
        val towerVoteSlots = input.towerVoteSlots.mapIndexed { index, slot ->
            normalizeU64(slot, "towerVoteSlots[$index]")
        }
        val bankForkHash = bankForkHash(
            finalizedSlot = witness.finalizedSlot,
            parentSlot = witness.parentSlot,
            bankSignatureCount = witness.bankSignatureCount,
            parentBankHash = witness.parentBankHash,
            bankHash = witness.bankHash,
            blockhash = witness.blockhash,
            accountsLtHash = witness.accountsLtHash,
            bankHashHardForkData = witness.bankHashHardForkData,
            transactionStatusRoot = witness.transactionStatusRoot,
            accountInclusionRoot = witness.accountInclusionRoot,
            accountsLtHashChecksum = witness.accountsLtHashChecksum,
            epoch = epoch.toString(),
        )
        val towerLockoutHash = towerLockoutHash(
            finalizedSlot = witness.finalizedSlot,
            rootedSlot = rooted.toString(),
            parentSlot = witness.parentSlot,
            parentBankHash = witness.parentBankHash,
            epoch = epoch.toString(),
        )
        val towerReplayHash = towerReplayHash(
            finalizedSlot = witness.finalizedSlot,
            rootedSlot = rooted.toString(),
            parentSlot = witness.parentSlot,
            bankForkHash = bankForkHash,
            towerVoteSlots = towerVoteSlots.map { it.toString() },
            epoch = epoch.toString(),
        )
        input.towerLockoutHash?.let { supplied ->
            require(normalizeHex32(supplied, "towerLockoutHash") == towerLockoutHash) {
                "towerLockoutHash must match finality context fields"
            }
        }
        input.towerReplayHash?.let { supplied ->
            require(normalizeHex32(supplied, "towerReplayHash") == towerReplayHash) {
                "towerReplayHash must match finality context fields"
            }
        }
        input.bankForkHash?.let { supplied ->
            require(normalizeHex32(supplied, "bankForkHash") == bankForkHash) {
                "bankForkHash must match finality context fields"
            }
        }
        return NormalizedFullLightClientAuditContext(
            version = 1,
            epoch = epoch,
            rootedSlot = rooted,
            parentSlot = parent,
            towerVoteSlots = towerVoteSlots,
            parentBankHash = witness.parentBankHash,
            bankSignatureCount = normalizeU64(witness.bankSignatureCount, "bankSignatureCount"),
            bankHashHardForkData = witness.bankHashHardForkData.copyOf(),
            epochStakeRoot = normalizeNonZeroHex32(input.epochStakeRoot, "epochStakeRoot"),
            stakeActivationHash = normalizeNonZeroHex32(input.stakeActivationHash, "stakeActivationHash"),
            stakeAccountStateHash = normalizeNonZeroHex32(input.stakeAccountStateHash, "stakeAccountStateHash"),
            stakeHistoryHash = normalizeNonZeroHex32(input.stakeHistoryHash, "stakeHistoryHash"),
            stakeHistorySysvarAccountHash =
                normalizeNonZeroHex32(input.stakeHistorySysvarAccountHash, "stakeHistorySysvarAccountHash"),
            accountInclusionRoot = witness.accountInclusionRoot,
            accountsLtHashChecksum = witness.accountsLtHashChecksum,
            accountsLtHashProofPublicInputsHash = witness.accountsLtHashProofPublicInputsHash,
            towerLockoutHash = towerLockoutHash,
            towerReplayHash = towerReplayHash,
            bankForkHash = bankForkHash,
        )
    }

    private fun canonicalFinalityContextBytes(context: NormalizedFullLightClientAuditContext): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(context.version)
        writeU64Le(out, context.epoch)
        writeU64Le(out, context.rootedSlot)
        writeU64Le(out, context.parentSlot)
        writeU32Le(out, context.towerVoteSlots.size)
        context.towerVoteSlots.forEach { writeU64Le(out, it) }
        out.write(hex32Bytes(context.parentBankHash, "parentBankHash"))
        writeU64Le(out, context.bankSignatureCount)
        writeVec(out, context.bankHashHardForkData)
        out.write(hex32Bytes(context.epochStakeRoot, "epochStakeRoot"))
        out.write(hex32Bytes(context.stakeActivationHash, "stakeActivationHash"))
        out.write(hex32Bytes(context.stakeAccountStateHash, "stakeAccountStateHash"))
        out.write(hex32Bytes(context.stakeHistoryHash, "stakeHistoryHash"))
        out.write(hex32Bytes(context.stakeHistorySysvarAccountHash, "stakeHistorySysvarAccountHash"))
        out.write(hex32Bytes(context.accountInclusionRoot, "accountInclusionRoot"))
        out.write(hex32Bytes(context.accountsLtHashChecksum, "accountsLtHashChecksum"))
        out.write(
            hex32Bytes(
                context.accountsLtHashProofPublicInputsHash,
                "accountsLtHashProofPublicInputsHash",
            ),
        )
        out.write(hex32Bytes(context.towerLockoutHash, "towerLockoutHash"))
        out.write(hex32Bytes(context.towerReplayHash, "towerReplayHash"))
        out.write(hex32Bytes(context.bankForkHash, "bankForkHash"))
        return out.toByteArray()
    }

    private fun canonicalVoteMessageBytes(witness: SolanaSccpWitness, finalityContextHash: String): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, DOMAIN_SOLANA)
        writeU64Le(out, normalizeU64(witness.finalizedSlot, "finalizedSlot"))
        out.write(hex32Bytes(witness.blockhash, "blockhash"))
        out.write(hex32Bytes(witness.bankHash, "bankHash"))
        out.write(hex32Bytes(witness.transactionStatusRoot, "transactionStatusRoot"))
        out.write(hex32Bytes(witness.messageProofHash, "messageProofHash"))
        out.write(hex32Bytes(finalityContextHash, "finalityContextHash"))
        return out.toByteArray()
    }

    private fun auditRoleColumns(
        value: NormalizedFullLightClientAuditInput,
        role: SolanaSccpFullLightClientAuditRole,
    ): List<String> =
        when (role) {
            SolanaSccpFullLightClientAuditRole.TOWER_REPLAY -> listOf(
                value.context.towerLockoutHash,
                value.context.towerReplayHash,
                value.context.bankForkHash,
                value.context.epochStakeRoot,
                value.context.stakeActivationHash,
                value.context.stakeAccountStateHash,
                value.context.stakeHistoryHash,
                value.context.stakeHistorySysvarAccountHash,
                value.context.accountInclusionRoot,
            )
            SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE -> listOf(
                value.context.accountInclusionRoot,
                value.context.accountsLtHashChecksum,
                value.context.accountsLtHashProofPublicInputsHash,
                value.openedAccountsLtHashContributionsHash,
                value.openedAccountsLtHashResidualChecksum,
                value.accountsLtHashProofHash,
            )
            SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE -> listOf(
                value.context.parentBankHash,
                value.witness.bankHash,
                value.witness.blockhash,
                value.witness.transactionStatusRoot,
                value.context.accountInclusionRoot,
                value.context.accountsLtHashChecksum,
                "0x" + hexLower(sccpWordU64Le(value.context.bankSignatureCount)),
                "0x" + hexLower(
                    hashBytes(
                        BANK_HASH_HARD_FORK_DATA_PREFIX_V1,
                        value.context.bankHashHardForkData,
                    ),
                ),
                value.context.bankForkHash,
                value.context.towerReplayHash,
            )
        }

    private fun fullLightClientAuditFastpqPublicInputs(
        value: NormalizedFullLightClientAuditInput,
        statementHash: String,
    ): SolanaSccpFullLightClientAuditFastpqPublicInputs {
        val out = ByteArrayOutputStream()
        out.write(auditRoleProfile(value.role).code)
        out.write(hex32Bytes(statementHash, "auditStatementHash"))
        val dsidHash = hashBytes(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1, out.toByteArray())
        val (oldRoot, newRoot, permRoot) = when (value.role) {
            SolanaSccpFullLightClientAuditRole.TOWER_REPLAY ->
                Triple(value.context.towerLockoutHash, value.context.towerReplayHash, value.context.bankForkHash)
            SolanaSccpFullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE ->
                Triple(
                    value.context.accountInclusionRoot,
                    value.context.accountsLtHashChecksum,
                    value.openedAccountsLtHashContributionsHash,
                )
            SolanaSccpFullLightClientAuditRole.BANK_FORK_CHOICE ->
                Triple(value.context.parentBankHash, value.witness.bankHash, value.context.bankForkHash)
        }
        return SolanaSccpFullLightClientAuditFastpqPublicInputs(
            dsid = "0x" + hexLower(dsidHash.copyOfRange(0, 16)),
            slot = value.witness.finalizedSlot,
            oldRoot = oldRoot,
            newRoot = newRoot,
            permRoot = permRoot,
            txSetHash = statementHash,
        )
    }

    private fun canonicalFullLightClientAuditContextBytes(
        value: NormalizedFullLightClientAuditInput,
        statementHash: String,
    ): ByteArray {
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
        out.write(hex32Bytes(value.finalityContextHash, "finalityContextHash"))
        out.write(hex32Bytes(statementHash, "auditStatementHash"))
        return out.toByteArray()
    }

    private fun fullLightClientAuditFastpqKey(
        prefix: String,
        profile: FullLightClientAuditRoleProfile,
    ): ByteArray =
        prefix.toByteArray(StandardCharsets.UTF_8) +
            byteArrayOf(0) +
            profile.circuitId.toByteArray(StandardCharsets.UTF_8)

    private fun sccpWordU8(value: Int): ByteArray {
        require(value >= 0 && value <= 0xFF) { "u8 value out of range" }
        val out = ByteArray(32)
        out[0] = value.toByte()
        return out
    }

    private data class OpenedLtHashContributionRow(
        val role: Int,
        val address: ByteArray,
        val accountHash: ByteArray,
        val rawDataHash: ByteArray,
        val accountLtHash: ByteArray,
    )

    private data class NormalizedOpenedAccountsLtHashContributions(
        val sourceDomain: Int,
        val finalizedSlot: BigInteger,
        val accountInclusionRoot: ByteArray,
        val accountsLtHashChecksum: ByteArray,
        val rows: List<OpenedLtHashContributionRow>,
        val openedAccountsLtHash: ByteArray,
        val openedAccountsLtHashChecksum: ByteArray,
        val residualAccountsLtHash: ByteArray,
        val residualAccountsLtHashChecksum: ByteArray,
    )

    private data class NormalizedAccountsLtHashProofRequest(
        val witness: SolanaSccpWitness,
        val opened: NormalizedOpenedAccountsLtHashContributions,
        val accountsLtHash: ByteArray,
        val openedContributionsHash: String,
        val residualChecksum: String,
    )

    private fun normalizeAccountsLtHashProofRequest(
        witnessInput: SolanaSccpWitnessInput,
        openedInput: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): NormalizedAccountsLtHashProofRequest {
        require(witnessInput.sourceStateVerifierId == MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1) {
            "sourceStateVerifierId must match Solana AccountsDB verifier profile"
        }
        val sourceStateVerifierHash = normalizeHex32(witnessInput.sourceStateVerifierHash, "sourceStateVerifierHash")
        require(sourceStateVerifierHash != ZERO_HASH_V1) {
            "sourceStateVerifierHash must not be zero"
        }
        require(sourceStateVerifierHash != TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1) {
            "sourceStateVerifierHash must not be the Solana template verifier hash"
        }
        val witness = normalizeWitness(witnessInputWithOpenedAccountsLtHash(witnessInput, openedInput))
        val opened = normalizeOpenedAccountsLtHashContributions(openedInput)
        require(normalizeU64(witness.finalizedSlot, "finalizedSlot") == opened.finalizedSlot) {
            "opened finalizedSlot must match witness"
        }
        require(hex32Bytes(witness.accountInclusionRoot, "accountInclusionRoot").contentEquals(opened.accountInclusionRoot)) {
            "opened accountInclusionRoot must match witness"
        }
        require(hex32Bytes(witness.accountsLtHashChecksum, "accountsLtHashChecksum").contentEquals(opened.accountsLtHashChecksum)) {
            "opened accountsLtHashChecksum must match witness"
        }
        return NormalizedAccountsLtHashProofRequest(
            witness = witness,
            opened = opened,
            accountsLtHash = openedInput.accountsLtHash.copyOf(),
            openedContributionsHash = openedAccountsLtHashContributionsHash(openedInput),
            residualChecksum = openedAccountsLtHashResidualChecksum(openedInput),
        )
    }

    private fun witnessInputWithOpenedAccountsLtHash(
        witnessInput: SolanaSccpWitnessInput,
        openedInput: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): SolanaSccpWitnessInput {
        witnessInput.accountsLtHash?.let { supplied ->
            require(supplied.contentEquals(openedInput.accountsLtHash)) {
                "witness accountsLtHash must match opened accountsLtHash"
            }
        }
        return witnessInput.copy(accountsLtHash = openedInput.accountsLtHash.copyOf())
    }

    private fun normalizeOpenedAccountsLtHashContributions(
        input: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): NormalizedOpenedAccountsLtHashContributions {
        require(input.sourceDomain == DOMAIN_SOLANA) { "sourceDomain must be Solana" }
        val finalizedSlot = normalizeU64(input.finalizedSlot, "finalizedSlot")
        val accountInclusionRoot = hex32Bytes(
            normalizeNonZeroHex32(input.accountInclusionRoot, "accountInclusionRoot"),
            "accountInclusionRoot",
        )
        val accountsLtHashChecksum = hex32Bytes(
            normalizeNonZeroHex32(input.accountsLtHashChecksum, "accountsLtHashChecksum"),
            "accountsLtHashChecksum",
        )
        require(input.accountsLtHash.size == ACCOUNTS_LT_HASH_BYTES) { "accountsLtHash must be 2048 bytes" }
        require(input.accountsLtHash.any { it.toInt() != 0 }) { "accountsLtHash must not be zero" }
        require(Blake3.hash(input.accountsLtHash).contentEquals(accountsLtHashChecksum)) {
            "accountsLtHashChecksum must match accountsLtHash"
        }
        val rows = openedLtHashContributionRows(input)
        val openedAccountsLtHash = ByteArray(ACCOUNTS_LT_HASH_BYTES)
        rows.forEach { addAccountsLtHashContribution(openedAccountsLtHash, it.accountLtHash) }
        val openedAccountsLtHashChecksum = Blake3.hash(openedAccountsLtHash)
        val residualAccountsLtHash = input.accountsLtHash.copyOf()
        subtractAccountsLtHashContribution(residualAccountsLtHash, openedAccountsLtHash)
        require(residualAccountsLtHash.any { it.toInt() != 0 }) {
            "openedAccountsLtHashResidual must not be zero"
        }
        val residualAccountsLtHashChecksum = Blake3.hash(residualAccountsLtHash)
        return NormalizedOpenedAccountsLtHashContributions(
            sourceDomain = input.sourceDomain,
            finalizedSlot = finalizedSlot,
            accountInclusionRoot = accountInclusionRoot,
            accountsLtHashChecksum = accountsLtHashChecksum,
            rows = rows,
            openedAccountsLtHash = openedAccountsLtHash,
            openedAccountsLtHashChecksum = openedAccountsLtHashChecksum,
            residualAccountsLtHash = residualAccountsLtHash,
            residualAccountsLtHashChecksum = residualAccountsLtHashChecksum,
        )
    }

    private fun openedLtHashContributionRows(
        input: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): List<OpenedLtHashContributionRow> {
        val deriveVoteLtHashes = input.validatorVoteAccountLtHashes.isEmpty()
        val deriveStakeLtHashes = input.validatorStakeAccountLtHashes.isEmpty()
        require(input.validatorVoteAccountOpenings.size <= MAX_VALIDATORS) {
            "validatorVoteAccountOpenings must contain at most $MAX_VALIDATORS entries"
        }
        require(input.validatorStakeAccountOpenings.size <= MAX_VALIDATORS) {
            "validatorStakeAccountOpenings must contain at most $MAX_VALIDATORS entries"
        }
        require(input.validatorVoteAccountOpenings.size == input.validatorVoteAccountRawData.size) {
            "validatorVoteAccountOpenings and validatorVoteAccountRawData must have matching lengths"
        }
        require(deriveVoteLtHashes || input.validatorVoteAccountOpenings.size == input.validatorVoteAccountLtHashes.size) {
            "validatorVoteAccountOpenings and validatorVoteAccountLtHashes must have matching lengths"
        }
        require(input.validatorStakeAccountOpenings.size == input.validatorStakeAccountRawData.size) {
            "validatorStakeAccountOpenings and validatorStakeAccountRawData must have matching lengths"
        }
        require(deriveStakeLtHashes || input.validatorStakeAccountOpenings.size == input.validatorStakeAccountLtHashes.size) {
            "validatorStakeAccountOpenings and validatorStakeAccountLtHashes must have matching lengths"
        }
        val rows = mutableListOf<OpenedLtHashContributionRow>()
        val seenAddresses = mutableSetOf<String>()
        fun pushRow(
            role: Int,
            opening: SolanaSccpAccountOpeningInput,
            rawData: ByteArray,
            suppliedAccountLtHash: ByteArray?,
            field: String,
            allowEmptyDerive: Boolean = false,
        ) {
            require(opening.address.size == 32) { "address must be 32 bytes" }
            require(seenAddresses.add(hexLower(opening.address))) {
                "opened account addresses must be unique"
            }
            val expectedAccountLtHash = accountLtHash(opening, rawData)
            val rowAccountLtHash = if (suppliedAccountLtHash != null && !(allowEmptyDerive && suppliedAccountLtHash.isEmpty())) {
                require(suppliedAccountLtHash.size == ACCOUNTS_LT_HASH_BYTES) { "$field must be 2048 bytes" }
                require(suppliedAccountLtHash.contentEquals(expectedAccountLtHash)) {
                    "$field must match the opening and rawData"
                }
                suppliedAccountLtHash.copyOf()
            } else {
                expectedAccountLtHash
            }
            rows.add(
                OpenedLtHashContributionRow(
                    role = role,
                    address = opening.address.copyOf(),
                    accountHash = hex32Bytes(accountOpeningHash(opening), "accountHash"),
                    rawDataHash = hex32Bytes(accountRawDataHash(rawData), "rawDataHash"),
                    accountLtHash = rowAccountLtHash,
                ),
            )
        }
        input.validatorVoteAccountOpenings.indices.forEach { index ->
            pushRow(
                OPENED_LT_HASH_ROLE_VOTE,
                input.validatorVoteAccountOpenings[index],
                input.validatorVoteAccountRawData[index],
                if (deriveVoteLtHashes) null else input.validatorVoteAccountLtHashes[index],
                "validatorVoteAccountLtHashes[$index]",
            )
        }
        input.validatorStakeAccountOpenings.indices.forEach { index ->
            pushRow(
                OPENED_LT_HASH_ROLE_STAKE,
                input.validatorStakeAccountOpenings[index],
                input.validatorStakeAccountRawData[index],
                if (deriveStakeLtHashes) null else input.validatorStakeAccountLtHashes[index],
                "validatorStakeAccountLtHashes[$index]",
            )
        }
        pushRow(
            OPENED_LT_HASH_ROLE_STAKE_HISTORY_SYSVAR,
            input.stakeHistorySysvarOpening,
            input.stakeHistorySysvarRawData,
            input.stakeHistorySysvarAccountLtHash,
            "stakeHistorySysvarAccountLtHash",
            allowEmptyDerive = true,
        )
        rows.sortWith { left, right ->
            val roleComparison = left.role.compareTo(right.role)
            if (roleComparison != 0) roleComparison else compareLexicographically(left.address, right.address)
        }
        return rows
    }

    private fun accountOpeningHash(opening: SolanaSccpAccountOpeningInput): String =
        accountOpeningHash(
            opening.address,
            opening.owner,
            opening.lamports,
            opening.rentEpoch,
            opening.executable,
            opening.dataHash,
        )

    private fun addAccountsLtHashContribution(target: ByteArray, contribution: ByteArray) {
        require(target.size == ACCOUNTS_LT_HASH_BYTES) { "accountsLtHash target must be 2048 bytes" }
        require(contribution.size == ACCOUNTS_LT_HASH_BYTES) { "accountLtHash contribution must be 2048 bytes" }
        for (index in 0 until LT_HASH_ELEMENTS) {
            val offset = index * 2
            val mixed = (
                ((target[offset].toInt() and 0xff) or ((target[offset + 1].toInt() and 0xff) shl 8)) +
                    ((contribution[offset].toInt() and 0xff) or ((contribution[offset + 1].toInt() and 0xff) shl 8))
                ) and 0xffff
            target[offset] = mixed.toByte()
            target[offset + 1] = (mixed ushr 8).toByte()
        }
    }

    private fun subtractAccountsLtHashContribution(target: ByteArray, contribution: ByteArray) {
        require(target.size == ACCOUNTS_LT_HASH_BYTES) { "accountsLtHash target must be 2048 bytes" }
        require(contribution.size == ACCOUNTS_LT_HASH_BYTES) { "accountLtHash contribution must be 2048 bytes" }
        for (index in 0 until LT_HASH_ELEMENTS) {
            val offset = index * 2
            val mixed = (
                ((target[offset].toInt() and 0xff) or ((target[offset + 1].toInt() and 0xff) shl 8)) -
                    ((contribution[offset].toInt() and 0xff) or ((contribution[offset + 1].toInt() and 0xff) shl 8))
                ) and 0xffff
            target[offset] = mixed.toByte()
            target[offset + 1] = (mixed ushr 8).toByte()
        }
    }

    private fun hashHex(prefix: String, payload: ByteArray): String = "0x" + hexLower(hashBytes(prefix, payload))

    private fun solanaMainnetGenesisHashPublicInput(): String =
        hashHex(MAINNET_GENESIS_HASH_PREFIX_V1, MAINNET_GENESIS_HASH.toByteArray(Charsets.UTF_8))

    private fun sourceMerkleNodeHash(left: ByteArray, right: ByteArray): ByteArray =
        hashBytes("sccp:source:node:v1", left + right)

    private fun sha256Hashv(parts: List<ByteArray>): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        parts.forEach { digest.update(it) }
        return digest.digest()
    }

    private fun agaveBankHashBytes(
        parentBankHash: ByteArray,
        bankSignatureCount: BigInteger,
        blockhash: ByteArray,
        accountsLtHash: ByteArray,
        bankHashHardForkData: ByteArray,
    ): ByteArray {
        require(bankSignatureCount != BigInteger.ZERO) { "bankSignatureCount must be nonzero" }
        require(accountsLtHash.size == ACCOUNTS_LT_HASH_BYTES) { "accountsLtHash must be 2048 bytes" }
        require(accountsLtHash.any { it.toInt() != 0 }) { "accountsLtHash must not be zero" }
        require(bankHashHardForkData.size <= MAX_BANK_HARD_FORK_HASH_DATA_BYTES) {
            "bankHashHardForkData is too large"
        }
        val signatureCountBytes = ByteArrayOutputStream()
        writeU64Le(signatureCountBytes, bankSignatureCount)
        var bankHash = sha256Hashv(listOf(parentBankHash, signatureCountBytes.toByteArray(), blockhash))
        bankHash = sha256Hashv(listOf(bankHash, accountsLtHash))
        if (bankHashHardForkData.isNotEmpty()) {
            bankHash = sha256Hashv(listOf(bankHash, bankHashHardForkData))
        }
        return bankHash
    }

    private data class RouteCanaryProgramDataEvidence(
        val verifierProgram: ByteArray,
        val verifierCodeHash: String,
        val rpcCommitment: String,
        val programOwner: String,
        val programdataOwner: String,
        val programAccountData: ByteArray,
        val programdataAddress: ByteArray,
        val programdataSlot: BigInteger,
        val expectedProgramdataSlot: BigInteger,
        val programAccountContextSlot: BigInteger,
        val programdataAccountContextSlot: BigInteger,
        val programdataMetadata: ByteArray,
        val programdataExecutable: ByteArray,
    )

    private fun canonicalPositiveU64(value: String, field: String): BigInteger {
        val text = normalizeNonEmpty(value, field)
        require(text == value && text.matches(Regex("[0-9]+"))) { "$field must be canonical decimal" }
        val numeric = BigInteger(text)
        require(numeric > BigInteger.ZERO) { "$field must be positive" }
        require(numeric <= MAX_U64) { "$field must fit u64" }
        require(text == numeric.toString()) { "$field must be canonical decimal" }
        return numeric
    }

    private fun strictBase64Bytes(value: String, field: String): ByteArray {
        require(value.trim() == value) { "$field must be canonical base64" }
        val decoded = Base64.getDecoder().decode(value)
        require(Base64.getEncoder().encodeToString(decoded) == value) { "$field must be canonical base64" }
        return decoded
    }

    private fun solanaUpgradeableProgramAccountData(programdataAddress: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32Le(out, SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG)
        out.write(programdataAddress)
        return out.toByteArray()
    }

    private fun solanaImmutableProgramdataMetadata(programdataSlot: BigInteger): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32Le(out, SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG)
        writeU64Le(out, programdataSlot)
        out.write(0)
        out.write(ByteArray(32))
        return out.toByteArray()
    }

    private fun solanaVerifierProgramCodeHash(programBytes: ByteArray): String {
        require(
            programBytes.isNotEmpty() &&
                programBytes.any { it.toInt() != 0 } &&
                programBytes.size >= SOLANA_BPF_ELF_MAGIC.size &&
                SOLANA_BPF_ELF_MAGIC.indices.all { programBytes[it] == SOLANA_BPF_ELF_MAGIC[it] },
        ) {
            "solanaProgramdataExecutable must be non-empty BPF ELF bytes"
        }
        return "0x" + hexLower(Blake2b.digest256(programBytes))
    }

    private fun normalizeRouteCanaryProgramDataEvidence(
        input: SolanaSccpRouteCanaryEvidenceInput,
    ): RouteCanaryProgramDataEvidence {
        val verifierProgram = decodeSolanaBase58Fixed(input.verifierIdentity, "verifierIdentity", PROGRAM_ID_BYTES)
        val programdataAddress = decodeSolanaBase58Fixed(
            input.solanaProgramdataAddress,
            "solanaProgramdataAddress",
            PROGRAM_ID_BYTES,
        )
        require(!verifierProgram.contentEquals(programdataAddress)) {
            "solanaProgramdataAddress must differ from verifierIdentity"
        }
        val programdataSlot = canonicalPositiveU64(input.solanaProgramdataSlot, "solanaProgramdataSlot")
        val expectedProgramdataSlot = canonicalPositiveU64(
            input.solanaExpectedProgramdataSlot,
            "solanaExpectedProgramdataSlot",
        )
        require(programdataSlot == expectedProgramdataSlot) {
            "solanaExpectedProgramdataSlot must match solanaProgramdataSlot"
        }
        val programContextSlot = canonicalPositiveU64(
            input.solanaProgramAccountContextSlot,
            "solanaProgramAccountContextSlot",
        )
        val programdataContextSlot = canonicalPositiveU64(
            input.solanaProgramdataAccountContextSlot,
            "solanaProgramdataAccountContextSlot",
        )
        require(programContextSlot >= programdataSlot && programdataContextSlot >= programdataSlot) {
            "Solana ProgramData context slots must be at or after programdataSlot"
        }
        val rpcCommitment = normalizeNonEmpty(input.solanaRpcCommitment, "solanaRpcCommitment")
        require(rpcCommitment == "finalized") { "solanaRpcCommitment must be finalized" }
        val programOwner = normalizeNonEmpty(input.solanaProgramOwner, "solanaProgramOwner")
        val programdataOwner = normalizeNonEmpty(input.solanaProgramdataOwner, "solanaProgramdataOwner")
        require(programOwner == UPGRADEABLE_LOADER_ID) {
            "solanaProgramOwner must be the BPF upgradeable loader"
        }
        require(programdataOwner == UPGRADEABLE_LOADER_ID) {
            "solanaProgramdataOwner must be the BPF upgradeable loader"
        }
        require(input.solanaProgramImmutable) { "solanaProgramImmutable must be true" }

        val programAccountData = strictBase64Bytes(
            input.solanaProgramAccountDataBase64,
            "solanaProgramAccountDataBase64",
        )
        require(programAccountData.contentEquals(solanaUpgradeableProgramAccountData(programdataAddress))) {
            "solanaProgramAccountDataBase64 must bind solanaProgramdataAddress"
        }
        val programdataMetadata = strictBase64Bytes(
            input.solanaProgramdataMetadataBase64,
            "solanaProgramdataMetadataBase64",
        )
        require(
            programdataMetadata.size == SOLANA_PROGRAMDATA_METADATA_LEN &&
                programdataMetadata.contentEquals(solanaImmutableProgramdataMetadata(programdataSlot)),
        ) {
            "solanaProgramdataMetadataBase64 must bind immutable ProgramData metadata"
        }
        val metadataHash = "0x" + hexLower(Blake2b.digest256(programdataMetadata))
        require(
            normalizeNonZeroHex32(
                input.solanaProgramdataMetadataBlake2b256,
                "solanaProgramdataMetadataBlake2b256",
            ) == metadataHash,
        ) {
            "solanaProgramdataMetadataBlake2b256 must match metadata bytes"
        }
        val programdataExecutable = strictBase64Bytes(
            input.solanaProgramdataExecutableBase64,
            "solanaProgramdataExecutableBase64",
        )
        val executableHash = solanaVerifierProgramCodeHash(programdataExecutable)
        require(
            normalizeNonZeroHex32(
                input.solanaProgramdataExecutableBlake2b256,
                "solanaProgramdataExecutableBlake2b256",
            ) == executableHash,
        ) {
            "solanaProgramdataExecutableBlake2b256 must match executable bytes"
        }
        require(normalizeNonZeroHex32(input.verifierCodeHash, "verifierCodeHash") == executableHash) {
            "verifierCodeHash must match ProgramData executable hash"
        }
        return RouteCanaryProgramDataEvidence(
            verifierProgram = verifierProgram,
            verifierCodeHash = executableHash,
            rpcCommitment = rpcCommitment,
            programOwner = programOwner,
            programdataOwner = programdataOwner,
            programAccountData = programAccountData,
            programdataAddress = programdataAddress,
            programdataSlot = programdataSlot,
            expectedProgramdataSlot = expectedProgramdataSlot,
            programAccountContextSlot = programContextSlot,
            programdataAccountContextSlot = programdataContextSlot,
            programdataMetadata = programdataMetadata,
            programdataExecutable = programdataExecutable,
        )
    }

    private fun normalizeNonEmpty(value: String, field: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$field must be non-empty" }
        return trimmed
    }

    private fun requireSourceStateProofLabel(value: String, field: String) {
        require(value.toByteArray(StandardCharsets.UTF_8).size <= SOURCE_STATE_MAX_PROOF_LABEL_BYTES) {
            "$field must be at most $SOURCE_STATE_MAX_PROOF_LABEL_BYTES bytes"
        }
    }

    private fun decodeSolanaBase58(value: String, field: String): ByteArray {
        val text = normalizeNonEmpty(value, field)
        var numeric = BigInteger.ZERO
        for (symbol in text) {
            require(symbol.code < BASE58_INDEX.size && BASE58_INDEX[symbol.code] >= 0) {
                "$field must be canonical base58"
            }
            numeric = numeric.multiply(BASE_58).add(BigInteger.valueOf(BASE58_INDEX[symbol.code].toLong()))
        }
        var payload = if (numeric.signum() == 0) ByteArray(0) else numeric.toByteArray()
        while (payload.size > 1 && payload[0].toInt() == 0) {
            payload = payload.copyOfRange(1, payload.size)
        }
        val leadingZeros = text.takeWhile { it == '1' }.length
        return ByteArray(leadingZeros) + payload
    }

    private fun decodeSolanaBase58Fixed(value: String, field: String, byteLength: Int): ByteArray {
        val raw = decodeSolanaBase58(value, field)
        require(raw.size == byteLength) { "$field must decode to $byteLength bytes" }
        require(raw.any { it.toInt() != 0 }) { "$field must not decode to zero" }
        return raw
    }

    private fun normalizeSolanaBase58Fixed(value: String, field: String, byteLength: Int): String {
        val text = normalizeNonEmpty(value, field)
        decodeSolanaBase58Fixed(text, field, byteLength)
        return text
    }

    private fun normalizeHex32(value: String, field: String): String = "0x" + hexLower(hex32Bytes(value, field))

    private fun normalizeHexBytes(value: String, field: String, byteLength: Int): String =
        "0x" + hexLower(hexBytes(value, field, byteLength))

    private fun normalizeNonZeroHex32(value: String, field: String): String {
        val bytes = nonZeroHex32Bytes(value, field)
        return "0x" + hexLower(bytes)
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

    private fun solanaHash32Bytes(value: String, field: String): ByteArray {
        val text = normalizeNonEmpty(value, field)
        var body = text
        if (body.startsWith("0x")) {
            body = body.substring(2)
        }
        if (body.length == 64 && isLowercaseHexBody(body)) {
            val bytes = hex32Bytes(text, field)
            require(bytes.any { it.toInt() != 0 }) { "$field must not be zero" }
            return bytes
        }
        return decodeSolanaBase58Fixed(text, field, 32)
    }

    private fun normalizeMessageProofHash(
        value: String,
        sourceEventDigest: String,
        transactionStatusRoot: String,
        transactionSignature: String,
        emitterProgramId: String,
        inclusionBranch: List<ByteArray>,
    ): String {
        if (inclusionBranch.isEmpty()) {
            return normalizeHex32(value, "messageProofHash")
        }
        val derived = messageProofHash(
            sourceEventDigest,
            transactionStatusRoot,
            transactionSignature,
            emitterProgramId,
            inclusionBranch,
        )
        if (value.trim().isEmpty()) {
            return derived
        }
        val provided = normalizeHex32(value, "messageProofHash")
        require(provided == derived) { "messageProofHash must match inclusionBranch" }
        return provided
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

    private fun hex32Bytes(value: String, field: String): ByteArray = hexBytes(value, field, 32)

    private fun hexBytes(value: String, field: String, byteLength: Int): ByteArray {
        require(value.trim() == value) { "$field must be canonical hex" }
        var body = value
        require(!body.startsWith("0X")) { "$field must be canonical hex" }
        if (body.startsWith("0x")) {
            body = body.substring(2)
        }
        require(body.none { it.isWhitespace() }) { "$field must be canonical hex" }
        require(body.length == byteLength * 2) { "$field must be $byteLength bytes" }
        require(isLowercaseHexBody(body)) { "$field must be canonical hex" }
        val out = ByteArray(byteLength)
        for (i in out.indices) {
            val byteText = body.substring(i * 2, i * 2 + 2)
            out[i] = byteText.toIntOrNull(16)?.toByte()
                ?: throw IllegalArgumentException("$field must be canonical hex")
        }
        return out
    }

    private fun isLowercaseHexBody(value: String): Boolean =
        value.all { it in '0'..'9' || it in 'a'..'f' }

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

    private fun encodeInstructionData(argumentBytes: List<ByteArray>): ByteArray {
        val out = ByteArrayOutputStream()
        writeVec(out, SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.toByteArray(StandardCharsets.UTF_8))
        argumentBytes.forEach { writeVec(out, it) }
        return out.toByteArray()
    }

    private fun canonicalVoteRosterBytes(
        validatorPublicKeys: List<ByteArray>,
        validatorStakes: List<String>,
    ): ByteArray {
        require(validatorPublicKeys.isNotEmpty()) { "validatorPublicKeys must be non-empty" }
        require(validatorPublicKeys.size <= MAX_VALIDATORS) {
            "validatorPublicKeys must contain 1..$MAX_VALIDATORS entries"
        }
        require(validatorPublicKeys.size == validatorStakes.size) {
            "validatorStakes must match validatorPublicKeys"
        }
        val seen = mutableSetOf<String>()
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, validatorPublicKeys.size)
        validatorPublicKeys.forEachIndexed { index, publicKey ->
            require(publicKey.size == 32) { "validatorPublicKeys[$index] must be 32 bytes" }
            require(publicKey.any { it.toInt() != 0 }) { "validatorPublicKeys[$index] must not be zero" }
            val publicKeyHex = hexLower(publicKey)
            require(seen.add(publicKeyHex)) { "validatorPublicKeys must not contain duplicates" }
            val stake = normalizeU64(validatorStakes[index], "validatorStakes[$index]")
            require(stake > BigInteger.ZERO) { "validatorStakes[$index] must be greater than zero" }
            writeVec(out, publicKey)
            writeU64Le(out, stake)
        }
        return out.toByteArray()
    }

    private fun normalizeFixed32List(
        values: List<ByteArray>,
        expectedSize: Int,
        label: String,
        unique: Boolean = true,
    ): List<ByteArray> {
        require(values.size == expectedSize) { "$label must match validatorPublicKeys" }
        val seen = mutableSetOf<String>()
        return values.mapIndexed { index, value ->
            require(value.size == 32) { "$label[$index] must be 32 bytes" }
            require(value.any { it.toInt() != 0 }) { "$label[$index] must not be zero" }
            if (unique) {
                require(seen.add(hexLower(value))) { "$label must not contain duplicates" }
            }
            value
        }
    }

    private fun writeVec(out: ByteArrayOutputStream, value: ByteArray) {
        writeU32Le(out, value.size)
        out.write(value)
    }

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0) { "u32 value must not be negative" }
        out.write(value and 0xFF)
        out.write((value ushr 8) and 0xFF)
        out.write((value ushr 16) and 0xFF)
        out.write((value ushr 24) and 0xFF)
    }

    private fun writeU16Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0 && value <= 0xFFFF) { "u16 value out of range" }
        out.write(value and 0xFF)
        out.write((value ushr 8) and 0xFF)
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        for (i in 0 until 8) {
            out.write(working.and(BigInteger.valueOf(0xFFL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun readU32Le(bytes: ByteArray, offset: Int): Int {
        require(offset >= 0 && offset + 4 <= bytes.size) { "rawData is too short" }
        return (bytes[offset].toInt() and 0xFF) or
            ((bytes[offset + 1].toInt() and 0xFF) shl 8) or
            ((bytes[offset + 2].toInt() and 0xFF) shl 16) or
            ((bytes[offset + 3].toInt() and 0xFF) shl 24)
    }

    private fun readU16Le(bytes: ByteArray, offset: Int): Int {
        require(offset >= 0 && offset + 2 <= bytes.size) { "rawData is too short" }
        return (bytes[offset].toInt() and 0xFF) or
            ((bytes[offset + 1].toInt() and 0xFF) shl 8)
    }

    private fun readU64Le(bytes: ByteArray, offset: Int): BigInteger {
        require(offset >= 0 && offset + 8 <= bytes.size) { "rawData is too short" }
        var value = BigInteger.ZERO
        for (index in 7 downTo 0) {
            value = value.shiftLeft(8).add(BigInteger.valueOf((bytes[offset + index].toInt() and 0xFF).toLong()))
        }
        return value
    }

    private fun sccpWordU32Le(value: Int): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32Le(out, value)
        return out.toByteArray() + ByteArray(28)
    }

    private fun sccpWordU64Le(value: BigInteger): ByteArray {
        val out = ByteArrayOutputStream()
        writeU64Le(out, value)
        return out.toByteArray() + ByteArray(24)
    }

    private fun hashBytes(prefix: String, payload: ByteArray): ByteArray =
        Blake2b.digest256(prefix.toByteArray(StandardCharsets.UTF_8) + payload)

    private fun compareLexicographically(left: ByteArray, right: ByteArray): Int {
        if (left.size != right.size) {
            return left.size - right.size
        }
        for (index in left.indices) {
            val diff = (left[index].toInt() and 0xFF) - (right[index].toInt() and 0xFF)
            if (diff != 0) {
                return diff
            }
        }
        return 0
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) {
            builder.append(String.format("%02x", byte.toInt() and 0xFF))
        }
        return builder.toString()
    }

    private data class AccountInclusionLevelNode(
        val hash: ByteArray,
        val indexes: List<Int>,
    )
}

/** Solana account inclusion root and per-leaf Merkle branches. */
data class SolanaSccpAccountInclusionWitness(
    val root: String,
    val branches: List<List<String>>,
)

/** Opened Solana accounts used to build the exact account-inclusion witness. */
data class SolanaSccpOpenedAccountInclusionWitnessInput(
    val finalizedSlot: String,
    val validatorVoteAccountOpenings: List<SolanaSccpAccountOpeningInput> = emptyList(),
    val validatorVoteAccountRawData: List<ByteArray> = emptyList(),
    val validatorStakeAccountOpenings: List<SolanaSccpAccountOpeningInput> = emptyList(),
    val validatorStakeAccountRawData: List<ByteArray> = emptyList(),
    val stakeHistorySysvarOpening: SolanaSccpAccountOpeningInput,
    val stakeHistorySysvarRawData: ByteArray,
    val expectedAccountInclusionRoot: String? = null,
)

/** Exact opened-account inclusion root and branches accepted by the Solana verifier. */
data class SolanaSccpOpenedAccountInclusionWitness(
    val root: String,
    val branches: List<List<String>>,
    val validatorVoteAccountBranches: List<List<String>>,
    val validatorStakeAccountBranches: List<List<String>>,
    val stakeHistorySysvarBranch: List<String>,
)

/** Parsed fields from a raw Solana `VoteStateVersions` account buffer. */
data class SolanaSccpParsedVoteStateAccountData(
    val nodePubkey: ByteArray,
    val authorizedVoter: ByteArray,
    val authorizedWithdrawer: ByteArray,
    val inflationRewardsCollector: ByteArray,
    val blockRevenueCollector: ByteArray,
    val inflationRewardsCommissionBps: String,
    val blockRevenueCommissionBps: String,
    val pendingDelegatorRewards: String,
    val blsPubkeyCompressed: ByteArray,
    val rootSlot: String,
    val towerVoteSlots: List<String>,
)

typealias SolanaSccpParsedVoteStateV1OrV3AccountData = SolanaSccpParsedVoteStateAccountData

/** Parsed fields from a raw Solana `StakeStateV2::Stake` account buffer. */
data class SolanaSccpParsedStakeStateV2StakeAccountData(
    val staker: ByteArray,
    val withdrawer: ByteArray,
    val voterPubkey: ByteArray,
    val delegatedStake: String,
    val activationEpoch: String,
    val deactivationEpoch: String,
    val warmupCooldownRateBytes: ByteArray,
    val creditsObserved: String,
    val stakeFlags: String,
)

/** Solana account opening metadata used by mobile proof-generation helpers. */
data class SolanaSccpAccountOpeningInput(
    val address: ByteArray,
    val owner: ByteArray,
    val lamports: String,
    val rentEpoch: String,
    val executable: Boolean = false,
    val dataHash: String,
)

/** Opened Solana AccountsLtHash rows supplied by a native/mobile source-state prover. */
data class SolanaSccpOpenedAccountsLtHashContributionsInput(
    val sourceDomain: Int = SccpSolana.DOMAIN_SOLANA,
    val finalizedSlot: String,
    val accountInclusionRoot: String,
    val accountsLtHashChecksum: String,
    val accountsLtHash: ByteArray,
    val validatorVoteAccountOpenings: List<SolanaSccpAccountOpeningInput> = emptyList(),
    val validatorVoteAccountRawData: List<ByteArray> = emptyList(),
    val validatorVoteAccountLtHashes: List<ByteArray> = emptyList(),
    val validatorStakeAccountOpenings: List<SolanaSccpAccountOpeningInput> = emptyList(),
    val validatorStakeAccountRawData: List<ByteArray> = emptyList(),
    val validatorStakeAccountLtHashes: List<ByteArray> = emptyList(),
    val stakeHistorySysvarOpening: SolanaSccpAccountOpeningInput,
    val stakeHistorySysvarRawData: ByteArray,
    val stakeHistorySysvarAccountLtHash: ByteArray = ByteArray(0),
)

/** FastPQ public inputs bound to a Solana AccountsLtHash source-state proof request. */
data class SolanaSccpAccountsLtHashFastpqPublicInputs(
    val dsid: String,
    val slot: String,
    val oldRoot: String,
    val newRoot: String,
    val permRoot: String,
    val txSetHash: String,
)

/** One FastPQ transition supplied to a Solana AccountsLtHash source-state prover. */
class SolanaSccpAccountsLtHashFastpqTransition(
    val key: String,
    val operation: String,
    oldValue: ByteArray,
    newValue: ByteArray,
) {
    private val oldValueStorage = oldValue.copyOf()
    private val newValueStorage = newValue.copyOf()

    val oldValue: ByteArray
        get() = oldValueStorage.copyOf()

    val newValue: ByteArray
        get() = newValueStorage.copyOf()
}

/** Source-state proof request for the nested Solana AccountsLtHash proof. */
class SolanaSccpAccountsLtHashProofRequest(
    val version: Int,
    val proofFamily: String,
    val circuitId: String,
    val parameterSet: String,
    val sourceDomain: Int,
    val finalizedSlot: String,
    val parentSlot: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val accountsLtHashProofPublicInputsHash: String,
    val openedAccountsLtHashContributionsHash: String,
    val openedAccountsLtHashResidualChecksum: String,
    statementBytes: ByteArray,
    accountCommitmentBytes: ByteArray,
    verificationContextBytes: ByteArray,
    schemaDescriptor: ByteArray,
    publicInputColumns: List<List<String>>,
    val fastpqPublicInputs: SolanaSccpAccountsLtHashFastpqPublicInputs,
    fastpqTransitions: List<SolanaSccpAccountsLtHashFastpqTransition>,
) {
    private val statementBytesStorage = statementBytes.copyOf()
    private val accountCommitmentBytesStorage = accountCommitmentBytes.copyOf()
    private val verificationContextBytesStorage = verificationContextBytes.copyOf()
    private val schemaDescriptorStorage = schemaDescriptor.copyOf()
    private val publicInputColumnsStorage = publicInputColumns.map { it.toList() }
    private val fastpqTransitionsStorage = fastpqTransitions.toList()

    val statementBytes: ByteArray
        get() = statementBytesStorage.copyOf()

    val accountCommitmentBytes: ByteArray
        get() = accountCommitmentBytesStorage.copyOf()

    val verificationContextBytes: ByteArray
        get() = verificationContextBytesStorage.copyOf()

    val schemaDescriptor: ByteArray
        get() = schemaDescriptorStorage.copyOf()

    val publicInputColumns: List<List<String>>
        get() = publicInputColumnsStorage.map { it.toList() }

    val fastpqTransitions: List<SolanaSccpAccountsLtHashFastpqTransition>
        get() = fastpqTransitionsStorage.toList()
}

/** Source-state verification proof capsule generated by a user-side prover. */
class SolanaSccpSourceStateVerificationProof(
    val version: Int = 1,
    val proofFamily: String = "stark-fri-v1",
    val circuitId: String,
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
    ): SolanaSccpSourceStateVerificationProof =
        SolanaSccpSourceStateVerificationProof(version, proofFamily, circuitId, proofBytes)

    operator fun component1(): Int = version
    operator fun component2(): String = proofFamily
    operator fun component3(): String = circuitId
    operator fun component4(): ByteArray = proofBytes

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is SolanaSccpSourceStateVerificationProof &&
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
        "SolanaSccpSourceStateVerificationProof(version=$version, " +
            "proofFamily=$proofFamily, circuitId=$circuitId, " +
            "proofBytes=${proofBytesStorage.size} bytes)"
}

/** Solana full light-client audit role proven by a user-side prover. */
enum class SolanaSccpFullLightClientAuditRole {
    /** Verifies rooted Tower lockouts and replay over the voted bank fork. */
    TOWER_REPLAY,

    /** Verifies the full AccountsDB lattice around the nested AccountsLtHash proof. */
    FULL_ACCOUNTSDB_LATTICE,

    /** Verifies bank hash and fork-choice state for the finalized vote. */
    BANK_FORK_CHOICE,
}

/** Input required to build Solana full light-client audit proof requests on UI/mobile clients. */
data class SolanaSccpFullLightClientAuditProofInput(
    val witnessInput: SolanaSccpWitnessInput,
    val openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput,
    val accountsLtHashProof: SolanaSccpSourceStateVerificationProof,
    val rootedSlot: String,
    val towerVoteSlots: List<String>,
    val epoch: String? = null,
    val epochStakeRoot: String,
    val stakeActivationHash: String,
    val stakeAccountStateHash: String,
    val stakeHistoryHash: String,
    val stakeHistorySysvarAccountHash: String,
    val sourceTrustAnchorHash: String,
    val consensusVerifierHash: String,
    val messageInclusionVerifierHash: String,
    val finalityPolicyHash: String,
    val sourceAdapterDeploymentReceiptHash: String,
    val solanaTowerReplayVerifierHash: String,
    val solanaFullAccountsdbLatticeVerifierHash: String,
    val solanaBankForkChoiceVerifierHash: String,
    val adapterVerifierVkHash: String? = null,
    val sourceVerifierMaterialHash: String? = null,
    val sourceAdapterDeploymentHash: String? = null,
    val fullLightClientGateHash: String? = null,
    val finalityContextHash: String? = null,
    val voteMessageHash: String? = null,
    val accountsLtHashProofHash: String? = null,
    val openedAccountsLtHashContributionsHash: String? = null,
    val openedAccountsLtHashResidualChecksum: String? = null,
    val towerLockoutHash: String? = null,
    val towerReplayHash: String? = null,
    val bankForkHash: String? = null,
)

/** FastPQ public inputs bound to a Solana full light-client audit role proof request. */
data class SolanaSccpFullLightClientAuditFastpqPublicInputs(
    val dsid: String,
    val slot: String,
    val oldRoot: String,
    val newRoot: String,
    val permRoot: String,
    val txSetHash: String,
)

/** One FastPQ transition supplied to a Solana full light-client audit role prover. */
class SolanaSccpFullLightClientAuditFastpqTransition(
    val key: String,
    val operation: String,
    oldValue: ByteArray,
    newValue: ByteArray,
) {
    private val oldValueStorage = oldValue.copyOf()
    private val newValueStorage = newValue.copyOf()

    val oldValue: ByteArray
        get() = oldValueStorage.copyOf()

    val newValue: ByteArray
        get() = newValueStorage.copyOf()
}

/** OpenVerify request for one Solana full light-client audit role proof. */
class SolanaSccpFullLightClientAuditProofRequest(
    val version: Int,
    val proofFamily: String,
    val circuitId: String,
    val parameterSet: String,
    val role: String,
    val roleCode: Int,
    val sourceDomain: Int,
    val finalizedSlot: String,
    val verifierId: String,
    val verifierHash: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterDeploymentHash: String,
    val fullLightClientGateHash: String,
    val finalityContextHash: String,
    val voteMessageHash: String,
    val accountsLtHashProofHash: String,
    val auditStatementHash: String,
    statementBytes: ByteArray,
    verificationContextBytes: ByteArray,
    schemaDescriptor: ByteArray,
    publicInputColumns: List<List<String>>,
    val fastpqPublicInputs: SolanaSccpFullLightClientAuditFastpqPublicInputs,
    fastpqTransitions: List<SolanaSccpFullLightClientAuditFastpqTransition>,
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

    val fastpqTransitions: List<SolanaSccpFullLightClientAuditFastpqTransition>
        get() = fastpqTransitionsStorage.toList()
}

/** Role-separated Solana full light-client audit proof requests. */
data class SolanaSccpFullLightClientAuditProofRequests(
    val towerReplay: SolanaSccpFullLightClientAuditProofRequest,
    val fullAccountsdbLattice: SolanaSccpFullLightClientAuditProofRequest,
    val bankForkChoice: SolanaSccpFullLightClientAuditProofRequest,
)

/** Solana destination ProgramData evidence collected by UI code before route canary submission. */
data class SolanaSccpRouteCanaryEvidenceInput(
    val routeAllowlistHash: String,
    val destinationBindingHash: String,
    val expectedDestinationBindingHash: String? = null,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val verifierIdentity: String,
    val verifierCodeHash: String,
    val solanaRpcCommitment: String = "finalized",
    val solanaProgramOwner: String = SccpSolana.UPGRADEABLE_LOADER_ID,
    val solanaProgramdataOwner: String = SccpSolana.UPGRADEABLE_LOADER_ID,
    val solanaProgramImmutable: Boolean = true,
    val solanaProgramAccountDataBase64: String,
    val solanaProgramdataAddress: String,
    val solanaProgramdataSlot: String,
    val solanaExpectedProgramdataSlot: String,
    val solanaProgramAccountContextSlot: String,
    val solanaProgramdataAccountContextSlot: String,
    val solanaProgramdataMetadataBlake2b256: String,
    val solanaProgramdataMetadataBase64: String,
    val solanaProgramdataExecutableBlake2b256: String,
    val solanaProgramdataExecutableBase64: String,
)

/** Raw Solana SCCP witness data collected by portal or mobile UI code. */
data class SolanaSccpWitnessInput(
    val targetDomain: Int = SccpSolana.DOMAIN_SORA,
    val mainnetGenesisHash: String = SccpSolana.MAINNET_GENESIS_HASH,
    val finalizedSlot: String,
    val parentSlot: String,
    val bankSignatureCount: String,
    val parentBankHash: String,
    val blockhash: String,
    val bankHash: String,
    val transactionStatusRoot: String,
    val messageProofHash: String,
    val accountInclusionRoot: String,
    val accountsLtHashChecksum: String,
    val accountsLtHashProofPublicInputsHash: String? = null,
    val bankHashHardForkData: ByteArray = ByteArray(0),
    val accountsLtHash: ByteArray? = null,
    val transactionSignature: String,
    val emitterProgramId: String,
    val messageId: String,
    val payloadHash: String,
    val commitmentRoot: String,
    val sourceEventDigest: String,
    val sourceStateVerifierId: String = SccpSolana.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
    val sourceStateVerifierHash: String = SccpSolana.ZERO_HASH_V1,
    val statementHash: String,
    val destinationBindingHash: String,
    val inclusionBranch: List<ByteArray> = emptyList(),
    val sourceAdapterDeploymentHash: String = SccpSolana.ZERO_HASH_V1,
    val sourceAdapterDeploymentReceiptHash: String = SccpSolana.ZERO_HASH_V1,
)

/** Canonical Solana SCCP witness passed into local proof generation. */
class SolanaSccpWitness(
    val version: Int,
    val sourceDomain: Int,
    val targetDomain: Int,
    val mainnetGenesisHash: String,
    val finalizedSlot: String,
    val parentSlot: String,
    val bankSignatureCount: String,
    val parentBankHash: String,
    val blockhash: String,
    val bankHash: String,
    val transactionStatusRoot: String,
    val messageProofHash: String,
    val accountInclusionRoot: String,
    val accountsLtHashChecksum: String,
    val accountsLtHashProofPublicInputsHash: String,
    bankHashHardForkData: ByteArray,
    accountsLtHash: ByteArray?,
    val transactionSignature: String,
    val emitterProgramId: String,
    val messageId: String,
    val payloadHash: String,
    val commitmentRoot: String,
    val sourceEventDigest: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val sourceAdapterDeploymentHash: String,
    val sourceAdapterDeploymentReceiptHash: String,
    inclusionBranch: List<ByteArray>,
) {
    private val bankHashHardForkDataStorage: ByteArray = bankHashHardForkData.copyOf()
    private val accountsLtHashStorage: ByteArray? = accountsLtHash?.copyOf()
    private val inclusionBranchStorage: List<ByteArray> = inclusionBranch.map { it.copyOf() }

    val bankHashHardForkData: ByteArray
        get() = bankHashHardForkDataStorage.copyOf()

    val accountsLtHash: ByteArray?
        get() = accountsLtHashStorage?.copyOf()

    val inclusionBranch: List<ByteArray>
        get() = inclusionBranchStorage.map { it.copyOf() }

    fun copy(
        version: Int = this.version,
        sourceDomain: Int = this.sourceDomain,
        targetDomain: Int = this.targetDomain,
        mainnetGenesisHash: String = this.mainnetGenesisHash,
        finalizedSlot: String = this.finalizedSlot,
        parentSlot: String = this.parentSlot,
        bankSignatureCount: String = this.bankSignatureCount,
        parentBankHash: String = this.parentBankHash,
        blockhash: String = this.blockhash,
        bankHash: String = this.bankHash,
        transactionStatusRoot: String = this.transactionStatusRoot,
        messageProofHash: String = this.messageProofHash,
        accountInclusionRoot: String = this.accountInclusionRoot,
        accountsLtHashChecksum: String = this.accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash: String = this.accountsLtHashProofPublicInputsHash,
        bankHashHardForkData: ByteArray = this.bankHashHardForkData,
        accountsLtHash: ByteArray? = this.accountsLtHash,
        transactionSignature: String = this.transactionSignature,
        emitterProgramId: String = this.emitterProgramId,
        messageId: String = this.messageId,
        payloadHash: String = this.payloadHash,
        commitmentRoot: String = this.commitmentRoot,
        sourceEventDigest: String = this.sourceEventDigest,
        sourceStateVerifierId: String = this.sourceStateVerifierId,
        sourceStateVerifierHash: String = this.sourceStateVerifierHash,
        sourceAdapterDeploymentHash: String = this.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: String = this.sourceAdapterDeploymentReceiptHash,
        inclusionBranch: List<ByteArray> = this.inclusionBranch,
    ): SolanaSccpWitness =
        SolanaSccpWitness(
            version,
            sourceDomain,
            targetDomain,
            mainnetGenesisHash,
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            blockhash,
            bankHash,
            transactionStatusRoot,
            messageProofHash,
            accountInclusionRoot,
            accountsLtHashChecksum,
            accountsLtHashProofPublicInputsHash,
            bankHashHardForkData,
            accountsLtHash,
            transactionSignature,
            emitterProgramId,
            messageId,
            payloadHash,
            commitmentRoot,
            sourceEventDigest,
            sourceStateVerifierId,
            sourceStateVerifierHash,
            sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash,
            inclusionBranch,
        )

    operator fun component1(): Int = version
    operator fun component2(): Int = sourceDomain
    operator fun component3(): Int = targetDomain
    operator fun component4(): String = mainnetGenesisHash
    operator fun component5(): String = finalizedSlot
    operator fun component6(): String = parentSlot
    operator fun component7(): String = bankSignatureCount
    operator fun component8(): String = parentBankHash
    operator fun component9(): String = blockhash
    operator fun component10(): String = bankHash
    operator fun component11(): String = transactionStatusRoot
    operator fun component12(): String = messageProofHash
    operator fun component13(): String = accountInclusionRoot
    operator fun component14(): String = accountsLtHashChecksum
    operator fun component15(): String = accountsLtHashProofPublicInputsHash
    operator fun component16(): ByteArray = bankHashHardForkData
    operator fun component17(): ByteArray? = accountsLtHash
    operator fun component18(): String = transactionSignature
    operator fun component19(): String = emitterProgramId
    operator fun component20(): String = messageId
    operator fun component21(): String = payloadHash
    operator fun component22(): String = commitmentRoot
    operator fun component23(): String = sourceEventDigest
    operator fun component24(): String = sourceStateVerifierId
    operator fun component25(): String = sourceStateVerifierHash
    operator fun component26(): String = sourceAdapterDeploymentHash
    operator fun component27(): String = sourceAdapterDeploymentReceiptHash
    operator fun component28(): List<ByteArray> = inclusionBranch

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is SolanaSccpWitness &&
            version == other.version &&
            sourceDomain == other.sourceDomain &&
            targetDomain == other.targetDomain &&
            mainnetGenesisHash == other.mainnetGenesisHash &&
            finalizedSlot == other.finalizedSlot &&
            parentSlot == other.parentSlot &&
            bankSignatureCount == other.bankSignatureCount &&
            parentBankHash == other.parentBankHash &&
            blockhash == other.blockhash &&
            bankHash == other.bankHash &&
            transactionStatusRoot == other.transactionStatusRoot &&
            messageProofHash == other.messageProofHash &&
            accountInclusionRoot == other.accountInclusionRoot &&
            accountsLtHashChecksum == other.accountsLtHashChecksum &&
            accountsLtHashProofPublicInputsHash == other.accountsLtHashProofPublicInputsHash &&
            bankHashHardForkDataStorage.contentEquals(other.bankHashHardForkDataStorage) &&
            nullableContentEquals(accountsLtHashStorage, other.accountsLtHashStorage) &&
            transactionSignature == other.transactionSignature &&
            emitterProgramId == other.emitterProgramId &&
            messageId == other.messageId &&
            payloadHash == other.payloadHash &&
            commitmentRoot == other.commitmentRoot &&
            sourceEventDigest == other.sourceEventDigest &&
            sourceStateVerifierId == other.sourceStateVerifierId &&
            sourceStateVerifierHash == other.sourceStateVerifierHash &&
            sourceAdapterDeploymentHash == other.sourceAdapterDeploymentHash &&
            sourceAdapterDeploymentReceiptHash == other.sourceAdapterDeploymentReceiptHash &&
            byteArrayListContentEquals(inclusionBranchStorage, other.inclusionBranchStorage)

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + sourceDomain
        result = 31 * result + targetDomain
        result = 31 * result + mainnetGenesisHash.hashCode()
        result = 31 * result + finalizedSlot.hashCode()
        result = 31 * result + parentSlot.hashCode()
        result = 31 * result + bankSignatureCount.hashCode()
        result = 31 * result + parentBankHash.hashCode()
        result = 31 * result + blockhash.hashCode()
        result = 31 * result + bankHash.hashCode()
        result = 31 * result + transactionStatusRoot.hashCode()
        result = 31 * result + messageProofHash.hashCode()
        result = 31 * result + accountInclusionRoot.hashCode()
        result = 31 * result + accountsLtHashChecksum.hashCode()
        result = 31 * result + accountsLtHashProofPublicInputsHash.hashCode()
        result = 31 * result + bankHashHardForkDataStorage.contentHashCode()
        result = 31 * result + (accountsLtHashStorage?.contentHashCode() ?: 0)
        result = 31 * result + transactionSignature.hashCode()
        result = 31 * result + emitterProgramId.hashCode()
        result = 31 * result + messageId.hashCode()
        result = 31 * result + payloadHash.hashCode()
        result = 31 * result + commitmentRoot.hashCode()
        result = 31 * result + sourceEventDigest.hashCode()
        result = 31 * result + sourceStateVerifierId.hashCode()
        result = 31 * result + sourceStateVerifierHash.hashCode()
        result = 31 * result + sourceAdapterDeploymentHash.hashCode()
        result = 31 * result + sourceAdapterDeploymentReceiptHash.hashCode()
        result = 31 * result + inclusionBranchStorage.fold(1) { acc, sibling ->
            31 * acc + sibling.contentHashCode()
        }
        return result
    }

    override fun toString(): String =
        "SolanaSccpWitness(version=$version, sourceDomain=$sourceDomain, targetDomain=$targetDomain, " +
            "mainnetGenesisHash=$mainnetGenesisHash, finalizedSlot=$finalizedSlot, parentSlot=$parentSlot, " +
            "bankSignatureCount=$bankSignatureCount, parentBankHash=$parentBankHash, blockhash=$blockhash, " +
            "bankHash=$bankHash, transactionStatusRoot=$transactionStatusRoot, messageProofHash=$messageProofHash, " +
            "accountInclusionRoot=$accountInclusionRoot, accountsLtHashChecksum=$accountsLtHashChecksum, " +
            "accountsLtHashProofPublicInputsHash=$accountsLtHashProofPublicInputsHash, " +
            "bankHashHardForkData=${bankHashHardForkDataStorage.size} bytes, " +
            "accountsLtHash=${accountsLtHashStorage?.size ?: 0} bytes, transactionSignature=$transactionSignature, " +
            "emitterProgramId=$emitterProgramId, messageId=$messageId, payloadHash=$payloadHash, " +
            "commitmentRoot=$commitmentRoot, sourceEventDigest=$sourceEventDigest, " +
            "sourceStateVerifierId=$sourceStateVerifierId, sourceStateVerifierHash=$sourceStateVerifierHash, " +
            "sourceAdapterDeploymentHash=$sourceAdapterDeploymentHash, " +
            "sourceAdapterDeploymentReceiptHash=$sourceAdapterDeploymentReceiptHash, " +
            "inclusionBranch=${inclusionBranchStorage.size} nodes)"

    private companion object {
        fun nullableContentEquals(left: ByteArray?, right: ByteArray?): Boolean =
            when {
                left === null -> right === null
                right === null -> false
                else -> left.contentEquals(right)
            }

        fun byteArrayListContentEquals(left: List<ByteArray>, right: List<ByteArray>): Boolean =
            left.size == right.size && left.indices.all { index ->
                left[index].contentEquals(right[index])
            }
    }
}

/** Public inputs exposed by the Solana SCCP proof request. */
data class SolanaSccpPublicInputs(
    val messageId: String,
    val payloadHash: String,
    val commitmentRoot: String,
    val finalizedSlot: String,
    val parentSlot: String,
    val bankSignatureCount: String,
    val parentBankHash: String,
    val blockhash: String,
    val bankHash: String,
    val transactionStatusRoot: String,
    val messageProofHash: String,
    val accountInclusionRoot: String,
    val accountsLtHashChecksum: String,
    val accountsLtHashProofPublicInputsHash: String,
    val sourceEventDigest: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val statementHash: String,
    val destinationBindingHash: String,
    val sourceAdapterDeploymentHash: String,
    val sourceAdapterDeploymentReceiptHash: String,
    val sourceAdapterDeploymentBindingHash: String,
)

/** Statement and verifier deployment context proved by the local Solana SCCP prover. */
data class SolanaSccpProofContext(
    val version: Int,
    val statementHash: String,
    val destinationBindingHash: String,
)

/** Solana StakeHistory sysvar entry bound into SCCP stake-history evidence. */
data class SolanaSccpStakeHistoryEntry(
    val epoch: String,
    val effective: String,
    val activating: String,
    val deactivating: String,
)

/** Source-adapter deployment binding carried by local Solana SCCP proof requests. */
data class SolanaSccpSourceAdapterDeploymentBinding(
    val version: Int,
    val sourceDomain: Int,
    val targetDomain: Int,
    val sourceAdapterDeploymentHash: String,
    val sourceAdapterDeploymentReceiptHash: String,
)

/** Request passed to a linked local Solana SCCP prover. */
data class SolanaSccpProofRequest(
    val version: Int,
    val backend: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val mainnetGenesisHash: String,
    val witnessHash: String,
    val proofContextHash: String,
    val sourceAdapterDeploymentBindingHash: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val publicInputs: SolanaSccpPublicInputs,
    val witness: SolanaSccpWitness,
    val proofContext: SolanaSccpProofContext,
    val sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding,
)

/** Proof envelope returned after local Solana SCCP proof generation. */
class SolanaSccpProofResult(
    val version: Int,
    val backend: String,
    proofBytes: ByteArray,
    val proofBase64: String,
    val publicInputs: SolanaSccpPublicInputs,
    val witnessHash: String,
    val proofContextHash: String,
    val sourceAdapterDeploymentBindingHash: String,
    val sourceStateVerifierId: String,
    val sourceStateVerifierHash: String,
    val proofContext: SolanaSccpProofContext,
    val sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding,
    val envelopeHash: String,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    fun copy(
        version: Int = this.version,
        backend: String = this.backend,
        proofBytes: ByteArray = this.proofBytes,
        proofBase64: String = this.proofBase64,
        publicInputs: SolanaSccpPublicInputs = this.publicInputs,
        witnessHash: String = this.witnessHash,
        proofContextHash: String = this.proofContextHash,
        sourceAdapterDeploymentBindingHash: String = this.sourceAdapterDeploymentBindingHash,
        sourceStateVerifierId: String = this.sourceStateVerifierId,
        sourceStateVerifierHash: String = this.sourceStateVerifierHash,
        proofContext: SolanaSccpProofContext = this.proofContext,
        sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding =
            this.sourceAdapterDeploymentBinding,
        envelopeHash: String = this.envelopeHash,
    ): SolanaSccpProofResult =
        SolanaSccpProofResult(
            version,
            backend,
            proofBytes,
            proofBase64,
            publicInputs,
            witnessHash,
            proofContextHash,
            sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId,
            sourceStateVerifierHash,
            proofContext,
            sourceAdapterDeploymentBinding,
            envelopeHash,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): ByteArray = proofBytes
    operator fun component4(): String = proofBase64
    operator fun component5(): SolanaSccpPublicInputs = publicInputs
    operator fun component6(): String = witnessHash
    operator fun component7(): String = proofContextHash
    operator fun component8(): String = sourceAdapterDeploymentBindingHash
    operator fun component9(): String = sourceStateVerifierId
    operator fun component10(): String = sourceStateVerifierHash
    operator fun component11(): SolanaSccpProofContext = proofContext
    operator fun component12(): SolanaSccpSourceAdapterDeploymentBinding =
        sourceAdapterDeploymentBinding
    operator fun component13(): String = envelopeHash

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is SolanaSccpProofResult &&
            version == other.version &&
            backend == other.backend &&
            proofBytesStorage.contentEquals(other.proofBytesStorage) &&
            proofBase64 == other.proofBase64 &&
            publicInputs == other.publicInputs &&
            witnessHash == other.witnessHash &&
            proofContextHash == other.proofContextHash &&
            sourceAdapterDeploymentBindingHash == other.sourceAdapterDeploymentBindingHash &&
            sourceStateVerifierId == other.sourceStateVerifierId &&
            sourceStateVerifierHash == other.sourceStateVerifierHash &&
            proofContext == other.proofContext &&
            sourceAdapterDeploymentBinding == other.sourceAdapterDeploymentBinding &&
            envelopeHash == other.envelopeHash

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + backend.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        result = 31 * result + proofBase64.hashCode()
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + witnessHash.hashCode()
        result = 31 * result + proofContextHash.hashCode()
        result = 31 * result + sourceAdapterDeploymentBindingHash.hashCode()
        result = 31 * result + sourceStateVerifierId.hashCode()
        result = 31 * result + sourceStateVerifierHash.hashCode()
        result = 31 * result + proofContext.hashCode()
        result = 31 * result + sourceAdapterDeploymentBinding.hashCode()
        result = 31 * result + envelopeHash.hashCode()
        return result
    }

    override fun toString(): String =
        "SolanaSccpProofResult(version=$version, backend=$backend, " +
            "proofBytes=${proofBytesStorage.size} bytes, proofBase64=$proofBase64, " +
            "publicInputs=$publicInputs, witnessHash=$witnessHash, " +
            "proofContextHash=$proofContextHash, " +
            "sourceAdapterDeploymentBindingHash=$sourceAdapterDeploymentBindingHash, " +
            "sourceStateVerifierId=$sourceStateVerifierId, " +
            "sourceStateVerifierHash=$sourceStateVerifierHash, proofContext=$proofContext, " +
            "sourceAdapterDeploymentBinding=$sourceAdapterDeploymentBinding, " +
            "envelopeHash=$envelopeHash)"
}

/** Transparent SCCP public inputs serialized into Solana verifier instruction data. */
data class SolanaSccpSubmissionPublicInputs(
    val version: Int = 1,
    val messageId: String,
    val payloadHash: String,
    val targetDomain: Int,
    val commitmentRoot: String,
    val finalityHeight: String,
    val finalityBlockHash: String,
)

/** Inputs for a Solana SCCP verifier program instruction. */
class SolanaSccpSubmissionInput(
    val publicInputs: SolanaSccpSubmissionPublicInputs,
    proofBytes: ByteArray,
    bundleBytes: ByteArray,
    val statementHash: String,
    val destinationBindingHash: String,
    val proofContextHash: String? = null,
    val proofResult: SolanaSccpProofResult? = null,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    constructor(
        publicInputs: SolanaSccpSubmissionPublicInputs,
        proofResult: SolanaSccpProofResult,
        bundleBytes: ByteArray,
    ) : this(
        publicInputs = publicInputs,
        proofBytes = SccpSolana.requireWrappedProofResultForSubmission(proofResult, publicInputs).proofBytes,
        bundleBytes = bundleBytes,
        statementHash = proofResult.proofContext.statementHash,
        destinationBindingHash = proofResult.proofContext.destinationBindingHash,
        proofContextHash = proofResult.proofContextHash,
        proofResult = proofResult,
    )

    fun copy(
        publicInputs: SolanaSccpSubmissionPublicInputs = this.publicInputs,
        proofBytes: ByteArray = this.proofBytes,
        bundleBytes: ByteArray = this.bundleBytes,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        proofContextHash: String? = this.proofContextHash,
        proofResult: SolanaSccpProofResult? = this.proofResult,
    ): SolanaSccpSubmissionInput =
        SolanaSccpSubmissionInput(
            publicInputs,
            proofBytes,
            bundleBytes,
            statementHash,
            destinationBindingHash,
            proofContextHash,
            proofResult,
        )

    operator fun component1(): SolanaSccpSubmissionPublicInputs = publicInputs
    operator fun component2(): ByteArray = proofBytes
    operator fun component3(): ByteArray = bundleBytes
    operator fun component4(): String = statementHash
    operator fun component5(): String = destinationBindingHash
    operator fun component6(): String? = proofContextHash
    operator fun component7(): SolanaSccpProofResult? = proofResult

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is SolanaSccpSubmissionInput &&
            publicInputs == other.publicInputs &&
            proofBytesStorage.contentEquals(other.proofBytesStorage) &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            proofContextHash == other.proofContextHash &&
            proofResult == other.proofResult

    override fun hashCode(): Int {
        var result = publicInputs.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + (proofContextHash?.hashCode() ?: 0)
        result = 31 * result + (proofResult?.hashCode() ?: 0)
        return result
    }

    override fun toString(): String =
        "SolanaSccpSubmissionInput(publicInputs=$publicInputs, " +
            "proofBytes=${proofBytesStorage.size} bytes, " +
            "bundleBytes=${bundleBytesStorage.size} bytes, statementHash=$statementHash, " +
            "destinationBindingHash=$destinationBindingHash, proofContextHash=$proofContextHash, " +
            "proofResult=${proofResult != null})"
}

/** One Solana SCCP submission argument in Rust template order. */
data class SolanaSccpSubmissionArgument(
    val key: String,
    val encoding: String,
    val bytesHex: String,
)

/** Prebuilt Solana SCCP verifier instruction data for wallet or RPC submission. */
class SolanaSccpSubmission private constructor(
    val version: Int,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    proofBytes: ByteArray,
    val publicInputs: SolanaSccpSubmissionPublicInputs,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    val statementHash: String,
    val destinationBindingHash: String,
    val proofContextHash: String,
    argumentsInput: List<SolanaSccpSubmissionArgument>,
    instructionData: ByteArray,
    val instructionDataHex: String,
    envelopeBytes: ByteArray,
    val envelopeHex: String,
    @Suppress("UNUSED_PARAMETER") privateMarker: Unit,
) {
    constructor(
        version: Int,
        envelopeEncoding: String,
        submissionKind: String,
        verifierEntrypoint: String,
        proofBytes: ByteArray,
        publicInputs: SolanaSccpSubmissionPublicInputs,
        publicInputsBytes: ByteArray,
        bundleBytes: ByteArray,
        statementHash: String,
        destinationBindingHash: String,
        proofContextHash: String,
        arguments: List<SolanaSccpSubmissionArgument>,
        instructionData: ByteArray,
        instructionDataHex: String,
        envelopeBytes: ByteArray,
        envelopeHex: String,
    ) : this(
        version,
        envelopeEncoding,
        submissionKind,
        verifierEntrypoint,
        proofBytes,
        publicInputs,
        publicInputsBytes,
        bundleBytes,
        statementHash,
        destinationBindingHash,
        proofContextHash,
        arguments.toList(),
        instructionData,
        instructionDataHex,
        envelopeBytes,
        envelopeHex,
        Unit,
    )

    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val instructionDataStorage: ByteArray = instructionData.copyOf()
    private val envelopeBytesStorage: ByteArray = envelopeBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val instructionData: ByteArray
        get() = instructionDataStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = envelopeBytesStorage.copyOf()

    val arguments: List<SolanaSccpSubmissionArgument> = argumentsInput.toList()

    fun copy(
        version: Int = this.version,
        envelopeEncoding: String = this.envelopeEncoding,
        submissionKind: String = this.submissionKind,
        verifierEntrypoint: String = this.verifierEntrypoint,
        proofBytes: ByteArray = this.proofBytes,
        publicInputs: SolanaSccpSubmissionPublicInputs = this.publicInputs,
        publicInputsBytes: ByteArray = this.publicInputsBytes,
        bundleBytes: ByteArray = this.bundleBytes,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        proofContextHash: String = this.proofContextHash,
        arguments: List<SolanaSccpSubmissionArgument> = this.arguments,
        instructionData: ByteArray = this.instructionData,
        instructionDataHex: String = this.instructionDataHex,
        envelopeBytes: ByteArray = this.envelopeBytes,
        envelopeHex: String = this.envelopeHex,
    ): SolanaSccpSubmission =
        SolanaSccpSubmission(
            version,
            envelopeEncoding,
            submissionKind,
            verifierEntrypoint,
            proofBytes,
            publicInputs,
            publicInputsBytes,
            bundleBytes,
            statementHash,
            destinationBindingHash,
            proofContextHash,
            arguments,
            instructionData,
            instructionDataHex,
            envelopeBytes,
            envelopeHex,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = envelopeEncoding
    operator fun component3(): String = submissionKind
    operator fun component4(): String = verifierEntrypoint
    operator fun component5(): ByteArray = proofBytes
    operator fun component6(): SolanaSccpSubmissionPublicInputs = publicInputs
    operator fun component7(): ByteArray = publicInputsBytes
    operator fun component8(): ByteArray = bundleBytes
    operator fun component9(): String = statementHash
    operator fun component10(): String = destinationBindingHash
    operator fun component11(): String = proofContextHash
    operator fun component12(): List<SolanaSccpSubmissionArgument> = arguments
    operator fun component13(): ByteArray = instructionData
    operator fun component14(): String = instructionDataHex
    operator fun component15(): ByteArray = envelopeBytes
    operator fun component16(): String = envelopeHex

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is SolanaSccpSubmission &&
            version == other.version &&
            envelopeEncoding == other.envelopeEncoding &&
            submissionKind == other.submissionKind &&
            verifierEntrypoint == other.verifierEntrypoint &&
            proofBytesStorage.contentEquals(other.proofBytesStorage) &&
            publicInputs == other.publicInputs &&
            publicInputsBytesStorage.contentEquals(other.publicInputsBytesStorage) &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            proofContextHash == other.proofContextHash &&
            arguments == other.arguments &&
            instructionDataStorage.contentEquals(other.instructionDataStorage) &&
            instructionDataHex == other.instructionDataHex &&
            envelopeBytesStorage.contentEquals(other.envelopeBytesStorage) &&
            envelopeHex == other.envelopeHex

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + envelopeEncoding.hashCode()
        result = 31 * result + submissionKind.hashCode()
        result = 31 * result + verifierEntrypoint.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + publicInputsBytesStorage.contentHashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + proofContextHash.hashCode()
        result = 31 * result + arguments.hashCode()
        result = 31 * result + instructionDataStorage.contentHashCode()
        result = 31 * result + instructionDataHex.hashCode()
        result = 31 * result + envelopeBytesStorage.contentHashCode()
        result = 31 * result + envelopeHex.hashCode()
        return result
    }

    override fun toString(): String =
        "SolanaSccpSubmission(version=$version, envelopeEncoding=$envelopeEncoding, " +
            "submissionKind=$submissionKind, verifierEntrypoint=$verifierEntrypoint, " +
            "proofBytes=${proofBytesStorage.size} bytes, publicInputs=$publicInputs, " +
            "publicInputsBytes=${publicInputsBytesStorage.size} bytes, " +
            "bundleBytes=${bundleBytesStorage.size} bytes, statementHash=$statementHash, " +
            "destinationBindingHash=$destinationBindingHash, proofContextHash=$proofContextHash, " +
            "arguments=$arguments, instructionData=${instructionDataStorage.size} bytes, " +
            "instructionDataHex=$instructionDataHex, envelopeBytes=${envelopeBytesStorage.size} bytes, " +
            "envelopeHex=$envelopeHex)"
}

/** Optional witness resolver backed by app-controlled Solana RPC calls. */
fun interface SolanaSccpWitnessProvider {
    fun resolveWitness(input: SolanaSccpWitnessInput): SolanaSccpWitnessInput
}

/** Local proof engine linked by the application bundle. */
fun interface SolanaSccpProofEngine {
    fun prove(request: SolanaSccpProofRequest): ByteArray
}

/** Local proof engine for nested Solana AccountsLtHash source-state requests. */
fun interface SolanaSccpAccountsLtHashProofEngine {
    fun prove(request: SolanaSccpAccountsLtHashProofRequest): ByteArray
}

/** Local proof engine for Solana full-light source-state audit role requests. */
fun interface SolanaSccpFullLightClientAuditProofEngine {
    fun prove(request: SolanaSccpFullLightClientAuditProofRequest): ByteArray
}

/** Role-separated Solana full-light audit proof capsules. */
data class SolanaSccpFullLightClientAuditProofs(
    val towerReplay: SolanaSccpSourceStateVerificationProof,
    val fullAccountsdbLattice: SolanaSccpSourceStateVerificationProof,
    val bankForkChoice: SolanaSccpSourceStateVerificationProof,
)

/** Source-state proof wrapper for UI and mobile Solana proof engines. */
class SolanaSccpSourceStateProver(
    private val accountsLtHashProofEngine: SolanaSccpAccountsLtHashProofEngine? = null,
    private val fullLightClientAuditProofEngine: SolanaSccpFullLightClientAuditProofEngine? = null,
) {
    fun proveAccountsLtHash(
        witnessInput: SolanaSccpWitnessInput,
        openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput,
    ): SolanaSccpSourceStateVerificationProof =
        proveAccountsLtHash(SccpSolana.buildAccountsLtHashProofRequest(witnessInput, openedAccounts))

    fun proveAccountsLtHash(
        request: SolanaSccpAccountsLtHashProofRequest,
    ): SolanaSccpSourceStateVerificationProof {
        SccpSolana.requireCanonicalSourceStateProofRequest(request)
        val engine = accountsLtHashProofEngine
            ?: throw IllegalStateException("Solana SCCP source-state prover is not linked")
        return SccpSolana.wrapSourceStateVerificationProof(
            engine.prove(callbackRequestSnapshot(request)),
            request,
        )
    }

    fun proveFullLightClientAudit(
        input: SolanaSccpFullLightClientAuditProofInput,
    ): SolanaSccpFullLightClientAuditProofs {
        val requests = SccpSolana.buildFullLightClientAuditProofRequests(input)
        return SolanaSccpFullLightClientAuditProofs(
            towerReplay = proveFullLightClientAudit(requests.towerReplay),
            fullAccountsdbLattice = proveFullLightClientAudit(requests.fullAccountsdbLattice),
            bankForkChoice = proveFullLightClientAudit(requests.bankForkChoice),
        )
    }

    fun proveFullLightClientAudit(
        request: SolanaSccpFullLightClientAuditProofRequest,
    ): SolanaSccpSourceStateVerificationProof {
        SccpSolana.requireCanonicalSourceStateProofRequest(request)
        val engine = fullLightClientAuditProofEngine
            ?: throw IllegalStateException("Solana SCCP source-state prover is not linked")
        return SccpSolana.wrapSourceStateVerificationProof(
            engine.prove(callbackRequestSnapshot(request)),
            request,
        )
    }

    private fun callbackRequestSnapshot(
        request: SolanaSccpAccountsLtHashProofRequest,
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
            accountsLtHashProofPublicInputsHash = request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash = request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum = request.openedAccountsLtHashResidualChecksum,
            statementBytes = request.statementBytes,
            accountCommitmentBytes = request.accountCommitmentBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = request.fastpqTransitions.map {
                SolanaSccpAccountsLtHashFastpqTransition(
                    key = it.key,
                    operation = it.operation,
                    oldValue = it.oldValue,
                    newValue = it.newValue,
                )
            },
        )

    private fun callbackRequestSnapshot(
        request: SolanaSccpFullLightClientAuditProofRequest,
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
            verifierHash = request.verifierHash,
            sourceStateVerifierId = request.sourceStateVerifierId,
            sourceStateVerifierHash = request.sourceStateVerifierHash,
            sourceVerifierMaterialHash = request.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash = request.sourceAdapterDeploymentHash,
            fullLightClientGateHash = request.fullLightClientGateHash,
            finalityContextHash = request.finalityContextHash,
            voteMessageHash = request.voteMessageHash,
            accountsLtHashProofHash = request.accountsLtHashProofHash,
            auditStatementHash = request.auditStatementHash,
            statementBytes = request.statementBytes,
            verificationContextBytes = request.verificationContextBytes,
            schemaDescriptor = request.schemaDescriptor,
            publicInputColumns = request.publicInputColumns,
            fastpqPublicInputs = request.fastpqPublicInputs,
            fastpqTransitions = request.fastpqTransitions.map {
                SolanaSccpFullLightClientAuditFastpqTransition(
                    key = it.key,
                    operation = it.operation,
                    oldValue = it.oldValue,
                    newValue = it.newValue,
                )
            },
        )
}

/** Local-first Solana SCCP proof wrapper for UI SDKs. */
class SolanaSccpProver(
    private val witnessProvider: SolanaSccpWitnessProvider? = null,
    private val proofEngine: SolanaSccpProofEngine? = null,
) {
    fun buildRequest(input: SolanaSccpWitnessInput): SolanaSccpProofRequest {
        val resolved = witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input
        return SccpSolana.buildProofRequest(resolved)
    }

    fun prove(input: SolanaSccpWitnessInput): SolanaSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine
            ?: throw IllegalStateException("Solana SCCP local prover is not linked")
        SccpSolana.requireProductionProofRequest(request)
        return SccpSolana.wrapProofResult(engine.prove(callbackRequestSnapshot(request)), request)
    }

    private fun callbackRequestSnapshot(request: SolanaSccpProofRequest): SolanaSccpProofRequest =
        request.copy(
            witness = request.witness.copy(
                bankHashHardForkData = request.witness.bankHashHardForkData.copyOf(),
                accountsLtHash = request.witness.accountsLtHash?.copyOf(),
                inclusionBranch = request.witness.inclusionBranch.map { it.copyOf() },
            ),
        )

    private fun witnessProviderInputSnapshot(input: SolanaSccpWitnessInput): SolanaSccpWitnessInput =
        input.copy(
            bankHashHardForkData = input.bankHashHardForkData.copyOf(),
            accountsLtHash = input.accountsLtHash?.copyOf(),
            inclusionBranch = input.inclusionBranch.map { it.copyOf() },
        )
}
