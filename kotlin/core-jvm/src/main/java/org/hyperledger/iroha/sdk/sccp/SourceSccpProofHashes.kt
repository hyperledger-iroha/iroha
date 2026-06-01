package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.bouncycastle.crypto.digests.KeccakDigest
import org.bouncycastle.crypto.ec.CustomNamedCurves
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Source-chain SCCP proof-hash helpers for local-first UI proof generation. */
object SccpSourceProofs {
    const val DOMAIN_SORA: Int = 0
    const val DOMAIN_ETH: Int = 1
    const val DOMAIN_BSC: Int = 2
    const val DOMAIN_SOL: Int = 3
    const val DOMAIN_TON: Int = 4
    const val DOMAIN_TRON: Int = 5
    const val DOMAIN_SORA_KUSAMA: Int = 6
    const val DOMAIN_SORA_POLKADOT: Int = 7
    const val DOMAIN_SORA2: Int = 8

    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private const val BSC_PARLIA_EXTRA_VANITY_BYTES: Int = 32
    private const val BSC_PARLIA_EXTRA_SEAL_BYTES: Int = 65
    private const val BSC_PARLIA_VALIDATOR_ADDRESS_BYTES: Int = 20
    private const val BSC_PARLIA_VALIDATOR_BLS_KEY_BYTES: Int = 48
    private const val BSC_MAX_PARLIA_VALIDATORS: Int = 255
    private const val BSC_MAX_VALIDATOR_SET_PAYLOAD_BYTES: Int =
        1 + 4 + BSC_MAX_PARLIA_VALIDATORS * (BSC_PARLIA_VALIDATOR_ADDRESS_BYTES + 8)
    private const val BSC_PARLIA_EPOCH_LENGTH_BLOCKS: Long = 200L
    private const val SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1: String = "sccp-source-adapter-v1"
    private const val SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1: String = "fastpq-lane-balanced"
    private const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"
    private const val EVM_DESTINATION_BINDING_LABEL_V1: String =
        "iroha:sccp:evm-destination-binding:v1"
    private const val TRON_DESTINATION_BINDING_LABEL_V1: String =
        "iroha:sccp:tron-destination-binding:v1"
    private const val BASE58_ALPHABET: String =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    const val SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1: String =
        "sccp-substrate-runtime-storage-v1"
    const val SOLANA_MAINNET_TOWER_REPLAY_VERIFIER_ID_V1: String =
        "sccp:sol:light-client:tower-replay-mainnet-beta:v1"
    const val SOLANA_MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1: String =
        "sccp:sol:light-client:full-accountsdb-lattice-mainnet-beta:v1"
    const val SOLANA_MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1: String =
        "sccp:sol:light-client:bank-fork-choice-mainnet-beta:v1"
    const val TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1: String =
        "sccp:ton:light-client:masterchain-config-mainnet:v1"
    const val TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1: String =
        "sccp:ton:light-client:validator-set-transition-mainnet:v1"
    const val TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1: String =
        "sccp:ton:light-client:shard-accounts-dictionary-mainnet:v1"
    private const val DESTINATION_BINDING_PREFIX_V1: String = "sccp:destination:binding:v1"
    private const val SOURCE_ADAPTER_FASTPQ_TRACE_ROOT_V1: Long = 0x002A_247F_81C6_F850L
    private const val SOURCE_ADAPTER_FASTPQ_LDE_ROOT_V1: Long = 0x6026_3388_DBBF_9B2AL
    private const val SOURCE_ADAPTER_FASTPQ_OMEGA_COSET_V1: Long = 0x6AF3_25E8_25AD_5C18L
    private const val EVM_MAX_RECEIPT_VALUE_BYTES: Int = 16 * 1024
    private const val ETH_EXECUTION_PAYLOAD_BODY_FIELD_INDEX: Int = 9
    private const val ETH_EXECUTION_PAYLOAD_BODY_BRANCH_DEPTH: Int = 4
    private const val ETH_MAX_SYNC_COMMITTEE_AUTHORITIES: Int = 512
    private const val ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES: Int = 48
    private const val ETH_SYNC_COMMITTEE_POP_BYTES: Int = 96
    private const val ETH_SYNC_COMMITTEE_SIGNATURE_BYTES: Int = 96
    private const val ETH_MAX_SYNC_COMMITTEE_PUBLIC_KEY_BYTES: Int = 96
    private const val ETH_MAX_SYNC_COMMITTEE_POP_BYTES: Int = 256
    private const val ETH_MAX_SYNC_COMMITTEE_PAYLOAD_BYTES: Int =
        1 + 4 + ETH_MAX_SYNC_COMMITTEE_AUTHORITIES *
            (4 + ETH_MAX_SYNC_COMMITTEE_PUBLIC_KEY_BYTES + 8 + 4 + ETH_MAX_SYNC_COMMITTEE_POP_BYTES)
    private const val TON_MAX_VALIDATORS: Int = 1024
    private const val TRON_MAX_MPT_PROOF_NODES: Int = 64
    private const val TRON_MAX_MPT_NODE_BYTES: Int = 16 * 1024
    private const val TRON_MAX_RAW_HEADER_BYTES: Int = 16 * 1024
    private const val TRON_MAX_RECEIPT_VALUE_BYTES: Int = 16 * 1024
    private const val TRON_MAX_TRANSACTION_BYTES: Int = 64 * 1024
    private const val TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES: Int = 64
    private const val TRON_SOURCE_CALL_SIGNATURES: Int = 1
    private const val TRON_MAX_WITNESSES: Int = 64
    private const val SUBSTRATE_MAX_AUTHORITIES: Int = 2048
    private const val SUBSTRATE_MAX_AUTHORITY_SET_PAYLOAD_BYTES: Int =
        1 + 4 + SUBSTRATE_MAX_AUTHORITIES * (32 + 8)
    private val SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY: ByteArray =
        hexBytes(
            "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7",
            "systemEventsStorageKey",
            32,
        )
    private const val SUBSTRATE_RUNTIME_STORAGE_PROOF_PUBLIC_INPUTS_PREFIX_V1: String =
        "sccp:substrate:runtime-storage-proof-public-inputs:v1"
    private const val SUBSTRATE_RUNTIME_STORAGE_FASTPQ_DSID_PREFIX_V1: String =
        "sccp:substrate:runtime-storage:fastpq:dsid:v1"
    private const val SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1: String =
        "fastpq-lane-balanced"
    private const val SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STATEMENT_KEY_V1: String =
        "sccp:substrate:runtime-storage:v1:statement"
    private const val SUBSTRATE_RUNTIME_STORAGE_FASTPQ_CONTEXT_KEY_V1: String =
        "sccp:substrate:runtime-storage:v1:context"
    private const val SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STORAGE_KEY_V1: String =
        "sccp:substrate:runtime-storage:v1:storage-key"
    private val SUBSTRATE_TEMPLATE_SOURCE_STATE_VERIFIER_HASHES: Map<Int, ByteArray> = mapOf(
        DOMAIN_SORA_KUSAMA to hexBytes(
            "0xaf2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c",
            "soraKusamaTemplateSourceStateVerifierHash",
            32,
        ),
        DOMAIN_SORA_POLKADOT to hexBytes(
            "0x664576f1a2409099c3b7dba82512c8757501f2869aedda0e45f858572b940b5d",
            "soraPolkadotTemplateSourceStateVerifierHash",
            32,
        ),
        DOMAIN_SORA2 to hexBytes(
            "0x20509eb56524c727b6d028cc6b43f10c17048d31b92d5a96d41c0512d16267ef",
            "sora2TemplateSourceStateVerifierHash",
            32,
        ),
    )
    private val TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH: ByteArray =
        hexBytes(
            "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
            "tonTemplateSourceStateVerifierHash",
            32,
        )
    private val SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH: ByteArray =
        hexBytes(
            SccpSolana.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
            "solanaTemplateSourceStateVerifierHash",
            32,
        )
    private val SOLANA_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH: ByteArray =
        hexBytes(
            "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
            "solanaTemplateSourceTrustAnchorHash",
            32,
        )
    private val SOLANA_TEMPLATE_CONSENSUS_VERIFIER_HASH: ByteArray =
        hexBytes(
            "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
            "solanaTemplateConsensusVerifierHash",
            32,
        )
    private val SOLANA_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH: ByteArray =
        hexBytes(
            "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
            "solanaTemplateMessageInclusionVerifierHash",
            32,
        )
    private val SOLANA_TEMPLATE_FINALITY_POLICY_HASH: ByteArray =
        hexBytes(
            "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56",
            "solanaTemplateFinalityPolicyHash",
            32,
        )
    private val TON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH: ByteArray =
        hexBytes(
            "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
            "tonTemplateSourceTrustAnchorHash",
            32,
        )
    private val TON_TEMPLATE_CONSENSUS_VERIFIER_HASH: ByteArray =
        hexBytes(
            "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
            "tonTemplateConsensusVerifierHash",
            32,
        )
    private val TON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH: ByteArray =
        hexBytes(
            "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
            "tonTemplateMessageInclusionVerifierHash",
            32,
        )
    private val TON_TEMPLATE_FINALITY_POLICY_HASH: ByteArray =
        hexBytes(
            "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
            "tonTemplateFinalityPolicyHash",
            32,
        )
    private val TRON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH: ByteArray =
        hexBytes(
            "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c",
            "tronTemplateSourceTrustAnchorHash",
            32,
        )
    private val TRON_TEMPLATE_CONSENSUS_VERIFIER_HASH: ByteArray =
        hexBytes(
            "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea",
            "tronTemplateConsensusVerifierHash",
            32,
        )
    private val TRON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH: ByteArray =
        hexBytes(
            "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc",
            "tronTemplateMessageInclusionVerifierHash",
            32,
        )
    private val TRON_TEMPLATE_FINALITY_POLICY_HASH: ByteArray =
        hexBytes(
            "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864",
            "tronTemplateFinalityPolicyHash",
            32,
        )
    private val SECP256K1_SCALAR_ORDER_BE: ByteArray =
        hexBytes("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", "secp256k1Order", 32)
    private val SECP256K1_SCALAR_HALF_ORDER_BE: ByteArray =
        hexBytes("7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0", "secp256k1HalfOrder", 32)
    private val SECP256K1_PARAMS = CustomNamedCurves.getByName("secp256k1")
    private val SECP256K1_SCALAR_ORDER = SECP256K1_PARAMS.n
    private val SECP256K1_FIELD_PRIME =
        BigInteger("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f", 16)
    private val EVM_RECEIPT_ROOT_VALUE_MARKER: ByteArray =
        "sccp:evm:receipt-root-value:v1".toByteArray(StandardCharsets.UTF_8)
    private val TRON_RECEIPT_ROOT_VALUE_MARKER: ByteArray =
        "sccp:tron:receipt-root-value:v1".toByteArray(StandardCharsets.UTF_8)
    private val TRON_SOURCE_MESSAGE_CALL_ABI: ByteArray =
        "submitSccpSourceEvent(uint32,uint32,bytes32)".toByteArray(StandardCharsets.UTF_8)
    private val TRON_TRIGGER_SMART_CONTRACT_TYPE_URL: ByteArray =
        "type.googleapis.com/protocol.TriggerSmartContract".toByteArray(StandardCharsets.UTF_8)
    private val TRON_SOURCE_BRIDGE_CONFIG_LABEL: ByteArray =
        "iroha:sccp:tron-source-bridge-config:v1".toByteArray(StandardCharsets.UTF_8)
    private const val TON_MASTERCHAIN_WORKCHAIN_ID: Int = -1
    private val TON_MASTERCHAIN_SHARD: BigInteger = BigInteger.ONE.shiftLeft(63)
    private const val TON_BASECHAIN_WORKCHAIN_ID: Int = 0

    /** FastPQ public inputs used by Substrate runtime-storage source-state proofs. */
    data class SubstrateRuntimeStorageFastpqPublicInputs(
        val dsid: String,
        val slot: String,
        val oldRoot: String,
        val newRoot: String,
        val permRoot: String,
        val txSetHash: String,
    )

    /** FastPQ metadata transition used by Substrate runtime-storage source-state proofs. */
    data class SubstrateRuntimeStorageFastpqTransition(
        val key: String,
        val operation: String,
        val oldValue: String,
        val newValue: String,
    )

    /** Canonical governed EVM-family destination binding derived from deployment material. */
    data class EvmDestinationBinding(
        val version: Int,
        val sourceDomain: Int,
        val targetDomain: Int,
        val networkId: String,
        val verifierAddress: String,
        val bridgeAddress: String,
        val verifierCodeHash: String,
        val verifierKeyHash: String,
        val verifierBackend: String,
        val proofFamily: String,
        val key: String,
        val hash: String,
    )

    /** Canonical governed TRON destination binding derived from deployment material. */
    data class TronDestinationBinding(
        val version: Int,
        val sourceDomain: Int,
        val targetDomain: Int,
        val networkId: String,
        val verifierAddress: String,
        val verifierCodeHash: String,
        val verifierKeyHash: String,
        val verifierBackend: String,
        val proofFamily: String,
        val key: String,
        val hash: String,
    )

    /** Deterministic proof request for JVM/Android Substrate runtime-storage provers. */
    class SubstrateRuntimeStorageProofRequest(
        val version: Int,
        val proofFamily: String,
        val circuitId: String,
        val parameterSet: String,
        val sourceDomain: Int,
        val finalizedBlockNumber: String,
        val grandpaSetId: String,
        val sourceStateVerifierId: String,
        val sourceStateVerifierHash: String,
        val runtimeStorageProofPublicInputsHash: String,
        val storageProofHash: String,
        statementBytes: ByteArray,
        verificationContextBytes: ByteArray,
        schemaDescriptor: ByteArray,
        publicInputColumns: List<List<String>>,
        val fastpqPublicInputs: SubstrateRuntimeStorageFastpqPublicInputs,
        fastpqTransitions: List<SubstrateRuntimeStorageFastpqTransition>,
    ) {
        private val statementBytesStorage: ByteArray = statementBytes.copyOf()
        private val verificationContextBytesStorage: ByteArray = verificationContextBytes.copyOf()
        private val schemaDescriptorStorage: ByteArray = schemaDescriptor.copyOf()
        private val publicInputColumnsStorage: List<List<String>> = publicInputColumns.map { it.toList() }
        private val fastpqTransitionsStorage: List<SubstrateRuntimeStorageFastpqTransition> =
            fastpqTransitions.toList()

        val statementBytes: ByteArray
            get() = statementBytesStorage.copyOf()

        val verificationContextBytes: ByteArray
            get() = verificationContextBytesStorage.copyOf()

        val schemaDescriptor: ByteArray
            get() = schemaDescriptorStorage.copyOf()

        val publicInputColumns: List<List<String>>
            get() = publicInputColumnsStorage.map { it.toList() }

        val fastpqTransitions: List<SubstrateRuntimeStorageFastpqTransition>
            get() = fastpqTransitionsStorage.toList()

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is SubstrateRuntimeStorageProofRequest) return false
            return version == other.version &&
                proofFamily == other.proofFamily &&
                circuitId == other.circuitId &&
                parameterSet == other.parameterSet &&
                sourceDomain == other.sourceDomain &&
                finalizedBlockNumber == other.finalizedBlockNumber &&
                grandpaSetId == other.grandpaSetId &&
                sourceStateVerifierId == other.sourceStateVerifierId &&
                sourceStateVerifierHash == other.sourceStateVerifierHash &&
                runtimeStorageProofPublicInputsHash == other.runtimeStorageProofPublicInputsHash &&
                storageProofHash == other.storageProofHash &&
                statementBytesStorage.contentEquals(other.statementBytesStorage) &&
                verificationContextBytesStorage.contentEquals(other.verificationContextBytesStorage) &&
                schemaDescriptorStorage.contentEquals(other.schemaDescriptorStorage) &&
                publicInputColumnsStorage == other.publicInputColumnsStorage &&
                fastpqPublicInputs == other.fastpqPublicInputs &&
                fastpqTransitionsStorage == other.fastpqTransitionsStorage
        }

        override fun hashCode(): Int {
            var result = version
            result = 31 * result + proofFamily.hashCode()
            result = 31 * result + circuitId.hashCode()
            result = 31 * result + parameterSet.hashCode()
            result = 31 * result + sourceDomain
            result = 31 * result + finalizedBlockNumber.hashCode()
            result = 31 * result + grandpaSetId.hashCode()
            result = 31 * result + sourceStateVerifierId.hashCode()
            result = 31 * result + sourceStateVerifierHash.hashCode()
            result = 31 * result + runtimeStorageProofPublicInputsHash.hashCode()
            result = 31 * result + storageProofHash.hashCode()
            result = 31 * result + statementBytesStorage.contentHashCode()
            result = 31 * result + verificationContextBytesStorage.contentHashCode()
            result = 31 * result + schemaDescriptorStorage.contentHashCode()
            result = 31 * result + publicInputColumnsStorage.hashCode()
            result = 31 * result + fastpqPublicInputs.hashCode()
            result = 31 * result + fastpqTransitionsStorage.hashCode()
            return result
        }

        override fun toString(): String =
            "SubstrateRuntimeStorageProofRequest(version=$version, proofFamily=$proofFamily, " +
                "circuitId=$circuitId, parameterSet=$parameterSet, sourceDomain=$sourceDomain, " +
                "finalizedBlockNumber=$finalizedBlockNumber, grandpaSetId=$grandpaSetId, " +
                "sourceStateVerifierId=$sourceStateVerifierId, " +
                "sourceStateVerifierHash=$sourceStateVerifierHash, " +
                "runtimeStorageProofPublicInputsHash=$runtimeStorageProofPublicInputsHash, " +
                "storageProofHash=$storageProofHash, statementBytes=${statementBytesStorage.size} bytes, " +
                "verificationContextBytes=${verificationContextBytesStorage.size} bytes, " +
                "schemaDescriptor=${schemaDescriptorStorage.size} bytes, " +
                "publicInputColumns=$publicInputColumnsStorage, fastpqPublicInputs=$fastpqPublicInputs, " +
                "fastpqTransitions=$fastpqTransitionsStorage)"
    }

    /** One BSC ValidatorSet storage-slot proof transcript entry. */
    data class BscValidatorStorageProof(
        val version: Int = 1,
        val validatorIndex: Int,
        val storageSlot: String,
        val storageValue: ByteArray,
        val storageValueHash: String,
        val storageProofNodes: List<ByteArray>,
    )

    /** BSC ValidatorSet account/storage proof transcript material. */
    data class BscValidatorSetMetadataProof(
        val version: Int = 1,
        val stateRoot: String,
        val nextValidatorSetPayloadHash: String,
        val validatorContractAddress: ByteArray,
        val accountProofNodes: List<ByteArray>,
        val storageRoot: String,
        val validatorSetLengthSlot: String,
        val validatorSetLengthValue: ByteArray,
        val validatorSetLengthValueHash: String,
        val validatorSetLengthProofNodes: List<ByteArray>,
        val validatorStorageProofs: List<BscValidatorStorageProof>,
    )

    /** BSC Parlia commit-seal transcript material. */
    data class BscCommitSealProof(
        val version: Int = 1,
        val totalPower: String,
        val signedPower: String,
        val commitMessageHash: String,
        val validatorPublicKeys: List<ByteArray>,
        val validatorPowers: List<String>,
        val signersBitmap: ByteArray,
        val signatures: List<ByteArray>,
        val validatorSetHash: String? = null,
    )

    /** TON masterchain validator-signature transcript material. */
    data class TonValidatorSignatureProof(
        val version: Int = 1,
        val totalWeight: String,
        val signedWeight: String,
        val blockMessageHash: String,
        val validatorPublicKeys: List<ByteArray>,
        val validatorWeights: List<String>,
        val signersBitmap: ByteArray,
        val signatures: List<ByteArray>,
        val validatorSetHash: String? = null,
    )

    /** Canonical OpenVerify verifier-key commitment for an SCCP source-adapter lane. */
    @JvmStatic
    @JvmOverloads
    fun sourceAdapterVerifierVkHash(sourceDomain: Int, targetDomain: Int = DOMAIN_SORA): String {
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        val normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain")
        require(normalizedTargetDomain == DOMAIN_SORA) {
            "targetDomain must be SORA for SCCP source-adapter verifier VKs"
        }
        val (chain, proofPlan, finalityModel) = sourceAdapterVerifierProfile(normalizedSourceDomain)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeVector(out, SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, chain.toByteArray(StandardCharsets.UTF_8))
        writeU32Le(out, normalizedSourceDomain)
        writeU32Le(out, normalizedTargetDomain)
        out.write(proofPlan)
        out.write(finalityModel)
        writeVector(out, SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1.toByteArray(StandardCharsets.UTF_8))
        writeU32Le(out, 128)
        writeU32Le(out, 23)
        writeU32Le(out, 16)
        writeU64Le(out, BigInteger.valueOf(SOURCE_ADAPTER_FASTPQ_TRACE_ROOT_V1))
        writeU32Le(out, 19)
        writeU64Le(out, BigInteger.valueOf(SOURCE_ADAPTER_FASTPQ_LDE_ROOT_V1))
        writeU32Le(out, 65_536)
        out.write(1)
        writeU32Le(out, 19)
        writeU64Le(out, BigInteger.valueOf(SOURCE_ADAPTER_FASTPQ_OMEGA_COSET_V1))
        writeVector(out, "Goldilocks".toByteArray(StandardCharsets.UTF_8))
        writeVector(out, "18446744069414584321".toByteArray(StandardCharsets.UTF_8))
        writeU32Le(out, 2)
        writeVector(out, "Poseidon2(Goldilocks)".toByteArray(StandardCharsets.UTF_8))
        writeVector(out, "SHA3-256".toByteArray(StandardCharsets.UTF_8))
        writeU32Le(out, 8)
        writeU32Le(out, 8)
        writeU32Le(out, 8)
        writeU32Le(out, 46)
        val circuitId = SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.toByteArray(StandardCharsets.UTF_8)
        val preimage = ByteArray(circuitId.size + out.size())
        System.arraycopy(circuitId, 0, preimage, 0, circuitId.size)
        System.arraycopy(out.toByteArray(), 0, preimage, circuitId.size, out.size())
        return "0x" + hexLower(sha256(preimage))
    }

    /** Governed destination binding key for a native SCCP lane. */
    @JvmStatic
    fun destinationBindingKey(domain: Int): String {
        val targetDomain = normalizeDomain(domain, "targetDomain")
        return destinationBindingProfile(targetDomain).bindingKey
    }

    /** Canonical SccpDestinationBindingV1 hash for a native SCCP lane. */
    @JvmStatic
    fun destinationBindingHash(domain: Int): String {
        val targetDomain = normalizeDomain(domain, "targetDomain")
        val profile = destinationBindingProfile(targetDomain)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, DOMAIN_SORA)
        writeU32Le(out, targetDomain)
        out.write(1)
        out.write(1)
        out.write(profile.verifierTarget)
        out.write(profile.backendFamily)
        writeVector(out, profile.bindingKey.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, profile.manifestSeed.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, STARK_FRI_PROOF_FAMILY_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, profile.verifierBackend.toByteArray(StandardCharsets.UTF_8))
        return hashHex(DESTINATION_BINDING_PREFIX_V1, out.toByteArray())
    }

    /** Governed EVM-family destination binding for UI-side SCCP proof generation. */
    @JvmStatic
    @JvmOverloads
    fun evmDestinationBinding(
        sourceDomain: Int = DOMAIN_SORA,
        targetDomain: Int = DOMAIN_ETH,
        networkId: String,
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        verifierBackend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        proofFamily: String = STARK_FRI_PROOF_FAMILY_V1,
    ): EvmDestinationBinding {
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        val normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain")
        require(normalizedSourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        require(normalizedTargetDomain == DOMAIN_ETH || normalizedTargetDomain == DOMAIN_BSC) {
            "targetDomain must be ETH or BSC"
        }
        require(verifierBackend == SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1) {
            "verifierBackend must be evm-groth16-bn254-v1"
        }
        require(proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "proofFamily must be stark-fri-v1"
        }

        val networkIdBytes = nonZeroHex32Bytes(networkId, "networkId")
        val verifierAddressBytes = nonZeroHexBytes(verifierAddress, "verifierAddress", 20)
        val bridgeAddressBytes = nonZeroHexBytes(bridgeAddress, "bridgeAddress", 20)
        require(!verifierAddressBytes.contentEquals(bridgeAddressBytes)) {
            "bridgeAddress must differ from verifierAddress"
        }
        val verifierCodeHashBytes = nonZeroHex32Bytes(verifierCodeHash, "verifierCodeHash")
        val verifierKeyHashBytes = nonZeroHex32Bytes(verifierKeyHash, "verifierKeyHash")
        val normalizedNetworkId = "0x" + hexLower(networkIdBytes)
        val normalizedVerifierAddress = "0x" + hexLower(verifierAddressBytes)
        val normalizedBridgeAddress = "0x" + hexLower(bridgeAddressBytes)
        val normalizedVerifierCodeHash = "0x" + hexLower(verifierCodeHashBytes)
        val normalizedVerifierKeyHash = "0x" + hexLower(verifierKeyHashBytes)
        val key = listOf(
            "evm",
            normalizedSourceDomain.toString(),
            normalizedTargetDomain.toString(),
            hexLower(networkIdBytes),
            normalizedVerifierAddress,
            normalizedBridgeAddress,
            normalizedVerifierCodeHash,
            normalizedVerifierKeyHash,
        ).joinToString(":")

        val payload = ByteArrayOutputStream()
        payload.write(keccak256(EVM_DESTINATION_BINDING_LABEL_V1.toByteArray(StandardCharsets.UTF_8)))
        payload.write(keccak256(verifierBackend.toByteArray(StandardCharsets.UTF_8)))
        payload.write(keccak256(proofFamily.toByteArray(StandardCharsets.UTF_8)))
        payload.write(networkIdBytes)
        payload.write(abiWordU32(normalizedSourceDomain, "sourceDomain"))
        payload.write(abiWordU32(normalizedTargetDomain, "targetDomain"))
        payload.write(abiWordAddress20(verifierAddressBytes, "verifierAddress"))
        payload.write(abiWordAddress20(bridgeAddressBytes, "bridgeAddress"))
        payload.write(verifierCodeHashBytes)
        payload.write(verifierKeyHashBytes)
        val hash = "0x" + hexLower(keccak256(payload.toByteArray()))

        return EvmDestinationBinding(
            version = 1,
            sourceDomain = normalizedSourceDomain,
            targetDomain = normalizedTargetDomain,
            networkId = normalizedNetworkId,
            verifierAddress = normalizedVerifierAddress,
            bridgeAddress = normalizedBridgeAddress,
            verifierCodeHash = normalizedVerifierCodeHash,
            verifierKeyHash = normalizedVerifierKeyHash,
            verifierBackend = verifierBackend,
            proofFamily = proofFamily,
            key = key,
            hash = hash,
        )
    }

    /** Canonical governed EVM-family destination binding hash for UI-side proof requests. */
    @JvmStatic
    @JvmOverloads
    fun evmDestinationBindingHash(
        sourceDomain: Int = DOMAIN_SORA,
        targetDomain: Int = DOMAIN_ETH,
        networkId: String,
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        verifierBackend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        proofFamily: String = STARK_FRI_PROOF_FAMILY_V1,
    ): String = evmDestinationBinding(
        sourceDomain = sourceDomain,
        targetDomain = targetDomain,
        networkId = networkId,
        verifierAddress = verifierAddress,
        bridgeAddress = bridgeAddress,
        verifierCodeHash = verifierCodeHash,
        verifierKeyHash = verifierKeyHash,
        verifierBackend = verifierBackend,
        proofFamily = proofFamily,
    ).hash

    /** Governed TRON destination binding for UI-side SCCP proof generation. */
    @JvmStatic
    @JvmOverloads
    fun tronDestinationBinding(
        sourceDomain: Int = DOMAIN_SORA,
        targetDomain: Int = DOMAIN_TRON,
        networkId: String,
        verifierAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        verifierBackend: String = SccpTron.GROTH16_BN254_PROOF_BACKEND_V1,
        proofFamily: String = STARK_FRI_PROOF_FAMILY_V1,
    ): TronDestinationBinding {
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        val normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain")
        require(normalizedSourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        require(normalizedTargetDomain == DOMAIN_TRON) { "targetDomain must be TRON" }
        require(verifierBackend == SccpTron.GROTH16_BN254_PROOF_BACKEND_V1) {
            "verifierBackend must be tron-groth16-bn254-v1"
        }
        require(proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "proofFamily must be stark-fri-v1"
        }

        val networkIdBytes = nonZeroHex32Bytes(networkId, "networkId")
        val verifierAddressPayload = tronBase58CheckPayload(verifierAddress, "verifierAddress")
        val verifierCodeHashBytes = nonZeroHex32Bytes(verifierCodeHash, "verifierCodeHash")
        val verifierKeyHashBytes = nonZeroHex32Bytes(verifierKeyHash, "verifierKeyHash")
        val normalizedNetworkId = "0x" + hexLower(networkIdBytes)
        val normalizedVerifierAddress = verifierAddress
        val normalizedVerifierCodeHash = "0x" + hexLower(verifierCodeHashBytes)
        val normalizedVerifierKeyHash = "0x" + hexLower(verifierKeyHashBytes)
        val key = listOf(
            "tron",
            normalizedSourceDomain.toString(),
            normalizedTargetDomain.toString(),
            hexLower(networkIdBytes),
            normalizedVerifierAddress,
            normalizedVerifierCodeHash,
            normalizedVerifierKeyHash,
        ).joinToString(":")

        val payload = ByteArrayOutputStream()
        payload.write(keccak256(TRON_DESTINATION_BINDING_LABEL_V1.toByteArray(StandardCharsets.UTF_8)))
        payload.write(keccak256(verifierBackend.toByteArray(StandardCharsets.UTF_8)))
        payload.write(keccak256(proofFamily.toByteArray(StandardCharsets.UTF_8)))
        payload.write(networkIdBytes)
        payload.write(abiWordU32(normalizedSourceDomain, "sourceDomain"))
        payload.write(abiWordU32(normalizedTargetDomain, "targetDomain"))
        payload.write(abiWordBytes21(verifierAddressPayload, "verifierAddress"))
        payload.write(verifierCodeHashBytes)
        payload.write(verifierKeyHashBytes)
        val hash = "0x" + hexLower(keccak256(payload.toByteArray()))

        return TronDestinationBinding(
            version = 1,
            sourceDomain = normalizedSourceDomain,
            targetDomain = normalizedTargetDomain,
            networkId = normalizedNetworkId,
            verifierAddress = normalizedVerifierAddress,
            verifierCodeHash = normalizedVerifierCodeHash,
            verifierKeyHash = normalizedVerifierKeyHash,
            verifierBackend = verifierBackend,
            proofFamily = proofFamily,
            key = key,
            hash = hash,
        )
    }

    /** Canonical governed TRON destination binding hash for UI-side proof requests. */
    @JvmStatic
    @JvmOverloads
    fun tronDestinationBindingHash(
        sourceDomain: Int = DOMAIN_SORA,
        targetDomain: Int = DOMAIN_TRON,
        networkId: String,
        verifierAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        verifierBackend: String = SccpTron.GROTH16_BN254_PROOF_BACKEND_V1,
        proofFamily: String = STARK_FRI_PROOF_FAMILY_V1,
    ): String = tronDestinationBinding(
        sourceDomain = sourceDomain,
        targetDomain = targetDomain,
        networkId = networkId,
        verifierAddress = verifierAddress,
        verifierCodeHash = verifierCodeHash,
        verifierKeyHash = verifierKeyHash,
        verifierBackend = verifierBackend,
        proofFamily = proofFamily,
    ).hash

    /** Canonical governed source-verifier material record bytes. */
    @JvmStatic
    @JvmOverloads
    fun canonicalSourceVerifierMaterialBytes(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        sourceStateVerifierHash: String? = null,
        bridgeAddress: String? = null,
        sourceBridgeEmitterCodeHash: String? = null,
        networkId: String? = null,
        ownerAddress: String? = null,
        configHash: String? = null,
    ): ByteArray {
        val material = normalizeSourceMaterial(
            sourceDomain = sourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
        )
        val out = ByteArrayOutputStream()
        writeSourceMaterialFields(out, material)
        out.write(0)
        return out.toByteArray()
    }

    /** Canonical governed source-verifier material record hash. */
    @JvmStatic
    @JvmOverloads
    fun sourceVerifierMaterialHash(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        sourceStateVerifierHash: String? = null,
        bridgeAddress: String? = null,
        sourceBridgeEmitterCodeHash: String? = null,
        networkId: String? = null,
        ownerAddress: String? = null,
        configHash: String? = null,
    ): String =
        hashHex(
            "sccp:source-verifier-material-record:v1",
            canonicalSourceVerifierMaterialBytes(
                sourceDomain,
                sourceTrustAnchorHash,
                consensusVerifierHash,
                messageInclusionVerifierHash,
                finalityPolicyHash,
                sourceStateVerifierHash,
                bridgeAddress,
                sourceBridgeEmitterCodeHash,
                networkId,
                ownerAddress,
                configHash,
            ),
        )

    /** Canonical governed source-adapter deployment record bytes. */
    @JvmStatic
    @JvmOverloads
    fun canonicalSourceAdapterEngineDeploymentBytes(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        deploymentReceiptHash: String,
        targetDomain: Int = DOMAIN_SORA,
        adapterVerifierVkHash: String? = null,
        sourceStateVerifierHash: String? = null,
        bridgeAddress: String? = null,
        sourceBridgeEmitterCodeHash: String? = null,
        networkId: String? = null,
        ownerAddress: String? = null,
        configHash: String? = null,
        solanaTowerReplayVerifierHash: String? = null,
        solanaFullAccountsdbLatticeVerifierHash: String? = null,
        solanaBankForkChoiceVerifierHash: String? = null,
        tonMasterchainConfigVerifierHash: String? = null,
        tonValidatorSetTransitionVerifierHash: String? = null,
        tonShardAccountsDictionaryVerifierHash: String? = null,
    ): ByteArray {
        val normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain")
        require(normalizedTargetDomain == DOMAIN_SORA) {
            "targetDomain must be SORA for SCCP source-adapter deployments"
        }
        val material = normalizeSourceMaterial(
            sourceDomain = sourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
        )
        val canonicalVkHash = sourceAdapterVerifierVkHash(material.sourceDomain, normalizedTargetDomain)
        val normalizedVkHash = adapterVerifierVkHash?.let { normalizeHex32(it) } ?: canonicalVkHash
        require(normalizedVkHash == canonicalVkHash) {
            "adapterVerifierVkHash must match the canonical source-adapter verifier profile"
        }
        val adapterVerifierVkHashBytes = hex32Bytes(normalizedVkHash, "adapterVerifierVkHash")
        val deploymentReceiptHashBytes = nonZeroHex32Bytes(deploymentReceiptHash, "deploymentReceiptHash")
        requirePairwiseNonzeroRoleHashSeparation(
            listOf(
                "sourceTrustAnchorHash" to material.sourceTrustAnchorHash,
                "consensusVerifierHash" to material.consensusVerifierHash,
                "messageInclusionVerifierHash" to material.messageInclusionVerifierHash,
                "finalityPolicyHash" to material.finalityPolicyHash,
                "sourceStateVerifierHash" to material.sourceStateVerifierHash,
                "adapterVerifierVkHash" to adapterVerifierVkHashBytes,
                "sourceBridgeEmitterCodeHash" to material.sourceBridgeEmitterCodeHash,
                "sourceBridgeNetworkId" to material.sourceBridgeNetworkId,
                "sourceBridgeConfigHash" to material.sourceBridgeConfigHash,
                "deploymentReceiptHash" to deploymentReceiptHashBytes,
            ),
            "SCCP source-adapter deployment",
        )
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, material.sourceDomain)
        writeU32Le(out, normalizedTargetDomain)
        writeVector(out, material.profile.chain.toByteArray(StandardCharsets.UTF_8))
        out.write(material.profile.proofPlan)
        out.write(material.profile.finalityModel)
        writeVector(out, STARK_FRI_PROOF_FAMILY_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.toByteArray(StandardCharsets.UTF_8))
        out.write(adapterVerifierVkHashBytes)
        writeSourceComponentFields(out, material)
        out.write(deploymentReceiptHashBytes)
        writeSourceAdapterDeploymentSolanaAuditFields(
            out = out,
            sourceDomain = material.sourceDomain,
            towerReplayVerifierHash = solanaTowerReplayVerifierHash,
            fullAccountsdbLatticeVerifierHash = solanaFullAccountsdbLatticeVerifierHash,
            bankForkChoiceVerifierHash = solanaBankForkChoiceVerifierHash,
            existingRoleHashes = listOf(
                material.sourceTrustAnchorHash,
                material.consensusVerifierHash,
                material.messageInclusionVerifierHash,
                material.finalityPolicyHash,
                material.sourceStateVerifierHash,
                adapterVerifierVkHashBytes,
                material.sourceBridgeEmitterCodeHash,
                material.sourceBridgeNetworkId,
                material.sourceBridgeConfigHash,
                deploymentReceiptHashBytes,
            ),
        )
        writeSourceAdapterDeploymentTonAuditFields(
            out = out,
            sourceDomain = material.sourceDomain,
            masterchainConfigVerifierHash = tonMasterchainConfigVerifierHash,
            validatorSetTransitionVerifierHash = tonValidatorSetTransitionVerifierHash,
            shardAccountsDictionaryVerifierHash = tonShardAccountsDictionaryVerifierHash,
            existingRoleHashes = listOf(
                material.sourceTrustAnchorHash,
                material.consensusVerifierHash,
                material.messageInclusionVerifierHash,
                material.finalityPolicyHash,
                material.sourceStateVerifierHash,
                adapterVerifierVkHashBytes,
                material.sourceBridgeEmitterCodeHash,
                material.sourceBridgeNetworkId,
                material.sourceBridgeConfigHash,
                deploymentReceiptHashBytes,
            ),
        )
        return out.toByteArray()
    }

    /** Canonical governed source-adapter deployment record hash. */
    @JvmStatic
    @JvmOverloads
    fun sourceAdapterEngineDeploymentHash(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        deploymentReceiptHash: String,
        targetDomain: Int = DOMAIN_SORA,
        adapterVerifierVkHash: String? = null,
        sourceStateVerifierHash: String? = null,
        bridgeAddress: String? = null,
        sourceBridgeEmitterCodeHash: String? = null,
        networkId: String? = null,
        ownerAddress: String? = null,
        configHash: String? = null,
        solanaTowerReplayVerifierHash: String? = null,
        solanaFullAccountsdbLatticeVerifierHash: String? = null,
        solanaBankForkChoiceVerifierHash: String? = null,
        tonMasterchainConfigVerifierHash: String? = null,
        tonValidatorSetTransitionVerifierHash: String? = null,
        tonShardAccountsDictionaryVerifierHash: String? = null,
    ): String =
        hashHex(
            "sccp:source-adapter-engine-deployment:v1",
            canonicalSourceAdapterEngineDeploymentBytes(
                sourceDomain,
                sourceTrustAnchorHash,
                consensusVerifierHash,
                messageInclusionVerifierHash,
                finalityPolicyHash,
                deploymentReceiptHash,
                targetDomain,
                adapterVerifierVkHash,
                sourceStateVerifierHash,
                bridgeAddress,
                sourceBridgeEmitterCodeHash,
                networkId,
                ownerAddress,
                configHash,
                solanaTowerReplayVerifierHash,
                solanaFullAccountsdbLatticeVerifierHash,
                solanaBankForkChoiceVerifierHash,
                tonMasterchainConfigVerifierHash,
                tonValidatorSetTransitionVerifierHash,
                tonShardAccountsDictionaryVerifierHash,
            ),
        )

    /** Canonical Solana full light-client deployment-gate hash for audited verifier bundles. */
    @JvmStatic
    @JvmOverloads
    fun solanaFullLightClientGateHash(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        deploymentReceiptHash: String,
        solanaTowerReplayVerifierHash: String,
        solanaFullAccountsdbLatticeVerifierHash: String,
        solanaBankForkChoiceVerifierHash: String,
        targetDomain: Int = DOMAIN_SORA,
        adapterVerifierVkHash: String? = null,
        sourceStateVerifierHash: String? = null,
        bridgeAddress: String? = null,
        sourceBridgeEmitterCodeHash: String? = null,
        networkId: String? = null,
        ownerAddress: String? = null,
        configHash: String? = null,
    ): String {
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        val normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain")
        require(normalizedSourceDomain == DOMAIN_SOL && normalizedTargetDomain == DOMAIN_SORA) {
            "Solana full light-client gate hash requires an audited Solana -> SORA deployment"
        }
        val material = normalizeSourceMaterial(
            sourceDomain = normalizedSourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
        )
        val verifierHashes = listOf(
            SOLANA_MAINNET_TOWER_REPLAY_VERIFIER_ID_V1 to
                nonZeroHex32Bytes(solanaTowerReplayVerifierHash, "solanaTowerReplayVerifierHash"),
            SOLANA_MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1 to
                nonZeroHex32Bytes(
                    solanaFullAccountsdbLatticeVerifierHash,
                    "solanaFullAccountsdbLatticeVerifierHash",
                ),
            SOLANA_MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1 to
                nonZeroHex32Bytes(solanaBankForkChoiceVerifierHash, "solanaBankForkChoiceVerifierHash"),
        )
        val materialHash = sourceVerifierMaterialHash(
            sourceDomain = normalizedSourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
        )
        val deploymentHash = sourceAdapterEngineDeploymentHash(
            sourceDomain = normalizedSourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            targetDomain = normalizedTargetDomain,
            adapterVerifierVkHash = adapterVerifierVkHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
            solanaTowerReplayVerifierHash = solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash = solanaBankForkChoiceVerifierHash,
        )
        val adapterVerifierVkHashBytes = hex32Bytes(
            adapterVerifierVkHash?.let { normalizeHex32(it) }
                ?: sourceAdapterVerifierVkHash(normalizedSourceDomain, normalizedTargetDomain),
            "adapterVerifierVkHash",
        )
        val deploymentReceiptHashBytes = nonZeroHex32Bytes(deploymentReceiptHash, "deploymentReceiptHash")
        requireSolanaFullLightClientAuditRoleSeparation(
            verifierHashes,
            listOf(
                material.sourceTrustAnchorHash,
                material.consensusVerifierHash,
                material.messageInclusionVerifierHash,
                material.finalityPolicyHash,
                material.sourceStateVerifierHash,
                adapterVerifierVkHashBytes,
                material.sourceBridgeEmitterCodeHash,
                material.sourceBridgeNetworkId,
                material.sourceBridgeConfigHash,
                deploymentReceiptHashBytes,
            ),
        )
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, material.sourceDomain)
        writeU32Le(out, normalizedTargetDomain)
        writeVector(out, material.profile.chain.toByteArray(StandardCharsets.UTF_8))
        out.write(material.profile.proofPlan)
        out.write(material.profile.finalityModel)
        writeVector(out, SccpSolana.MAINNET_GENESIS_HASH.toByteArray(StandardCharsets.UTF_8))
        out.write(hex32Bytes(materialHash, "sourceVerifierMaterialHash"))
        out.write(hex32Bytes(deploymentHash, "sourceAdapterDeploymentHash"))
        verifierHashes.forEach { (verifierId, verifierHash) ->
            writeVector(out, verifierId.toByteArray(StandardCharsets.UTF_8))
            out.write(verifierHash)
        }
        return hashHex("sccp:solana:full-light-client-gate:v1", out.toByteArray())
    }

    /** Canonical TON full light-client deployment-gate hash for audited verifier bundles. */
    @JvmStatic
    @JvmOverloads
    fun tonFullLightClientGateHash(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        deploymentReceiptHash: String,
        tonMasterchainConfigVerifierHash: String,
        tonValidatorSetTransitionVerifierHash: String,
        tonShardAccountsDictionaryVerifierHash: String,
        targetDomain: Int = DOMAIN_SORA,
        adapterVerifierVkHash: String? = null,
        sourceStateVerifierHash: String? = null,
        bridgeAddress: String? = null,
        sourceBridgeEmitterCodeHash: String? = null,
        networkId: String? = null,
        ownerAddress: String? = null,
        configHash: String? = null,
    ): String {
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        val normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain")
        require(normalizedSourceDomain == DOMAIN_TON && normalizedTargetDomain == DOMAIN_SORA) {
            "TON full light-client gate hash requires an audited TON -> SORA deployment"
        }
        val material = normalizeSourceMaterial(
            sourceDomain = normalizedSourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
        )
        val verifierHashes = listOf(
            TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1 to
                nonZeroHex32Bytes(tonMasterchainConfigVerifierHash, "tonMasterchainConfigVerifierHash"),
            TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1 to
                nonZeroHex32Bytes(
                    tonValidatorSetTransitionVerifierHash,
                    "tonValidatorSetTransitionVerifierHash",
                ),
            TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1 to
                nonZeroHex32Bytes(
                    tonShardAccountsDictionaryVerifierHash,
                    "tonShardAccountsDictionaryVerifierHash",
                ),
        )
        requireTonFullLightClientAuditRoleSeparation(
            verifierHashes,
            listOf(
                material.sourceTrustAnchorHash,
                material.consensusVerifierHash,
                material.messageInclusionVerifierHash,
                material.finalityPolicyHash,
                material.sourceStateVerifierHash,
                hex32Bytes(
                    adapterVerifierVkHash
                        ?: sourceAdapterVerifierVkHash(normalizedSourceDomain, normalizedTargetDomain),
                    "adapterVerifierVkHash",
                ),
                material.sourceBridgeEmitterCodeHash,
                material.sourceBridgeNetworkId,
                material.sourceBridgeConfigHash,
                nonZeroHex32Bytes(deploymentReceiptHash, "deploymentReceiptHash"),
            ),
        )
        val materialHash = sourceVerifierMaterialHash(
            sourceDomain = normalizedSourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
        )
        val deploymentHash = sourceAdapterEngineDeploymentHash(
            sourceDomain = normalizedSourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            deploymentReceiptHash = deploymentReceiptHash,
            targetDomain = normalizedTargetDomain,
            adapterVerifierVkHash = adapterVerifierVkHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = bridgeAddress,
            sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash,
            networkId = networkId,
            ownerAddress = ownerAddress,
            configHash = configHash,
            tonMasterchainConfigVerifierHash = tonMasterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash = tonValidatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash = tonShardAccountsDictionaryVerifierHash,
        )
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, material.sourceDomain)
        writeU32Le(out, normalizedTargetDomain)
        writeVector(out, material.profile.chain.toByteArray(StandardCharsets.UTF_8))
        out.write(material.profile.proofPlan)
        out.write(material.profile.finalityModel)
        writeI32Le(out, -239)
        writeVector(out, SccpTon.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, material.profile.sourceStateVerifierId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.sourceStateVerifierHash)
        out.write(hex32Bytes(materialHash, "sourceVerifierMaterialHash"))
        out.write(hex32Bytes(deploymentHash, "sourceAdapterDeploymentHash"))
        verifierHashes.forEach { (verifierId, verifierHash) ->
            writeVector(out, verifierId.toByteArray(StandardCharsets.UTF_8))
            out.write(verifierHash)
        }
        return hashHex("sccp:ton:full-light-client-gate:v1", out.toByteArray())
    }

    @JvmStatic
    @JvmOverloads
    fun canonicalEvmReceiptProofBytes(
        sourceEventDigest: String,
        beaconSlot: String,
        executionBlockNumber: String,
        executionBlockHash: String,
        executionReceiptsRoot: String,
        beaconFinalizedRoot: String,
        syncCommitteeRoot: String,
        receiptRootIndex: String,
        receiptTrieProofNodes: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        sourceDomain: Int = DOMAIN_ETH,
    ): ByteArray {
        validateTronMptProofNodes(receiptTrieProofNodes)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        out.write(hex32Bytes(sourceEventDigest, "sourceEventDigest"))
        writeU64Le(out, normalizeU64(beaconSlot, "beaconSlot"))
        writeU64Le(out, normalizeU64(executionBlockNumber, "executionBlockNumber"))
        out.write(hex32Bytes(executionBlockHash, "executionBlockHash"))
        out.write(hex32Bytes(executionReceiptsRoot, "executionReceiptsRoot"))
        out.write(hex32Bytes(beaconFinalizedRoot, "beaconFinalizedRoot"))
        out.write(hex32Bytes(syncCommitteeRoot, "syncCommitteeRoot"))
        writeU64Le(out, normalizeU64(receiptRootIndex, "receiptRootIndex"))
        writeU32Le(out, receiptTrieProofNodes.size)
        receiptTrieProofNodes.forEach { node -> writeVector(out, node) }
        writeBranch(out, inclusionBranch)
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun evmReceiptProofHash(
        sourceEventDigest: String,
        beaconSlot: String,
        executionBlockNumber: String,
        executionBlockHash: String,
        executionReceiptsRoot: String,
        beaconFinalizedRoot: String,
        syncCommitteeRoot: String,
        receiptRootIndex: String,
        receiptTrieProofNodes: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        sourceDomain: Int = DOMAIN_ETH,
    ): String =
        hashHex(
            "sccp:evm:receipt-proof:v1",
            canonicalEvmReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                beaconSlot = beaconSlot,
                executionBlockNumber = executionBlockNumber,
                executionBlockHash = executionBlockHash,
                executionReceiptsRoot = executionReceiptsRoot,
                beaconFinalizedRoot = beaconFinalizedRoot,
                syncCommitteeRoot = syncCommitteeRoot,
                receiptRootIndex = receiptRootIndex,
                receiptTrieProofNodes = receiptTrieProofNodes,
                inclusionBranch = inclusionBranch,
                sourceDomain = sourceDomain,
            ),
        )

    @JvmStatic
    fun canonicalEthSyncCommitteePayloadBytes(
        syncCommitteePublicKeys: List<ByteArray>,
        syncCommitteeWeights: List<String>,
        syncCommitteePops: List<ByteArray>,
    ): ByteArray {
        require(
            syncCommitteePublicKeys.isNotEmpty() &&
                syncCommitteePublicKeys.size == syncCommitteeWeights.size &&
                syncCommitteePublicKeys.size == syncCommitteePops.size,
        ) {
            "syncCommitteePublicKeys, syncCommitteeWeights, and syncCommitteePops must be non-empty equal-length arrays"
        }
        require(syncCommitteePublicKeys.size <= ETH_MAX_SYNC_COMMITTEE_AUTHORITIES) {
            "syncCommitteePublicKeys must contain at most $ETH_MAX_SYNC_COMMITTEE_AUTHORITIES entries"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, syncCommitteePublicKeys.size)
        val seenPublicKeys = HashSet<String>()
        for (index in syncCommitteePublicKeys.indices) {
            val publicKey = syncCommitteePublicKeys[index]
            require(publicKey.size == ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES) {
                "syncCommitteePublicKeys[$index] must be $ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES bytes"
            }
            require(!isZero(publicKey)) {
                "syncCommitteePublicKeys[$index] must not be zero"
            }
            require(seenPublicKeys.add(hexLower(publicKey))) { "syncCommitteePublicKeys[$index] must be unique" }
            val weight = normalizeU64(syncCommitteeWeights[index], "syncCommitteeWeights[$index]")
            require(weight != BigInteger.ZERO) { "syncCommitteeWeights[$index] must not be zero" }
            val pop = syncCommitteePops[index]
            require(pop.size == ETH_SYNC_COMMITTEE_POP_BYTES) {
                "syncCommitteePops[$index] must be $ETH_SYNC_COMMITTEE_POP_BYTES bytes"
            }
            require(!isZero(pop)) {
                "syncCommitteePops[$index] must not be zero"
            }
            writeVector(out, publicKey)
            writeU64Le(out, weight)
            writeVector(out, pop)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun ethSyncCommitteeHashFromPayload(payload: ByteArray): String {
        validateEthSyncCommitteePayload(payload)
        return hashHex("sccp:eth:sync-committee:v1", payload)
    }

    @JvmStatic
    fun ethSyncCommitteeHash(
        syncCommitteePublicKeys: List<ByteArray>,
        syncCommitteeWeights: List<String>,
        syncCommitteePops: List<ByteArray>,
    ): String =
        ethSyncCommitteeHashFromPayload(
            canonicalEthSyncCommitteePayloadBytes(
                syncCommitteePublicKeys,
                syncCommitteeWeights,
                syncCommitteePops,
            ),
        )

    @JvmStatic
    fun ethSyncCommitteePayloadHash(payload: ByteArray): String {
        validateEthSyncCommitteePayload(payload)
        return hashHex("sccp:eth:sync-committee-payload:v1", payload)
    }

    @JvmStatic
    fun ethSyncCommitteePayloadHash(
        syncCommitteePublicKeys: List<ByteArray>,
        syncCommitteeWeights: List<String>,
        syncCommitteePops: List<ByteArray>,
    ): String =
        ethSyncCommitteePayloadHash(
            canonicalEthSyncCommitteePayloadBytes(
                syncCommitteePublicKeys,
                syncCommitteeWeights,
                syncCommitteePops,
            ),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalEthSyncCommitteeTransitionMessageBytes(
        fromSyncPeriod: String,
        toSyncPeriod: String,
        transitionSlot: String,
        finalizedBeaconRoot: String,
        parentSyncCommitteeHash: String,
        nextSyncCommitteeHash: String,
        nextSyncCommitteePayloadHash: String,
        nextSyncCommitteeBranchHash: String,
        sourceDomain: Int = DOMAIN_ETH,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        writeU64Le(out, normalizeU64(fromSyncPeriod, "fromSyncPeriod"))
        writeU64Le(out, normalizeU64(toSyncPeriod, "toSyncPeriod"))
        writeU64Le(out, normalizeU64(transitionSlot, "transitionSlot"))
        out.write(hex32Bytes(finalizedBeaconRoot, "finalizedBeaconRoot"))
        out.write(hex32Bytes(parentSyncCommitteeHash, "parentSyncCommitteeHash"))
        out.write(hex32Bytes(nextSyncCommitteeHash, "nextSyncCommitteeHash"))
        out.write(hex32Bytes(nextSyncCommitteePayloadHash, "nextSyncCommitteePayloadHash"))
        out.write(hex32Bytes(nextSyncCommitteeBranchHash, "nextSyncCommitteeBranchHash"))
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun ethSyncCommitteeTransitionMessageHash(
        fromSyncPeriod: String,
        toSyncPeriod: String,
        transitionSlot: String,
        finalizedBeaconRoot: String,
        parentSyncCommitteeHash: String,
        nextSyncCommitteeHash: String,
        nextSyncCommitteePayloadHash: String,
        nextSyncCommitteeBranchHash: String,
        sourceDomain: Int = DOMAIN_ETH,
    ): String =
        hashHex(
            "sccp:eth:sync-committee-transition-message:v1",
            canonicalEthSyncCommitteeTransitionMessageBytes(
                fromSyncPeriod = fromSyncPeriod,
                toSyncPeriod = toSyncPeriod,
                transitionSlot = transitionSlot,
                finalizedBeaconRoot = finalizedBeaconRoot,
                parentSyncCommitteeHash = parentSyncCommitteeHash,
                nextSyncCommitteeHash = nextSyncCommitteeHash,
                nextSyncCommitteePayloadHash = nextSyncCommitteePayloadHash,
                nextSyncCommitteeBranchHash = nextSyncCommitteeBranchHash,
                sourceDomain = sourceDomain,
            ),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalEthBeaconSyncCommitteeProofBytes(
        totalWeight: String,
        signedWeight: String,
        syncCommitteeMessageHash: String,
        syncCommitteePublicKeys: List<ByteArray>,
        syncCommitteeWeights: List<String>,
        syncCommitteePops: List<ByteArray>,
        signersBitmap: ByteArray,
        aggregateSignature: ByteArray,
        version: Int = 1,
    ): ByteArray {
        requireV1Version(version, "syncCommitteeProof.version")
        canonicalEthSyncCommitteePayloadBytes(syncCommitteePublicKeys, syncCommitteeWeights, syncCommitteePops)
        val normalizedWeights =
            syncCommitteeWeights.mapIndexed { index, weight ->
                normalizeU64(weight, "syncCommitteeWeights[$index]")
            }
        require(signersBitmap.size == (syncCommitteePublicKeys.size + 7) / 8) {
            "signersBitmap length must match syncCommitteePublicKeys"
        }
        val signerIndices = ethSyncCommitteeSignerIndices(signersBitmap, syncCommitteePublicKeys.size)
        require(aggregateSignature.size == ETH_SYNC_COMMITTEE_SIGNATURE_BYTES) {
            "aggregateSignature must be $ETH_SYNC_COMMITTEE_SIGNATURE_BYTES bytes"
        }
        require(!isZero(aggregateSignature)) {
            "aggregateSignature must not be all zero"
        }
        val totalWeightValue = normalizeU64(totalWeight, "totalWeight")
        val signedWeightValue = normalizeU64(signedWeight, "signedWeight")
        val computedTotalWeight = normalizedWeights.fold(BigInteger.ZERO) { sum, weight -> sum + weight }
        require(totalWeightValue == computedTotalWeight) {
            "totalWeight must match syncCommitteeWeights"
        }
        val computedSignedWeight =
            signerIndices.fold(BigInteger.ZERO) { sum, index -> sum + normalizedWeights[index] }
        require(signedWeightValue == computedSignedWeight) {
            "signedWeight must match signersBitmap"
        }
        require(signedWeightValue * BigInteger.valueOf(3) > totalWeightValue * BigInteger.valueOf(2)) {
            "signedWeight must be greater than two thirds of totalWeight"
        }
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU64Le(out, totalWeightValue)
        writeU64Le(out, signedWeightValue)
        out.write(hex32Bytes(syncCommitteeMessageHash, "syncCommitteeMessageHash"))
        writeU32Le(out, syncCommitteePublicKeys.size)
        syncCommitteePublicKeys.forEach { writeVector(out, it) }
        writeU32Le(out, syncCommitteeWeights.size)
        normalizedWeights.forEach { writeU64Le(out, it) }
        writeU32Le(out, syncCommitteePops.size)
        syncCommitteePops.forEach { writeVector(out, it) }
        writeVector(out, signersBitmap)
        writeVector(out, aggregateSignature)
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun canonicalEthSyncCommitteeTransitionSignatureBytes(
        fromSyncPeriod: String,
        toSyncPeriod: String,
        transitionSlot: String,
        finalizedBeaconRoot: String,
        parentSyncCommitteeHash: String,
        nextSyncCommitteeHash: String,
        nextSyncCommitteePayload: ByteArray,
        nextSyncCommitteePayloadHash: String,
        nextSyncCommitteeBranchHash: String,
        transitionMessageHash: String,
        totalWeight: String,
        signedWeight: String,
        syncCommitteePublicKeys: List<ByteArray>,
        syncCommitteeWeights: List<String>,
        syncCommitteePops: List<ByteArray>,
        signersBitmap: ByteArray,
        aggregateSignature: ByteArray,
        sourceDomain: Int = DOMAIN_ETH,
        version: Int = 1,
        proofVersion: Int = 1,
    ): ByteArray {
        requireV1Version(version, "ETH sync-committee transition signature version")
        require(ethSyncCommitteePayloadHash(nextSyncCommitteePayload) == normalizeHex32(nextSyncCommitteePayloadHash)) {
            "nextSyncCommitteePayloadHash must match nextSyncCommitteePayload"
        }
        require(ethSyncCommitteeHashFromPayload(nextSyncCommitteePayload) == normalizeHex32(nextSyncCommitteeHash)) {
            "nextSyncCommitteeHash must match nextSyncCommitteePayload"
        }
        val parentHash =
            ethSyncCommitteeHash(syncCommitteePublicKeys, syncCommitteeWeights, syncCommitteePops)
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        writeU64Le(out, normalizeU64(fromSyncPeriod, "fromSyncPeriod"))
        writeU64Le(out, normalizeU64(toSyncPeriod, "toSyncPeriod"))
        writeU64Le(out, normalizeU64(transitionSlot, "transitionSlot"))
        out.write(hex32Bytes(finalizedBeaconRoot, "finalizedBeaconRoot"))
        out.write(hex32Bytes(parentSyncCommitteeHash, "parentSyncCommitteeHash"))
        out.write(hex32Bytes(nextSyncCommitteeHash, "nextSyncCommitteeHash"))
        writeVector(out, nextSyncCommitteePayload)
        out.write(hex32Bytes(nextSyncCommitteePayloadHash, "nextSyncCommitteePayloadHash"))
        out.write(hex32Bytes(nextSyncCommitteeBranchHash, "nextSyncCommitteeBranchHash"))
        out.write(hex32Bytes(transitionMessageHash, "transitionMessageHash"))
        out.write(hex32Bytes(parentHash, "parentSyncCommitteeHash"))
        out.write(
            canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = totalWeight,
                signedWeight = signedWeight,
                syncCommitteeMessageHash = transitionMessageHash,
                syncCommitteePublicKeys = syncCommitteePublicKeys,
                syncCommitteeWeights = syncCommitteeWeights,
                syncCommitteePops = syncCommitteePops,
                signersBitmap = signersBitmap,
                aggregateSignature = aggregateSignature,
                version = proofVersion,
            ),
        )
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun ethSyncCommitteeTransitionSignatureHash(
        fromSyncPeriod: String,
        toSyncPeriod: String,
        transitionSlot: String,
        finalizedBeaconRoot: String,
        parentSyncCommitteeHash: String,
        nextSyncCommitteeHash: String,
        nextSyncCommitteePayload: ByteArray,
        nextSyncCommitteePayloadHash: String,
        nextSyncCommitteeBranchHash: String,
        transitionMessageHash: String,
        totalWeight: String,
        signedWeight: String,
        syncCommitteePublicKeys: List<ByteArray>,
        syncCommitteeWeights: List<String>,
        syncCommitteePops: List<ByteArray>,
        signersBitmap: ByteArray,
        aggregateSignature: ByteArray,
        sourceDomain: Int = DOMAIN_ETH,
        version: Int = 1,
        proofVersion: Int = 1,
    ): String =
        hashHex(
            "sccp:eth:sync-committee-transition-signature:v1",
            canonicalEthSyncCommitteeTransitionSignatureBytes(
                fromSyncPeriod = fromSyncPeriod,
                toSyncPeriod = toSyncPeriod,
                transitionSlot = transitionSlot,
                finalizedBeaconRoot = finalizedBeaconRoot,
                parentSyncCommitteeHash = parentSyncCommitteeHash,
                nextSyncCommitteeHash = nextSyncCommitteeHash,
                nextSyncCommitteePayload = nextSyncCommitteePayload,
                nextSyncCommitteePayloadHash = nextSyncCommitteePayloadHash,
                nextSyncCommitteeBranchHash = nextSyncCommitteeBranchHash,
                transitionMessageHash = transitionMessageHash,
                totalWeight = totalWeight,
                signedWeight = signedWeight,
                syncCommitteePublicKeys = syncCommitteePublicKeys,
                syncCommitteeWeights = syncCommitteeWeights,
                syncCommitteePops = syncCommitteePops,
                signersBitmap = signersBitmap,
                aggregateSignature = aggregateSignature,
                sourceDomain = sourceDomain,
                version = version,
                proofVersion = proofVersion,
            ),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalBscReceiptProofBytes(
        sourceEventDigest: String,
        validatorEpoch: String,
        blockNumber: String,
        blockHash: String,
        receiptsRoot: String,
        validatorSetHash: String,
        commitSealHash: String,
        receiptRootIndex: String,
        receiptTrieProofNodes: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        sourceDomain: Int = DOMAIN_BSC,
    ): ByteArray {
        validateTronMptProofNodes(receiptTrieProofNodes)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        out.write(hex32Bytes(sourceEventDigest, "sourceEventDigest"))
        writeU64Le(out, normalizeU64(validatorEpoch, "validatorEpoch"))
        writeU64Le(out, normalizeU64(blockNumber, "blockNumber"))
        out.write(hex32Bytes(blockHash, "blockHash"))
        out.write(hex32Bytes(receiptsRoot, "receiptsRoot"))
        out.write(hex32Bytes(validatorSetHash, "validatorSetHash"))
        out.write(hex32Bytes(commitSealHash, "commitSealHash"))
        writeU64Le(out, normalizeU64(receiptRootIndex, "receiptRootIndex"))
        writeU32Le(out, receiptTrieProofNodes.size)
        receiptTrieProofNodes.forEach { node -> writeVector(out, node) }
        writeBranch(out, inclusionBranch)
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun bscReceiptProofHash(
        sourceEventDigest: String,
        validatorEpoch: String,
        blockNumber: String,
        blockHash: String,
        receiptsRoot: String,
        validatorSetHash: String,
        commitSealHash: String,
        receiptRootIndex: String,
        receiptTrieProofNodes: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        sourceDomain: Int = DOMAIN_BSC,
    ): String =
        hashHex(
            "sccp:bsc:receipt-proof:v1",
            canonicalBscReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                validatorEpoch = validatorEpoch,
                blockNumber = blockNumber,
                blockHash = blockHash,
                receiptsRoot = receiptsRoot,
                validatorSetHash = validatorSetHash,
                commitSealHash = commitSealHash,
                receiptRootIndex = receiptRootIndex,
                receiptTrieProofNodes = receiptTrieProofNodes,
                inclusionBranch = inclusionBranch,
                sourceDomain = sourceDomain,
            ),
        )

    @JvmStatic
    fun canonicalBscValidatorSetPayloadBytes(
        validatorAddresses: List<String>,
        validatorPowers: List<String>,
    ): ByteArray {
        require(validatorAddresses.isNotEmpty() && validatorAddresses.size == validatorPowers.size) {
            "validatorAddresses and validatorPowers must be non-empty equal-length arrays"
        }
        require(validatorAddresses.size <= BSC_MAX_PARLIA_VALIDATORS) {
            "validatorAddresses must contain at most $BSC_MAX_PARLIA_VALIDATORS entries"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, validatorAddresses.size)
        val seenAddresses = HashSet<String>()
        validatorAddresses.zip(validatorPowers).forEachIndexed { index, (addressValue, powerValue) ->
            val address = hexBytes(addressValue, "validatorAddresses[$index]", 20)
            require(address.any { it.toInt() != 0 }) { "validatorAddresses[$index] must not be zero" }
            val addressHex = hexLower(address)
            require(seenAddresses.add(addressHex)) { "validatorAddresses[$index] must be unique" }
            val power = normalizeU64(powerValue, "validatorPowers[$index]")
            require(power != BigInteger.ZERO) { "validatorPowers[$index] must not be zero" }
            out.write(address)
            writeU64Le(out, power)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun bscValidatorSetPayloadHash(payload: ByteArray): String =
        keccakHashHex("sccp:bsc:validator-set-payload:v1", payload)

    @JvmStatic
    fun bscValidatorSetPayloadHash(
        validatorAddresses: List<String>,
        validatorPowers: List<String>,
    ): String =
        bscValidatorSetPayloadHash(canonicalBscValidatorSetPayloadBytes(validatorAddresses, validatorPowers))

    @JvmStatic
    fun bscValidatorSetHashFromPayload(payload: ByteArray): String {
        validateBscValidatorSetPayload(payload)
        return keccakHashHex("sccp:bsc:validator-set:v1", payload)
    }

    @JvmStatic
    fun bscValidatorSetHashFromPayload(
        validatorAddresses: List<String>,
        validatorPowers: List<String>,
    ): String =
        bscValidatorSetHashFromPayload(canonicalBscValidatorSetPayloadBytes(validatorAddresses, validatorPowers))

    @JvmStatic
    @JvmOverloads
    fun canonicalBscCommitMessageBytes(
        validatorEpoch: String,
        blockNumber: String,
        blockHash: String,
        receiptsRoot: String,
        validatorSetHash: String,
        sourceDomain: Int = DOMAIN_BSC,
    ): ByteArray {
        require(sourceDomain == DOMAIN_BSC) { "sourceDomain must be BSC" }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        writeU64Le(out, normalizeU64(validatorEpoch, "validatorEpoch"))
        writeU64Le(out, normalizeU64(blockNumber, "blockNumber"))
        out.write(hex32Bytes(blockHash, "blockHash"))
        out.write(hex32Bytes(receiptsRoot, "receiptsRoot"))
        out.write(hex32Bytes(validatorSetHash, "validatorSetHash"))
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun bscCommitMessageHash(
        validatorEpoch: String,
        blockNumber: String,
        blockHash: String,
        receiptsRoot: String,
        validatorSetHash: String,
        sourceDomain: Int = DOMAIN_BSC,
    ): String =
        keccakHashHex(
            "sccp:bsc:commit-message:v1",
            canonicalBscCommitMessageBytes(
                validatorEpoch = validatorEpoch,
                blockNumber = blockNumber,
                blockHash = blockHash,
                receiptsRoot = receiptsRoot,
                validatorSetHash = validatorSetHash,
                sourceDomain = sourceDomain,
            ),
        )

    @JvmStatic
    fun canonicalBscCommitSealBytes(proof: BscCommitSealProof): ByteArray {
        requireV1Version(proof.version, "BSC commit seal version")
        val totalPower = normalizeU64(proof.totalPower, "totalPower")
        val signedPower = normalizeU64(proof.signedPower, "signedPower")
        val commitMessageHash = nonZeroHex32Bytes(proof.commitMessageHash, "commitMessageHash")
        require(proof.validatorPublicKeys.isNotEmpty()) {
            "validatorPublicKeys and validatorPowers must be non-empty bounded arrays"
        }
        require(proof.validatorPublicKeys.size == proof.validatorPowers.size) {
            "validatorPublicKeys and validatorPowers must be non-empty bounded arrays"
        }
        require(proof.validatorPublicKeys.size <= BSC_MAX_PARLIA_VALIDATORS) {
            "validatorPublicKeys and validatorPowers must be non-empty bounded arrays"
        }

        val validatorAddresses = ArrayList<ByteArray>()
        val validatorPowers = ArrayList<BigInteger>()
        val seenAddresses = HashSet<String>()
        proof.validatorPublicKeys.forEachIndexed { index, publicKey ->
            val address = bscValidatorAddress20(publicKey, "validatorPublicKeys[$index]")
            require(seenAddresses.add(hexLower(address))) {
                "validatorPublicKeys[$index] must derive a unique address"
            }
            validatorAddresses.add(address)
            validatorPowers.add(normalizeU64(proof.validatorPowers[index], "validatorPowers[$index]"))
        }

        val validatorSetPayload = canonicalBscValidatorSetPayloadBytesFromAddressPowers(
            validatorAddresses,
            validatorPowers,
        )
        val validatorSetHash = keccakHashBytes("sccp:bsc:validator-set:v1", validatorSetPayload)
        proof.validatorSetHash?.let { supplied ->
            require(hex32Bytes(supplied, "validatorSetHash").contentEquals(validatorSetHash)) {
                "validatorSetHash must match validatorPublicKeys and validatorPowers"
            }
        }
        val computedTotalPower = validatorPowers.fold(BigInteger.ZERO) { sum, power -> sum.add(power) }
        require(computedTotalPower == totalPower) { "totalPower must equal validatorPowers sum" }

        val signerIndices = bscSignerIndicesFromBitmap(proof.signersBitmap, validatorAddresses.size)
        require(proof.signatures.size == signerIndices.size) {
            "signatures length must equal selected signers"
        }
        var computedSignedPower = BigInteger.ZERO
        proof.signatures.zip(signerIndices).forEachIndexed { signatureIndex, (signature, signerIndex) ->
            require(tronRecoverableSignatureIsCanonical(signature)) {
                "signatures[$signatureIndex] must be a canonical recoverable secp256k1 signature"
            }
            val recoveredAddress = tronRecoveredSignerAddress20(commitMessageHash, signature)
            require(recoveredAddress != null && recoveredAddress.contentEquals(validatorAddresses[signerIndex])) {
                "signatures[$signatureIndex] must recover the selected validator address"
            }
            computedSignedPower = computedSignedPower.add(validatorPowers[signerIndex])
        }
        require(computedSignedPower == signedPower) { "signedPower must equal selected validator power" }
        require(computedSignedPower.multiply(BigInteger.valueOf(3L)) > totalPower.multiply(BigInteger.valueOf(2L))) {
            "signedPower must be greater than two thirds of totalPower"
        }

        val out = ByteArrayOutputStream()
        out.write(proof.version)
        writeU64Le(out, totalPower)
        writeU64Le(out, signedPower)
        out.write(commitMessageHash)
        out.write(validatorSetHash)
        writeVector(out, proof.signersBitmap)
        writeU32Le(out, proof.signatures.size)
        proof.signatures.forEach { signature -> writeVector(out, signature) }
        return out.toByteArray()
    }

    @JvmStatic
    fun bscCommitSealHash(proof: BscCommitSealProof): String =
        keccakHashHex("sccp:bsc:commit-seal:v1", canonicalBscCommitSealBytes(proof))

    @JvmStatic
    fun bscValidatorSetStorageValueHash(storageValue: ByteArray): String =
        keccakHashHex("sccp:bsc:validator-set-storage-value:v1", storageValue)

    @JvmStatic
    fun canonicalBscValidatorSetMetadataProofBytes(proof: BscValidatorSetMetadataProof): ByteArray {
        requireV1Version(proof.version, "BSC ValidatorSet metadata proof version")
        require(proof.validatorContractAddress.size == BSC_PARLIA_VALIDATOR_ADDRESS_BYTES) {
            "validatorContractAddress must be 20 bytes"
        }
        validateMptProofNodes(proof.accountProofNodes, "accountProofNodes")
        validateMptProofNodes(proof.validatorSetLengthProofNodes, "validatorSetLengthProofNodes")
        require(proof.validatorStorageProofs.isNotEmpty() && proof.validatorStorageProofs.size <= BSC_MAX_PARLIA_VALIDATORS) {
            "validatorStorageProofs must contain 1..$BSC_MAX_PARLIA_VALIDATORS entries"
        }
        val out = ByteArrayOutputStream()
        out.write(proof.version)
        out.write(hex32Bytes(proof.stateRoot, "stateRoot"))
        out.write(hex32Bytes(proof.nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"))
        writeVector(out, proof.validatorContractAddress)
        writeU32Le(out, proof.accountProofNodes.size)
        proof.accountProofNodes.forEach { node -> writeVector(out, node) }
        out.write(hex32Bytes(proof.storageRoot, "storageRoot"))
        out.write(hex32Bytes(proof.validatorSetLengthSlot, "validatorSetLengthSlot"))
        val validatorSetLengthValueHash =
            hex32Bytes(proof.validatorSetLengthValueHash, "validatorSetLengthValueHash")
        require(
            validatorSetLengthValueHash.contentEquals(
                hex32Bytes(
                    bscValidatorSetStorageValueHash(proof.validatorSetLengthValue),
                    "validatorSetLengthValueHash",
                ),
            ),
        ) { "validatorSetLengthValueHash must match validatorSetLengthValue" }
        writeVector(out, proof.validatorSetLengthValue)
        out.write(validatorSetLengthValueHash)
        writeU32Le(out, proof.validatorSetLengthProofNodes.size)
        proof.validatorSetLengthProofNodes.forEach { node -> writeVector(out, node) }
        writeU32Le(out, proof.validatorStorageProofs.size)
        proof.validatorStorageProofs.forEach { storageProof ->
            out.write(canonicalBscValidatorStorageProofBytes(storageProof))
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun bscValidatorSetMetadataProofHash(proof: BscValidatorSetMetadataProof): String =
        keccakHashHex(
            "sccp:bsc:validator-set-metadata:v1",
            canonicalBscValidatorSetMetadataProofBytes(proof),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalBscValidatorSetTransitionMessageBytes(
        fromValidatorEpoch: String,
        toValidatorEpoch: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentValidatorSetHash: String,
        nextValidatorSetHash: String,
        nextValidatorSetPayloadHash: String,
        validatorSetMetadataProofHash: String,
        sourceDomain: Int = DOMAIN_BSC,
    ): ByteArray {
        require(sourceDomain == DOMAIN_BSC) { "sourceDomain must be BSC" }
        val fromEpoch = normalizeU64(fromValidatorEpoch, "fromValidatorEpoch")
        val toEpoch = normalizeU64(toValidatorEpoch, "toValidatorEpoch")
        require(fromEpoch.add(BigInteger.ONE) == toEpoch) {
            "toValidatorEpoch must equal fromValidatorEpoch + 1"
        }
        val transitionBlock = normalizeU64(transitionBlockNumber, "transitionBlockNumber")
        require(transitionBlock == toEpoch.multiply(BigInteger.valueOf(BSC_PARLIA_EPOCH_LENGTH_BLOCKS))) {
            "transitionBlockNumber must be the BSC Parlia epoch-start block"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        writeU64Le(out, fromEpoch)
        writeU64Le(out, toEpoch)
        writeU64Le(out, transitionBlock)
        out.write(hex32Bytes(transitionBlockHash, "transitionBlockHash"))
        out.write(hex32Bytes(parentValidatorSetHash, "parentValidatorSetHash"))
        out.write(hex32Bytes(nextValidatorSetHash, "nextValidatorSetHash"))
        out.write(hex32Bytes(nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"))
        out.write(hex32Bytes(validatorSetMetadataProofHash, "validatorSetMetadataProofHash"))
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun bscValidatorSetTransitionMessageHash(
        fromValidatorEpoch: String,
        toValidatorEpoch: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentValidatorSetHash: String,
        nextValidatorSetHash: String,
        nextValidatorSetPayloadHash: String,
        validatorSetMetadataProofHash: String,
        sourceDomain: Int = DOMAIN_BSC,
    ): String =
        keccakHashHex(
            "sccp:bsc:validator-set-transition-message:v1",
            canonicalBscValidatorSetTransitionMessageBytes(
                fromValidatorEpoch = fromValidatorEpoch,
                toValidatorEpoch = toValidatorEpoch,
                transitionBlockNumber = transitionBlockNumber,
                transitionBlockHash = transitionBlockHash,
                parentValidatorSetHash = parentValidatorSetHash,
                nextValidatorSetHash = nextValidatorSetHash,
                nextValidatorSetPayloadHash = nextValidatorSetPayloadHash,
                validatorSetMetadataProofHash = validatorSetMetadataProofHash,
                sourceDomain = sourceDomain,
            ),
        )

    @JvmStatic
    fun bscValidatorSetPayloadFromParliaExtra(extraData: ByteArray): ByteArray {
        val candidates = bscParliaValidatorSetPayloadCandidatesFromExtra(extraData)
        require(candidates.size == 1) {
            "BSC Parlia extraData must contain one unambiguous validator set"
        }
        return candidates[0]
    }

    @JvmStatic
    fun bscValidatorSetPayloadFromHeaderRlp(headerRlp: ByteArray): ByteArray {
        val fields = rlpListByteFields(headerRlp)
        require(fields.size >= 13) { "BSC Parlia header RLP must contain an extraData field" }
        return bscValidatorSetPayloadFromParliaExtra(fields[12])
    }

    @JvmStatic
    fun canonicalTonValidatorSetBytes(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): ByteArray {
        val normalized = normalizeTonValidatorSet(validatorPublicKeys, validatorWeights)
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalized.first.size)
        normalized.first.zip(normalized.second).forEach { (publicKey, weight) ->
            out.write(publicKey)
            writeU64Le(out, weight)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun canonicalTonValidatorSetPayloadBytes(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): ByteArray = canonicalTonValidatorSetBytes(validatorPublicKeys, validatorWeights)

    @JvmStatic
    fun tonValidatorSetHashFromPayload(payload: ByteArray): String {
        validateTonValidatorSetPayload(payload)
        return hashHex("sccp:ton:validator-set:v1", payload)
    }

    @JvmStatic
    fun tonValidatorSetPayloadHash(payload: ByteArray): String {
        validateTonValidatorSetPayload(payload)
        return hashHex("sccp:ton:validator-set-payload:v1", payload)
    }

    @JvmStatic
    fun tonValidatorSetHash(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): String =
        tonValidatorSetHashFromPayload(canonicalTonValidatorSetBytes(validatorPublicKeys, validatorWeights))

    @JvmStatic
    @JvmOverloads
    fun canonicalTonMasterchainBlockMessageBytes(
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
        sourceDomain: Int = DOMAIN_TON,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        writeU64Le(out, normalizeU64(masterchainSeqno, "masterchainSeqno"))
        require(masterchainWorkchainId == TON_MASTERCHAIN_WORKCHAIN_ID) {
            "masterchainWorkchainId must be TON masterchain"
        }
        val normalizedShard = normalizeU64(masterchainShard, "masterchainShard")
        require(normalizedShard == TON_MASTERCHAIN_SHARD) {
            "masterchainShard must be TON masterchain shard"
        }
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
    @JvmOverloads
    fun tonMasterchainBlockMessageHash(
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
        sourceDomain: Int = DOMAIN_TON,
    ): String =
        hashHex(
            "sccp:ton:masterchain-block-message:v1",
            canonicalTonMasterchainBlockMessageBytes(
                masterchainSeqno = masterchainSeqno,
                masterchainWorkchainId = masterchainWorkchainId,
                masterchainShard = masterchainShard,
                masterchainBlockHash = masterchainBlockHash,
                masterchainFileHash = masterchainFileHash,
                validatorSetHash = validatorSetHash,
                masterchainConfigRoot = masterchainConfigRoot,
                masterchainConfigProofHash = masterchainConfigProofHash,
                shardWorkchainId = shardWorkchainId,
                shardShard = shardShard,
                shardSeqno = shardSeqno,
                shardBlockHash = shardBlockHash,
                shardFileHash = shardFileHash,
                shardStateRoot = shardStateRoot,
                transactionRoot = transactionRoot,
                shardProofHash = shardProofHash,
                sourceDomain = sourceDomain,
            ),
        )

    @JvmStatic
    fun canonicalTonMasterchainValidatorSignaturesBytes(proof: TonValidatorSignatureProof): ByteArray {
        val derivedValidatorSetHash = tonValidatorSetHash(proof.validatorPublicKeys, proof.validatorWeights)
        if (proof.validatorSetHash != null) {
            require(normalizeHex32(proof.validatorSetHash) == derivedValidatorSetHash) {
                "validatorSetHash must match validator public keys and weights"
            }
        }
        val out = ByteArrayOutputStream()
        out.write(canonicalTonValidatorSignaturesProofBytes(proof))
        out.write(hex32Bytes(derivedValidatorSetHash, "validatorSetHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun tonMasterchainValidatorSignaturesHash(proof: TonValidatorSignatureProof): String =
        hashHex(
            "sccp:ton:masterchain-signatures:v1",
            canonicalTonMasterchainValidatorSignaturesBytes(proof),
        )

    @JvmStatic
    fun ethExecutionPayloadHeaderRootFromRlp(headerRlp: ByteArray): String {
        val fields = rlpListByteFields(headerRlp)
        require(fields.size >= 19) { "headerRlp must include Deneb/Fulu execution payload fields" }
        return "0x" + hexLower(
            sszMerkleizeChunks(
                listOf(
                    sszByteVectorRoot(fields[0], 32, "parentHash"),
                    sszByteVectorRoot(fields[2], 20, "feeRecipient"),
                    sszByteVectorRoot(fields[3], 32, "stateRoot"),
                    sszByteVectorRoot(fields[5], 32, "receiptsRoot"),
                    sszByteVectorRoot(fields[6], 256, "logsBloom"),
                    sszByteVectorRoot(fields[13], 32, "prevRandao"),
                    sszU64ChunkFromRlp(fields[8], "blockNumber"),
                    sszU64ChunkFromRlp(fields[9], "gasLimit"),
                    sszU64ChunkFromRlp(fields[10], "gasUsed"),
                    sszU64ChunkFromRlp(fields[11], "timestamp"),
                    sszByteListRoot(fields[12], 32, "extraData"),
                    sszU256ChunkFromRlp(fields[15], "baseFeePerGas"),
                    keccak256(headerRlp),
                    sszByteVectorRoot(fields[4], 32, "transactionsRoot"),
                    sszByteVectorRoot(fields[16], 32, "withdrawalsRoot"),
                    sszU64ChunkFromRlp(fields[17], "blobGasUsed"),
                    sszU64ChunkFromRlp(fields[18], "excessBlobGas"),
                ),
            ),
        )
    }

    @JvmStatic
    fun ethBeaconBodyRootFromExecutionPayloadBranch(
        executionPayloadHeaderRoot: String,
        executionPayloadBranch: List<ByteArray>,
    ): String {
        require(executionPayloadBranch.size == ETH_EXECUTION_PAYLOAD_BODY_BRANCH_DEPTH) {
            "executionPayloadBranch must contain $ETH_EXECUTION_PAYLOAD_BODY_BRANCH_DEPTH siblings"
        }
        return "0x" + hexLower(
            sszMerkleRootFromBranch(
                hex32Bytes(executionPayloadHeaderRoot, "executionPayloadHeaderRoot"),
                ETH_EXECUTION_PAYLOAD_BODY_FIELD_INDEX,
                executionPayloadBranch,
                "executionPayloadBranch",
            ),
        )
    }

    @JvmStatic
    fun ethBeaconBlockHeaderRoot(
        beaconSlot: String,
        beaconProposerIndex: String,
        beaconParentRoot: String,
        beaconStateRoot: String,
        beaconBodyRoot: String,
    ): String =
        "0x" + hexLower(
            sszMerkleizeChunks(
                listOf(
                    sszU64Chunk(normalizeU64(beaconSlot, "beaconSlot")),
                    sszU64Chunk(normalizeU64(beaconProposerIndex, "beaconProposerIndex")),
                    hex32Bytes(beaconParentRoot, "beaconParentRoot"),
                    hex32Bytes(beaconStateRoot, "beaconStateRoot"),
                    hex32Bytes(beaconBodyRoot, "beaconBodyRoot"),
                ),
            ),
        )

    @JvmStatic
    fun canonicalEvmReceiptRootMptValue(receiptRoot: String): ByteArray {
        val value =
            rlpList(
                listOf(
                    rlpBytes(EVM_RECEIPT_ROOT_VALUE_MARKER),
                    rlpBytes(hex32Bytes(receiptRoot, "receiptRoot")),
                ),
            )
        require(value.isNotEmpty() && value.size <= EVM_MAX_RECEIPT_VALUE_BYTES) {
            "EVM receipt root MPT value must contain 1..$EVM_MAX_RECEIPT_VALUE_BYTES bytes"
        }
        return value
    }

    @JvmStatic
    fun canonicalTronReceiptRootMptValue(receiptRoot: String): ByteArray {
        val value =
            rlpList(
                listOf(
                    rlpBytes(TRON_RECEIPT_ROOT_VALUE_MARKER),
                    rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot")),
                ),
            )
        require(value.isNotEmpty() && value.size <= TRON_MAX_RECEIPT_VALUE_BYTES) {
            "TRON receipt root MPT value must contain 1..$TRON_MAX_RECEIPT_VALUE_BYTES bytes"
        }
        return value
    }

    @JvmStatic
    fun canonicalTronReceiptProofBytes(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        inclusionBranch: List<ByteArray>,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"))
        out.write(nonZeroHex32Bytes(receiptRoot, "receiptRoot"))
        out.write(nonZeroHex32Bytes(transactionRoot, "transactionRoot"))
        writeBranch(out, inclusionBranch, requireNonEmpty = true)
        return out.toByteArray()
    }

    @JvmStatic
    fun tronReceiptProofHash(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        inclusionBranch: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:tron:receipt-proof:v1",
            canonicalTronReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = receiptRoot,
                transactionRoot = transactionRoot,
                inclusionBranch = inclusionBranch,
            ),
        )

    @JvmStatic
    fun canonicalTronReceiptStateProofBytes(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        receiptRootIndex: String,
        receiptTrieProofNodes: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
    ): ByteArray {
        validateTronMptProofNodes(receiptTrieProofNodes)
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"))
        out.write(nonZeroHex32Bytes(receiptRoot, "receiptRoot"))
        out.write(nonZeroHex32Bytes(transactionRoot, "transactionRoot"))
        writeU64Le(out, normalizeU64(receiptRootIndex, "receiptRootIndex"))
        writeU32Le(out, receiptTrieProofNodes.size)
        receiptTrieProofNodes.forEach { node -> writeVector(out, node) }
        writeBranch(out, inclusionBranch, requireNonEmpty = true)
        return out.toByteArray()
    }

    @JvmStatic
    fun tronReceiptStateProofHash(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        receiptRootIndex: String,
        receiptTrieProofNodes: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:tron:receipt-state-proof:v1",
            canonicalTronReceiptStateProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = receiptRoot,
                transactionRoot = transactionRoot,
                receiptRootIndex = receiptRootIndex,
                receiptTrieProofNodes = receiptTrieProofNodes,
                inclusionBranch = inclusionBranch,
            ),
        )

    @JvmStatic
    fun tronSourceMessageCallData(sourceDomain: Int, targetDomain: Int, sourceEventDigest: String): ByteArray {
        require(sourceDomain == DOMAIN_TRON) { "sourceDomain must be TRON for SCCP TRON source-call calldata" }
        require(targetDomain == DOMAIN_SORA) { "targetDomain must be SORA for SCCP TRON source-call calldata" }
        val selector = keccak256(TRON_SOURCE_MESSAGE_CALL_ABI).copyOfRange(0, 4)
        val digest = nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest")
        return selector +
            abiWordU32(sourceDomain, "sourceDomain") +
            abiWordU32(targetDomain, "targetDomain") +
            digest
    }

    @JvmStatic
    fun canonicalTronTransactionSourceProofBytes(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        transactionIndex: String,
        transactionCount: String,
        transactionBytes: ByteArray,
        transactionMerkleBranch: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
    ): ByteArray =
        canonicalTronTransactionSourceProofBytesInternal(
            sourceEventDigest = sourceEventDigest,
            receiptRoot = receiptRoot,
            transactionRoot = transactionRoot,
            transactionIndex = transactionIndex,
            transactionCount = transactionCount,
            transactionBytes = transactionBytes,
            transactionMerkleBranch = transactionMerkleBranch,
            inclusionBranch = inclusionBranch,
            expectedContractAddress = null,
            expectedOwnerAddress = null,
        )

    @JvmStatic
    fun canonicalTronTransactionSourceProofBytes(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        transactionIndex: String,
        transactionCount: String,
        transactionBytes: ByteArray,
        transactionMerkleBranch: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        sourceBridgeEmitterAddress: String?,
        sourceBridgeOwnerAddress: String?,
    ): ByteArray =
        canonicalTronTransactionSourceProofBytesInternal(
            sourceEventDigest = sourceEventDigest,
            receiptRoot = receiptRoot,
            transactionRoot = transactionRoot,
            transactionIndex = transactionIndex,
            transactionCount = transactionCount,
            transactionBytes = transactionBytes,
            transactionMerkleBranch = transactionMerkleBranch,
            inclusionBranch = inclusionBranch,
            expectedContractAddress = sourceBridgeEmitterAddress?.let {
                nonZeroHexBytes(it, "sourceBridgeEmitterAddress", 20)
            },
            expectedOwnerAddress = sourceBridgeOwnerAddress?.let {
                nonZeroHexBytes(it, "sourceBridgeOwnerAddress", 20)
            },
        )

    private fun canonicalTronTransactionSourceProofBytesInternal(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        transactionIndex: String,
        transactionCount: String,
        transactionBytes: ByteArray,
        transactionMerkleBranch: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        expectedContractAddress: ByteArray?,
        expectedOwnerAddress: ByteArray?,
    ): ByteArray {
        val index = normalizeU64(transactionIndex, "transactionIndex")
        val count = normalizeU64(transactionCount, "transactionCount")
        require(count > BigInteger.ZERO && index < count) {
            "transactionIndex must be less than non-zero transactionCount"
        }
        require(transactionBytes.isNotEmpty() && transactionBytes.size <= TRON_MAX_TRANSACTION_BYTES) {
            "transactionBytes must contain 1..$TRON_MAX_TRANSACTION_BYTES bytes"
        }
        validateTronTransactionSourceCall(
            transactionBytes,
            nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"),
            expectedContractAddress,
            expectedOwnerAddress,
        )
        validateTronTransactionMerkleBranch(transactionMerkleBranch)
        val transactionRootBytes = nonZeroHex32Bytes(transactionRoot, "transactionRoot")
        require(
            tronTransactionMerkleRootFromBranch(
                transactionBytes,
                index,
                count,
                transactionMerkleBranch,
            ).contentEquals(transactionRootBytes),
        ) {
            "transactionRoot must match transactionBytes and transactionMerkleBranch"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"))
        out.write(nonZeroHex32Bytes(receiptRoot, "receiptRoot"))
        out.write(transactionRootBytes)
        writeU64Le(out, index)
        writeU64Le(out, count)
        writeVector(out, transactionBytes)
        writeU32Le(out, transactionMerkleBranch.size)
        transactionMerkleBranch.forEach { sibling -> out.write(sibling) }
        writeBranch(out, inclusionBranch, requireNonEmpty = true)
        return out.toByteArray()
    }

    @JvmStatic
    fun tronTransactionSourceProofHash(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        transactionIndex: String,
        transactionCount: String,
        transactionBytes: ByteArray,
        transactionMerkleBranch: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:tron:transaction-source-proof:v1",
            canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = receiptRoot,
                transactionRoot = transactionRoot,
                transactionIndex = transactionIndex,
                transactionCount = transactionCount,
                transactionBytes = transactionBytes,
                transactionMerkleBranch = transactionMerkleBranch,
                inclusionBranch = inclusionBranch,
            ),
        )

    @JvmStatic
    fun tronTransactionSourceProofHash(
        sourceEventDigest: String,
        receiptRoot: String,
        transactionRoot: String,
        transactionIndex: String,
        transactionCount: String,
        transactionBytes: ByteArray,
        transactionMerkleBranch: List<ByteArray>,
        inclusionBranch: List<ByteArray>,
        sourceBridgeEmitterAddress: String?,
        sourceBridgeOwnerAddress: String?,
    ): String =
        hashHex(
            "sccp:tron:transaction-source-proof:v1",
            canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = receiptRoot,
                transactionRoot = transactionRoot,
                transactionIndex = transactionIndex,
                transactionCount = transactionCount,
                transactionBytes = transactionBytes,
                transactionMerkleBranch = transactionMerkleBranch,
                inclusionBranch = inclusionBranch,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                sourceBridgeOwnerAddress = sourceBridgeOwnerAddress,
            ),
        )

    @JvmStatic
    fun canonicalTronRawBlockHeaderBytes(
        number: String,
        txTrieRoot: String,
        accountStateRoot: String,
        parentBlockId: String,
        witnessAddress: String,
        headerVersion: Int,
        timestampMs: String,
    ): ByteArray {
        val blockNumber = normalizeU64(number, "number")
        require(blockNumber != BigInteger.ZERO) { "number must not be zero" }
        val timestamp = normalizeU64(timestampMs, "timestampMs")
        require(timestamp != BigInteger.ZERO) { "timestampMs must not be zero" }
        require(headerVersion > 0) { "headerVersion must be a non-zero u32" }
        val txRoot = hex32Bytes(txTrieRoot, "txTrieRoot")
        require(!isZero(txRoot)) { "txTrieRoot must not be zero" }
        val accountRoot = hex32Bytes(accountStateRoot, "accountStateRoot")
        require(!isZero(accountRoot)) { "accountStateRoot must not be zero" }
        val parentId = hex32Bytes(parentBlockId, "parentBlockId")
        require(!isZero(parentId)) { "parentBlockId must not be zero" }
        val witness = hexBytes(witnessAddress, "witnessAddress", 21)
        require(isNonZeroTronAddress(witness)) { "witnessAddress must be a TRON 0x41-prefixed address" }

        val out = ByteArrayOutputStream()
        writeProtobufU64(out, 1, timestamp)
        writeProtobufBytes(out, 2, txRoot)
        writeProtobufBytes(out, 3, parentId)
        writeProtobufU64(out, 7, blockNumber)
        writeProtobufBytes(out, 9, witness)
        writeProtobufU64(out, 10, BigInteger.valueOf(headerVersion.toLong()))
        writeProtobufBytes(out, 11, accountRoot)
        return out.toByteArray()
    }

    @JvmStatic
    fun tronRawBlockHeaderHash(rawData: ByteArray): String =
        "0x" + hexLower(sha256(rawData))

    @JvmStatic
    fun tronBlockIdFromRawDataHash(number: String, rawDataHash: String): String {
        val blockNumber = normalizeU64(number, "number")
        require(blockNumber != BigInteger.ZERO) { "number must not be zero" }
        return "0x" + hexLower(
            tronBlockIdBytesFromRawDataHash(blockNumber, hex32Bytes(rawDataHash, "rawDataHash")),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun canonicalTronSolidBlockHeaderProofBytes(
        rawData: ByteArray,
        witnessSignature: ByteArray,
        parentRawData: ByteArray,
        parentWitnessSignature: ByteArray,
        rawDataHash: String,
        parentRawDataHash: String,
        blockId: String,
        txTrieRoot: String,
        accountStateRoot: String,
        parentBlockId: String,
        witnessAddress: String,
        timestampMs: String,
        headerVersion: Int,
        version: Int = 1,
    ): ByteArray {
        require(version == 1) { "version must be 1" }
        require(rawData.isNotEmpty() && parentRawData.isNotEmpty()) {
            "rawData and parentRawData must not be empty"
        }
        require(rawData.size <= TRON_MAX_RAW_HEADER_BYTES && parentRawData.size <= TRON_MAX_RAW_HEADER_BYTES) {
            "rawData and parentRawData must be at most $TRON_MAX_RAW_HEADER_BYTES bytes"
        }
        require(witnessSignature.size == 65 && parentWitnessSignature.size == 65) {
            "TRON header signatures must be 65 bytes"
        }
        require(tronRecoverableSignatureIsCanonical(witnessSignature) && tronRecoverableSignatureIsCanonical(parentWitnessSignature)) {
            "TRON header signatures must be canonical low-S with recovery id 0..3 or 27..30"
        }
        val witness = hexBytes(witnessAddress, "witnessAddress", 21)
        require(isNonZeroTronAddress(witness)) { "witnessAddress must be a TRON 0x41-prefixed address" }
        val timestamp = normalizeU64(timestampMs, "timestampMs")
        require(timestamp != BigInteger.ZERO) { "timestampMs must not be zero" }
        require(headerVersion > 0) { "headerVersion must be a non-zero u32" }
        val txRoot = hex32Bytes(txTrieRoot, "txTrieRoot")
        require(!isZero(txRoot)) { "txTrieRoot must not be zero" }
        val accountRoot = hex32Bytes(accountStateRoot, "accountStateRoot")
        require(!isZero(accountRoot)) { "accountStateRoot must not be zero" }
        val parentId = hex32Bytes(parentBlockId, "parentBlockId")
        require(!isZero(parentId)) { "parentBlockId must not be zero" }
        val fields = decodeTronRawBlockHeaderFields(rawData, "rawData")
        val parentFields = decodeTronRawBlockHeaderFields(parentRawData, "parentRawData")
        val rawHash = hex32Bytes(rawDataHash, "rawDataHash")
        val parentRawHash = hex32Bytes(parentRawDataHash, "parentRawDataHash")
        val suppliedBlockId = hex32Bytes(blockId, "blockId")
        require(rawHash.contentEquals(sha256(rawData))) { "rawDataHash must match rawData" }
        require(parentRawHash.contentEquals(sha256(parentRawData))) {
            "parentRawDataHash must match parentRawData"
        }
        require(suppliedBlockId.contentEquals(tronBlockIdBytesFromRawDataHash(fields.number, rawHash))) {
            "blockId must match rawDataHash and block number"
        }
        require(parentId.contentEquals(fields.parentBlockId)) { "parentBlockId must match rawData" }
        require(
            parentId.contentEquals(
                tronBlockIdBytesFromRawDataHash(parentFields.number, parentRawHash),
            ),
        ) {
            "parentBlockId must match parentRawDataHash and parent block number"
        }
        require(parentFields.number.add(BigInteger.ONE) == fields.number && parentFields.timestampMs < fields.timestampMs) {
            "rawData must be the direct child of parentRawData"
        }
        require(
            txRoot.contentEquals(fields.txTrieRoot) &&
                accountRoot.contentEquals(fields.accountStateRoot) &&
                witness.contentEquals(fields.witnessAddress) &&
                timestamp == fields.timestampMs &&
                headerVersion == fields.headerVersion,
        ) {
            "TRON solid-block header fields must match rawData"
        }

        val out = ByteArrayOutputStream()
        out.write(version)
        writeVector(out, rawData)
        writeVector(out, witnessSignature)
        writeVector(out, parentRawData)
        writeVector(out, parentWitnessSignature)
        out.write(rawHash)
        out.write(parentRawHash)
        out.write(suppliedBlockId)
        out.write(txRoot)
        out.write(accountRoot)
        out.write(parentId)
        writeVector(out, witness)
        writeU64Le(out, timestamp)
        writeU32Le(out, headerVersion)
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun tronSolidBlockHeaderProofHash(
        rawData: ByteArray,
        witnessSignature: ByteArray,
        parentRawData: ByteArray,
        parentWitnessSignature: ByteArray,
        rawDataHash: String,
        parentRawDataHash: String,
        blockId: String,
        txTrieRoot: String,
        accountStateRoot: String,
        parentBlockId: String,
        witnessAddress: String,
        timestampMs: String,
        headerVersion: Int,
        version: Int = 1,
    ): String =
        hashHex(
            "sccp:tron:solid-block-header-proof:v1",
            canonicalTronSolidBlockHeaderProofBytes(
                rawData = rawData,
                witnessSignature = witnessSignature,
                parentRawData = parentRawData,
                parentWitnessSignature = parentWitnessSignature,
                rawDataHash = rawDataHash,
                parentRawDataHash = parentRawDataHash,
                blockId = blockId,
                txTrieRoot = txTrieRoot,
                accountStateRoot = accountStateRoot,
                parentBlockId = parentBlockId,
                witnessAddress = witnessAddress,
                timestampMs = timestampMs,
                headerVersion = headerVersion,
                version = version,
            ),
        )

    @JvmStatic
    fun canonicalSubstrateStorageProofBytes(
        sourceDomain: Int,
        sourceEventDigest: String,
        sourceEventLeafIndex: String,
        finalizedBlockNumber: String,
        grandpaSetId: String,
        blockHash: String,
        authoritySetHash: String,
        eventsRoot: String,
        inclusionBranch: List<ByteArray>,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        out.write(hex32Bytes(sourceEventDigest, "sourceEventDigest"))
        out.write(SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY)
        writeU64Le(out, normalizeU64(sourceEventLeafIndex, "sourceEventLeafIndex"))
        writeU64Le(out, normalizeU64(finalizedBlockNumber, "finalizedBlockNumber"))
        writeU64Le(out, normalizeU64(grandpaSetId, "grandpaSetId"))
        out.write(hex32Bytes(blockHash, "blockHash"))
        out.write(hex32Bytes(authoritySetHash, "authoritySetHash"))
        out.write(hex32Bytes(eventsRoot, "eventsRoot"))
        writeBranch(out, inclusionBranch)
        return out.toByteArray()
    }

    @JvmStatic
    fun substrateStorageProofHash(
        sourceDomain: Int,
        sourceEventDigest: String,
        sourceEventLeafIndex: String,
        finalizedBlockNumber: String,
        grandpaSetId: String,
        blockHash: String,
        authoritySetHash: String,
        eventsRoot: String,
        inclusionBranch: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:substrate:storage-proof:v1",
            canonicalSubstrateStorageProofBytes(
                sourceDomain = sourceDomain,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = sourceEventLeafIndex,
                finalizedBlockNumber = finalizedBlockNumber,
                grandpaSetId = grandpaSetId,
                blockHash = blockHash,
                authoritySetHash = authoritySetHash,
                eventsRoot = eventsRoot,
                inclusionBranch = inclusionBranch,
            ),
        )

    private fun isSubstrateRuntimeStorageSourceDomain(sourceDomain: Int): Boolean =
        sourceDomain == DOMAIN_SORA_KUSAMA ||
            sourceDomain == DOMAIN_SORA_POLKADOT ||
            sourceDomain == DOMAIN_SORA2

    private fun substrateRuntimeStorageSourceMaterial(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        sourceStateVerifierHash: String,
    ): NormalizedSourceMaterial {
        require(isSubstrateRuntimeStorageSourceDomain(sourceDomain)) {
            "sourceDomain must be a Substrate-family SCCP source domain"
        }
        val material = normalizeSourceMaterial(
            sourceDomain = sourceDomain,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            bridgeAddress = null,
            sourceBridgeEmitterCodeHash = null,
            networkId = null,
            ownerAddress = null,
            configHash = null,
        )
        require(material.profile.sourceStateVerifierId.isNotEmpty() && material.sourceStateVerifierHash.any { it != 0.toByte() }) {
            "sourceStateVerifierHash must bind a deployed Substrate runtime-storage verifier"
        }
        require(
            !material.sourceStateVerifierHash.contentEquals(
                requireNotNull(SUBSTRATE_TEMPLATE_SOURCE_STATE_VERIFIER_HASHES[sourceDomain]),
            ),
        ) {
            "sourceStateVerifierHash must not be the Substrate template verifier hash"
        }
        return material
    }

    private fun wordU32Le(value: Int): ByteArray {
        val out = ByteArray(32)
        val word = ByteArrayOutputStream()
        writeU32Le(word, value)
        System.arraycopy(word.toByteArray(), 0, out, 0, 4)
        return out
    }

    private fun wordU64Le(value: BigInteger): ByteArray {
        val out = ByteArray(32)
        val word = ByteArrayOutputStream()
        writeU64Le(word, value)
        System.arraycopy(word.toByteArray(), 0, out, 0, 8)
        return out
    }

    @JvmStatic
    fun canonicalSubstrateRuntimeStorageVerificationStatementBytes(
        sourceDomain: Int,
        sourceEventDigest: String,
        sourceEventLeafIndex: String,
        finalizedBlockNumber: String,
        grandpaSetId: String,
        blockHash: String,
        authoritySetHash: String,
        eventsRoot: String,
        inclusionBranch: List<ByteArray>,
        storageProofHash: String? = null,
    ): ByteArray {
        require(isSubstrateRuntimeStorageSourceDomain(normalizeDomain(sourceDomain, "sourceDomain"))) {
            "sourceDomain must be a Substrate-family SCCP source domain"
        }
        val statement = canonicalSubstrateStorageProofBytes(
            sourceDomain = sourceDomain,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = sourceEventLeafIndex,
            finalizedBlockNumber = finalizedBlockNumber,
            grandpaSetId = grandpaSetId,
            blockHash = blockHash,
            authoritySetHash = authoritySetHash,
            eventsRoot = eventsRoot,
            inclusionBranch = inclusionBranch,
        )
        if (storageProofHash != null) {
            val supplied = "0x" + hexLower(hex32Bytes(storageProofHash, "storageProofHash"))
            require(supplied == hashHex("sccp:substrate:storage-proof:v1", statement)) {
                "storageProofHash must match Substrate runtime-storage statement"
            }
        }
        return statement
    }

    @JvmStatic
    fun substrateRuntimeStorageProofPublicInputsHash(
        sourceDomain: Int,
        sourceEventDigest: String,
        sourceEventLeafIndex: String,
        finalizedBlockNumber: String,
        grandpaSetId: String,
        blockHash: String,
        authoritySetHash: String,
        eventsRoot: String,
        inclusionBranch: List<ByteArray>,
        storageProofHash: String? = null,
    ): String =
        hashHex(
            SUBSTRATE_RUNTIME_STORAGE_PROOF_PUBLIC_INPUTS_PREFIX_V1,
            canonicalSubstrateRuntimeStorageVerificationStatementBytes(
                sourceDomain = sourceDomain,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = sourceEventLeafIndex,
                finalizedBlockNumber = finalizedBlockNumber,
                grandpaSetId = grandpaSetId,
                blockHash = blockHash,
                authoritySetHash = authoritySetHash,
                eventsRoot = eventsRoot,
                inclusionBranch = inclusionBranch,
                storageProofHash = storageProofHash,
            ),
        )

    @JvmStatic
    fun canonicalSubstrateRuntimeStorageVerificationContextBytes(
        sourceDomain: Int,
        sourceEventDigest: String,
        sourceEventLeafIndex: String,
        finalizedBlockNumber: String,
        grandpaSetId: String,
        blockHash: String,
        authoritySetHash: String,
        eventsRoot: String,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        sourceStateVerifierHash: String,
        inclusionBranch: List<ByteArray>,
        storageProofHash: String? = null,
    ): ByteArray {
        val material = substrateRuntimeStorageSourceMaterial(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
        )
        val out = ByteArrayOutputStream()
        out.write(1)
        writeVector(out, SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, material.profile.sourceStateVerifierId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.sourceStateVerifierHash)
        writeVector(out, material.profile.sourceTrustAnchorId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.sourceTrustAnchorHash)
        writeVector(out, material.profile.consensusVerifierId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.consensusVerifierHash)
        writeVector(out, material.profile.messageInclusionVerifierId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.messageInclusionVerifierHash)
        writeVector(out, material.profile.finalityPolicyId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.finalityPolicyHash)
        out.write(
            hex32Bytes(
                substrateRuntimeStorageProofPublicInputsHash(
                    sourceDomain = sourceDomain,
                    sourceEventDigest = sourceEventDigest,
                    sourceEventLeafIndex = sourceEventLeafIndex,
                    finalizedBlockNumber = finalizedBlockNumber,
                    grandpaSetId = grandpaSetId,
                    blockHash = blockHash,
                    authoritySetHash = authoritySetHash,
                    eventsRoot = eventsRoot,
                    inclusionBranch = inclusionBranch,
                    storageProofHash = storageProofHash,
                ),
                "runtimeStorageProofPublicInputsHash",
            ),
        )
        return out.toByteArray()
    }

    @JvmStatic
    fun substrateRuntimeStoragePublicInputColumns(
        sourceDomain: Int,
        sourceEventDigest: String,
        sourceEventLeafIndex: String,
        finalizedBlockNumber: String,
        grandpaSetId: String,
        blockHash: String,
        authoritySetHash: String,
        eventsRoot: String,
        inclusionBranch: List<ByteArray>,
        storageProofHash: String? = null,
    ): List<List<String>> {
        val computedStorageProofHash = substrateStorageProofHash(
            sourceDomain = sourceDomain,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = sourceEventLeafIndex,
            finalizedBlockNumber = finalizedBlockNumber,
            grandpaSetId = grandpaSetId,
            blockHash = blockHash,
            authoritySetHash = authoritySetHash,
            eventsRoot = eventsRoot,
            inclusionBranch = inclusionBranch,
        )
        if (storageProofHash != null) {
            val supplied = "0x" + hexLower(hex32Bytes(storageProofHash, "storageProofHash"))
            require(supplied == computedStorageProofHash) {
                "storageProofHash must match Substrate runtime-storage statement"
            }
        }
        val publicInputsHash = substrateRuntimeStorageProofPublicInputsHash(
            sourceDomain = sourceDomain,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = sourceEventLeafIndex,
            finalizedBlockNumber = finalizedBlockNumber,
            grandpaSetId = grandpaSetId,
            blockHash = blockHash,
            authoritySetHash = authoritySetHash,
            eventsRoot = eventsRoot,
            inclusionBranch = inclusionBranch,
            storageProofHash = computedStorageProofHash,
        )
        return listOf(
            listOf("0x" + hexLower(wordU32Le(normalizeDomain(sourceDomain, "sourceDomain")))),
            listOf("0x" + hexLower(wordU64Le(normalizeU64(finalizedBlockNumber, "finalizedBlockNumber")))),
            listOf("0x" + hexLower(wordU64Le(normalizeU64(grandpaSetId, "grandpaSetId")))),
            listOf("0x" + hexLower(hex32Bytes(blockHash, "blockHash"))),
            listOf("0x" + hexLower(hex32Bytes(authoritySetHash, "authoritySetHash"))),
            listOf("0x" + hexLower(hex32Bytes(eventsRoot, "eventsRoot"))),
            listOf(computedStorageProofHash),
            listOf("0x" + hexLower(hex32Bytes(sourceEventDigest, "sourceEventDigest"))),
            listOf("0x" + hexLower(SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY)),
            listOf("0x" + hexLower(wordU64Le(normalizeU64(sourceEventLeafIndex, "sourceEventLeafIndex")))),
            listOf(publicInputsHash),
        )
    }

    @JvmStatic
    fun substrateRuntimeStorageOpenVerifySchemaDescriptor(sourceDomain: Int): ByteArray {
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        require(isSubstrateRuntimeStorageSourceDomain(normalizedSourceDomain)) {
            "sourceDomain must be a Substrate-family SCCP source domain"
        }
        val chain = sourceAdapterVerifierProfile(normalizedSourceDomain).first
        val out = ByteArrayOutputStream()
        out.write(1)
        writeVector(out, SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, chain.toByteArray(StandardCharsets.UTF_8))
        writeU32Le(out, normalizedSourceDomain)
        listOf(
            "source_domain",
            "finalized_block_number",
            "grandpa_set_id",
            "block_hash",
            "authority_set_hash",
            "events_root",
            "storage_proof_hash",
            "source_event_digest",
            "system_events_storage_key",
            "source_event_leaf_index",
            "runtime_storage_proof_public_inputs_hash",
        ).forEach { writeVector(out, it.toByteArray(StandardCharsets.UTF_8)) }
        return out.toByteArray()
    }

    @JvmStatic
    fun buildSubstrateRuntimeStorageProofRequest(
        sourceDomain: Int,
        sourceEventDigest: String,
        sourceEventLeafIndex: String,
        finalizedBlockNumber: String,
        grandpaSetId: String,
        blockHash: String,
        authoritySetHash: String,
        eventsRoot: String,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        sourceStateVerifierHash: String,
        inclusionBranch: List<ByteArray>,
        storageProofHash: String? = null,
    ): SubstrateRuntimeStorageProofRequest {
        val material = substrateRuntimeStorageSourceMaterial(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
        )
        val statement = canonicalSubstrateRuntimeStorageVerificationStatementBytes(
            sourceDomain = sourceDomain,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = sourceEventLeafIndex,
            finalizedBlockNumber = finalizedBlockNumber,
            grandpaSetId = grandpaSetId,
            blockHash = blockHash,
            authoritySetHash = authoritySetHash,
            eventsRoot = eventsRoot,
            inclusionBranch = inclusionBranch,
            storageProofHash = storageProofHash,
        )
        val computedStorageProofHash = hashHex("sccp:substrate:storage-proof:v1", statement)
        val publicInputsHash = substrateRuntimeStorageProofPublicInputsHash(
            sourceDomain = sourceDomain,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = sourceEventLeafIndex,
            finalizedBlockNumber = finalizedBlockNumber,
            grandpaSetId = grandpaSetId,
            blockHash = blockHash,
            authoritySetHash = authoritySetHash,
            eventsRoot = eventsRoot,
            inclusionBranch = inclusionBranch,
            storageProofHash = computedStorageProofHash,
        )
        val context = canonicalSubstrateRuntimeStorageVerificationContextBytes(
            sourceDomain = sourceDomain,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = sourceEventLeafIndex,
            finalizedBlockNumber = finalizedBlockNumber,
            grandpaSetId = grandpaSetId,
            blockHash = blockHash,
            authoritySetHash = authoritySetHash,
            eventsRoot = eventsRoot,
            sourceTrustAnchorHash = sourceTrustAnchorHash,
            consensusVerifierHash = consensusVerifierHash,
            messageInclusionVerifierHash = messageInclusionVerifierHash,
            finalityPolicyHash = finalityPolicyHash,
            sourceStateVerifierHash = sourceStateVerifierHash,
            inclusionBranch = inclusionBranch,
            storageProofHash = computedStorageProofHash,
        )
        val dsidHash = hashBytes(
            SUBSTRATE_RUNTIME_STORAGE_FASTPQ_DSID_PREFIX_V1,
            hex32Bytes(publicInputsHash, "runtimeStorageProofPublicInputsHash"),
        )
        val transitions = listOf(
            SubstrateRuntimeStorageFastpqTransition(
                SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STATEMENT_KEY_V1,
                "meta_set",
                "0x",
                "0x" + hexLower(statement),
            ),
            SubstrateRuntimeStorageFastpqTransition(
                SUBSTRATE_RUNTIME_STORAGE_FASTPQ_CONTEXT_KEY_V1,
                "meta_set",
                "0x",
                "0x" + hexLower(context),
            ),
            SubstrateRuntimeStorageFastpqTransition(
                SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STORAGE_KEY_V1,
                "meta_set",
                "0x",
                "0x" + hexLower(SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY),
            ),
        ).sortedBy { it.key }
        return SubstrateRuntimeStorageProofRequest(
            version = 1,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
            circuitId = SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1,
            parameterSet = SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1,
            sourceDomain = normalizeDomain(sourceDomain, "sourceDomain"),
            finalizedBlockNumber = normalizeU64(finalizedBlockNumber, "finalizedBlockNumber").toString(),
            grandpaSetId = normalizeU64(grandpaSetId, "grandpaSetId").toString(),
            sourceStateVerifierId = material.profile.sourceStateVerifierId,
            sourceStateVerifierHash = "0x" + hexLower(material.sourceStateVerifierHash),
            runtimeStorageProofPublicInputsHash = publicInputsHash,
            storageProofHash = computedStorageProofHash,
            statementBytes = statement,
            verificationContextBytes = context,
            schemaDescriptor = substrateRuntimeStorageOpenVerifySchemaDescriptor(sourceDomain),
            publicInputColumns = substrateRuntimeStoragePublicInputColumns(
                sourceDomain = sourceDomain,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = sourceEventLeafIndex,
                finalizedBlockNumber = finalizedBlockNumber,
                grandpaSetId = grandpaSetId,
                blockHash = blockHash,
                authoritySetHash = authoritySetHash,
                eventsRoot = eventsRoot,
                inclusionBranch = inclusionBranch,
                storageProofHash = computedStorageProofHash,
            ),
            fastpqPublicInputs = SubstrateRuntimeStorageFastpqPublicInputs(
                dsid = "0x" + hexLower(dsidHash.copyOfRange(0, 16)),
                slot = normalizeU64(finalizedBlockNumber, "finalizedBlockNumber").toString(),
                oldRoot = "0x" + hexLower(hex32Bytes(authoritySetHash, "authoritySetHash")),
                newRoot = "0x" + hexLower(hex32Bytes(blockHash, "blockHash")),
                permRoot = "0x" + hexLower(hex32Bytes(eventsRoot, "eventsRoot")),
                txSetHash = publicInputsHash,
            ),
            fastpqTransitions = transitions,
        )
    }

    @JvmStatic
    fun canonicalTronWitnessSchedulePayloadBytes(
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
    ): ByteArray {
        require(witnessAddresses.isNotEmpty() && witnessAddresses.size == witnessWeights.size) {
            "witnessAddresses and witnessWeights must be non-empty equal-length arrays"
        }
        require(witnessAddresses.size <= TRON_MAX_WITNESSES) {
            "witnessAddresses must contain at most $TRON_MAX_WITNESSES entries"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, witnessAddresses.size)
        val seenAddresses = HashSet<String>()
        var totalWeight = BigInteger.ZERO
        witnessAddresses.zip(witnessWeights).forEachIndexed { index, (addressValue, weightValue) ->
            val address = hexBytes(addressValue, "witnessAddresses[$index]", 21)
            require(isNonZeroTronAddress(address)) {
                "witnessAddresses[$index] must be a TRON 0x41-prefixed address"
            }
            val addressHex = hexLower(address)
            require(seenAddresses.add(addressHex)) { "witnessAddresses[$index] must be unique" }
            val weight = normalizeU64(weightValue, "witnessWeights[$index]")
            require(weight != BigInteger.ZERO) { "witnessWeights[$index] must not be zero" }
            totalWeight = totalWeight.add(weight)
            require(totalWeight <= MAX_U64) { "witnessWeights total must fit u64" }
            out.write(address)
            writeU64Le(out, weight)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun tronWitnessSchedulePayloadHash(payload: ByteArray): String {
        validateTronWitnessSchedulePayload(payload)
        return hashHex("sccp:tron:witness-schedule-payload:v1", payload)
    }

    @JvmStatic
    fun tronWitnessSchedulePayloadHash(
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
    ): String =
        tronWitnessSchedulePayloadHash(
            canonicalTronWitnessSchedulePayloadBytes(witnessAddresses, witnessWeights),
        )

    @JvmStatic
    fun tronWitnessScheduleHashFromPayload(payload: ByteArray): String {
        validateTronWitnessSchedulePayload(payload)
        return hashHex("sccp:tron:witness-schedule:v1", payload)
    }

    @JvmStatic
    fun tronWitnessScheduleHashFromPayload(
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
    ): String =
        tronWitnessScheduleHashFromPayload(
            canonicalTronWitnessSchedulePayloadBytes(witnessAddresses, witnessWeights),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalTronSolidBlockMessageBytes(
        sourceDomain: Int,
        solidBlockNumber: String,
        blockHash: String,
        witnessScheduleHash: String,
        receiptRoot: String,
        transactionRoot: String,
        receiptProofHash: String,
        version: Int = 1,
    ): ByteArray {
        requireV1Version(version, "TRON solid-block message version")
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        require(normalizedSourceDomain == DOMAIN_TRON) { "sourceDomain must be TRON" }
        val normalizedSolidBlockNumber = normalizeU64(solidBlockNumber, "solidBlockNumber")
        require(normalizedSolidBlockNumber != BigInteger.ZERO) { "solidBlockNumber must not be zero" }
        val blockHashBytes = nonZeroHex32Bytes(blockHash, "blockHash")
        val witnessScheduleHashBytes = nonZeroHex32Bytes(witnessScheduleHash, "witnessScheduleHash")
        val receiptRootBytes = nonZeroHex32Bytes(receiptRoot, "receiptRoot")
        val transactionRootBytes = nonZeroHex32Bytes(transactionRoot, "transactionRoot")
        val receiptProofHashBytes = nonZeroHex32Bytes(receiptProofHash, "receiptProofHash")
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU32Le(out, normalizedSourceDomain)
        writeU64Le(out, normalizedSolidBlockNumber)
        out.write(blockHashBytes)
        out.write(witnessScheduleHashBytes)
        out.write(receiptRootBytes)
        out.write(transactionRootBytes)
        out.write(receiptProofHashBytes)
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun tronSolidBlockMessageHash(
        sourceDomain: Int,
        solidBlockNumber: String,
        blockHash: String,
        witnessScheduleHash: String,
        receiptRoot: String,
        transactionRoot: String,
        receiptProofHash: String,
        version: Int = 1,
    ): String =
        keccakHashHex(
            "sccp:tron:solid-block-message:v1",
            canonicalTronSolidBlockMessageBytes(
                sourceDomain = sourceDomain,
                solidBlockNumber = solidBlockNumber,
                blockHash = blockHash,
                witnessScheduleHash = witnessScheduleHash,
                receiptRoot = receiptRoot,
                transactionRoot = transactionRoot,
                receiptProofHash = receiptProofHash,
                version = version,
            ),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalTronWitnessSealBytes(
        totalWeight: String,
        signedWeight: String,
        solidBlockMessageHash: String,
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
        version: Int = 1,
    ): ByteArray {
        val proof = normalizeTronWitnessSealProof(
            version = version,
            totalWeight = totalWeight,
            signedWeight = signedWeight,
            solidBlockMessageHash = solidBlockMessageHash,
            witnessAddresses = witnessAddresses,
            witnessWeights = witnessWeights,
            signersBitmap = signersBitmap,
            signatures = signatures,
        )
        val out = ByteArrayOutputStream()
        out.write(canonicalTronWitnessSealProofBytes(proof))
        out.write(proof.witnessScheduleHash)
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun tronWitnessSealHash(
        totalWeight: String,
        signedWeight: String,
        solidBlockMessageHash: String,
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
        version: Int = 1,
    ): String =
        hashHex(
            "sccp:tron:witness-seal:v1",
            canonicalTronWitnessSealBytes(
                totalWeight = totalWeight,
                signedWeight = signedWeight,
                solidBlockMessageHash = solidBlockMessageHash,
                witnessAddresses = witnessAddresses,
                witnessWeights = witnessWeights,
                signersBitmap = signersBitmap,
                signatures = signatures,
                version = version,
            ),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalTronWitnessScheduleTransitionMessageBytes(
        sourceDomain: Int,
        fromWitnessScheduleEpoch: String,
        toWitnessScheduleEpoch: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentWitnessScheduleHash: String,
        nextWitnessScheduleHash: String,
        nextWitnessSchedulePayloadHash: String? = null,
        nextWitnessSchedulePayload: ByteArray? = null,
        version: Int = 1,
    ): ByteArray =
        canonicalTronWitnessScheduleTransitionMessageBytes(
            normalizeTronWitnessScheduleTransitionMessage(
                version = version,
                sourceDomain = sourceDomain,
                fromWitnessScheduleEpoch = fromWitnessScheduleEpoch,
                toWitnessScheduleEpoch = toWitnessScheduleEpoch,
                transitionBlockNumber = transitionBlockNumber,
                transitionBlockHash = transitionBlockHash,
                parentWitnessScheduleHash = parentWitnessScheduleHash,
                nextWitnessScheduleHash = nextWitnessScheduleHash,
                nextWitnessSchedulePayloadHash = nextWitnessSchedulePayloadHash,
                nextWitnessSchedulePayload = nextWitnessSchedulePayload,
            ),
        )

    @JvmStatic
    @JvmOverloads
    fun tronWitnessScheduleTransitionMessageHash(
        sourceDomain: Int,
        fromWitnessScheduleEpoch: String,
        toWitnessScheduleEpoch: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentWitnessScheduleHash: String,
        nextWitnessScheduleHash: String,
        nextWitnessSchedulePayloadHash: String? = null,
        nextWitnessSchedulePayload: ByteArray? = null,
        version: Int = 1,
    ): String =
        keccakHashHex(
            "sccp:tron:witness-schedule-transition-message:v1",
            canonicalTronWitnessScheduleTransitionMessageBytes(
                sourceDomain = sourceDomain,
                fromWitnessScheduleEpoch = fromWitnessScheduleEpoch,
                toWitnessScheduleEpoch = toWitnessScheduleEpoch,
                transitionBlockNumber = transitionBlockNumber,
                transitionBlockHash = transitionBlockHash,
                parentWitnessScheduleHash = parentWitnessScheduleHash,
                nextWitnessScheduleHash = nextWitnessScheduleHash,
                nextWitnessSchedulePayloadHash = nextWitnessSchedulePayloadHash,
                nextWitnessSchedulePayload = nextWitnessSchedulePayload,
                version = version,
            ),
        )

    @JvmStatic
    @JvmOverloads
    fun canonicalTronWitnessScheduleTransitionSealBytes(
        sourceDomain: Int,
        fromWitnessScheduleEpoch: String,
        toWitnessScheduleEpoch: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentWitnessScheduleHash: String,
        nextWitnessScheduleHash: String,
        nextWitnessSchedulePayload: ByteArray,
        transitionMessageHash: String,
        totalWeight: String,
        signedWeight: String,
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
        version: Int = 1,
    ): ByteArray {
        val message = normalizeTronWitnessScheduleTransitionMessage(
            version = version,
            sourceDomain = sourceDomain,
            fromWitnessScheduleEpoch = fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch = toWitnessScheduleEpoch,
            transitionBlockNumber = transitionBlockNumber,
            transitionBlockHash = transitionBlockHash,
            parentWitnessScheduleHash = parentWitnessScheduleHash,
            nextWitnessScheduleHash = nextWitnessScheduleHash,
            nextWitnessSchedulePayloadHash = null,
            nextWitnessSchedulePayload = nextWitnessSchedulePayload,
        )
        val expectedTransitionMessageHash = hex32Bytes(
            tronWitnessScheduleTransitionMessageHash(
                sourceDomain = sourceDomain,
                fromWitnessScheduleEpoch = fromWitnessScheduleEpoch,
                toWitnessScheduleEpoch = toWitnessScheduleEpoch,
                transitionBlockNumber = transitionBlockNumber,
                transitionBlockHash = transitionBlockHash,
                parentWitnessScheduleHash = parentWitnessScheduleHash,
                nextWitnessScheduleHash = nextWitnessScheduleHash,
                nextWitnessSchedulePayloadHash = null,
                nextWitnessSchedulePayload = nextWitnessSchedulePayload,
                version = version,
            ),
            "transitionMessageHash",
        )
        val transitionMessageHashBytes = nonZeroHex32Bytes(transitionMessageHash, "transitionMessageHash")
        require(transitionMessageHashBytes.contentEquals(expectedTransitionMessageHash)) {
            "transitionMessageHash must match transition message fields"
        }
        val proof = normalizeTronWitnessSealProof(
            version = version,
            totalWeight = totalWeight,
            signedWeight = signedWeight,
            solidBlockMessageHash = transitionMessageHash,
            witnessAddresses = witnessAddresses,
            witnessWeights = witnessWeights,
            signersBitmap = signersBitmap,
            signatures = signatures,
        )
        require(proof.witnessScheduleHash.contentEquals(message.parentWitnessScheduleHash)) {
            "parentWitnessScheduleHash must match witness seal proof"
        }
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU32Le(out, message.sourceDomain)
        writeU64Le(out, message.fromWitnessScheduleEpoch)
        writeU64Le(out, message.toWitnessScheduleEpoch)
        writeU64Le(out, message.transitionBlockNumber)
        out.write(message.transitionBlockHash)
        out.write(message.parentWitnessScheduleHash)
        out.write(message.nextWitnessScheduleHash)
        writeVector(out, nextWitnessSchedulePayload)
        out.write(message.nextWitnessSchedulePayloadHash)
        out.write(transitionMessageHashBytes)
        out.write(proof.witnessScheduleHash)
        out.write(canonicalTronWitnessSealProofBytes(proof))
        return out.toByteArray()
    }

    @JvmStatic
    @JvmOverloads
    fun tronWitnessScheduleTransitionSealHash(
        sourceDomain: Int,
        fromWitnessScheduleEpoch: String,
        toWitnessScheduleEpoch: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentWitnessScheduleHash: String,
        nextWitnessScheduleHash: String,
        nextWitnessSchedulePayload: ByteArray,
        transitionMessageHash: String,
        totalWeight: String,
        signedWeight: String,
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
        version: Int = 1,
    ): String =
        hashHex(
            "sccp:tron:witness-schedule-transition-seal:v1",
            canonicalTronWitnessScheduleTransitionSealBytes(
                sourceDomain = sourceDomain,
                fromWitnessScheduleEpoch = fromWitnessScheduleEpoch,
                toWitnessScheduleEpoch = toWitnessScheduleEpoch,
                transitionBlockNumber = transitionBlockNumber,
                transitionBlockHash = transitionBlockHash,
                parentWitnessScheduleHash = parentWitnessScheduleHash,
                nextWitnessScheduleHash = nextWitnessScheduleHash,
                nextWitnessSchedulePayload = nextWitnessSchedulePayload,
                transitionMessageHash = transitionMessageHash,
                totalWeight = totalWeight,
                signedWeight = signedWeight,
                witnessAddresses = witnessAddresses,
                witnessWeights = witnessWeights,
                signersBitmap = signersBitmap,
                signatures = signatures,
                version = version,
            ),
        )

    @JvmStatic
    fun canonicalSubstrateAuthoritySetPayloadBytes(
        authorityPublicKeys: List<String>,
        authorityWeights: List<String>,
    ): ByteArray {
        require(authorityPublicKeys.isNotEmpty() && authorityPublicKeys.size == authorityWeights.size) {
            "authorityPublicKeys and authorityWeights must be non-empty equal-length arrays"
        }
        require(authorityPublicKeys.size <= SUBSTRATE_MAX_AUTHORITIES) {
            "authorityPublicKeys must contain at most $SUBSTRATE_MAX_AUTHORITIES entries"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, authorityPublicKeys.size)
        val seenPublicKeys = HashSet<String>()
        authorityPublicKeys.zip(authorityWeights).forEachIndexed { index, (publicKeyValue, weightValue) ->
            val publicKey = hexBytes(publicKeyValue, "authorityPublicKeys[$index]", 32)
            require(!isZero(publicKey)) { "authorityPublicKeys[$index] must not be zero" }
            val publicKeyHex = hexLower(publicKey)
            require(seenPublicKeys.add(publicKeyHex)) { "authorityPublicKeys[$index] must be unique" }
            val weight = normalizeU64(weightValue, "authorityWeights[$index]")
            require(weight != BigInteger.ZERO) { "authorityWeights[$index] must not be zero" }
            out.write(publicKey)
            writeU64Le(out, weight)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun substrateAuthoritySetPayloadHash(payload: ByteArray): String {
        validateSubstrateAuthoritySetPayload(payload)
        return hashHex("sccp:substrate:authority-set-payload:v1", payload)
    }

    @JvmStatic
    fun substrateAuthoritySetPayloadHash(
        authorityPublicKeys: List<String>,
        authorityWeights: List<String>,
    ): String =
        substrateAuthoritySetPayloadHash(
            canonicalSubstrateAuthoritySetPayloadBytes(authorityPublicKeys, authorityWeights),
        )

    @JvmStatic
    fun substrateAuthoritySetHashFromPayload(payload: ByteArray): String {
        validateSubstrateAuthoritySetPayload(payload)
        return hashHex("sccp:substrate:authority-set:v1", payload)
    }

    @JvmStatic
    fun substrateAuthoritySetHashFromPayload(
        authorityPublicKeys: List<String>,
        authorityWeights: List<String>,
    ): String =
        substrateAuthoritySetHashFromPayload(
            canonicalSubstrateAuthoritySetPayloadBytes(authorityPublicKeys, authorityWeights),
        )

    @JvmStatic
    fun canonicalSubstrateAuthoritySetTransitionMessageBytes(
        sourceDomain: Int,
        fromGrandpaSetId: String,
        toGrandpaSetId: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentAuthoritySetHash: String,
        nextAuthoritySetHash: String,
        nextAuthoritySetPayloadHash: String,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        writeU64Le(out, normalizeU64(fromGrandpaSetId, "fromGrandpaSetId"))
        writeU64Le(out, normalizeU64(toGrandpaSetId, "toGrandpaSetId"))
        writeU64Le(out, normalizeU64(transitionBlockNumber, "transitionBlockNumber"))
        out.write(hex32Bytes(transitionBlockHash, "transitionBlockHash"))
        out.write(hex32Bytes(parentAuthoritySetHash, "parentAuthoritySetHash"))
        out.write(hex32Bytes(nextAuthoritySetHash, "nextAuthoritySetHash"))
        out.write(hex32Bytes(nextAuthoritySetPayloadHash, "nextAuthoritySetPayloadHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun substrateAuthoritySetTransitionMessageHash(
        sourceDomain: Int,
        fromGrandpaSetId: String,
        toGrandpaSetId: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentAuthoritySetHash: String,
        nextAuthoritySetHash: String,
        nextAuthoritySetPayloadHash: String,
    ): String =
        hashHex(
            "sccp:substrate:authority-set-transition-message:v1",
            canonicalSubstrateAuthoritySetTransitionMessageBytes(
                sourceDomain,
                fromGrandpaSetId,
                toGrandpaSetId,
                transitionBlockNumber,
                transitionBlockHash,
                parentAuthoritySetHash,
                nextAuthoritySetHash,
                nextAuthoritySetPayloadHash,
            ),
        )

    @JvmStatic
    fun canonicalSubstrateGrandpaJustificationProofBytes(
        version: Int,
        totalWeight: String,
        signedWeight: String,
        precommitMessageHash: String,
        authorityPublicKeys: List<String>,
        authorityWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
    ): ByteArray {
        requireV1Version(version, "Substrate GRANDPA justification version")
        require(authorityPublicKeys.isNotEmpty() && authorityPublicKeys.size == authorityWeights.size) {
            "authorityPublicKeys and authorityWeights must be non-empty equal-length arrays"
        }
        require(authorityPublicKeys.size <= SUBSTRATE_MAX_AUTHORITIES) {
            "authorityPublicKeys must contain at most $SUBSTRATE_MAX_AUTHORITIES entries"
        }
        require(signatures.size <= SUBSTRATE_MAX_AUTHORITIES) {
            "signatures must contain at most $SUBSTRATE_MAX_AUTHORITIES entries"
        }
        val totalWeightValue = normalizeU64(totalWeight, "totalWeight")
        val signedWeightValue = normalizeU64(signedWeight, "signedWeight")
        val precommitHashBytes = hex32Bytes(precommitMessageHash, "precommitMessageHash")
        val publicKeys = ArrayList<ByteArray>()
        val seenPublicKeys = HashSet<String>()
        authorityPublicKeys.forEachIndexed { index, publicKey ->
            val publicKeyBytes = hex32Bytes(publicKey, "authorityPublicKeys[$index]")
            require(!isZero(publicKeyBytes)) { "authorityPublicKeys[$index] must not be zero" }
            require(seenPublicKeys.add(hexLower(publicKeyBytes))) {
                "authorityPublicKeys[$index] must be unique"
            }
            publicKeys += publicKeyBytes
        }
        val weights = authorityWeights.mapIndexed { index, weightValue ->
            val weight = normalizeU64(weightValue, "authorityWeights[$index]")
            require(weight != BigInteger.ZERO) { "authorityWeights[$index] must not be zero" }
            weight
        }
        val computedTotalWeight = weights.fold(BigInteger.ZERO) { sum, weight -> sum + weight }
        require(totalWeightValue == computedTotalWeight) { "totalWeight must match authorityWeights" }
        val signerIndices = substrateAuthoritySignerIndices(signersBitmap, publicKeys.size)
        require(signatures.size == signerIndices.size) {
            "signatures length must match signersBitmap"
        }
        val computedSignedWeight = signerIndices.fold(BigInteger.ZERO) { sum, index -> sum + weights[index] }
        require(signedWeightValue == computedSignedWeight) {
            "signedWeight must match signersBitmap"
        }
        require(signedWeightValue * BigInteger.valueOf(3) > totalWeightValue * BigInteger.valueOf(2)) {
            "signedWeight must be greater than two thirds of totalWeight"
        }
        signatures.forEachIndexed { index, signature ->
            require(signature.size == 64) { "signatures[$index] must be 64 bytes" }
            require(!isZero(signature)) { "signatures[$index] must not be all zero" }
        }
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU64Le(out, totalWeightValue)
        writeU64Le(out, signedWeightValue)
        out.write(precommitHashBytes)
        writeU32Le(out, publicKeys.size)
        publicKeys.forEach { publicKeyBytes ->
            writeVector(out, publicKeyBytes)
        }
        writeU32Le(out, weights.size)
        weights.forEach { weight -> writeU64Le(out, weight) }
        writeVector(out, signersBitmap)
        writeU32Le(out, signatures.size)
        signatures.forEach { signature -> writeVector(out, signature) }
        return out.toByteArray()
    }

    @JvmStatic
    fun canonicalSubstrateAuthoritySetTransitionJustificationBytes(
        version: Int,
        sourceDomain: Int,
        fromGrandpaSetId: String,
        toGrandpaSetId: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentAuthoritySetHash: String,
        nextAuthoritySetHash: String,
        nextAuthoritySetPayload: ByteArray,
        nextAuthoritySetPayloadHash: String,
        transitionMessageHash: String,
        proofVersion: Int,
        totalWeight: String,
        signedWeight: String,
        authorityPublicKeys: List<String>,
        authorityWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
    ): ByteArray {
        requireV1Version(version, "Substrate authority-set transition justification version")
        require(substrateAuthoritySetPayloadHash(nextAuthoritySetPayload) == normalizeHex32(nextAuthoritySetPayloadHash)) {
            "nextAuthoritySetPayloadHash must match nextAuthoritySetPayload"
        }
        require(substrateAuthoritySetHashFromPayload(nextAuthoritySetPayload) == normalizeHex32(nextAuthoritySetHash)) {
            "nextAuthoritySetHash must match nextAuthoritySetPayload"
        }
        val parentHash = substrateAuthoritySetHashFromPayload(authorityPublicKeys, authorityWeights)
        require(parentHash == normalizeHex32(parentAuthoritySetHash)) {
            "parentAuthoritySetHash must match grandpaJustification authority set"
        }
        val out = ByteArrayOutputStream()
        out.write(version)
        writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"))
        writeU64Le(out, normalizeU64(fromGrandpaSetId, "fromGrandpaSetId"))
        writeU64Le(out, normalizeU64(toGrandpaSetId, "toGrandpaSetId"))
        writeU64Le(out, normalizeU64(transitionBlockNumber, "transitionBlockNumber"))
        out.write(hex32Bytes(transitionBlockHash, "transitionBlockHash"))
        out.write(hex32Bytes(parentAuthoritySetHash, "parentAuthoritySetHash"))
        out.write(hex32Bytes(nextAuthoritySetHash, "nextAuthoritySetHash"))
        writeVector(out, nextAuthoritySetPayload)
        out.write(hex32Bytes(nextAuthoritySetPayloadHash, "nextAuthoritySetPayloadHash"))
        out.write(hex32Bytes(transitionMessageHash, "transitionMessageHash"))
        out.write(hex32Bytes(parentHash, "parentAuthoritySetHash"))
        out.write(
            canonicalSubstrateGrandpaJustificationProofBytes(
                proofVersion,
                totalWeight,
                signedWeight,
                transitionMessageHash,
                authorityPublicKeys,
                authorityWeights,
                signersBitmap,
                signatures,
            ),
        )
        return out.toByteArray()
    }

    @JvmStatic
    fun substrateAuthoritySetTransitionJustificationHash(
        version: Int,
        sourceDomain: Int,
        fromGrandpaSetId: String,
        toGrandpaSetId: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentAuthoritySetHash: String,
        nextAuthoritySetHash: String,
        nextAuthoritySetPayload: ByteArray,
        nextAuthoritySetPayloadHash: String,
        transitionMessageHash: String,
        proofVersion: Int,
        totalWeight: String,
        signedWeight: String,
        authorityPublicKeys: List<String>,
        authorityWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
    ): String =
        hashHex(
            "sccp:substrate:authority-set-transition-justification:v1",
            canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                version,
                sourceDomain,
                fromGrandpaSetId,
                toGrandpaSetId,
                transitionBlockNumber,
                transitionBlockHash,
                parentAuthoritySetHash,
                nextAuthoritySetHash,
                nextAuthoritySetPayload,
                nextAuthoritySetPayloadHash,
                transitionMessageHash,
                proofVersion,
                totalWeight,
                signedWeight,
                authorityPublicKeys,
                authorityWeights,
                signersBitmap,
                signatures,
            ),
        )

    private fun writeVector(out: ByteArrayOutputStream, value: ByteArray) {
        writeU32Le(out, value.size)
        out.write(value)
    }

    private data class NormalizedTronWitnessSealProof(
        val version: Int,
        val totalWeight: BigInteger,
        val signedWeight: BigInteger,
        val solidBlockMessageHash: ByteArray,
        val witnessAddresses: List<ByteArray>,
        val witnessWeights: List<BigInteger>,
        val signersBitmap: ByteArray,
        val signatures: List<ByteArray>,
        val witnessScheduleHash: ByteArray,
    )

    private data class NormalizedTronWitnessScheduleTransitionMessage(
        val version: Int,
        val sourceDomain: Int,
        val fromWitnessScheduleEpoch: BigInteger,
        val toWitnessScheduleEpoch: BigInteger,
        val transitionBlockNumber: BigInteger,
        val transitionBlockHash: ByteArray,
        val parentWitnessScheduleHash: ByteArray,
        val nextWitnessScheduleHash: ByteArray,
        val nextWitnessSchedulePayloadHash: ByteArray,
    )

    private fun tronWitnessSealSignerIndices(bitmap: ByteArray, rosterLength: Int): List<Int> {
        require(rosterLength > 0 && bitmap.size == (rosterLength + 7) / 8) {
            "signersBitmap length must match witness roster"
        }
        val indices = mutableListOf<Int>()
        bitmap.forEachIndexed { byteIndex, value ->
            val unsigned = value.toInt() and 0xff
            for (bitIndex in 0 until 8) {
                if (((unsigned ushr bitIndex) and 1) == 0) continue
                val witnessIndex = byteIndex * 8 + bitIndex
                require(witnessIndex < rosterLength) {
                    "signersBitmap sets a bit outside the witness roster"
                }
                indices += witnessIndex
            }
        }
        require(indices.isNotEmpty()) { "signersBitmap must select at least one witness" }
        return indices
    }

    private fun normalizeTronWitnessSealProof(
        version: Int,
        totalWeight: String,
        signedWeight: String,
        solidBlockMessageHash: String,
        witnessAddresses: List<String>,
        witnessWeights: List<String>,
        signersBitmap: ByteArray,
        signatures: List<ByteArray>,
    ): NormalizedTronWitnessSealProof {
        requireV1Version(version, "TRON witness seal version")
        val totalWeightValue = normalizeU64(totalWeight, "totalWeight")
        val signedWeightValue = normalizeU64(signedWeight, "signedWeight")
        require(totalWeightValue != BigInteger.ZERO) { "totalWeight must not be zero" }
        require(signedWeightValue != BigInteger.ZERO) { "signedWeight must not be zero" }
        val messageHash = nonZeroHex32Bytes(solidBlockMessageHash, "solidBlockMessageHash")
        require(witnessAddresses.isNotEmpty() && witnessAddresses.size == witnessWeights.size) {
            "witnessAddresses and witnessWeights must be non-empty equal-length arrays"
        }
        require(witnessAddresses.size <= TRON_MAX_WITNESSES) {
            "witnessAddresses must contain at most $TRON_MAX_WITNESSES entries"
        }
        val normalizedAddresses = mutableListOf<ByteArray>()
        val normalizedWeights = mutableListOf<BigInteger>()
        val seenAddresses = HashSet<String>()
        var computedTotalWeight = BigInteger.ZERO
        witnessAddresses.zip(witnessWeights).forEachIndexed { index, (addressValue, weightValue) ->
            val address = hexBytes(addressValue, "witnessAddresses[$index]", 21)
            require(isNonZeroTronAddress(address)) {
                "witnessAddresses[$index] must be a TRON 0x41-prefixed address"
            }
            require(seenAddresses.add(hexLower(address))) {
                "witnessAddresses[$index] must be unique"
            }
            val weight = normalizeU64(weightValue, "witnessWeights[$index]")
            require(weight != BigInteger.ZERO) { "witnessWeights[$index] must not be zero" }
            computedTotalWeight = computedTotalWeight.add(weight)
            require(computedTotalWeight <= MAX_U64) { "witnessWeights total must fit u64" }
            normalizedAddresses += address
            normalizedWeights += weight
        }
        require(computedTotalWeight == totalWeightValue) {
            "totalWeight must equal the witness weight sum"
        }
        val signerIndices = tronWitnessSealSignerIndices(signersBitmap, normalizedAddresses.size)
        require(signatures.size == signerIndices.size) {
            "signatures length must match signersBitmap"
        }
        var computedSignedWeight = BigInteger.ZERO
        val normalizedSignatures = mutableListOf<ByteArray>()
        signatures.forEachIndexed { signatureIndex, signature ->
            require(tronRecoverableSignatureIsCanonical(signature)) {
                "signatures[$signatureIndex] must be a canonical low-S 65-byte TRON signature"
            }
            val witnessIndex = signerIndices[signatureIndex]
            val recoveredSigner = tronRecoveredSignerAddress20(messageHash, signature)
            val expectedAddress = normalizedAddresses[witnessIndex]
            require(
                recoveredSigner != null &&
                    recoveredSigner.contentEquals(expectedAddress.copyOfRange(1, expectedAddress.size)),
            ) {
                "witness seal signature does not recover to declared signer"
            }
            computedSignedWeight = computedSignedWeight.add(normalizedWeights[witnessIndex])
            normalizedSignatures += signature.copyOf()
        }
        require(computedSignedWeight == signedWeightValue) {
            "signedWeight must equal the signersBitmap witness weight sum"
        }
        require(computedSignedWeight.multiply(BigInteger.valueOf(3)) > computedTotalWeight.multiply(BigInteger.valueOf(2))) {
            "signedWeight must exceed two thirds of totalWeight"
        }
        val witnessPayload = canonicalTronWitnessSchedulePayloadBytes(witnessAddresses, witnessWeights)
        return NormalizedTronWitnessSealProof(
            version = version,
            totalWeight = totalWeightValue,
            signedWeight = signedWeightValue,
            solidBlockMessageHash = messageHash,
            witnessAddresses = normalizedAddresses.map { it.copyOf() },
            witnessWeights = normalizedWeights.toList(),
            signersBitmap = signersBitmap.copyOf(),
            signatures = normalizedSignatures,
            witnessScheduleHash = hashBytes("sccp:tron:witness-schedule:v1", witnessPayload),
        )
    }

    private fun canonicalTronWitnessSealProofBytes(
        proof: NormalizedTronWitnessSealProof,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(proof.version)
        writeU64Le(out, proof.totalWeight)
        writeU64Le(out, proof.signedWeight)
        out.write(proof.solidBlockMessageHash)
        writeU32Le(out, proof.witnessAddresses.size)
        proof.witnessAddresses.forEach { writeVector(out, it) }
        writeU32Le(out, proof.witnessWeights.size)
        proof.witnessWeights.forEach { writeU64Le(out, it) }
        writeVector(out, proof.signersBitmap)
        writeU32Le(out, proof.signatures.size)
        proof.signatures.forEach { writeVector(out, it) }
        return out.toByteArray()
    }

    private fun normalizeTronWitnessScheduleTransitionMessage(
        version: Int,
        sourceDomain: Int,
        fromWitnessScheduleEpoch: String,
        toWitnessScheduleEpoch: String,
        transitionBlockNumber: String,
        transitionBlockHash: String,
        parentWitnessScheduleHash: String,
        nextWitnessScheduleHash: String,
        nextWitnessSchedulePayloadHash: String?,
        nextWitnessSchedulePayload: ByteArray?,
    ): NormalizedTronWitnessScheduleTransitionMessage {
        requireV1Version(version, "TRON witness-schedule transition message version")
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        require(normalizedSourceDomain == DOMAIN_TRON) { "sourceDomain must be TRON" }
        val fromEpoch = normalizeU64(fromWitnessScheduleEpoch, "fromWitnessScheduleEpoch")
        val toEpoch = normalizeU64(toWitnessScheduleEpoch, "toWitnessScheduleEpoch")
        require(fromEpoch.add(BigInteger.ONE) == toEpoch) {
            "toWitnessScheduleEpoch must equal fromWitnessScheduleEpoch + 1"
        }
        val blockNumber = normalizeU64(transitionBlockNumber, "transitionBlockNumber")
        require(blockNumber != BigInteger.ZERO) { "transitionBlockNumber must not be zero" }
        val transitionBlockHashBytes = nonZeroHex32Bytes(transitionBlockHash, "transitionBlockHash")
        val parentScheduleHash = nonZeroHex32Bytes(parentWitnessScheduleHash, "parentWitnessScheduleHash")
        val nextScheduleHash = nonZeroHex32Bytes(nextWitnessScheduleHash, "nextWitnessScheduleHash")
        val payloadHash = when {
            nextWitnessSchedulePayloadHash == null && nextWitnessSchedulePayload == null ->
                throw IllegalArgumentException(
                    "nextWitnessSchedulePayloadHash or nextWitnessSchedulePayload is required",
                )
            nextWitnessSchedulePayloadHash == null ->
                hex32Bytes(
                    tronWitnessSchedulePayloadHash(requireNotNull(nextWitnessSchedulePayload)),
                    "nextWitnessSchedulePayloadHash",
                )
            else -> nonZeroHex32Bytes(nextWitnessSchedulePayloadHash, "nextWitnessSchedulePayloadHash")
        }
        if (nextWitnessSchedulePayload != null) {
            val derivedPayloadHash = hex32Bytes(
                tronWitnessSchedulePayloadHash(nextWitnessSchedulePayload),
                "nextWitnessSchedulePayloadHash",
            )
            require(payloadHash.contentEquals(derivedPayloadHash)) {
                "nextWitnessSchedulePayloadHash must match nextWitnessSchedulePayload"
            }
            val derivedScheduleHash = hex32Bytes(
                tronWitnessScheduleHashFromPayload(nextWitnessSchedulePayload),
                "nextWitnessScheduleHash",
            )
            require(nextScheduleHash.contentEquals(derivedScheduleHash)) {
                "nextWitnessScheduleHash must match nextWitnessSchedulePayload"
            }
        }
        return NormalizedTronWitnessScheduleTransitionMessage(
            version = version,
            sourceDomain = normalizedSourceDomain,
            fromWitnessScheduleEpoch = fromEpoch,
            toWitnessScheduleEpoch = toEpoch,
            transitionBlockNumber = blockNumber,
            transitionBlockHash = transitionBlockHashBytes,
            parentWitnessScheduleHash = parentScheduleHash,
            nextWitnessScheduleHash = nextScheduleHash,
            nextWitnessSchedulePayloadHash = payloadHash,
        )
    }

    private fun canonicalTronWitnessScheduleTransitionMessageBytes(
        message: NormalizedTronWitnessScheduleTransitionMessage,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(message.version)
        writeU32Le(out, message.sourceDomain)
        writeU64Le(out, message.fromWitnessScheduleEpoch)
        writeU64Le(out, message.toWitnessScheduleEpoch)
        writeU64Le(out, message.transitionBlockNumber)
        out.write(message.transitionBlockHash)
        out.write(message.parentWitnessScheduleHash)
        out.write(message.nextWitnessScheduleHash)
        out.write(message.nextWitnessSchedulePayloadHash)
        return out.toByteArray()
    }

    private fun sourceAdapterVerifierProfile(sourceDomain: Int): Triple<String, Int, Int> =
        when (sourceDomain) {
            DOMAIN_ETH -> Triple("eth", 1, 1)
            DOMAIN_BSC -> Triple("bsc", 2, 2)
            DOMAIN_SOL -> Triple("sol", 3, 3)
            DOMAIN_TON -> Triple("ton", 4, 4)
            DOMAIN_TRON -> Triple("tron", 5, 5)
            DOMAIN_SORA_KUSAMA -> Triple("sora-kusama", 6, 6)
            DOMAIN_SORA_POLKADOT -> Triple("sora-polkadot", 6, 6)
            DOMAIN_SORA2 -> Triple("sora2", 6, 6)
            else -> throw IllegalArgumentException(
                "sourceDomain is not a supported SCCP source-adapter lane",
            )
        }

    private data class DestinationBindingProfile(
        val verifierTarget: Int,
        val backendFamily: Int,
        val bindingKey: String,
        val manifestSeed: String,
        val verifierBackend: String,
    )

    private fun destinationBindingProfile(targetDomain: Int): DestinationBindingProfile =
        when (targetDomain) {
            DOMAIN_SOL -> DestinationBindingProfile(
                2,
                2,
                "sccp:0:3:sol:solana-program-v1:2",
                "iroha:sccp:bridge-proof:message:stark-fri:v1:sol",
                "solana-program-v1",
            )
            DOMAIN_TON -> DestinationBindingProfile(
                3,
                3,
                "sccp:0:4:ton:ton-contract-v1:3",
                "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                "ton-contract-v1",
            )
            DOMAIN_SORA_KUSAMA -> DestinationBindingProfile(
                5,
                5,
                "sccp:0:6:sora-kusama:substrate-runtime-v1:5",
                "iroha:sccp:bridge-proof:message:stark-fri:v1:sora-kusama",
                "substrate-runtime-v1",
            )
            DOMAIN_SORA_POLKADOT -> DestinationBindingProfile(
                5,
                5,
                "sccp:0:7:sora-polkadot:substrate-runtime-v1:5",
                "iroha:sccp:bridge-proof:message:stark-fri:v1:sora-polkadot",
                "substrate-runtime-v1",
            )
            DOMAIN_SORA2 -> DestinationBindingProfile(
                5,
                5,
                "sccp:0:8:sora2:substrate-runtime-v1:5",
                "iroha:sccp:bridge-proof:message:stark-fri:v1:sora2",
                "substrate-runtime-v1",
            )
            else -> throw IllegalArgumentException(
                "targetDomain is not a supported native SCCP destination lane",
            )
        }

    private data class SourceRecordProfile(
        val chain: String,
        val proofPlan: Int,
        val finalityModel: Int,
        val sourceTrustAnchorId: String,
        val consensusVerifierId: String,
        val messageInclusionVerifierId: String,
        val finalityPolicyId: String,
        val sourceStateVerifierId: String = "",
        val sourceBridgeEmitterId: String = "",
        val requiresSourceBridge: Boolean = false,
        val requiresSourceBridgeConfig: Boolean = false,
    )

    private data class NormalizedSourceMaterial(
        val sourceDomain: Int,
        val profile: SourceRecordProfile,
        val sourceTrustAnchorHash: ByteArray,
        val consensusVerifierHash: ByteArray,
        val messageInclusionVerifierHash: ByteArray,
        val finalityPolicyHash: ByteArray,
        val sourceStateVerifierHash: ByteArray,
        val sourceBridgeEmitterAddress: ByteArray,
        val sourceBridgeEmitterCodeHash: ByteArray,
        val sourceBridgeNetworkId: ByteArray,
        val sourceBridgeOwnerAddress: ByteArray,
        val sourceBridgeConfigHash: ByteArray,
    )

    private fun sourceRecordProfile(sourceDomain: Int): SourceRecordProfile {
        val (chain, proofPlan, finalityModel) = sourceAdapterVerifierProfile(sourceDomain)
        return when (sourceDomain) {
            DOMAIN_ETH -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:eth:source-trust-anchor:ethereum-mainnet-beacon-finalized-checkpoint:v1",
                "sccp:eth:consensus-verifier:beacon-sync-committee-execution-header-mainnet:v1",
                "sccp:eth:message-inclusion-verifier:execution-receipt-trie-branch-mainnet:v1",
                "sccp:eth:finality-policy:beacon-finalized-checkpoint-mainnet:v1",
                sourceBridgeEmitterId = "sccp:eth:source-bridge-emitter:ethereum-mainnet:v1",
                requiresSourceBridge = true,
            )
            DOMAIN_BSC -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:bsc:source-trust-anchor:bsc-mainnet-validator-set:v1",
                "sccp:bsc:consensus-verifier:validator-set-seal-mainnet:v1",
                "sccp:bsc:message-inclusion-verifier:receipt-trie-branch-mainnet:v1",
                "sccp:bsc:finality-policy:validator-set-finality-mainnet:v1",
                sourceBridgeEmitterId = "sccp:bsc:source-bridge-emitter:bsc-mainnet:v1",
                requiresSourceBridge = true,
            )
            DOMAIN_SOL -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:sol:source-trust-anchor:solana-mainnet-beta-genesis:v1",
                "sccp:sol:consensus-verifier:finalized-slot-bankhash-mainnet-beta:v1",
                "sccp:sol:message-inclusion-verifier:transaction-status-root-branch:v1",
                "sccp:sol:finality-policy:finalized-slot-mainnet-beta:v1",
                sourceStateVerifierId = "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1",
            )
            DOMAIN_TON -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1",
                "sccp:ton:consensus-verifier:masterchain-block-proof:v1",
                "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1",
                "sccp:ton:finality-policy:masterchain-finality:v1",
                sourceStateVerifierId = "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1",
            )
            DOMAIN_TRON -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1",
                "sccp:tron:consensus-verifier:dpos-solid-block-mainnet:v1",
                "sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1",
                "sccp:tron:finality-policy:solid-block-mainnet:v1",
                sourceBridgeEmitterId = "sccp:tron:source-bridge-emitter:tron-mainnet:v1",
                requiresSourceBridge = true,
                requiresSourceBridgeConfig = true,
            )
            DOMAIN_SORA_KUSAMA -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:sora-kusama:source-trust-anchor:grandpa-authority-set:v1",
                "sccp:sora-kusama:consensus-verifier:grandpa-finalized-header:v1",
                "sccp:sora-kusama:message-inclusion-verifier:events-storage-proof:v1",
                "sccp:sora-kusama:finality-policy:grandpa-finality:v1",
                sourceStateVerifierId = "sccp:sora-kusama:source-state-verifier:runtime-storage-proof:v1",
            )
            DOMAIN_SORA_POLKADOT -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:sora-polkadot:source-trust-anchor:grandpa-authority-set:v1",
                "sccp:sora-polkadot:consensus-verifier:grandpa-finalized-header:v1",
                "sccp:sora-polkadot:message-inclusion-verifier:events-storage-proof:v1",
                "sccp:sora-polkadot:finality-policy:grandpa-finality:v1",
                sourceStateVerifierId = "sccp:sora-polkadot:source-state-verifier:runtime-storage-proof:v1",
            )
            DOMAIN_SORA2 -> SourceRecordProfile(
                chain,
                proofPlan,
                finalityModel,
                "sccp:sora2:source-trust-anchor:grandpa-authority-set:v1",
                "sccp:sora2:consensus-verifier:grandpa-finalized-header:v1",
                "sccp:sora2:message-inclusion-verifier:events-storage-proof:v1",
                "sccp:sora2:finality-policy:grandpa-finality:v1",
                sourceStateVerifierId = "sccp:sora2:source-state-verifier:runtime-storage-proof:v1",
            )
            else -> throw IllegalArgumentException(
                "sourceDomain is not a supported SCCP source material lane",
            )
        }
    }

    private fun rejectTonTemplateSourceMaterialComponent(
        sourceDomain: Int,
        value: ByteArray,
        label: String,
    ) {
        if (sourceDomain != DOMAIN_TON) return
        val template = when (label) {
            "sourceTrustAnchorHash" -> TON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH
            "consensusVerifierHash" -> TON_TEMPLATE_CONSENSUS_VERIFIER_HASH
            "messageInclusionVerifierHash" -> TON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH
            "finalityPolicyHash" -> TON_TEMPLATE_FINALITY_POLICY_HASH
            "sourceStateVerifierHash" -> TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH
            else -> return
        }
        require(!value.contentEquals(template)) {
            if (label == "sourceStateVerifierHash") {
                "sourceStateVerifierHash must not be the TON template verifier hash"
            } else {
                "$label must not be the TON template component hash"
            }
        }
    }

    private fun rejectSolanaTemplateSourceMaterialComponent(
        sourceDomain: Int,
        value: ByteArray,
        label: String,
    ) {
        if (sourceDomain != DOMAIN_SOL) return
        val template = when (label) {
            "sourceTrustAnchorHash" -> SOLANA_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH
            "consensusVerifierHash" -> SOLANA_TEMPLATE_CONSENSUS_VERIFIER_HASH
            "messageInclusionVerifierHash" -> SOLANA_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH
            "finalityPolicyHash" -> SOLANA_TEMPLATE_FINALITY_POLICY_HASH
            "sourceStateVerifierHash" -> SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH
            else -> return
        }
        require(!value.contentEquals(template)) {
            if (label == "sourceStateVerifierHash") {
                "sourceStateVerifierHash must not be the Solana template verifier hash"
            } else {
                "$label must not be the Solana template component hash"
            }
        }
    }

    private fun rejectTronTemplateSourceMaterialComponent(
        sourceDomain: Int,
        value: ByteArray,
        label: String,
    ) {
        if (sourceDomain != DOMAIN_TRON) return
        val template = when (label) {
            "sourceTrustAnchorHash" -> TRON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH
            "consensusVerifierHash" -> TRON_TEMPLATE_CONSENSUS_VERIFIER_HASH
            "messageInclusionVerifierHash" -> TRON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH
            "finalityPolicyHash" -> TRON_TEMPLATE_FINALITY_POLICY_HASH
            else -> return
        }
        require(!value.contentEquals(template)) {
            "$label must not be the TRON template component hash"
        }
    }

    private fun abiWordAddress20(value: ByteArray, label: String): ByteArray {
        require(value.size == 20) { "$label must be 20 bytes" }
        val out = ByteArray(32)
        System.arraycopy(value, 0, out, 12, 20)
        return out
    }

    private fun abiWordBytes21(value: ByteArray, label: String): ByteArray {
        require(value.size == 21) { "$label must be 21 bytes" }
        val out = ByteArray(32)
        System.arraycopy(value, 0, out, 11, 21)
        return out
    }

    internal fun tronBase58CheckPayload(value: String, field: String): ByteArray {
        require(value.isNotEmpty()) { "$field must not be empty" }
        require(value.trim() == value) { "$field must be a canonical Base58Check address" }
        val decoded = base58Decode(value, field)
        require(decoded.size == 25) { "$field must be a TRON Base58Check address" }
        val payload = decoded.copyOfRange(0, 21)
        val checksum = decoded.copyOfRange(21, 25)
        val expectedChecksum = sha256(sha256(payload)).copyOfRange(0, 4)
        require(checksum.contentEquals(expectedChecksum) && isNonZeroTronAddress(payload)) {
            "$field must be a valid non-zero TRON Base58Check address"
        }
        return payload
    }

    private fun base58Decode(value: String, field: String): ByteArray {
        require(value.isNotEmpty()) { "$field must not be empty" }
        val bytes = ArrayList<Int>()
        value.forEach { char ->
            val digit = BASE58_ALPHABET.indexOf(char)
            require(digit >= 0) { "$field must be Base58Check" }
            var carry = digit
            for (index in bytes.indices.reversed()) {
                carry += bytes[index] * 58
                bytes[index] = carry and 0xff
                carry = carry ushr 8
            }
            while (carry > 0) {
                bytes.add(0, carry and 0xff)
                carry = carry ushr 8
            }
        }
        var leadingZeroCount = 0
        while (leadingZeroCount < value.length && value[leadingZeroCount] == '1') {
            leadingZeroCount += 1
        }
        val out = ByteArray(leadingZeroCount + bytes.size)
        for (index in bytes.indices) {
            out[leadingZeroCount + index] = bytes[index].toByte()
        }
        return out
    }

    private fun tronSourceBridgeConfigHash(
        sourceDomain: Int,
        bridgeAddress: ByteArray,
        networkId: ByteArray,
        ownerAddress: ByteArray,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(keccak256(TRON_SOURCE_BRIDGE_CONFIG_LABEL))
        out.write(abiWordAddress20(bridgeAddress, "sourceBridgeEmitterAddress"))
        out.write(networkId)
        out.write(abiWordU32(sourceDomain, "sourceDomain"))
        out.write(abiWordU32(DOMAIN_SORA, "targetDomain"))
        out.write(abiWordAddress20(ownerAddress, "sourceBridgeOwnerAddress"))
        return keccak256(out.toByteArray())
    }

    private fun normalizeSourceMaterial(
        sourceDomain: Int,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        sourceStateVerifierHash: String?,
        bridgeAddress: String?,
        sourceBridgeEmitterCodeHash: String?,
        networkId: String?,
        ownerAddress: String?,
        configHash: String?,
    ): NormalizedSourceMaterial {
        val normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain")
        val profile = sourceRecordProfile(normalizedSourceDomain)
        val normalizedSourceStateVerifierHash = if (profile.sourceStateVerifierId.isNotEmpty()) {
            nonZeroHex32Bytes(requireNotNull(sourceStateVerifierHash) {
                "sourceStateVerifierHash is required"
            }, "sourceStateVerifierHash")
        } else {
            require(sourceStateVerifierHash == null) {
                "sourceStateVerifierHash is not used for sourceDomain"
            }
            ByteArray(32)
        }
        rejectTonTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedSourceStateVerifierHash,
            "sourceStateVerifierHash",
        )
        rejectSolanaTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedSourceStateVerifierHash,
            "sourceStateVerifierHash",
        )
        val normalizedSourceTrustAnchorHash = nonZeroHex32Bytes(sourceTrustAnchorHash, "sourceTrustAnchorHash")
        rejectTonTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedSourceTrustAnchorHash,
            "sourceTrustAnchorHash",
        )
        rejectSolanaTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedSourceTrustAnchorHash,
            "sourceTrustAnchorHash",
        )
        rejectTronTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedSourceTrustAnchorHash,
            "sourceTrustAnchorHash",
        )
        val normalizedConsensusVerifierHash = nonZeroHex32Bytes(consensusVerifierHash, "consensusVerifierHash")
        rejectTonTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedConsensusVerifierHash,
            "consensusVerifierHash",
        )
        rejectSolanaTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedConsensusVerifierHash,
            "consensusVerifierHash",
        )
        rejectTronTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedConsensusVerifierHash,
            "consensusVerifierHash",
        )
        val normalizedMessageInclusionVerifierHash = nonZeroHex32Bytes(
            messageInclusionVerifierHash,
            "messageInclusionVerifierHash",
        )
        rejectTonTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedMessageInclusionVerifierHash,
            "messageInclusionVerifierHash",
        )
        rejectSolanaTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedMessageInclusionVerifierHash,
            "messageInclusionVerifierHash",
        )
        rejectTronTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedMessageInclusionVerifierHash,
            "messageInclusionVerifierHash",
        )
        val normalizedFinalityPolicyHash = nonZeroHex32Bytes(finalityPolicyHash, "finalityPolicyHash")
        rejectTonTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedFinalityPolicyHash,
            "finalityPolicyHash",
        )
        rejectSolanaTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedFinalityPolicyHash,
            "finalityPolicyHash",
        )
        rejectTronTemplateSourceMaterialComponent(
            normalizedSourceDomain,
            normalizedFinalityPolicyHash,
            "finalityPolicyHash",
        )
        val normalizedSourceBridgeEmitterAddress = if (profile.requiresSourceBridge) {
            nonZeroHexBytes(requireNotNull(bridgeAddress) { "bridgeAddress is required" }, "bridgeAddress", 20)
        } else {
            require(bridgeAddress == null) {
                "sourceBridgeEmitterAddress is not used for sourceDomain"
            }
            ByteArray(0)
        }
        val normalizedSourceBridgeEmitterCodeHash = if (profile.requiresSourceBridge) {
            nonZeroHex32Bytes(requireNotNull(sourceBridgeEmitterCodeHash) {
                "sourceBridgeEmitterCodeHash is required"
            }, "sourceBridgeEmitterCodeHash")
        } else {
            require(sourceBridgeEmitterCodeHash == null) {
                "sourceBridgeEmitterCodeHash is not used for sourceDomain"
            }
            ByteArray(32)
        }
        val normalizedSourceBridgeNetworkId = if (profile.requiresSourceBridgeConfig) {
            nonZeroHex32Bytes(requireNotNull(networkId) { "networkId is required" }, "networkId")
        } else {
            require(networkId == null) {
                "sourceBridgeNetworkId is not used for sourceDomain"
            }
            ByteArray(32)
        }
        val normalizedSourceBridgeOwnerAddress = if (profile.requiresSourceBridgeConfig) {
            nonZeroHexBytes(requireNotNull(ownerAddress) { "ownerAddress is required" }, "ownerAddress", 20)
        } else {
            require(ownerAddress == null) {
                "sourceBridgeOwnerAddress is not used for sourceDomain"
            }
            ByteArray(0)
        }
        val normalizedSourceBridgeConfigHash = if (profile.requiresSourceBridgeConfig) {
            nonZeroHex32Bytes(requireNotNull(configHash) { "configHash is required" }, "configHash")
        } else {
            require(configHash == null) {
                "sourceBridgeConfigHash is not used for sourceDomain"
            }
            ByteArray(32)
        }
        if (
            normalizedSourceDomain == DOMAIN_TRON &&
            !normalizedSourceBridgeConfigHash.contentEquals(
                tronSourceBridgeConfigHash(
                    normalizedSourceDomain,
                    normalizedSourceBridgeEmitterAddress,
                    normalizedSourceBridgeNetworkId,
                    normalizedSourceBridgeOwnerAddress,
                )
            )
        ) {
            throw IllegalArgumentException("sourceBridgeConfigHash must match TRON source bridge config fields")
        }
        requirePairwiseNonzeroRoleHashSeparation(
            listOf(
                "sourceTrustAnchorHash" to normalizedSourceTrustAnchorHash,
                "consensusVerifierHash" to normalizedConsensusVerifierHash,
                "messageInclusionVerifierHash" to normalizedMessageInclusionVerifierHash,
                "finalityPolicyHash" to normalizedFinalityPolicyHash,
                "sourceStateVerifierHash" to normalizedSourceStateVerifierHash,
                "sourceBridgeEmitterCodeHash" to normalizedSourceBridgeEmitterCodeHash,
                "sourceBridgeNetworkId" to normalizedSourceBridgeNetworkId,
                "sourceBridgeConfigHash" to normalizedSourceBridgeConfigHash,
            ),
            "SCCP source verifier material",
        )
        return NormalizedSourceMaterial(
            sourceDomain = normalizedSourceDomain,
            profile = profile,
            sourceTrustAnchorHash = normalizedSourceTrustAnchorHash,
            consensusVerifierHash = normalizedConsensusVerifierHash,
            messageInclusionVerifierHash = normalizedMessageInclusionVerifierHash,
            finalityPolicyHash = normalizedFinalityPolicyHash,
            sourceStateVerifierHash = normalizedSourceStateVerifierHash,
            sourceBridgeEmitterAddress = normalizedSourceBridgeEmitterAddress,
            sourceBridgeEmitterCodeHash = normalizedSourceBridgeEmitterCodeHash,
            sourceBridgeNetworkId = normalizedSourceBridgeNetworkId,
            sourceBridgeOwnerAddress = normalizedSourceBridgeOwnerAddress,
            sourceBridgeConfigHash = normalizedSourceBridgeConfigHash,
        )
    }

    private fun requirePairwiseNonzeroRoleHashSeparation(
        roleHashes: List<Pair<String, ByteArray>>,
        label: String,
    ) {
        for (index in roleHashes.indices) {
            val (roleField, roleHash) = roleHashes[index]
            if (isZero(roleHash)) continue
            for (otherIndex in index + 1 until roleHashes.size) {
                val (otherRoleField, otherRoleHash) = roleHashes[otherIndex]
                require(isZero(otherRoleHash) || !roleHash.contentEquals(otherRoleHash)) {
                    "$label hashes must be role-separated: $otherRoleField matches $roleField"
                }
            }
        }
    }

    private fun writeSourceMaterialFields(out: ByteArrayOutputStream, material: NormalizedSourceMaterial) {
        out.write(1)
        writeU32Le(out, material.sourceDomain)
        writeVector(out, material.profile.chain.toByteArray(StandardCharsets.UTF_8))
        out.write(material.profile.proofPlan)
        out.write(material.profile.finalityModel)
        writeVector(out, SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.toByteArray(StandardCharsets.UTF_8))
        writeSourceComponentFields(out, material)
    }

    private fun writeSourceComponentFields(out: ByteArrayOutputStream, material: NormalizedSourceMaterial) {
        writeVector(out, material.profile.sourceTrustAnchorId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.sourceTrustAnchorHash)
        writeVector(out, material.profile.consensusVerifierId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.consensusVerifierHash)
        writeVector(out, material.profile.messageInclusionVerifierId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.messageInclusionVerifierHash)
        writeVector(out, material.profile.finalityPolicyId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.finalityPolicyHash)
        writeVector(out, material.profile.sourceStateVerifierId.toByteArray(StandardCharsets.UTF_8))
        out.write(material.sourceStateVerifierHash)
        writeVector(out, material.profile.sourceBridgeEmitterId.toByteArray(StandardCharsets.UTF_8))
        writeVector(out, material.sourceBridgeEmitterAddress)
        out.write(material.sourceBridgeEmitterCodeHash)
        out.write(material.sourceBridgeNetworkId)
        writeVector(out, material.sourceBridgeOwnerAddress)
        out.write(material.sourceBridgeConfigHash)
    }

    private fun writeSourceAdapterDeploymentSolanaAuditFields(
        out: ByteArrayOutputStream,
        sourceDomain: Int,
        towerReplayVerifierHash: String?,
        fullAccountsdbLatticeVerifierHash: String?,
        bankForkChoiceVerifierHash: String?,
        existingRoleHashes: List<ByteArray>,
    ) {
        val verifierHashes = listOf(
            SOLANA_MAINNET_TOWER_REPLAY_VERIFIER_ID_V1 to (
                towerReplayVerifierHash?.let { hex32Bytes(it, "solanaTowerReplayVerifierHash") }
                    ?: ByteArray(32)
                ),
            SOLANA_MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1 to (
                fullAccountsdbLatticeVerifierHash?.let {
                    hex32Bytes(it, "solanaFullAccountsdbLatticeVerifierHash")
                } ?: ByteArray(32)
                ),
            SOLANA_MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1 to (
                bankForkChoiceVerifierHash?.let { hex32Bytes(it, "solanaBankForkChoiceVerifierHash") }
                    ?: ByteArray(32)
                ),
        )
        val nonzeroCount = verifierHashes.count { (_, hash) -> !isZero(hash) }
        if (nonzeroCount == 0) {
            return
        }
        require(sourceDomain == DOMAIN_SOL && nonzeroCount == verifierHashes.size) {
            "Solana audit verifier hashes must be all non-zero and only used for Solana deployments"
        }
        requireSolanaFullLightClientAuditRoleSeparation(verifierHashes, existingRoleHashes)
        out.write(1)
        verifierHashes.forEach { (verifierId, verifierHash) ->
            writeVector(out, verifierId.toByteArray(StandardCharsets.UTF_8))
            out.write(verifierHash)
        }
    }

    private fun requireSolanaFullLightClientAuditRoleSeparation(
        verifierHashes: List<Pair<String, ByteArray>>,
        existingRoleHashes: List<ByteArray>,
    ) {
        for (index in verifierHashes.indices) {
            val (verifierId, verifierHash) = verifierHashes[index]
            for (otherIndex in index + 1 until verifierHashes.size) {
                require(!verifierHash.contentEquals(verifierHashes[otherIndex].second)) {
                    "Solana full-light-client audit verifier hashes must be role-separated"
                }
            }
            val templateHashes = listOf(
                SOLANA_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH,
                SOLANA_TEMPLATE_CONSENSUS_VERIFIER_HASH,
                SOLANA_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH,
                SOLANA_TEMPLATE_FINALITY_POLICY_HASH,
                SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH,
            )
            require(templateHashes.none { verifierHash.contentEquals(it) }) {
                "Solana full-light-client audit verifier hash must not reuse built-in template material: $verifierId"
            }
            for (existingRoleHash in existingRoleHashes) {
                require(isZero(existingRoleHash) || !verifierHash.contentEquals(existingRoleHash)) {
                    "Solana full-light-client audit verifier hash must not reuse existing source-adapter material: $verifierId"
                }
            }
        }
    }

    private fun writeSourceAdapterDeploymentTonAuditFields(
        out: ByteArrayOutputStream,
        sourceDomain: Int,
        masterchainConfigVerifierHash: String?,
        validatorSetTransitionVerifierHash: String?,
        shardAccountsDictionaryVerifierHash: String?,
        existingRoleHashes: List<ByteArray>,
    ) {
        val verifierHashes = listOf(
            TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1 to (
                masterchainConfigVerifierHash?.let { hex32Bytes(it, "tonMasterchainConfigVerifierHash") }
                    ?: ByteArray(32)
                ),
            TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1 to (
                validatorSetTransitionVerifierHash?.let {
                    hex32Bytes(it, "tonValidatorSetTransitionVerifierHash")
                } ?: ByteArray(32)
                ),
            TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1 to (
                shardAccountsDictionaryVerifierHash?.let {
                    hex32Bytes(it, "tonShardAccountsDictionaryVerifierHash")
                } ?: ByteArray(32)
                ),
        )
        val nonzeroCount = verifierHashes.count { (_, hash) -> !isZero(hash) }
        if (nonzeroCount == 0) {
            return
        }
        require(sourceDomain == DOMAIN_TON && nonzeroCount == verifierHashes.size) {
            "TON audit verifier hashes must be all non-zero and only used for TON deployments"
        }
        requireTonFullLightClientAuditRoleSeparation(verifierHashes, existingRoleHashes)
        out.write(2)
        verifierHashes.forEach { (verifierId, verifierHash) ->
            writeVector(out, verifierId.toByteArray(StandardCharsets.UTF_8))
            out.write(verifierHash)
        }
    }

    private fun requireTonFullLightClientAuditRoleSeparation(
        verifierHashes: List<Pair<String, ByteArray>>,
        existingRoleHashes: List<ByteArray>,
    ) {
        for (index in verifierHashes.indices) {
            val (verifierId, verifierHash) = verifierHashes[index]
            for (otherIndex in index + 1 until verifierHashes.size) {
                require(!verifierHash.contentEquals(verifierHashes[otherIndex].second)) {
                    "TON full-light-client audit verifier hashes must be role-separated"
                }
            }
            val templateHashes = listOf(
                TON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH,
                TON_TEMPLATE_CONSENSUS_VERIFIER_HASH,
                TON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH,
                TON_TEMPLATE_FINALITY_POLICY_HASH,
                TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH,
            )
            require(templateHashes.none { verifierHash.contentEquals(it) }) {
                "TON full-light-client audit verifier hash must not reuse built-in template material: $verifierId"
            }
            for (existingRoleHash in existingRoleHashes) {
                require(isZero(existingRoleHash) || !verifierHash.contentEquals(existingRoleHash)) {
                    "TON full-light-client audit verifier hash must not reuse existing source-adapter material: $verifierId"
                }
            }
        }
    }

    private fun canonicalBscValidatorStorageProofBytes(proof: BscValidatorStorageProof): ByteArray {
        requireV1Version(proof.version, "BSC validator storage proof version")
        validateMptProofNodes(proof.storageProofNodes, "storageProofNodes")
        val out = ByteArrayOutputStream()
        out.write(proof.version)
        writeU32Le(out, proof.validatorIndex)
        out.write(hex32Bytes(proof.storageSlot, "storageSlot"))
        val storageValueHash = hex32Bytes(proof.storageValueHash, "storageValueHash")
        require(
            storageValueHash.contentEquals(
                hex32Bytes(bscValidatorSetStorageValueHash(proof.storageValue), "storageValueHash"),
            ),
        ) { "storageValueHash must match storageValue" }
        writeVector(out, proof.storageValue)
        out.write(storageValueHash)
        writeU32Le(out, proof.storageProofNodes.size)
        proof.storageProofNodes.forEach { node -> writeVector(out, node) }
        return out.toByteArray()
    }

    private fun normalizeTonValidatorSet(
        validatorPublicKeys: List<ByteArray>,
        validatorWeights: List<String>,
    ): Pair<List<ByteArray>, List<BigInteger>> {
        require(validatorPublicKeys.isNotEmpty() && validatorPublicKeys.size == validatorWeights.size) {
            "validatorPublicKeys and validatorWeights must be non-empty equal-length arrays"
        }
        require(validatorPublicKeys.size <= TON_MAX_VALIDATORS) {
            "validatorPublicKeys must contain at most $TON_MAX_VALIDATORS entries"
        }
        val seenPublicKeys = HashSet<String>()
        val publicKeys = ArrayList<ByteArray>(validatorPublicKeys.size)
        val weights = ArrayList<BigInteger>(validatorWeights.size)
        validatorPublicKeys.zip(validatorWeights).forEachIndexed { index, (publicKey, weightValue) ->
            require(publicKey.size == 32) { "validatorPublicKeys[$index] must be 32 bytes" }
            require(!isZero(publicKey)) { "validatorPublicKeys[$index] must not be zero" }
            require(seenPublicKeys.add(hexLower(publicKey))) { "validatorPublicKeys[$index] must be unique" }
            val weight = normalizeU64(weightValue, "validatorWeights[$index]")
            require(weight != BigInteger.ZERO) { "validatorWeights[$index] must not be zero" }
            publicKeys.add(publicKey)
            weights.add(weight)
        }
        return Pair(publicKeys, weights)
    }

    private fun validateTonValidatorSetPayload(payload: ByteArray) {
        var cursor = 0
        require(payload.size >= 5 && payload[cursor].toInt() == 1) {
            "validatorSetPayload must have version 1"
        }
        cursor += 1
        val count = readU32Le(payload, cursor)
        cursor += 4
        require(count > 0 && count <= TON_MAX_VALIDATORS && payload.size - cursor == count * 40) {
            "validatorSetPayload length is invalid"
        }
        val seenPublicKeys = HashSet<String>()
        for (index in 0 until count) {
            val publicKey = payload.copyOfRange(cursor, cursor + 32)
            cursor += 32
            require(!isZero(publicKey)) { "validatorPublicKeys[$index] must not be zero" }
            require(seenPublicKeys.add(hexLower(publicKey))) {
                "validatorPublicKeys[$index] must be unique"
            }
            val weight = readU64Le(payload, cursor)
            cursor += 8
            require(weight != BigInteger.ZERO) { "validatorWeights[$index] must not be zero" }
        }
        require(cursor == payload.size) { "validatorSetPayload has trailing bytes" }
    }

    private fun tonSignerIndicesFromBitmap(bitmap: ByteArray, rosterLength: Int): List<Int> {
        require(bitmap.size == (rosterLength + 7) / 8) {
            "signersBitmap length must match validatorPublicKeys"
        }
        val indices = ArrayList<Int>()
        for (byteIndex in bitmap.indices) {
            val value = bitmap[byteIndex].toInt() and 0xff
            for (bit in 0 until 8) {
                if (((value ushr bit) and 1) == 0) continue
                val index = byteIndex * 8 + bit
                require(index < rosterLength) { "signersBitmap must not set padding bits" }
                indices.add(index)
            }
        }
        return indices
    }

    private fun canonicalTonValidatorSignaturesProofBytes(proof: TonValidatorSignatureProof): ByteArray {
        val normalized = normalizeTonValidatorSet(proof.validatorPublicKeys, proof.validatorWeights)
        requireV1Version(proof.version, "TON validator signature proof version")
        val totalWeight = normalizeU64(proof.totalWeight, "totalWeight")
        val signedWeight = normalizeU64(proof.signedWeight, "signedWeight")
        val computedTotalWeight = normalized.second.fold(BigInteger.ZERO) { sum, weight -> sum + weight }
        require(totalWeight == computedTotalWeight) { "totalWeight must match validatorWeights" }
        val signerIndices = tonSignerIndicesFromBitmap(proof.signersBitmap, normalized.first.size)
        require(signerIndices.isNotEmpty()) { "signersBitmap must select at least one validator" }
        require(proof.signatures.size == signerIndices.size) { "signatures length must match signersBitmap" }
        val computedSignedWeight =
            signerIndices.fold(BigInteger.ZERO) { sum, index -> sum + normalized.second[index] }
        require(signedWeight == computedSignedWeight) { "signedWeight must match signersBitmap" }
        require(signedWeight * BigInteger.valueOf(3) > totalWeight * BigInteger.valueOf(2)) {
            "signedWeight must be greater than two thirds of totalWeight"
        }
        val out = ByteArrayOutputStream()
        out.write(proof.version)
        writeU64Le(out, totalWeight)
        writeU64Le(out, signedWeight)
        out.write(nonZeroHex32Bytes(proof.blockMessageHash, "blockMessageHash"))
        writeU32Le(out, normalized.first.size)
        normalized.first.forEach { publicKey -> writeVector(out, publicKey) }
        writeU32Le(out, normalized.second.size)
        normalized.second.forEach { weight -> writeU64Le(out, weight) }
        writeVector(out, proof.signersBitmap)
        writeU32Le(out, proof.signatures.size)
        proof.signatures.forEachIndexed { index, signature ->
            require(signature.size == 64) { "signatures[$index] must be 64 bytes" }
            require(!isZero(signature)) { "signatures[$index] must not be all zero" }
            writeVector(out, signature)
        }
        return out.toByteArray()
    }

    private fun rlpLengthPrefix(length: Int, shortOffset: Int, longOffset: Int): ByteArray {
        if (length < 56) {
            return byteArrayOf((shortOffset + length).toByte())
        }
        var remaining = length
        val lengthBytes = ArrayList<Byte>()
        while (remaining > 0) {
            lengthBytes.add(0, (remaining and 0xff).toByte())
            remaining = remaining ushr 8
        }
        val out = ByteArrayOutputStream()
        out.write(longOffset + lengthBytes.size)
        lengthBytes.forEach { byte -> out.write(byte.toInt() and 0xff) }
        return out.toByteArray()
    }

    private fun rlpBytes(value: ByteArray): ByteArray {
        if (value.size == 1 && (value[0].toInt() and 0xff) < 0x80) {
            return value.copyOf()
        }
        val out = ByteArrayOutputStream()
        out.write(rlpLengthPrefix(value.size, 0x80, 0xb7))
        out.write(value)
        return out.toByteArray()
    }

    private fun rlpList(fields: List<ByteArray>): ByteArray {
        val payload = ByteArrayOutputStream()
        fields.forEach { field -> payload.write(field) }
        val payloadBytes = payload.toByteArray()
        val out = ByteArrayOutputStream()
        out.write(rlpLengthPrefix(payloadBytes.size, 0xc0, 0xf7))
        out.write(payloadBytes)
        return out.toByteArray()
    }

    private fun validateTronMptProofNodes(nodes: List<ByteArray>) {
        validateMptProofNodes(nodes, "receiptTrieProofNodes")
    }

    private fun validateMptProofNodes(nodes: List<ByteArray>, field: String) {
        require(nodes.isNotEmpty() && nodes.size <= TRON_MAX_MPT_PROOF_NODES) {
            "$field must contain 1..$TRON_MAX_MPT_PROOF_NODES entries"
        }
        nodes.forEachIndexed { index, node ->
            require(node.isNotEmpty() && node.size <= TRON_MAX_MPT_NODE_BYTES) {
                "$field[$index] must contain 1..$TRON_MAX_MPT_NODE_BYTES bytes"
            }
        }
    }

    private fun validateTronTransactionMerkleBranch(branch: List<ByteArray>) {
        require(branch.size <= TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES) {
            "transactionMerkleBranch must contain at most $TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES entries"
        }
        branch.forEachIndexed { index, sibling ->
            require(sibling.size == 32) { "transactionMerkleBranch[$index] must be 32 bytes" }
        }
    }

    private fun tronTransactionMerkleRootFromBranch(
        transactionBytes: ByteArray,
        transactionIndex: BigInteger,
        transactionCount: BigInteger,
        transactionMerkleBranch: List<ByteArray>,
    ): ByteArray {
        var current = sha256(transactionBytes)
        var index = transactionIndex
        var count = transactionCount
        var branchCursor = 0
        while (count > BigInteger.ONE) {
            if (!index.testBit(0)) {
                if (index.add(BigInteger.ONE) < count) {
                    require(branchCursor < transactionMerkleBranch.size) {
                        "transactionMerkleBranch is too short for transactionIndex/count"
                    }
                    current = sszHashNode(current, transactionMerkleBranch[branchCursor])
                    branchCursor += 1
                }
            } else {
                require(branchCursor < transactionMerkleBranch.size) {
                    "transactionMerkleBranch is too short for transactionIndex/count"
                }
                current = sszHashNode(transactionMerkleBranch[branchCursor], current)
                branchCursor += 1
            }
            index = index.shiftRight(1)
            count = count.add(BigInteger.ONE).divide(BigInteger.valueOf(2))
        }
        require(branchCursor == transactionMerkleBranch.size) {
            "transactionMerkleBranch has unused siblings for transactionIndex/count"
        }
        return current
    }

    private fun tronSourceTransactionError(): IllegalArgumentException =
        IllegalArgumentException("transactionBytes must be a successful TRON TriggerSmartContract source call")

    private fun readProtobufBytesField(bytes: ByteArray, cursor: IntArray, label: String): ByteArray {
        val length = readCanonicalProtobufVarint(bytes, cursor, label).intValueExact()
        val end = cursor[0] + length
        require(length >= 0 && end <= bytes.size) { "$label contains truncated protobuf bytes field" }
        val value = bytes.copyOfRange(cursor[0], end)
        cursor[0] = end
        return value
    }

    private fun protobufFieldNumber(key: BigInteger, label: String): Int {
        try {
            return key.shiftRight(3).intValueExact()
        } catch (error: ArithmeticException) {
            throw IllegalArgumentException("$label protobuf field number is too large", error)
        }
    }

    private fun tronTransactionResultSuccess(result: ByteArray): Boolean {
        val cursor = intArrayOf(0)
        var feeSeen = false
        var retSeen = false
        var contractRetSeen = false
        while (cursor[0] < result.size) {
            val key = readCanonicalProtobufVarint(result, cursor, "transactionResult")
            val fieldNumber = protobufFieldNumber(key, "transactionResult")
            val wireType = key.and(BigInteger.valueOf(0x07L)).toInt()
            when {
                fieldNumber == 1 && wireType == 0 && !feeSeen -> {
                    feeSeen = true
                    readCanonicalProtobufVarint(result, cursor, "transactionResult")
                }
                fieldNumber == 2 && wireType == 0 && !retSeen -> {
                    retSeen = true
                    if (readCanonicalProtobufVarint(result, cursor, "transactionResult") != BigInteger.ZERO) return false
                }
                fieldNumber == 3 && wireType == 0 && !contractRetSeen -> {
                    contractRetSeen = true
                    if (readCanonicalProtobufVarint(result, cursor, "transactionResult") != BigInteger.ONE) return false
                }
                else -> return false
            }
        }
        return contractRetSeen
    }

    private fun readTronProtobufAnyValue(parameter: ByteArray): ByteArray? {
        val cursor = intArrayOf(0)
        var typeUrl: ByteArray? = null
        var value: ByteArray? = null
        while (cursor[0] < parameter.size) {
            val key = readCanonicalProtobufVarint(parameter, cursor, "triggerParameter")
            val fieldNumber = protobufFieldNumber(key, "triggerParameter")
            val wireType = key.and(BigInteger.valueOf(0x07L)).toInt()
            when {
                fieldNumber == 1 && wireType == 2 && typeUrl == null ->
                    typeUrl = readProtobufBytesField(parameter, cursor, "triggerParameter")
                fieldNumber == 2 && wireType == 2 && value == null ->
                    value = readProtobufBytesField(parameter, cursor, "triggerParameter")
                else -> return null
            }
        }
        return if (typeUrl?.contentEquals(TRON_TRIGGER_SMART_CONTRACT_TYPE_URL) == true) value else null
    }

    private fun tronTriggerSourceCallOwnerAddress(
        trigger: ByteArray,
        sourceEventDigest: ByteArray,
        expectedContractAddress: ByteArray?,
        expectedOwnerAddress: ByteArray?,
    ): ByteArray? {
        val cursor = intArrayOf(0)
        var ownerAddress: ByteArray? = null
        var contractAddress: ByteArray? = null
        var data: ByteArray? = null
        var callValueSeen = false
        var callTokenValueSeen = false
        var tokenIdSeen = false
        while (cursor[0] < trigger.size) {
            val key = readCanonicalProtobufVarint(trigger, cursor, "triggerContract")
            val fieldNumber = protobufFieldNumber(key, "triggerContract")
            val wireType = key.and(BigInteger.valueOf(0x07L)).toInt()
            when {
                fieldNumber == 1 && wireType == 2 && ownerAddress == null ->
                    ownerAddress = readProtobufBytesField(trigger, cursor, "triggerContract")
                fieldNumber == 2 && wireType == 2 && contractAddress == null ->
                    contractAddress = readProtobufBytesField(trigger, cursor, "triggerContract")
                fieldNumber == 3 && wireType == 0 && !callValueSeen -> {
                    callValueSeen = true
                    if (readCanonicalProtobufVarint(trigger, cursor, "triggerContract") != BigInteger.ZERO) return null
                }
                fieldNumber == 4 && wireType == 2 && data == null ->
                    data = readProtobufBytesField(trigger, cursor, "triggerContract")
                fieldNumber == 5 && wireType == 0 && !callTokenValueSeen -> {
                    callTokenValueSeen = true
                    if (readCanonicalProtobufVarint(trigger, cursor, "triggerContract") != BigInteger.ZERO) return null
                }
                fieldNumber == 6 && wireType == 0 && !tokenIdSeen -> {
                    tokenIdSeen = true
                    if (readCanonicalProtobufVarint(trigger, cursor, "triggerContract") != BigInteger.ZERO) return null
                }
                else -> return null
            }
        }
        val expectedCallData =
            keccak256(TRON_SOURCE_MESSAGE_CALL_ABI).copyOfRange(0, 4) +
                abiWordU32(DOMAIN_TRON, "sourceDomain") +
                abiWordU32(DOMAIN_SORA, "targetDomain") +
                sourceEventDigest
        val owner = ownerAddress ?: return null
        val contract = contractAddress ?: return null
        if (!isNonZeroTronAddress(owner) || !isNonZeroTronAddress(contract)) {
            return null
        }
        val ownerAddress20 = owner.copyOfRange(1, owner.size)
        val contractAddress20 = contract.copyOfRange(1, contract.size)
        if (expectedContractAddress != null && !contractAddress20.contentEquals(expectedContractAddress)) {
            return null
        }
        if (expectedOwnerAddress != null && !ownerAddress20.contentEquals(expectedOwnerAddress)) {
            return null
        }
        if (data?.contentEquals(expectedCallData) != true) {
            return null
        }
        return ownerAddress20
    }

    private fun tronContractSourceCallOwnerAddress(
        contract: ByteArray,
        sourceEventDigest: ByteArray,
        expectedContractAddress: ByteArray?,
        expectedOwnerAddress: ByteArray?,
    ): ByteArray? {
        val cursor = intArrayOf(0)
        var contractType: BigInteger? = null
        var parameter: ByteArray? = null
        while (cursor[0] < contract.size) {
            val key = readCanonicalProtobufVarint(contract, cursor, "transactionContract")
            val fieldNumber = protobufFieldNumber(key, "transactionContract")
            val wireType = key.and(BigInteger.valueOf(0x07L)).toInt()
            when {
                fieldNumber == 1 && wireType == 0 && contractType == null ->
                    contractType = readCanonicalProtobufVarint(contract, cursor, "transactionContract")
                fieldNumber == 2 && wireType == 2 && parameter == null ->
                    parameter = readProtobufBytesField(contract, cursor, "transactionContract")
                else -> return null
            }
        }
        val trigger = parameter?.let(::readTronProtobufAnyValue)
        return if (contractType == BigInteger.valueOf(31L) && trigger != null) {
            tronTriggerSourceCallOwnerAddress(
                trigger,
                sourceEventDigest,
                expectedContractAddress,
                expectedOwnerAddress,
            )
        } else {
            null
        }
    }

    private fun tronRawDataSourceCallOwnerAddress(
        rawData: ByteArray,
        sourceEventDigest: ByteArray,
        expectedContractAddress: ByteArray?,
        expectedOwnerAddress: ByteArray?,
    ): ByteArray? {
        val cursor = intArrayOf(0)
        var refBlockBytesSeen = false
        var refBlockNumSeen = false
        var refBlockHashSeen = false
        var expirationMs: BigInteger? = null
        var timestampMs: BigInteger? = null
        var feeLimitSeen = false
        var contractCount = 0
        var matchedContract: ByteArray? = null
        while (cursor[0] < rawData.size) {
            val key = readCanonicalProtobufVarint(rawData, cursor, "rawData")
            val fieldNumber = protobufFieldNumber(key, "rawData")
            val wireType = key.and(BigInteger.valueOf(0x07L)).toInt()
            when {
                fieldNumber == 1 && wireType == 2 && !refBlockBytesSeen -> {
                    refBlockBytesSeen = true
                    val value = readProtobufBytesField(rawData, cursor, "rawData")
                    if (value.size != 2 || value.all { it == 0.toByte() }) return null
                }
                fieldNumber == 3 && wireType == 0 && !refBlockNumSeen -> {
                    refBlockNumSeen = true
                    readCanonicalProtobufVarint(rawData, cursor, "rawData")
                }
                fieldNumber == 4 && wireType == 2 && !refBlockHashSeen -> {
                    refBlockHashSeen = true
                    val value = readProtobufBytesField(rawData, cursor, "rawData")
                    if (value.size != 8 || value.all { it == 0.toByte() }) return null
                }
                fieldNumber == 8 && wireType == 0 && expirationMs == null -> {
                    expirationMs = readCanonicalProtobufVarint(rawData, cursor, "rawData")
                    if (expirationMs == BigInteger.ZERO) return null
                }
                fieldNumber == 11 && wireType == 2 -> {
                    contractCount += 1
                    if (contractCount > 1) return null
                    matchedContract = tronContractSourceCallOwnerAddress(
                        readProtobufBytesField(rawData, cursor, "rawData"),
                        sourceEventDigest,
                        expectedContractAddress,
                        expectedOwnerAddress,
                    )
                }
                fieldNumber == 14 && wireType == 0 && timestampMs == null -> {
                    timestampMs = readCanonicalProtobufVarint(rawData, cursor, "rawData")
                    if (timestampMs == BigInteger.ZERO) return null
                }
                fieldNumber == 18 && wireType == 0 && !feeLimitSeen -> {
                    feeLimitSeen = true
                    if (readCanonicalProtobufVarint(rawData, cursor, "rawData") == BigInteger.ZERO) return null
                }
                else -> return null
            }
        }
        val expiration = expirationMs
        val timestamp = timestampMs
        return if (refBlockBytesSeen &&
            refBlockHashSeen &&
            expiration != null &&
            timestamp != null &&
            expiration > timestamp &&
            feeLimitSeen &&
            contractCount == 1
        ) {
            matchedContract
        } else {
            null
        }
    }

    private fun validateTronTransactionSourceCall(
        transactionBytes: ByteArray,
        sourceEventDigest: ByteArray,
        expectedContractAddress: ByteArray?,
        expectedOwnerAddress: ByteArray?,
    ) {
        val cursor = intArrayOf(0)
        var rawData: ByteArray? = null
        val signatures = mutableListOf<ByteArray>()
        var resultCount = 0
        var resultSuccess = false
        while (cursor[0] < transactionBytes.size) {
            val key = readCanonicalProtobufVarint(transactionBytes, cursor, "transactionBytes")
            val fieldNumber = protobufFieldNumber(key, "transactionBytes")
            val wireType = key.and(BigInteger.valueOf(0x07L)).toInt()
            when {
                fieldNumber == 1 && wireType == 2 && rawData == null ->
                    rawData = readProtobufBytesField(transactionBytes, cursor, "transactionBytes")
                fieldNumber == 2 && wireType == 2 -> {
                    if (signatures.size >= TRON_SOURCE_CALL_SIGNATURES) throw tronSourceTransactionError()
                    val signature = readProtobufBytesField(transactionBytes, cursor, "transactionBytes")
                    if (!tronRecoverableSignatureIsCanonical(signature)) throw tronSourceTransactionError()
                    signatures.add(signature)
                }
                fieldNumber == 5 && wireType == 2 -> {
                    if (resultCount >= 1) throw tronSourceTransactionError()
                    resultSuccess =
                        tronTransactionResultSuccess(readProtobufBytesField(transactionBytes, cursor, "transactionBytes"))
                    resultCount += 1
                }
                else -> throw tronSourceTransactionError()
            }
        }
        val rawDataValue = rawData
        if (rawDataValue == null || signatures.size != TRON_SOURCE_CALL_SIGNATURES) {
            throw tronSourceTransactionError()
        }
        val ownerAddress = tronRawDataSourceCallOwnerAddress(
            rawDataValue,
            sourceEventDigest,
            expectedContractAddress,
            expectedOwnerAddress,
        )
        val recoveredSigner = tronRecoveredSignerAddress20(sha256(rawDataValue), signatures[0])
        if (
            resultCount != 1 ||
            !resultSuccess ||
            ownerAddress == null ||
            recoveredSigner == null ||
            !recoveredSigner.contentEquals(ownerAddress)
        ) {
            throw tronSourceTransactionError()
        }
    }

    private fun writeProtobufVarint(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        require(working >= BigInteger.ZERO && working <= MAX_U64) { "protobuf varint value must fit u64" }
        while (working >= BigInteger.valueOf(0x80L)) {
            out.write(working.and(BigInteger.valueOf(0x7fL)).or(BigInteger.valueOf(0x80L)).toInt())
            working = working.shiftRight(7)
        }
        out.write(working.toInt())
    }

    private fun writeProtobufU64(out: ByteArrayOutputStream, fieldNumber: Int, value: BigInteger) {
        require(fieldNumber > 0) { "protobuf field number must be positive" }
        writeProtobufVarint(out, BigInteger.valueOf((fieldNumber shl 3).toLong()))
        writeProtobufVarint(out, value)
    }

    private fun writeProtobufBytes(out: ByteArrayOutputStream, fieldNumber: Int, value: ByteArray) {
        require(fieldNumber > 0) { "protobuf field number must be positive" }
        writeProtobufVarint(out, BigInteger.valueOf(((fieldNumber shl 3) or 2).toLong()))
        writeProtobufVarint(out, BigInteger.valueOf(value.size.toLong()))
        out.write(value)
    }

    private data class TronRawBlockHeaderFields(
        val number: BigInteger,
        val txTrieRoot: ByteArray,
        val accountStateRoot: ByteArray,
        val parentBlockId: ByteArray,
        val witnessAddress: ByteArray,
        val headerVersion: Int,
        val timestampMs: BigInteger,
    )

    private fun protobufVarintLength(value: BigInteger): Int {
        var working = value
        var length = 1
        while (working >= BigInteger.valueOf(0x80L)) {
            length += 1
            working = working.shiftRight(7)
        }
        return length
    }

    private fun readCanonicalProtobufVarint(bytes: ByteArray, cursor: IntArray, label: String): BigInteger {
        val start = cursor[0]
        var value = BigInteger.ZERO
        var shift = 0
        for (index in 0 until 10) {
            require(cursor[0] < bytes.size) { "$label contains truncated protobuf varint" }
            val byte = bytes[cursor[0]].toInt() and 0xff
            cursor[0] += 1
            val chunk = byte and 0x7f
            require(index != 9 || chunk <= 1) { "$label protobuf varint must fit u64" }
            value = value.or(BigInteger.valueOf(chunk.toLong()).shiftLeft(shift))
            if ((byte and 0x80) == 0) {
                require(cursor[0] - start == protobufVarintLength(value)) {
                    "$label protobuf varint must be canonical"
                }
                return value
            }
            shift += 7
        }
        throw IllegalArgumentException("$label protobuf varint must fit u64")
    }

    private fun decodeTronRawBlockHeaderFields(rawData: ByteArray, label: String): TronRawBlockHeaderFields {
        val cursor = intArrayOf(0)
        var number: BigInteger? = null
        var txTrieRoot: ByteArray? = null
        var accountStateRoot: ByteArray? = null
        var parentBlockId: ByteArray? = null
        var witnessIdSeen = false
        var witnessAddress: ByteArray? = null
        var headerVersion: Int? = null
        var timestampMs: BigInteger? = null

        fun readBytes(byteLength: Int, fieldLabel: String): ByteArray {
            val length = readCanonicalProtobufVarint(rawData, cursor, label).intValueExact()
            val end = cursor[0] + length
            require(length == byteLength && end <= rawData.size) { "$fieldLabel must be $byteLength bytes" }
            val value = rawData.copyOfRange(cursor[0], end)
            cursor[0] = end
            return value
        }

        while (cursor[0] < rawData.size) {
            val key = readCanonicalProtobufVarint(rawData, cursor, label)
            val fieldNumber = key.shiftRight(3).intValueExact()
            val wireType = key.and(BigInteger.valueOf(0x07L)).toInt()
            when (fieldNumber) {
                1 -> {
                    require(wireType == 0 && timestampMs == null) {
                        "$label must contain one canonical timestamp field"
                    }
                    timestampMs = readCanonicalProtobufVarint(rawData, cursor, label)
                }
                2 -> {
                    require(wireType == 2 && txTrieRoot == null) {
                        "$label must contain one canonical txTrieRoot field"
                    }
                    txTrieRoot = readBytes(32, "txTrieRoot")
                }
                3 -> {
                    require(wireType == 2 && parentBlockId == null) {
                        "$label must contain one canonical parentBlockId field"
                    }
                    parentBlockId = readBytes(32, "parentBlockId")
                }
                7 -> {
                    require(wireType == 0 && number == null) {
                        "$label must contain one canonical number field"
                    }
                    number = readCanonicalProtobufVarint(rawData, cursor, label)
                }
                8 -> {
                    require(wireType == 0 && !witnessIdSeen) {
                        "$label must contain at most one canonical witnessId field"
                    }
                    witnessIdSeen = true
                    readCanonicalProtobufVarint(rawData, cursor, label)
                }
                9 -> {
                    require(wireType == 2 && witnessAddress == null) {
                        "$label must contain one canonical witnessAddress field"
                    }
                    witnessAddress = readBytes(21, "witnessAddress")
                }
                10 -> {
                    require(wireType == 0 && headerVersion == null) {
                        "$label must contain one canonical headerVersion field"
                    }
                    val value = readCanonicalProtobufVarint(rawData, cursor, label)
                    require(value <= BigInteger.valueOf(0xffff_ffffL)) {
                        "headerVersion must be a non-zero u32"
                    }
                    headerVersion = value.toInt()
                }
                11 -> {
                    require(wireType == 2 && accountStateRoot == null) {
                        "$label must contain one canonical accountStateRoot field"
                    }
                    accountStateRoot = readBytes(32, "accountStateRoot")
                }
                else -> throw IllegalArgumentException("$label contains an unsupported protobuf field")
            }
        }

        val parsedNumber = requireNotNull(number) { "$label must be a canonical TRON raw block header" }
        val parsedTimestamp = requireNotNull(timestampMs) { "$label must be a canonical TRON raw block header" }
        val parsedHeaderVersion = requireNotNull(headerVersion) { "$label must be a canonical TRON raw block header" }
        val parsedTxTrieRoot = requireNotNull(txTrieRoot) { "$label must be a canonical TRON raw block header" }
        val parsedAccountStateRoot = requireNotNull(accountStateRoot) {
            "$label must be a canonical TRON raw block header"
        }
        val parsedParentBlockId = requireNotNull(parentBlockId) { "$label must be a canonical TRON raw block header" }
        val parsedWitnessAddress = requireNotNull(witnessAddress) { "$label must be a canonical TRON raw block header" }
        require(
            parsedNumber != BigInteger.ZERO &&
                parsedTimestamp != BigInteger.ZERO &&
                parsedHeaderVersion != 0 &&
                !isZero(parsedTxTrieRoot) &&
                !isZero(parsedAccountStateRoot) &&
                !isZero(parsedParentBlockId) &&
                isNonZeroTronAddress(parsedWitnessAddress),
        ) {
            "$label must be a canonical TRON raw block header"
        }
        return TronRawBlockHeaderFields(
            number = parsedNumber,
            txTrieRoot = parsedTxTrieRoot,
            accountStateRoot = parsedAccountStateRoot,
            parentBlockId = parsedParentBlockId,
            witnessAddress = parsedWitnessAddress,
            headerVersion = parsedHeaderVersion,
            timestampMs = parsedTimestamp,
        )
    }

    private fun tronBlockIdBytesFromRawDataHash(number: BigInteger, rawDataHash: ByteArray): ByteArray {
        val blockId = rawDataHash.copyOf()
        val numberBytes = ByteArrayOutputStream()
        writeU64Be(numberBytes, number)
        val encodedNumber = numberBytes.toByteArray()
        System.arraycopy(encodedNumber, 0, blockId, 0, encodedNumber.size)
        return blockId
    }

    private fun validateBscValidatorSetPayload(payload: ByteArray) {
        require(payload.size <= BSC_MAX_VALIDATOR_SET_PAYLOAD_BYTES) {
            "validatorSetPayload must be at most $BSC_MAX_VALIDATOR_SET_PAYLOAD_BYTES bytes"
        }
        var cursor = 0
        require(payload.isNotEmpty() && payload[cursor].toInt() == 1) {
            "validatorSetPayload must have version 1"
        }
        cursor += 1
        val count = readU32Le(payload, cursor)
        cursor += 4
        require(
            count > 0 &&
                count <= BSC_MAX_PARLIA_VALIDATORS &&
                payload.size - cursor == count * (BSC_PARLIA_VALIDATOR_ADDRESS_BYTES + 8),
        ) {
            "validatorSetPayload has an invalid validator count"
        }
        val seenAddresses = HashSet<String>()
        for (index in 0 until count) {
            val address = payload.copyOfRange(cursor, cursor + BSC_PARLIA_VALIDATOR_ADDRESS_BYTES)
            cursor += BSC_PARLIA_VALIDATOR_ADDRESS_BYTES
            require(address.any { it.toInt() != 0 }) { "validatorAddresses[$index] must not be zero" }
            require(seenAddresses.add(hexLower(address))) { "validatorAddresses[$index] must be unique" }
            val power = readU64Le(payload, cursor)
            cursor += 8
            require(power != BigInteger.ZERO) { "validatorPowers[$index] must not be zero" }
        }
        require(cursor == payload.size) { "validatorSetPayload has trailing bytes" }
    }

    private fun validateEthSyncCommitteePayload(payload: ByteArray) {
        require(payload.size <= ETH_MAX_SYNC_COMMITTEE_PAYLOAD_BYTES) {
            "syncCommitteePayload must be at most $ETH_MAX_SYNC_COMMITTEE_PAYLOAD_BYTES bytes"
        }
        var cursor = 0
        require(payload.size >= 5 && payload[cursor].toInt() == 1) {
            "syncCommitteePayload must have version 1"
        }
        cursor += 1
        val count = readU32Le(payload, cursor)
        cursor += 4
        require(count > 0) { "syncCommitteePayload must not be empty" }
        require(count <= ETH_MAX_SYNC_COMMITTEE_AUTHORITIES) {
            "syncCommitteePayload must contain at most $ETH_MAX_SYNC_COMMITTEE_AUTHORITIES entries"
        }
        val seenPublicKeys = HashSet<String>()
        for (index in 0 until count) {
            val publicKeyLen = readU32Le(payload, cursor)
            cursor += 4
            require(publicKeyLen == ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES && cursor + publicKeyLen <= payload.size) {
                "syncCommitteePublicKeys[$index] is invalid"
            }
            val publicKey = payload.copyOfRange(cursor, cursor + publicKeyLen)
            cursor += publicKeyLen
            require(!isZero(publicKey)) { "syncCommitteePublicKeys[$index] must not be zero" }
            require(seenPublicKeys.add(hexLower(publicKey))) { "syncCommitteePublicKeys[$index] must be unique" }
            val weight = readU64Le(payload, cursor)
            cursor += 8
            require(weight != BigInteger.ZERO) { "syncCommitteeWeights[$index] must not be zero" }
            val popLen = readU32Le(payload, cursor)
            cursor += 4
            require(popLen == ETH_SYNC_COMMITTEE_POP_BYTES && cursor + popLen <= payload.size) {
                "syncCommitteePops[$index] is invalid"
            }
            val pop = payload.copyOfRange(cursor, cursor + popLen)
            require(!isZero(pop)) { "syncCommitteePops[$index] must not be zero" }
            cursor += popLen
        }
        require(cursor == payload.size) { "syncCommitteePayload has trailing bytes" }
    }

    private fun ethSyncCommitteeSignerIndices(signersBitmap: ByteArray, committeeSize: Int): List<Int> {
        require(signersBitmap.size == (committeeSize + 7) / 8) {
            "signersBitmap length must match syncCommitteePublicKeys"
        }
        val signerIndices = ArrayList<Int>()
        for (byteIndex in signersBitmap.indices) {
            val value = signersBitmap[byteIndex].toInt() and 0xff
            for (bit in 0 until 8) {
                if (((value shr bit) and 1) == 0) continue
                val index = byteIndex * 8 + bit
                require(index < committeeSize) { "signersBitmap must not set padding bits" }
                signerIndices.add(index)
            }
        }
        require(signerIndices.isNotEmpty()) {
            "signersBitmap must select at least one sync committee member"
        }
        return signerIndices
    }

    private fun substrateAuthoritySignerIndices(signersBitmap: ByteArray, authorityCount: Int): List<Int> {
        require(signersBitmap.size == (authorityCount + 7) / 8) {
            "signersBitmap length must match authorityPublicKeys"
        }
        val signerIndices = ArrayList<Int>()
        for (byteIndex in signersBitmap.indices) {
            val value = signersBitmap[byteIndex].toInt() and 0xff
            for (bit in 0 until 8) {
                if (((value shr bit) and 1) == 0) continue
                val index = byteIndex * 8 + bit
                require(index < authorityCount) { "signersBitmap must not set padding bits" }
                signerIndices.add(index)
            }
        }
        require(signerIndices.isNotEmpty()) {
            "signersBitmap must select at least one authority"
        }
        return signerIndices
    }

    private fun validateSubstrateAuthoritySetPayload(payload: ByteArray) {
        require(payload.size <= SUBSTRATE_MAX_AUTHORITY_SET_PAYLOAD_BYTES) {
            "authoritySetPayload must be at most $SUBSTRATE_MAX_AUTHORITY_SET_PAYLOAD_BYTES bytes"
        }
        var cursor = 0
        require(payload.isNotEmpty() && payload[cursor].toInt() == 1) {
            "authoritySetPayload must have version 1"
        }
        cursor += 1
        val count = readU32Le(payload, cursor)
        cursor += 4
        require(
            count > 0 &&
                count <= SUBSTRATE_MAX_AUTHORITIES &&
                payload.size - cursor == count * 40,
        ) {
            "authoritySetPayload length is invalid"
        }
        val seenPublicKeys = HashSet<String>()
        for (index in 0 until count) {
            val publicKey = payload.copyOfRange(cursor, cursor + 32)
            cursor += 32
            require(!isZero(publicKey)) { "authorityPublicKeys[$index] must not be zero" }
            require(seenPublicKeys.add(hexLower(publicKey))) {
                "authorityPublicKeys[$index] must be unique"
            }
            val weight = readU64Le(payload, cursor)
            cursor += 8
            require(weight != BigInteger.ZERO) { "authorityWeights[$index] must not be zero" }
        }
        require(cursor == payload.size) { "authoritySetPayload has trailing bytes" }
    }

    private fun validateTronWitnessSchedulePayload(payload: ByteArray) {
        require(payload.size >= 5 && payload[0].toInt() == 1) {
            "witnessSchedulePayload must be a canonical TRON witness schedule payload"
        }
        val count = readU32Le(payload, 1)
        require(count > 0 && count <= TRON_MAX_WITNESSES && payload.size == 5 + count * 29) {
            "witnessSchedulePayload must be a canonical TRON witness schedule payload"
        }
        val seenAddresses = HashSet<String>()
        var cursor = 5
        var totalWeight = BigInteger.ZERO
        for (index in 0 until count) {
            val address = payload.copyOfRange(cursor, cursor + 21)
            cursor += 21
            require(isNonZeroTronAddress(address)) {
                "witnessSchedulePayload witness $index must be a TRON 0x41-prefixed address"
            }
            require(seenAddresses.add(hexLower(address))) {
                "witnessSchedulePayload witness $index must be unique"
            }
            val weight = readU64Le(payload, cursor)
            cursor += 8
            require(weight != BigInteger.ZERO) {
                "witnessSchedulePayload witness $index weight must not be zero"
            }
            totalWeight = totalWeight.add(weight)
            require(totalWeight <= MAX_U64) {
                "witnessSchedulePayload total weight must fit u64"
            }
        }
    }

    private fun readU32Le(bytes: ByteArray, offset: Int): Int {
        require(offset >= 0 && offset + 4 <= bytes.size) { "payload is truncated" }
        return (bytes[offset].toInt() and 0xff) or
            ((bytes[offset + 1].toInt() and 0xff) shl 8) or
            ((bytes[offset + 2].toInt() and 0xff) shl 16) or
            ((bytes[offset + 3].toInt() and 0xff) shl 24)
    }

    private fun readU64Le(bytes: ByteArray, offset: Int): BigInteger {
        require(offset >= 0 && offset + 8 <= bytes.size) { "payload is truncated" }
        var value = BigInteger.ZERO
        for (index in 7 downTo 0) {
            value = value.shiftLeft(8).or(BigInteger.valueOf((bytes[offset + index].toInt() and 0xff).toLong()))
        }
        return value
    }

    private fun writeBranch(
        out: ByteArrayOutputStream,
        inclusionBranch: List<ByteArray>,
        requireNonEmpty: Boolean = false,
    ) {
        require(!requireNonEmpty || inclusionBranch.isNotEmpty()) { "inclusionBranch must not be empty" }
        writeU32Le(out, inclusionBranch.size)
        inclusionBranch.forEachIndexed { index, sibling ->
            require(sibling.size == 32) { "inclusionBranch[$index] must be 32 bytes" }
            out.write(sibling)
        }
    }

    private fun normalizeDomain(value: Int, field: String): Int {
        require(value >= 0) { "$field must be a u32 domain id" }
        return value
    }

    private fun requireV1Version(value: Int, field: String) {
        require(value == 1) { "$field must be 1" }
    }

    private fun isCanonicalDecimalText(value: String): Boolean {
        if (value == "0") return true
        return value.isNotEmpty() &&
            value[0] in '1'..'9' &&
            value.all { it in '0'..'9' }
    }

    private fun normalizeU64(value: String, field: String): BigInteger {
        require(isCanonicalDecimalText(value)) { "$field must be an unsigned integer" }
        val numeric = BigInteger(value)
        require(numeric <= MAX_U64) { "$field must fit u64" }
        return numeric
    }

    private fun hex32Bytes(value: String, field: String): ByteArray {
        return hexBytes(value, field, 32)
    }

    private fun nonZeroHex32Bytes(value: String, field: String): ByteArray {
        val bytes = hex32Bytes(value, field)
        require(!isZero(bytes)) { "$field must not be zero" }
        return bytes
    }

    private fun nonZeroHexBytes(value: String, field: String, byteLength: Int): ByteArray {
        val bytes = hexBytes(value, field, byteLength)
        require(!isZero(bytes)) { "$field must not be zero" }
        return bytes
    }

    private fun isNonZeroTronAddress(address: ByteArray): Boolean =
        address.size == 21 &&
            address[0] == 0x41.toByte() &&
            address.copyOfRange(1, address.size).any { it != 0.toByte() }

    private fun normalizeHex32(value: String): String = "0x" + hexLower(hex32Bytes(value, "hex32"))

    private fun hexBytes(value: String, field: String, byteLength: Int): ByteArray {
        require(value.trim() == value) { "$field must be canonical hex" }
        var body = value
        if (body.startsWith("0x", ignoreCase = true)) {
            body = body.substring(2)
        }
        require(body.length == byteLength * 2) { "$field must be $byteLength bytes" }
        val out = ByteArray(byteLength)
        for (i in out.indices) {
            out[i] = body.substring(i * 2, i * 2 + 2).toIntOrNull(16)?.toByte()
                ?: throw IllegalArgumentException("$field must be canonical hex")
        }
        return out
    }

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0) { "u32 value must not be negative" }
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun abiWordU32(value: Int, field: String): ByteArray {
        val normalized = normalizeDomain(value, field)
        val out = ByteArray(32)
        out[28] = ((normalized ushr 24) and 0xff).toByte()
        out[29] = ((normalized ushr 16) and 0xff).toByte()
        out[30] = ((normalized ushr 8) and 0xff).toByte()
        out[31] = (normalized and 0xff).toByte()
        return out
    }

    private fun writeI32Le(out: ByteArrayOutputStream, value: Int) {
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        for (index in 0 until 8) {
            out.write(working.and(BigInteger.valueOf(0xffL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun writeU64Be(out: ByteArrayOutputStream, value: BigInteger) {
        for (index in 7 downTo 0) {
            out.write(value.shiftRight(index * 8).and(BigInteger.valueOf(0xffL)).toInt())
        }
    }

    private fun canonicalBscValidatorSetPayloadBytesFromAddresses(addresses: List<ByteArray>): ByteArray {
        require(addresses.isNotEmpty() && addresses.size <= BSC_MAX_PARLIA_VALIDATORS) {
            "validatorAddresses must be a non-empty bounded array"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, addresses.size)
        val seenAddresses = HashSet<String>()
        addresses.forEachIndexed { index, address ->
            require(address.size == BSC_PARLIA_VALIDATOR_ADDRESS_BYTES) {
                "validatorAddresses[$index] must be 20 bytes"
            }
            require(!isZero(address)) { "validatorAddresses[$index] must not be zero" }
            require(seenAddresses.add(hexLower(address))) { "validatorAddresses[$index] must be unique" }
            out.write(address)
            writeU64Le(out, BigInteger.ONE)
        }
        return out.toByteArray()
    }

    private fun canonicalBscValidatorSetPayloadBytesFromAddressPowers(
        addresses: List<ByteArray>,
        powers: List<BigInteger>,
    ): ByteArray {
        require(addresses.isNotEmpty() && addresses.size == powers.size && addresses.size <= BSC_MAX_PARLIA_VALIDATORS) {
            "validatorAddresses and validatorPowers must be non-empty bounded arrays"
        }
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32Le(out, addresses.size)
        val seenAddresses = HashSet<String>()
        addresses.zip(powers).forEachIndexed { index, (address, power) ->
            require(address.size == BSC_PARLIA_VALIDATOR_ADDRESS_BYTES) {
                "validatorAddresses[$index] must be 20 bytes"
            }
            require(!isZero(address)) { "validatorAddresses[$index] must not be zero" }
            require(seenAddresses.add(hexLower(address))) { "validatorAddresses[$index] must be unique" }
            require(power != BigInteger.ZERO) { "validatorPowers[$index] must not be zero" }
            out.write(address)
            writeU64Le(out, power)
        }
        return out.toByteArray()
    }

    private fun bscSignerIndicesFromBitmap(signersBitmap: ByteArray, rosterLength: Int): List<Int> {
        require(signersBitmap.size == (rosterLength + 7) / 8) { "signersBitmap has invalid length" }
        val indices = ArrayList<Int>()
        signersBitmap.forEachIndexed { byteIndex, value ->
            val unsignedValue = value.toInt() and 0xff
            for (bit in 0 until 8) {
                val validatorIndex = byteIndex * 8 + bit
                val bitSet = (unsignedValue and (1 shl bit)) != 0
                if (validatorIndex >= rosterLength) {
                    require(!bitSet) { "signersBitmap padding bits must be zero" }
                } else if (bitSet) {
                    indices.add(validatorIndex)
                }
            }
        }
        require(indices.isNotEmpty()) { "signersBitmap must select at least one signer" }
        return indices
    }

    private fun bscValidatorAddress20(publicKey: ByteArray, label: String): ByteArray {
        require(
            (publicKey.size == 33 && (publicKey[0] == 0x02.toByte() || publicKey[0] == 0x03.toByte())) ||
                (publicKey.size == 65 && publicKey[0] == 0x04.toByte()),
        ) {
            "$label must be a compressed or uncompressed secp256k1 public key"
        }
        val point = try {
            SECP256K1_PARAMS.curve.decodePoint(publicKey).normalize()
        } catch (exception: IllegalArgumentException) {
            throw IllegalArgumentException("$label must be a valid secp256k1 public key", exception)
        }
        val compressed = publicKey.size == 33
        require(point.getEncoded(compressed).contentEquals(publicKey)) {
            "$label must be a canonical secp256k1 public key"
        }
        val uncompressed = point.getEncoded(false)
        return keccak256(uncompressed.copyOfRange(1, uncompressed.size)).copyOfRange(12, 32)
    }

    private fun bscParliaValidatorSetPayloadCandidatesFromExtra(extraData: ByteArray): List<ByteArray> {
        val candidates = ArrayList<ByteArray>()
        fun pushCandidate(addresses: List<ByteArray>) {
            try {
                val payload = canonicalBscValidatorSetPayloadBytesFromAddresses(addresses)
                if (!candidates.any { it.contentEquals(payload) }) {
                    candidates.add(payload)
                }
            } catch (ignored: IllegalArgumentException) {
                if (ignored.message == null) return
                return
            }
        }

        val minimumExtra = BSC_PARLIA_EXTRA_VANITY_BYTES + BSC_PARLIA_EXTRA_SEAL_BYTES
        if (extraData.size <= minimumExtra) return candidates
        val regionStart = BSC_PARLIA_EXTRA_VANITY_BYTES
        val regionEnd = extraData.size - BSC_PARLIA_EXTRA_SEAL_BYTES
        val validatorRegion = extraData.copyOfRange(regionStart, regionEnd)
        if (validatorRegion.isEmpty()) return candidates

        if (validatorRegion.size % BSC_PARLIA_VALIDATOR_ADDRESS_BYTES == 0) {
            val count = validatorRegion.size / BSC_PARLIA_VALIDATOR_ADDRESS_BYTES
            if (count <= BSC_MAX_PARLIA_VALIDATORS) {
                val addresses = ArrayList<ByteArray>()
                var offset = 0
                while (offset < validatorRegion.size) {
                    addresses.add(validatorRegion.copyOfRange(offset, offset + BSC_PARLIA_VALIDATOR_ADDRESS_BYTES))
                    offset += BSC_PARLIA_VALIDATOR_ADDRESS_BYTES
                }
                pushCandidate(addresses)
            }
        }

        val lubanCount = validatorRegion[0].toInt() and 0xff
        val lubanStride = BSC_PARLIA_VALIDATOR_ADDRESS_BYTES + BSC_PARLIA_VALIDATOR_BLS_KEY_BYTES
        val lubanRegionLength = 1 + lubanCount * lubanStride
        if (
            lubanCount != 0 &&
            lubanCount <= BSC_MAX_PARLIA_VALIDATORS &&
            validatorRegion.size >= lubanRegionLength
        ) {
            val addresses = ArrayList<ByteArray>()
            for (index in 0 until lubanCount) {
                val start = 1 + index * lubanStride
                addresses.add(validatorRegion.copyOfRange(start, start + BSC_PARLIA_VALIDATOR_ADDRESS_BYTES))
            }
            pushCandidate(addresses)
        }

        return candidates
    }

    private sealed class RlpItem {
        data class Bytes(val value: ByteArray) : RlpItem()
        data class ListPayload(val value: ByteArray) : RlpItem()
    }

    private data class RlpReadResult(val item: RlpItem, val next: Int)

    private fun readRlpLength(bytes: ByteArray, offset: Int, lengthOfLength: Int): Int {
        require(lengthOfLength > 0 && lengthOfLength <= 4 && offset + lengthOfLength <= bytes.size) {
            "invalid RLP length"
        }
        require(bytes[offset].toInt() != 0) { "non-canonical RLP length" }
        var length = 0
        for (index in 0 until lengthOfLength) {
            length = length * 256 + (bytes[offset + index].toInt() and 0xff)
        }
        return length
    }

    private fun rlpItemAt(bytes: ByteArray, cursor: Int): RlpReadResult {
        require(cursor < bytes.size) { "RLP cursor out of bounds" }
        val first = bytes[cursor].toInt() and 0xff
        if (first <= 0x7f) {
            return RlpReadResult(RlpItem.Bytes(byteArrayOf(bytes[cursor])), cursor + 1)
        }
        if (first <= 0xb7) {
            val length = first - 0x80
            val start = cursor + 1
            val end = start + length
            require(end <= bytes.size && !(length == 1 && (bytes[start].toInt() and 0xff) < 0x80)) {
                "non-canonical RLP string"
            }
            return RlpReadResult(RlpItem.Bytes(bytes.copyOfRange(start, end)), end)
        }
        if (first <= 0xbf) {
            val lengthOfLength = first - 0xb7
            val length = readRlpLength(bytes, cursor + 1, lengthOfLength)
            require(length >= 56) { "non-canonical RLP long string" }
            val start = cursor + 1 + lengthOfLength
            val end = start + length
            require(end <= bytes.size) { "RLP string out of bounds" }
            return RlpReadResult(RlpItem.Bytes(bytes.copyOfRange(start, end)), end)
        }
        if (first <= 0xf7) {
            val length = first - 0xc0
            val start = cursor + 1
            val end = start + length
            require(end <= bytes.size) { "RLP list out of bounds" }
            return RlpReadResult(RlpItem.ListPayload(bytes.copyOfRange(start, end)), end)
        }
        val lengthOfLength = first - 0xf7
        val length = readRlpLength(bytes, cursor + 1, lengthOfLength)
        require(length >= 56) { "non-canonical RLP long list" }
        val start = cursor + 1 + lengthOfLength
        val end = start + length
        require(end <= bytes.size) { "RLP list out of bounds" }
        return RlpReadResult(RlpItem.ListPayload(bytes.copyOfRange(start, end)), end)
    }

    private fun rlpListByteFields(bytes: ByteArray): List<ByteArray> {
        val outer = rlpItemAt(bytes, 0)
        require(outer.next == bytes.size && outer.item is RlpItem.ListPayload) {
            "headerRlp must be an RLP list"
        }
        val listPayload = outer.item.value
        val fields = ArrayList<ByteArray>()
        var cursor = 0
        while (cursor < listPayload.size) {
            val item = rlpItemAt(listPayload, cursor)
            require(item.item is RlpItem.Bytes) { "headerRlp must contain only RLP byte fields" }
            fields.add(item.item.value)
            cursor = item.next
        }
        return fields
    }

    private fun sszHashNode(left: ByteArray, right: ByteArray): ByteArray {
        require(left.size == 32 && right.size == 32) { "SSZ node inputs must be 32 bytes" }
        val preimage = ByteArray(64)
        System.arraycopy(left, 0, preimage, 0, left.size)
        System.arraycopy(right, 0, preimage, left.size, right.size)
        return sha256(preimage)
    }

    private fun sszMerkleizeChunks(inputChunks: List<ByteArray>): ByteArray {
        if (inputChunks.isEmpty()) {
            return ByteArray(32)
        }
        var chunks = ArrayList<ByteArray>(inputChunks.size)
        inputChunks.forEach { chunk ->
            require(chunk.size == 32) { "SSZ chunk must be 32 bytes" }
            chunks.add(chunk)
        }
        var paddedLength = 1
        while (paddedLength < chunks.size) {
            paddedLength *= 2
        }
        while (chunks.size < paddedLength) {
            chunks.add(ByteArray(32))
        }
        while (chunks.size > 1) {
            val next = ArrayList<ByteArray>(chunks.size / 2)
            var index = 0
            while (index < chunks.size) {
                next.add(sszHashNode(chunks[index], chunks[index + 1]))
                index += 2
            }
            chunks = next
        }
        return chunks[0]
    }

    private fun readMinimalBeU64(bytes: ByteArray, field: String): BigInteger {
        if (bytes.isEmpty()) {
            return BigInteger.ZERO
        }
        require(bytes.size <= 8 && !(bytes.size > 1 && bytes[0].toInt() == 0)) {
            "$field must be a canonical RLP u64"
        }
        var out = BigInteger.ZERO
        for (byte in bytes) {
            out = out.shiftLeft(8).or(BigInteger.valueOf((byte.toInt() and 0xff).toLong()))
        }
        return out
    }

    private fun sszU64Chunk(value: BigInteger): ByteArray {
        require(value >= BigInteger.ZERO && value <= MAX_U64) { "SSZ u64 is out of range" }
        val out = ByteArray(32)
        var working = value
        for (index in 0 until 8) {
            out[index] = working.and(BigInteger.valueOf(0xffL)).toByte()
            working = working.shiftRight(8)
        }
        return out
    }

    private fun sszU64ChunkFromRlp(bytes: ByteArray, field: String): ByteArray =
        sszU64Chunk(readMinimalBeU64(bytes, field))

    private fun sszU256ChunkFromRlp(bytes: ByteArray, field: String): ByteArray {
        require(bytes.size <= 32 && !(bytes.size > 1 && bytes[0].toInt() == 0)) {
            "$field must be a canonical RLP uint256"
        }
        val out = ByteArray(32)
        for (index in bytes.indices) {
            out[index] = bytes[bytes.size - 1 - index]
        }
        return out
    }

    private fun sszByteVectorRoot(bytes: ByteArray, expectedLength: Int, field: String): ByteArray {
        require(bytes.size == expectedLength) { "$field must be $expectedLength bytes" }
        val chunks = ArrayList<ByteArray>()
        var offset = 0
        while (offset < bytes.size) {
            val chunk = ByteArray(32)
            val length = Math.min(32, bytes.size - offset)
            System.arraycopy(bytes, offset, chunk, 0, length)
            chunks.add(chunk)
            offset += 32
        }
        return sszMerkleizeChunks(chunks)
    }

    private fun sszMixInLength(root: ByteArray, length: Int): ByteArray =
        sszHashNode(root, sszU64Chunk(BigInteger.valueOf(length.toLong())))

    private fun sszByteListRoot(bytes: ByteArray, maxLength: Int, field: String): ByteArray {
        require(bytes.size <= maxLength) { "$field must be at most $maxLength bytes" }
        val limitChunks = Math.max(1, (maxLength + 31) / 32)
        val chunks = ArrayList<ByteArray>()
        var offset = 0
        while (offset < bytes.size) {
            val chunk = ByteArray(32)
            val length = Math.min(32, bytes.size - offset)
            System.arraycopy(bytes, offset, chunk, 0, length)
            chunks.add(chunk)
            offset += 32
        }
        while (chunks.size < limitChunks) {
            chunks.add(ByteArray(32))
        }
        return sszMixInLength(sszMerkleizeChunks(chunks), bytes.size)
    }

    private fun sszMerkleRootFromBranch(
        leaf: ByteArray,
        leafIndex: Int,
        branch: List<ByteArray>,
        field: String,
    ): ByteArray {
        require(leaf.size == 32) { "$field leaf must be 32 bytes" }
        var current = leaf
        var index = leafIndex
        for (branchIndex in branch.indices) {
            val sibling = branch[branchIndex]
            require(sibling.size == 32) { "$field[$branchIndex] must be 32 bytes" }
            current = if ((index and 1) == 1) {
                sszHashNode(sibling, current)
            } else {
                sszHashNode(current, sibling)
            }
            index = index ushr 1
        }
        return current
    }

    private fun hashHex(prefix: String, payload: ByteArray): String {
        return "0x" + hexLower(hashBytes(prefix, payload))
    }

    private fun hashBytes(prefix: String, payload: ByteArray): ByteArray {
        val prefixBytes = prefix.toByteArray(StandardCharsets.UTF_8)
        val preimage = ByteArray(prefixBytes.size + payload.size)
        System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.size)
        System.arraycopy(payload, 0, preimage, prefixBytes.size, payload.size)
        return Blake2b.digest256(preimage)
    }

    private fun keccakHashHex(prefix: String, payload: ByteArray): String {
        return "0x" + hexLower(keccakHashBytes(prefix, payload))
    }

    private fun keccakHashBytes(prefix: String, payload: ByteArray): ByteArray {
        val prefixBytes = prefix.toByteArray(StandardCharsets.UTF_8)
        val preimage = ByteArray(prefixBytes.size + payload.size)
        System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.size)
        System.arraycopy(payload, 0, preimage, prefixBytes.size, payload.size)
        return keccak256(preimage)
    }

    private fun keccak256(input: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(input, 0, input.size)
        val out = ByteArray(32)
        digest.doFinal(out, 0)
        return out
    }

    private fun sha256(input: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(input)

    private fun tronRecoverableSignatureIsCanonical(signature: ByteArray): Boolean {
        if (signature.size != 65) return false
        val recoveryId = signature[64].toInt() and 0xff
        if (recoveryId !in 0..3 && recoveryId !in 27..30) return false
        val rValue = signature.copyOfRange(0, 32)
        val sValue = signature.copyOfRange(32, 64)
        return !isZero(rValue) &&
            compareUnsignedBytes(rValue, SECP256K1_SCALAR_ORDER_BE) < 0 &&
            !isZero(sValue) &&
            compareUnsignedBytes(sValue, SECP256K1_SCALAR_HALF_ORDER_BE) <= 0
    }

    private fun tronRecoveredSignerAddress20(messageHash: ByteArray, signature: ByteArray): ByteArray? {
        if (messageHash.size != 32 || !tronRecoverableSignatureIsCanonical(signature)) return null
        val recoveryIdByte = signature[64].toInt() and 0xff
        val recoveryId = if (recoveryIdByte >= 27) recoveryIdByte - 27 else recoveryIdByte
        val rValue = BigInteger(1, signature.copyOfRange(0, 32))
        val sValue = BigInteger(1, signature.copyOfRange(32, 64))
        val x = rValue.add(SECP256K1_SCALAR_ORDER.multiply(BigInteger.valueOf((recoveryId / 2).toLong())))
        if (x >= SECP256K1_FIELD_PRIME) return null
        val compressed = ByteArray(33)
        compressed[0] = if ((recoveryId and 1) == 1) 0x03 else 0x02
        val xBytes = bigIntegerToFixedBytes(x, 32)
        System.arraycopy(xBytes, 0, compressed, 1, 32)
        val rPoint = try {
            SECP256K1_PARAMS.curve.decodePoint(compressed)
        } catch (exception: IllegalArgumentException) {
            return null
        }
        if (!rPoint.multiply(SECP256K1_SCALAR_ORDER).isInfinity) return null
        val eValue = BigInteger(1, messageHash).mod(SECP256K1_SCALAR_ORDER)
        val publicKey = rPoint
            .multiply(sValue)
            .subtract(SECP256K1_PARAMS.g.multiply(eValue))
            .multiply(rValue.modInverse(SECP256K1_SCALAR_ORDER))
            .normalize()
        if (publicKey.isInfinity) return null
        val encoded = publicKey.getEncoded(false)
        return keccak256(encoded.copyOfRange(1, encoded.size)).copyOfRange(12, 32)
    }

    private fun bigIntegerToFixedBytes(value: BigInteger, byteLength: Int): ByteArray {
        val raw = value.toByteArray()
        val out = ByteArray(byteLength)
        val copyLength = minOf(raw.size, byteLength)
        System.arraycopy(raw, raw.size - copyLength, out, byteLength - copyLength, copyLength)
        return out
    }

    private fun compareUnsignedBytes(left: ByteArray, right: ByteArray): Int {
        if (left.size != right.size) return left.size - right.size
        for (index in left.indices) {
            val leftByte = left[index].toInt() and 0xff
            val rightByte = right[index].toInt() and 0xff
            if (leftByte != rightByte) return leftByte - rightByte
        }
        return 0
    }

    private fun isZero(bytes: ByteArray): Boolean = bytes.all { it == 0.toByte() }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) {
            builder.append(String.format("%02x", byte.toInt() and 0xff))
        }
        return builder.toString()
    }
}
