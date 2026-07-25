// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/**
 * Strict JSON models for Native AMX V2 control receipts.
 *
 * Participant controls are evidence only. Parsing this model never applies
 * participant-lane economic effects; canonical application remains a global
 * carrier responsibility.
 */
object NativeAmxV2 {
    /** Current coordinated first-release receipt version. */
    const val RECEIPT_VERSION: Int = 2

    /** Maximum number of sources in one grouped participant settlement. */
    const val MAX_GROUP_SOURCES: Int = 4_096

    /** Maximum number of participant legs carried by one receipt. */
    const val MAX_RECEIPT_LEGS: Int = 255

    /** Maximum Native AMX validator count. */
    const val MAX_VALIDATORS: Int = 128

    /** Exact byte length of a BLS-Normal PoP or aggregate signature. */
    const val BLS_PROOF_BYTES: Int = 96

    private val U32_MAX: BigInteger = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE)
    private val U64_MAX: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val BIG_TWO: BigInteger = BigInteger.valueOf(2)
    private val BLS12_381_BASE_FIELD = BigInteger(
        "1A0111EA397FE69A4B1BA7B6434BACD7" +
            "64774B84F38512BF6730D2A0F6B0F624" +
            "1EABFFFEB153FFFFB9FEFFFFFFFFAAAB",
        16,
    )
    private val BLS12_381_SCALAR_FIELD = BigInteger(
        "73EDA753299D7D483339D80809A1D805" +
            "53BDA402FFFE5BFEFFFFFFFF00000001",
        16,
    )
    private const val DESCRIPTOR_PREIMAGE_TYPE =
        "iroha_data_model::block::consensus::LaneBlockDescriptorPreimage"
    private const val PROPOSAL_PREIMAGE_TYPE =
        "iroha_data_model::block::consensus::LaneBlockProposalPreimage"
    private const val SETTLEMENT_TYPE =
        "iroha_data_model::block::consensus::LaneBlockCommitment"

    private class BlsNormalPeerId(
        val literal: String,
        val compressedKey: ByteArray,
    ) {
        val orderingKey: ByteArray =
            byteArrayOf(2) + compressedKey
    }

    private data class JacobianPoint(
        val x: BigInteger,
        val y: BigInteger,
        val z: BigInteger,
    )

    private val BLS_NORMAL_PEER_CACHE: MutableMap<String, BlsNormalPeerId> =
        Collections.synchronizedMap(
            object : LinkedHashMap<String, BlsNormalPeerId>(512, 0.75f, true) {
                override fun removeEldestEntry(
                    eldest: MutableMap.MutableEntry<String, BlsNormalPeerId>?,
                ): Boolean = size > 512
            },
        )

    /** Typed prepare/commit phase from the mandatory tagged phase object. */
    enum class Phase(private val wireName: String) {
        /** Participant Prepare certificate. */
        PREPARE("prepare"),

        /** Participant Commit certificate. */
        COMMIT("commit"),
        ;

        companion object {
            internal fun fromWire(value: String): Phase =
                entries.firstOrNull { it.wireName == value }
                    ?: throw IllegalArgumentException("unsupported Native AMX V2 phase")
        }
    }

    /** Exact raw 32-byte source identity, encoded as uppercase hexadecimal. */
    class SourceId(val value: String) {
        init {
            require(SOURCE_ID.matches(value)) {
                "Native AMX source ID must be exactly 32 uppercase hexadecimal bytes"
            }
        }

        override fun equals(other: Any?): Boolean =
            other is SourceId && value == other.value

        override fun hashCode(): Int = value.hashCode()

        override fun toString(): String = value
    }

    /** Typed transaction-entrypoint hash; it cannot be substituted by a source ID. */
    class TransactionEntrypointHash(val value: String) {
        init {
            requireCanonicalNonzeroHash(value, "transaction entrypoint hash")
        }

        override fun equals(other: Any?): Boolean =
            other is TransactionEntrypointHash && value == other.value

        override fun hashCode(): Int = value.hashCode()

        override fun toString(): String = value
    }

    /** Canonical non-zero Iroha hash literal used by Native AMX controls. */
    class ConsensusHash(val value: String) {
        init {
            requireCanonicalNonzeroHash(value, "Native AMX hash")
        }

        override fun equals(other: Any?): Boolean =
            other is ConsensusHash && value == other.value

        override fun hashCode(): Int = value.hashCode()

        override fun toString(): String = value
    }

    /** Immutable bounded byte string. */
    class Bytes private constructor(bytes: ByteArray) {
        private val value = bytes.copyOf()

        /** Return a defensive copy of the bytes. */
        fun toByteArray(): ByteArray = value.copyOf()

        /** Byte length. */
        val size: Int
            get() = value.size

        internal fun unsignedByte(index: Int): Int = value[index].toInt() and 0xff

        internal fun countOneBits(): Int =
            value.sumOf { Integer.bitCount(it.toInt() and 0xff) }

        override fun equals(other: Any?): Boolean =
            other is Bytes && value.contentEquals(other.value)

        override fun hashCode(): Int = value.contentHashCode()

        companion object {
            internal fun fromJson(value: Any?, path: String): Bytes {
                val items = array(value, path)
                val bytes = ByteArray(items.size)
                items.forEachIndexed { index, item ->
                    val number = boundedInt(item, "$path[$index]", 0, 0xff)
                    bytes[index] = number.toByte()
                }
                return Bytes(bytes)
            }
        }
    }

    /** Frozen global consensus round. Its view is independent of lane-local views. */
    class Round(
        val contextId: ConsensusHash,
        val height: BigInteger,
        val view: BigInteger,
    ) {
        override fun equals(other: Any?): Boolean =
            other is Round &&
                contextId == other.contextId &&
                height == other.height &&
                view == other.view

        override fun hashCode(): Int = listOf(contextId, height, view).hashCode()
    }

    /** Signed Native AMX V2 participant-attestation body. */
    class AttestationBody internal constructor(
        val round: Round,
        val epoch: BigInteger,
        val chainIdHash: ConsensusHash,
        val sourceId: SourceId,
        val transactionEntrypointHash: TransactionEntrypointHash,
        val planDigest: ConsensusHash,
        val phase: Phase,
        val coordinatorLaneId: Long,
        val coordinatorDataspaceId: BigInteger,
        val coordinatorLaneIncarnation: ConsensusHash,
        val participantLaneId: Long,
        val participantDataspaceId: BigInteger,
        val participantLaneIncarnation: ConsensusHash,
        val participantPreviousBlockHeight: BigInteger,
        val participantPreviousBlockDescriptorHash: ConsensusHash?,
        val participantLaneBlockHeight: BigInteger,
        val participantLaneBlockView: BigInteger,
        val participantProposalHash: ConsensusHash,
        val participantSettlementCommitment: ConsensusHash,
        val participantValidatorSetHash: ConsensusHash,
        val participantValidatorCount: Int,
        val participantMinQuorum: Int,
        val authorityContextHeight: BigInteger,
        val plannedCoordinatorBlockHeight: BigInteger,
        val coordinatorLaneBlockView: BigInteger,
        val coordinatorProposalHash: ConsensusHash,
    ) {
        internal fun hasSameIdentity(other: AttestationBody): Boolean =
            identity() == other.identity()

        private fun identity(): List<Any?> = listOf(
            round,
            epoch,
            chainIdHash,
            sourceId,
            transactionEntrypointHash,
            planDigest,
            coordinatorLaneId,
            coordinatorDataspaceId,
            coordinatorLaneIncarnation,
            participantLaneId,
            participantDataspaceId,
            participantLaneIncarnation,
            participantPreviousBlockHeight,
            participantPreviousBlockDescriptorHash,
            participantLaneBlockHeight,
            participantLaneBlockView,
            participantProposalHash,
            participantSettlementCommitment,
            participantValidatorSetHash,
            participantValidatorCount,
            participantMinQuorum,
            authorityContextHeight,
            plannedCoordinatorBlockHeight,
            coordinatorLaneBlockView,
            coordinatorProposalHash,
        )

        override fun equals(other: Any?): Boolean =
            other is AttestationBody && phase == other.phase && hasSameIdentity(other)

        override fun hashCode(): Int = 31 * identity().hashCode() + phase.hashCode()
    }

    /** Validator-set proof for one Native AMX attestation body. */
    class AttestationQc internal constructor(
        val body: AttestationBody,
        val validatorSetHashVersion: Int,
        val validatorSetHash: ConsensusHash,
        validatorSet: List<String>,
        validatorSetPops: List<Bytes>,
        val signersBitmap: Bytes,
        val aggregateSignature: Bytes,
    ) {
        val validatorSet: List<String> =
            Collections.unmodifiableList(validatorSet.toList())
        val validatorSetPops: List<Bytes> =
            Collections.unmodifiableList(validatorSetPops.toList())

        internal fun hasSameCommittee(other: AttestationQc): Boolean =
            validatorSetHashVersion == other.validatorSetHashVersion &&
                validatorSetHash == other.validatorSetHash &&
                validatorSet == other.validatorSet &&
                validatorSetPops == other.validatorSetPops

        override fun equals(other: Any?): Boolean =
            other is AttestationQc &&
                body == other.body &&
                hasSameCommittee(other) &&
                signersBitmap == other.signersBitmap &&
                aggregateSignature == other.aggregateSignature

        override fun hashCode(): Int = listOf(
            body,
            validatorSetHashVersion,
            validatorSetHash,
            validatorSet,
            validatorSetPops,
            signersBitmap,
            aggregateSignature,
        ).hashCode()
    }

    /** One zero-effect source row in a participant settlement. */
    class SettlementReceipt internal constructor(
        val sourceId: SourceId,
        val localAmount: String,
        val xorDue: String,
        val xorAfterHaircut: String,
        val xorVariance: String,
        val timestampMs: BigInteger,
    ) {
        override fun equals(other: Any?): Boolean =
            other is SettlementReceipt &&
                sourceId == other.sourceId &&
                localAmount == other.localAmount &&
                xorDue == other.xorDue &&
                xorAfterHaircut == other.xorAfterHaircut &&
                xorVariance == other.xorVariance &&
                timestampMs == other.timestampMs

        override fun hashCode(): Int = listOf(
            sourceId,
            localAmount,
            xorDue,
            xorAfterHaircut,
            xorVariance,
            timestampMs,
        ).hashCode()
    }

    /** Exact terminal participant settlement certified by a Native AMX leg. */
    class ParticipantSettlement internal constructor(
        val blockHeight: BigInteger,
        val laneId: Long,
        val laneIncarnation: ConsensusHash,
        val dataspaceId: BigInteger,
        val transactionCount: Long,
        val totalLocalAmount: String,
        val totalXorDue: String,
        val totalXorAfterHaircut: String,
        val totalXorVariance: String,
        receipts: List<SettlementReceipt>,
    ) {
        val receipts: List<SettlementReceipt> =
            Collections.unmodifiableList(receipts.toList())

        override fun equals(other: Any?): Boolean =
            other is ParticipantSettlement &&
                blockHeight == other.blockHeight &&
                laneId == other.laneId &&
                laneIncarnation == other.laneIncarnation &&
                dataspaceId == other.dataspaceId &&
                transactionCount == other.transactionCount &&
                totalLocalAmount == other.totalLocalAmount &&
                totalXorDue == other.totalXorDue &&
                totalXorAfterHaircut == other.totalXorAfterHaircut &&
                totalXorVariance == other.totalXorVariance &&
                receipts == other.receipts

        override fun hashCode(): Int = listOf(
            blockHeight,
            laneId,
            laneIncarnation,
            dataspaceId,
            transactionCount,
            totalLocalAmount,
            totalXorDue,
            totalXorAfterHaircut,
            totalXorVariance,
            receipts,
        ).hashCode()
    }

    /** Exact control-only participant lane-block descriptor. */
    class ParticipantDescriptor internal constructor(
        val laneId: Long,
        val dataspaceId: BigInteger,
        val laneIncarnation: ConsensusHash,
        val proposalHeight: BigInteger,
        val previousLaneBlockHeight: BigInteger,
        val previousLaneBlockDescriptorHash: ConsensusHash?,
        val laneBlockHeight: BigInteger,
        val laneBlockView: BigInteger,
        val subjectHash: ConsensusHash,
        val payloadOwnershipHash: ConsensusHash,
        val rbcInstanceHash: ConsensusHash,
        acceptedCandidateIndices: List<BigInteger>,
        acceptedTransactionHashes: List<TransactionEntrypointHash>,
        val validatorSetHashVersion: Int,
        val validatorSetHash: ConsensusHash,
        validatorSet: List<String>,
        val validatorCount: Int,
        val minQuorum: Int,
        val qcModeTag: String,
        val descriptorHash: ConsensusHash,
    ) {
        val acceptedCandidateIndices: List<BigInteger> =
            Collections.unmodifiableList(acceptedCandidateIndices.toList())
        val acceptedTransactionHashes: List<TransactionEntrypointHash> =
            Collections.unmodifiableList(acceptedTransactionHashes.toList())
        val validatorSet: List<String> =
            Collections.unmodifiableList(validatorSet.toList())

        override fun equals(other: Any?): Boolean =
            other is ParticipantDescriptor &&
                laneId == other.laneId &&
                dataspaceId == other.dataspaceId &&
                laneIncarnation == other.laneIncarnation &&
                proposalHeight == other.proposalHeight &&
                previousLaneBlockHeight == other.previousLaneBlockHeight &&
                previousLaneBlockDescriptorHash == other.previousLaneBlockDescriptorHash &&
                laneBlockHeight == other.laneBlockHeight &&
                laneBlockView == other.laneBlockView &&
                subjectHash == other.subjectHash &&
                payloadOwnershipHash == other.payloadOwnershipHash &&
                rbcInstanceHash == other.rbcInstanceHash &&
                acceptedCandidateIndices == other.acceptedCandidateIndices &&
                acceptedTransactionHashes == other.acceptedTransactionHashes &&
                validatorSetHashVersion == other.validatorSetHashVersion &&
                validatorSetHash == other.validatorSetHash &&
                validatorSet == other.validatorSet &&
                validatorCount == other.validatorCount &&
                minQuorum == other.minQuorum &&
                qcModeTag == other.qcModeTag &&
                descriptorHash == other.descriptorHash

        override fun hashCode(): Int = listOf(
            laneId,
            dataspaceId,
            laneIncarnation,
            proposalHeight,
            previousLaneBlockHeight,
            previousLaneBlockDescriptorHash,
            laneBlockHeight,
            laneBlockView,
            subjectHash,
            payloadOwnershipHash,
            rbcInstanceHash,
            acceptedCandidateIndices,
            acceptedTransactionHashes,
            validatorSetHashVersion,
            validatorSetHash,
            validatorSet,
            validatorCount,
            minQuorum,
            qcModeTag,
            descriptorHash,
        ).hashCode()
    }

    /** Participant proposal without a proposer-local recovery payload hint. */
    class ParticipantProposal internal constructor(
        val descriptor: ParticipantDescriptor,
        val proposalHash: ConsensusHash,
    ) {
        override fun equals(other: Any?): Boolean =
            other is ParticipantProposal &&
                descriptor == other.descriptor &&
                proposalHash == other.proposalHash

        override fun hashCode(): Int = 31 * descriptor.hashCode() + proposalHash.hashCode()
    }

    /** Prepare/Commit proof for one participant route. */
    class Leg internal constructor(
        val laneId: Long,
        val dataspaceId: BigInteger,
        val laneIncarnation: ConsensusHash,
        val participantProposal: ParticipantProposal,
        val participantSettlement: ParticipantSettlement,
        val participantSettlementHash: ConsensusHash,
        val prepareQc: AttestationQc,
        val commitQc: AttestationQc,
        /**
         * True when the current transaction entrypoint is absent from this
         * participant descriptor and block-wide mixed-role validation must
         * establish the anchor later.
         */
        val requiresMixedRoleAnchorValidation: Boolean,
    ) {
        override fun equals(other: Any?): Boolean =
            other is Leg &&
                laneId == other.laneId &&
                dataspaceId == other.dataspaceId &&
                laneIncarnation == other.laneIncarnation &&
                participantProposal == other.participantProposal &&
                participantSettlement == other.participantSettlement &&
                participantSettlementHash == other.participantSettlementHash &&
                prepareQc == other.prepareQc &&
                commitQc == other.commitQc &&
                requiresMixedRoleAnchorValidation == other.requiresMixedRoleAnchorValidation

        override fun hashCode(): Int = listOf(
            laneId,
            dataspaceId,
            laneIncarnation,
            participantProposal,
            participantSettlement,
            participantSettlementHash,
            prepareQc,
            commitQc,
            requiresMixedRoleAnchorValidation,
        ).hashCode()
    }

    /** Context-bound Native AMX V2 receipt for one source. */
    class Receipt internal constructor(
        val version: Int,
        val sourceId: SourceId,
        val chainIdHash: ConsensusHash,
        val planDigest: ConsensusHash,
        val laneId: Long,
        val dataspaceId: BigInteger,
        val laneIncarnation: ConsensusHash,
        val authorityContextHeight: BigInteger,
        val laneBlockHeight: BigInteger,
        val laneBlockView: BigInteger,
        val coordinatorProposalHash: ConsensusHash,
        legs: List<Leg>,
    ) {
        val legs: List<Leg> = Collections.unmodifiableList(legs.toList())

        override fun equals(other: Any?): Boolean =
            other is Receipt &&
                version == other.version &&
                sourceId == other.sourceId &&
                chainIdHash == other.chainIdHash &&
                planDigest == other.planDigest &&
                laneId == other.laneId &&
                dataspaceId == other.dataspaceId &&
                laneIncarnation == other.laneIncarnation &&
                authorityContextHeight == other.authorityContextHeight &&
                laneBlockHeight == other.laneBlockHeight &&
                laneBlockView == other.laneBlockView &&
                coordinatorProposalHash == other.coordinatorProposalHash &&
                legs == other.legs

        override fun hashCode(): Int = listOf(
            version,
            sourceId,
            chainIdHash,
            planDigest,
            laneId,
            dataspaceId,
            laneIncarnation,
            authorityContextHeight,
            laneBlockHeight,
            laneBlockView,
            coordinatorProposalHash,
            legs,
        ).hashCode()
    }

    /** One lane settlement containing an ordered Native AMX source group. */
    class ReceiptGroup internal constructor(
        val blockHeight: BigInteger,
        val laneId: Long,
        val laneIncarnation: ConsensusHash,
        val dataspaceId: BigInteger,
        val transactionCount: Long,
        receipts: List<Receipt>,
    ) {
        val receipts: List<Receipt> = Collections.unmodifiableList(receipts.toList())

        override fun equals(other: Any?): Boolean =
            other is ReceiptGroup &&
                blockHeight == other.blockHeight &&
                laneId == other.laneId &&
                laneIncarnation == other.laneIncarnation &&
                dataspaceId == other.dataspaceId &&
                transactionCount == other.transactionCount &&
                receipts == other.receipts

        override fun hashCode(): Int = listOf(
            blockHeight,
            laneId,
            laneIncarnation,
            dataspaceId,
            transactionCount,
            receipts,
        ).hashCode()
    }

    /** Return true only for a canonical, non-infinity BLS-Normal subgroup point. */
    @JvmStatic
    fun isCanonicalBlsNormalPeerId(value: String): Boolean =
        try {
            decodeBlsNormalPeerId(value, "Native AMX validator")
            true
        } catch (_: IllegalArgumentException) {
            false
        }

    /**
     * Determine whether a participant proposal needs block-wide mixed-role
     * anchor validation. The current entrypoint may occur either once or not
     * at all; duplicates are malformed.
     */
    @JvmStatic
    fun requiresMixedRoleAnchorValidation(
        descriptor: ParticipantDescriptor,
        transactionEntrypointHash: TransactionEntrypointHash,
    ): Boolean {
        val matches = descriptor.acceptedTransactionHashes.count {
            it == transactionEntrypointHash
        }
        require(matches <= 1) {
            "Native AMX participant descriptor repeats the current transaction entrypoint"
        }
        return matches == 0
    }

    /** Parse and strictly validate one Native AMX receipt-group JSON object. */
    @JvmStatic
    fun parseReceiptGroup(json: String): ReceiptGroup {
        return parseReceiptGroupValue(parseJson(json))
    }

    /** Parse UTF-8 JSON and strictly validate one Native AMX receipt group. */
    @JvmStatic
    fun parseReceiptGroup(json: ByteArray): ReceiptGroup =
        parseReceiptGroup(String(json, StandardCharsets.UTF_8))

    /** Strictly validate a map produced by the SDK JSON parser. */
    @JvmStatic
    fun parseReceiptGroup(value: Map<String, Any?>): ReceiptGroup =
        parseReceiptGroupValue(value)

    /** Parse and strictly validate one standalone Native AMX V2 receipt. */
    @JvmStatic
    fun parseReceipt(json: String): Receipt =
        parseReceipt(parseJson(json), "native AMX V2 receipt")

    /** Parse UTF-8 JSON and strictly validate one standalone receipt. */
    @JvmStatic
    fun parseReceipt(json: ByteArray): Receipt =
        parseReceipt(String(json, StandardCharsets.UTF_8))

    /** Strictly validate a standalone receipt map produced by the SDK JSON parser. */
    @JvmStatic
    fun parseReceipt(value: Map<String, Any?>): Receipt =
        parseReceipt(value, "native AMX V2 receipt")

    private fun parseJson(json: String): Any? =
        try {
            JsonParser.parse(json)
        } catch (error: IllegalStateException) {
            throw IllegalArgumentException("malformed Native AMX V2 JSON", error)
        }

    private fun parseReceiptGroupValue(value: Any?): ReceiptGroup {
        val path = "native AMX receipt group"
        val record = exactObject(value, GROUP_FIELDS, path)
        val blockHeight = positiveU64(record["block_height"], "$path.block_height")
        val laneId = laneId(record["lane_id"], "$path.lane_id")
        val laneIncarnation = hash(record["lane_incarnation"], "$path.lane_incarnation")
        val dataspaceId = unsignedU64(record["dataspace_id"], "$path.dataspace_id")
        val transactionCount = sourceCount(record["tx_count"], "$path.tx_count")
        canonicalQuantity(record["total_local_amount"], "$path.total_local_amount")
        canonicalQuantity(record["total_xor_due"], "$path.total_xor_due")
        canonicalQuantity(
            record["total_xor_after_haircut"],
            "$path.total_xor_after_haircut",
        )
        canonicalQuantity(record["total_xor_variance"], "$path.total_xor_variance")
        array(record["receipts"], "$path.receipts")
        array(record["nexus_fee_receipts"], "$path.nexus_fee_receipts")
        val nativeReceipts = array(
            record["native_amx_receipts"],
            "$path.native_amx_receipts",
        )
        require(nativeReceipts.size in 1..MAX_GROUP_SOURCES) {
            "$path.native_amx_receipts must contain 1..$MAX_GROUP_SOURCES sources"
        }
        val parsed = nativeReceipts.mapIndexed { index, receipt ->
            parseReceipt(receipt, "$path.native_amx_receipts[$index]")
        }
        val orderedSources = parsed.map(Receipt::sourceId)
        requireStrictlyOrdered(orderedSources.map(SourceId::value), "$path source IDs")
        parsed.forEachIndexed { index, receipt ->
            require(
                receipt.laneId == laneId &&
                    receipt.dataspaceId == dataspaceId &&
                    receipt.laneIncarnation == laneIncarnation &&
                    receipt.laneBlockHeight == blockHeight,
            ) {
                "$path.native_amx_receipts[$index] has mismatched coordinator coordinates"
            }
            receipt.legs.forEach { leg ->
                require(
                    leg.participantSettlement.receipts.map(SettlementReceipt::sourceId) ==
                        orderedSources,
                ) {
                    "$path.native_amx_receipts[$index] does not bind the exact ordered source group"
                }
            }
        }
        return ReceiptGroup(
            blockHeight,
            laneId,
            laneIncarnation,
            dataspaceId,
            transactionCount,
            parsed,
        )
    }

    private fun parseReceipt(value: Any?, path: String): Receipt {
        val record = exactObject(value, RECEIPT_FIELDS, path)
        val version = int(record["version"], "$path.version")
        require(version == RECEIPT_VERSION) { "$path.version must equal $RECEIPT_VERSION" }
        val sourceId = source(record["source_id"], "$path.source_id")
        val chainIdHash = hash(record["chain_id_hash"], "$path.chain_id_hash")
        val planDigest = hash(record["plan_digest"], "$path.plan_digest")
        val laneId = laneId(record["lane_id"], "$path.lane_id")
        val dataspaceId = unsignedU64(record["dataspace_id"], "$path.dataspace_id")
        val laneIncarnation = hash(record["lane_incarnation"], "$path.lane_incarnation")
        val authorityHeight = positiveU64(
            record["authority_context_height"],
            "$path.authority_context_height",
        )
        val laneBlockHeight = positiveU64(
            record["lane_block_height"],
            "$path.lane_block_height",
        )
        val laneBlockView = unsignedU64(record["lane_block_view"], "$path.lane_block_view")
        val coordinatorProposalHash = hash(
            record["coordinator_proposal_hash"],
            "$path.coordinator_proposal_hash",
        )
        val legValues = array(record["legs"], "$path.legs")
        require(legValues.size in 1..MAX_RECEIPT_LEGS) {
            "$path.legs must contain 1..$MAX_RECEIPT_LEGS routes"
        }
        val legs = legValues.mapIndexed { index, leg ->
            parseLeg(leg, "$path.legs[$index]")
        }
        require(legs.map { it.laneId to it.dataspaceId }.toSet().size == legs.size) {
            "$path.legs contains duplicate participant routes"
        }
        val firstBody = legs.first().prepareQc.body
        legs.forEachIndexed { index, leg ->
            val body = leg.prepareQc.body
            require(
                body.round == firstBody.round &&
                    body.epoch == firstBody.epoch &&
                    body.round.height == authorityHeight &&
                    body.chainIdHash == chainIdHash &&
                    body.sourceId == sourceId &&
                    body.transactionEntrypointHash == firstBody.transactionEntrypointHash &&
                    body.planDigest == planDigest &&
                    body.coordinatorLaneId == laneId &&
                    body.coordinatorDataspaceId == dataspaceId &&
                    body.coordinatorLaneIncarnation == laneIncarnation &&
                    body.authorityContextHeight == authorityHeight &&
                    body.plannedCoordinatorBlockHeight == laneBlockHeight &&
                    body.coordinatorLaneBlockView == laneBlockView &&
                    body.coordinatorProposalHash == coordinatorProposalHash,
            ) { "$path.legs[$index] carries a mismatched signed coordinator identity" }
            if (leg.laneId == laneId && leg.dataspaceId == dataspaceId) {
                val descriptor = leg.participantProposal.descriptor
                require(
                    !leg.requiresMixedRoleAnchorValidation &&
                        descriptor.laneIncarnation == laneIncarnation &&
                        descriptor.laneBlockHeight == laneBlockHeight &&
                        descriptor.laneBlockView == laneBlockView &&
                        leg.participantProposal.proposalHash == coordinatorProposalHash,
                ) { "$path.legs[$index] same-route proposal is not the coordinator identity" }
            }
        }
        return Receipt(
            version,
            sourceId,
            chainIdHash,
            planDigest,
            laneId,
            dataspaceId,
            laneIncarnation,
            authorityHeight,
            laneBlockHeight,
            laneBlockView,
            coordinatorProposalHash,
            legs,
        )
    }

    private fun parseLeg(value: Any?, path: String): Leg {
        val record = exactObject(value, LEG_FIELDS, path)
        val laneId = laneId(record["lane_id"], "$path.lane_id")
        val dataspaceId = unsignedU64(record["dataspace_id"], "$path.dataspace_id")
        val proposal = parseProposal(record["participant_proposal"], "$path.participant_proposal")
        val settlement = parseParticipantSettlement(
            record["participant_settlement"],
            "$path.participant_settlement",
        )
        val settlementHash = hash(
            record["participant_settlement_hash"],
            "$path.participant_settlement_hash",
        )
        val prepareQc = parseQc(record["prepare_qc"], "$path.prepare_qc")
        val commitQc = parseQc(record["commit_qc"], "$path.commit_qc")
        require(prepareQc.body.phase == Phase.PREPARE) {
            "$path.prepare_qc carries the wrong phase"
        }
        require(commitQc.body.phase == Phase.COMMIT) {
            "$path.commit_qc carries the wrong phase"
        }
        require(prepareQc.body.hasSameIdentity(commitQc.body)) {
            "$path Prepare and Commit bodies have different identities"
        }
        require(prepareQc.hasSameCommittee(commitQc)) {
            "$path Prepare and Commit committees differ"
        }
        val body = prepareQc.body
        val descriptor = proposal.descriptor
        require(
            body.participantLaneId == laneId &&
                body.participantDataspaceId == dataspaceId &&
                descriptor.laneId == laneId &&
                descriptor.dataspaceId == dataspaceId &&
                descriptor.laneIncarnation == body.participantLaneIncarnation &&
                descriptor.proposalHeight == body.authorityContextHeight &&
                descriptor.previousLaneBlockHeight == body.participantPreviousBlockHeight &&
                descriptor.previousLaneBlockDescriptorHash ==
                body.participantPreviousBlockDescriptorHash &&
                descriptor.laneBlockHeight == body.participantLaneBlockHeight &&
                descriptor.laneBlockView == body.participantLaneBlockView &&
                proposal.proposalHash == body.participantProposalHash &&
                descriptor.validatorSetHashVersion == prepareQc.validatorSetHashVersion &&
                descriptor.validatorSetHash == prepareQc.validatorSetHash &&
                descriptor.validatorSet == prepareQc.validatorSet &&
                descriptor.validatorCount == body.participantValidatorCount &&
                descriptor.minQuorum == body.participantMinQuorum,
        ) { "$path participant proposal differs from its signed body" }

        val matchingEntrypoints = descriptor.acceptedTransactionHashes.indices.filter {
            descriptor.acceptedTransactionHashes[it] == body.transactionEntrypointHash
        }
        val requiresMixedRoleAnchorValidation =
            requiresMixedRoleAnchorValidation(
                descriptor,
                body.transactionEntrypointHash,
            )
        if (!requiresMixedRoleAnchorValidation) {
            val position = matchingEntrypoints.single()
            require(
                descriptor.acceptedCandidateIndices.size == settlement.receipts.size &&
                    descriptor.acceptedTransactionHashes.size == settlement.receipts.size &&
                    settlement.receipts[position].sourceId == body.sourceId,
            ) { "$path participant descriptor and grouped settlement are not aligned" }
        }

        val settlementSources = settlement.receipts.map(SettlementReceipt::sourceId)
        require(
            settlementHash == body.participantSettlementCommitment &&
                settlementHash == computeParticipantSettlementHash(settlement) &&
                settlement.blockHeight == body.participantLaneBlockHeight &&
                settlement.laneId == laneId &&
                settlement.dataspaceId == dataspaceId &&
                settlement.laneIncarnation == body.participantLaneIncarnation &&
                settlement.transactionCount == settlement.receipts.size.toLong() &&
                settlement.totalLocalAmount == "0" &&
                settlement.totalXorDue == "0" &&
                settlement.totalXorAfterHaircut == "0" &&
                settlement.totalXorVariance == "0" &&
                settlementSources.count { it == body.sourceId } == 1 &&
                settlement.receipts.all {
                    it.localAmount == "0" &&
                        it.xorDue == "0" &&
                        it.xorAfterHaircut == "0" &&
                        it.xorVariance == "0" &&
                        it.timestampMs == body.authorityContextHeight
                },
        ) { "$path participant settlement differs from its signed body" }
        return Leg(
            laneId,
            dataspaceId,
            body.participantLaneIncarnation,
            proposal,
            settlement,
            settlementHash,
            prepareQc,
            commitQc,
            requiresMixedRoleAnchorValidation,
        )
    }

    private fun parseQc(value: Any?, path: String): AttestationQc {
        val record = exactObject(value, QC_FIELDS, path)
        val body = parseBody(record["body"], "$path.body")
        val version = int(
            record["validator_set_hash_version"],
            "$path.validator_set_hash_version",
        )
        require(version == 1) { "$path.validator_set_hash_version must equal 1" }
        val validatorSetHash = hash(
            record["validator_set_hash"],
            "$path.validator_set_hash",
        )
        val validators = canonicalBlsNormalValidatorSet(
            array(record["validator_set"], "$path.validator_set").mapIndexed { index, item ->
                string(item, "$path.validator_set[$index]")
            },
            "$path.validator_set",
        )
        require(validators.size in 1..MAX_VALIDATORS) {
            "$path.validator_set must contain 1..$MAX_VALIDATORS validators"
        }
        val pops = array(record["validator_set_pops"], "$path.validator_set_pops")
            .mapIndexed { index, item ->
                Bytes.fromJson(item, "$path.validator_set_pops[$index]").also {
                    require(it.size == BLS_PROOF_BYTES) {
                        "$path.validator_set_pops[$index] must contain $BLS_PROOF_BYTES bytes"
                    }
                    require((0 until it.size).any { offset -> it.unsignedByte(offset) != 0 }) {
                        "$path.validator_set_pops[$index] must not be all zeroes"
                    }
                }
            }
        require(pops.size == validators.size) {
            "$path.validator_set_pops must align with validator_set"
        }
        val bitmap = Bytes.fromJson(record["signers_bitmap"], "$path.signers_bitmap")
        val expectedBitmapBytes = (validators.size + 7) / 8
        require(bitmap.size == expectedBitmapBytes) {
            "$path.signers_bitmap must contain exactly $expectedBitmapBytes bytes"
        }
        val trailingBits = validators.size % 8
        if (trailingBits != 0) {
            require(
                bitmap.unsignedByte(bitmap.size - 1) and (0xff shl trailingBits) == 0,
            ) { "$path.signers_bitmap sets an out-of-range validator bit" }
        }
        val expectedQuorum = validators.size - (validators.size - 1) / 3
        require(
            body.participantValidatorCount == validators.size &&
                body.participantMinQuorum == expectedQuorum &&
                body.participantValidatorSetHash == validatorSetHash &&
                validatorSetHash == computeValidatorSetHash(validators) &&
                bitmap.countOneBits() >= expectedQuorum,
        ) { "$path contains inconsistent validator count, quorum, hash, or bitmap" }
        val signature = Bytes.fromJson(
            record["bls_aggregate_signature"],
            "$path.bls_aggregate_signature",
        )
        require(signature.size == BLS_PROOF_BYTES) {
            "$path.bls_aggregate_signature must contain $BLS_PROOF_BYTES bytes"
        }
        require((0 until signature.size).any { signature.unsignedByte(it) != 0 }) {
            "$path.bls_aggregate_signature must not be all zeroes"
        }
        return AttestationQc(
            body,
            version,
            validatorSetHash,
            validators,
            pops,
            bitmap,
            signature,
        )
    }

    private fun parseBody(value: Any?, path: String): AttestationBody {
        val record = exactObject(value, BODY_FIELDS, path)
        val round = parseRound(record["round"], "$path.round")
        val previousHeight = unsignedU64(
            record["participant_previous_block_height"],
            "$path.participant_previous_block_height",
        )
        val previousHash = record["participant_previous_block_descriptor_hash"]?.let {
            hash(it, "$path.participant_previous_block_descriptor_hash")
        }
        val participantHeight = positiveU64(
            record["participant_lane_block_height"],
            "$path.participant_lane_block_height",
        )
        require(previousHeight.add(BigInteger.ONE) == participantHeight) {
            "$path participant block heights must be contiguous"
        }
        require((previousHeight == BigInteger.ZERO) == (previousHash == null)) {
            "$path participant predecessor hash geometry is inconsistent"
        }
        return AttestationBody(
            round = round,
            epoch = unsignedU64(record["epoch"], "$path.epoch"),
            chainIdHash = hash(record["chain_id_hash"], "$path.chain_id_hash"),
            sourceId = source(record["source_id"], "$path.source_id"),
            transactionEntrypointHash = entrypoint(
                record["tx_entrypoint_hash"],
                "$path.tx_entrypoint_hash",
            ),
            planDigest = hash(record["plan_digest"], "$path.plan_digest"),
            phase = parsePhase(record["phase"], "$path.phase"),
            coordinatorLaneId = laneId(
                record["coordinator_lane_id"],
                "$path.coordinator_lane_id",
            ),
            coordinatorDataspaceId = unsignedU64(
                record["coordinator_dataspace_id"],
                "$path.coordinator_dataspace_id",
            ),
            coordinatorLaneIncarnation = hash(
                record["coordinator_lane_incarnation"],
                "$path.coordinator_lane_incarnation",
            ),
            participantLaneId = laneId(
                record["participant_lane_id"],
                "$path.participant_lane_id",
            ),
            participantDataspaceId = unsignedU64(
                record["participant_dataspace_id"],
                "$path.participant_dataspace_id",
            ),
            participantLaneIncarnation = hash(
                record["participant_lane_incarnation"],
                "$path.participant_lane_incarnation",
            ),
            participantPreviousBlockHeight = previousHeight,
            participantPreviousBlockDescriptorHash = previousHash,
            participantLaneBlockHeight = participantHeight,
            participantLaneBlockView = unsignedU64(
                record["participant_lane_block_view"],
                "$path.participant_lane_block_view",
            ),
            participantProposalHash = hash(
                record["participant_proposal_hash"],
                "$path.participant_proposal_hash",
            ),
            participantSettlementCommitment = hash(
                record["participant_settlement_commitment"],
                "$path.participant_settlement_commitment",
            ),
            participantValidatorSetHash = hash(
                record["participant_validator_set_hash"],
                "$path.participant_validator_set_hash",
            ),
            participantValidatorCount = boundedInt(
                record["participant_validator_count"],
                "$path.participant_validator_count",
                1,
                MAX_VALIDATORS,
            ),
            participantMinQuorum = boundedInt(
                record["participant_min_quorum"],
                "$path.participant_min_quorum",
                1,
                MAX_VALIDATORS,
            ),
            authorityContextHeight = positiveU64(
                record["authority_context_height"],
                "$path.authority_context_height",
            ),
            plannedCoordinatorBlockHeight = positiveU64(
                record["planned_coordinator_block_height"],
                "$path.planned_coordinator_block_height",
            ),
            coordinatorLaneBlockView = unsignedU64(
                record["coordinator_lane_block_view"],
                "$path.coordinator_lane_block_view",
            ),
            coordinatorProposalHash = hash(
                record["coordinator_proposal_hash"],
                "$path.coordinator_proposal_hash",
            ),
        )
    }

    private fun parseRound(value: Any?, path: String): Round {
        val record = exactObject(value, ROUND_FIELDS, path)
        val context = array(record["context_id"], "$path.context_id")
        require(context.size == 1) { "$path.context_id must be a one-hash tuple" }
        return Round(
            hash(context.single(), "$path.context_id[0]"),
            positiveU64(record["height"], "$path.height"),
            unsignedU64(record["view"], "$path.view"),
        )
    }

    private fun parsePhase(value: Any?, path: String): Phase {
        val record = exactObject(value, PHASE_FIELDS, path)
        require(record["detail"] == null) { "$path.detail must be null" }
        return Phase.fromWire(string(record["phase"], "$path.phase"))
    }

    private fun parseProposal(value: Any?, path: String): ParticipantProposal {
        val record = exactObject(value, PROPOSAL_FIELDS, path)
        val descriptor = parseDescriptor(record["descriptor"], "$path.descriptor")
        val proposalHash = hash(record["proposal_hash"], "$path.proposal_hash")
        require(proposalHash == computeProposalHash(descriptor)) {
            "$path.proposal_hash does not match the canonical proposal preimage"
        }
        return ParticipantProposal(descriptor, proposalHash)
    }

    private fun parseDescriptor(value: Any?, path: String): ParticipantDescriptor {
        val record = exactObject(
            value,
            DESCRIPTOR_REQUIRED_FIELDS,
            path,
            DESCRIPTOR_OPTIONAL_FIELDS,
        )
        val previousHeight = unsignedU64(
            record["previous_lane_block_height"],
            "$path.previous_lane_block_height",
        )
        val previousHash = optionalHash(
            record,
            "previous_lane_block_descriptor_hash",
            "$path.previous_lane_block_descriptor_hash",
        )
        require((previousHeight == BigInteger.ZERO) == (previousHash == null)) {
            "$path predecessor hash geometry is inconsistent"
        }
        val laneBlockHeight = positiveU64(record["lane_block_height"], "$path.lane_block_height")
        require(previousHeight.add(BigInteger.ONE) == laneBlockHeight) {
            "$path lane block heights must be contiguous"
        }
        val candidateIndices = array(
            record["accepted_candidate_indices"],
            "$path.accepted_candidate_indices",
        ).mapIndexed { index, item ->
            unsignedU64(item, "$path.accepted_candidate_indices[$index]")
        }
        val transactionHashes = array(
            record["accepted_transaction_hashes"],
            "$path.accepted_transaction_hashes",
        ).mapIndexed { index, item ->
            entrypoint(item, "$path.accepted_transaction_hashes[$index]")
        }
        require(
            candidateIndices.size in 1..MAX_GROUP_SOURCES &&
                candidateIndices.size == transactionHashes.size &&
                candidateIndices.toSet().size == candidateIndices.size &&
                transactionHashes.toSet().size == transactionHashes.size,
        ) { "$path accepted work must be matching bounded unique lists" }
        val validators = canonicalBlsNormalValidatorSet(
            array(record["validator_set"], "$path.validator_set").mapIndexed { index, item ->
                string(item, "$path.validator_set[$index]")
            },
            "$path.validator_set",
        )
        require(validators.size in 1..MAX_VALIDATORS) {
            "$path.validator_set must contain 1..$MAX_VALIDATORS validators"
        }
        val validatorCount = boundedInt(
            record["validator_count"],
            "$path.validator_count",
            1,
            MAX_VALIDATORS,
        )
        val minQuorum = boundedInt(
            record["min_quorum"],
            "$path.min_quorum",
            1,
            MAX_VALIDATORS,
        )
        val expectedQuorum = validators.size - (validators.size - 1) / 3
        val version = int(
            record["validator_set_hash_version"],
            "$path.validator_set_hash_version",
        )
        require(
            version == 1 &&
                validatorCount == validators.size &&
                minQuorum == expectedQuorum,
        ) { "$path contains inconsistent validator version, count, or quorum" }
        val qcModeTag = string(record["qc_mode_tag"], "$path.qc_mode_tag")
        require(qcModeTag.isNotBlank() && qcModeTag == qcModeTag.trim()) {
            "$path.qc_mode_tag must be an exact non-empty string"
        }
        val validatorSetHash = hash(
            record["validator_set_hash"],
            "$path.validator_set_hash",
        )
        val descriptorHash = hash(record["descriptor_hash"], "$path.descriptor_hash")
        val descriptor = ParticipantDescriptor(
            laneId = laneId(record["lane_id"], "$path.lane_id"),
            dataspaceId = unsignedU64(record["dataspace_id"], "$path.dataspace_id"),
            laneIncarnation = hash(record["lane_incarnation"], "$path.lane_incarnation"),
            proposalHeight = positiveU64(record["proposal_height"], "$path.proposal_height"),
            previousLaneBlockHeight = previousHeight,
            previousLaneBlockDescriptorHash = previousHash,
            laneBlockHeight = laneBlockHeight,
            laneBlockView = unsignedU64(record["lane_block_view"], "$path.lane_block_view"),
            subjectHash = hash(record["subject_hash"], "$path.subject_hash"),
            payloadOwnershipHash = hash(
                record["payload_ownership_hash"],
                "$path.payload_ownership_hash",
            ),
            rbcInstanceHash = hash(record["rbc_instance_hash"], "$path.rbc_instance_hash"),
            acceptedCandidateIndices = candidateIndices,
            acceptedTransactionHashes = transactionHashes,
            validatorSetHashVersion = version,
            validatorSetHash = validatorSetHash,
            validatorSet = validators,
            validatorCount = validatorCount,
            minQuorum = minQuorum,
            qcModeTag = qcModeTag,
            descriptorHash = descriptorHash,
        )
        require(
            validatorSetHash == computeValidatorSetHash(validators) &&
                descriptorHash == computeDescriptorHash(descriptor),
        ) { "$path validator-set or descriptor hash does not match its canonical preimage" }
        return descriptor
    }

    private fun parseParticipantSettlement(value: Any?, path: String): ParticipantSettlement {
        val record = exactObject(value, GROUP_FIELDS, path)
        require(record["swap_metadata"] == null) {
            "$path.swap_metadata must be null for control-only participant settlement"
        }
        require(array(record["nexus_fee_receipts"], "$path.nexus_fee_receipts").isEmpty()) {
            "$path.nexus_fee_receipts must be empty"
        }
        require(array(record["native_amx_receipts"], "$path.native_amx_receipts").isEmpty()) {
            "$path.native_amx_receipts must be empty"
        }
        val receiptValues = array(record["receipts"], "$path.receipts")
        require(receiptValues.size in 1..MAX_GROUP_SOURCES) {
            "$path.receipts must contain 1..$MAX_GROUP_SOURCES grouped sources"
        }
        val receipts = receiptValues.mapIndexed { index, receipt ->
            parseSettlementReceipt(receipt, "$path.receipts[$index]")
        }
        requireStrictlyOrdered(receipts.map { it.sourceId.value }, "$path.receipts source IDs")
        return ParticipantSettlement(
            blockHeight = positiveU64(record["block_height"], "$path.block_height"),
            laneId = laneId(record["lane_id"], "$path.lane_id"),
            laneIncarnation = hash(record["lane_incarnation"], "$path.lane_incarnation"),
            dataspaceId = unsignedU64(record["dataspace_id"], "$path.dataspace_id"),
            transactionCount = sourceCount(record["tx_count"], "$path.tx_count"),
            totalLocalAmount = canonicalQuantity(
                record["total_local_amount"],
                "$path.total_local_amount",
            ),
            totalXorDue = canonicalQuantity(record["total_xor_due"], "$path.total_xor_due"),
            totalXorAfterHaircut = canonicalQuantity(
                record["total_xor_after_haircut"],
                "$path.total_xor_after_haircut",
            ),
            totalXorVariance = canonicalQuantity(
                record["total_xor_variance"],
                "$path.total_xor_variance",
            ),
            receipts = receipts,
        )
    }

    private fun parseSettlementReceipt(value: Any?, path: String): SettlementReceipt {
        val record = exactObject(value, SETTLEMENT_RECEIPT_FIELDS, path)
        return SettlementReceipt(
            source(record["source_id"], "$path.source_id"),
            canonicalQuantity(record["local_amount"], "$path.local_amount"),
            canonicalQuantity(record["xor_due"], "$path.xor_due"),
            canonicalQuantity(record["xor_after_haircut"], "$path.xor_after_haircut"),
            canonicalQuantity(record["xor_variance"], "$path.xor_variance"),
            unsignedU64(record["timestamp_ms"], "$path.timestamp_ms"),
        )
    }

    private fun exactObject(
        value: Any?,
        required: Set<String>,
        path: String,
        optional: Set<String> = emptySet(),
    ): Map<String, Any?> {
        val record = objectValue(value, path)
        val allowed = required + optional
        val unknown = record.keys - allowed
        require(unknown.isEmpty()) { "$path contains unknown field `${unknown.sorted().first()}`" }
        val missing = required - record.keys
        require(missing.isEmpty()) { "$path is missing required field `${missing.sorted().first()}`" }
        return record
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, path: String): Map<String, Any?> {
        require(value is Map<*, *>) { "$path must be a JSON object" }
        require(value.keys.all { it is String }) { "$path contains a non-string key" }
        return value as Map<String, Any?>
    }

    @Suppress("UNCHECKED_CAST")
    private fun array(value: Any?, path: String): List<Any?> {
        require(value is List<*>) { "$path must be a JSON array" }
        return value as List<Any?>
    }

    private fun string(value: Any?, path: String): String {
        require(value is String) { "$path must be a string" }
        return value
    }

    private fun int(value: Any?, path: String): Int {
        val parsed = unsignedU64(value, path)
        require(parsed <= BigInteger.valueOf(Int.MAX_VALUE.toLong())) {
            "$path must fit in a signed 32-bit integer"
        }
        return parsed.toInt()
    }

    private fun boundedInt(value: Any?, path: String, minimum: Int, maximum: Int): Int {
        val parsed = int(value, path)
        require(parsed in minimum..maximum) { "$path must be in $minimum..$maximum" }
        return parsed
    }

    private fun laneId(value: Any?, path: String): Long {
        val parsed = unsignedU64(value, path)
        require(parsed <= U32_MAX) { "$path must fit in an unsigned 32-bit integer" }
        return parsed.toLong()
    }

    private fun sourceCount(value: Any?, path: String): Long =
        boundedInt(value, path, 1, MAX_GROUP_SOURCES).toLong()

    private fun positiveU64(value: Any?, path: String): BigInteger {
        val parsed = unsignedU64(value, path)
        require(parsed.signum() > 0) { "$path must be positive" }
        return parsed
    }

    private fun unsignedU64(value: Any?, path: String): BigInteger {
        require(
            value is Byte ||
                value is Short ||
                value is Int ||
                value is Long ||
                value is BigInteger,
        ) { "$path must be an integer" }
        val parsed = when (value) {
            is BigInteger -> value
            is Byte -> BigInteger.valueOf(value.toLong())
            is Short -> BigInteger.valueOf(value.toLong())
            is Int -> BigInteger.valueOf(value.toLong())
            is Long -> BigInteger.valueOf(value)
            else -> error("unreachable")
        }
        require(parsed.signum() >= 0 && parsed <= U64_MAX) {
            "$path must fit in an unsigned 64-bit integer"
        }
        return parsed
    }

    private fun source(value: Any?, path: String): SourceId =
        SourceId(string(value, path))

    private fun entrypoint(value: Any?, path: String): TransactionEntrypointHash =
        TransactionEntrypointHash(string(value, path))

    private fun hash(value: Any?, path: String): ConsensusHash =
        ConsensusHash(string(value, path))

    private fun optionalHash(
        record: Map<String, Any?>,
        field: String,
        path: String,
    ): ConsensusHash? =
        if (record.containsKey(field)) {
            val value = record[field]
            require(value != null) { "$path must not be null when present" }
            hash(value, path)
        } else {
            null
        }

    private fun canonicalQuantity(value: Any?, path: String): String {
        val text = string(value, path)
        require(text.length <= 155 && QUANTITY.matches(text)) {
            "$path must be a canonical bounded non-negative quantity"
        }
        return text
    }

    private fun canonicalBlsNormalValidatorSet(
        values: List<String>,
        path: String,
    ): List<String> {
        val decoded = values.mapIndexed { index, value ->
            decodeBlsNormalPeerId(value, "$path[$index]")
        }
        require(
            decoded.zipWithNext().all { (left, right) ->
                compareUnsigned(left.orderingKey, right.orderingKey) < 0
            },
        ) { "$path must be strictly ordered by canonical BLS-Normal PeerId" }
        return decoded.map(BlsNormalPeerId::literal)
    }

    private fun decodeBlsNormalPeerId(
        value: String,
        path: String,
    ): BlsNormalPeerId {
        require(value == value.trim()) { "$path must not contain surrounding whitespace" }
        val bare =
            if (value.startsWith("bls_normal:")) {
                value.removePrefix("bls_normal:")
            } else {
                value
            }
        require(BLS_NORMAL_PEER_ID.matches(bare)) {
            "$path must be a canonical BLS-Normal PeerId"
        }
        synchronized(BLS_NORMAL_PEER_CACHE) {
            BLS_NORMAL_PEER_CACHE[bare]?.let { return it }
        }

        val compressed = hexBytes(bare.substring(6))
        val first = compressed[0].toInt() and 0xff
        val compressedFlag = first and 0x80 != 0
        val infinityFlag = first and 0x40 != 0
        val signFlag = first and 0x20 != 0
        val xBytes = compressed.copyOf()
        xBytes[0] = (first and 0x1f).toByte()
        val x = BigInteger(1, xBytes)
        require(compressedFlag && !infinityFlag && x < BLS12_381_BASE_FIELD) {
            "$path contains an invalid BLS-Normal compressed point"
        }
        val rhs =
            x.modPow(BigInteger.valueOf(3), BLS12_381_BASE_FIELD)
                .add(BigInteger.valueOf(4))
                .mod(BLS12_381_BASE_FIELD)
        var y = rhs.modPow(
            BLS12_381_BASE_FIELD.add(BigInteger.ONE).shiftRight(2),
            BLS12_381_BASE_FIELD,
        )
        require(y.multiply(y).mod(BLS12_381_BASE_FIELD) == rhs) {
            "$path contains an invalid BLS-Normal compressed point"
        }
        if ((y.shiftLeft(1) > BLS12_381_BASE_FIELD) != signFlag) {
            y = BLS12_381_BASE_FIELD.subtract(y)
        }
        require(isInBlsNormalSubgroup(x, y)) {
            "$path contains a non-subgroup BLS-Normal public key"
        }
        val decoded = BlsNormalPeerId(bare, compressed)
        synchronized(BLS_NORMAL_PEER_CACHE) {
            return BLS_NORMAL_PEER_CACHE[bare] ?: decoded.also {
                BLS_NORMAL_PEER_CACHE[bare] = it
            }
        }
    }

    private fun isInBlsNormalSubgroup(x: BigInteger, y: BigInteger): Boolean {
        var result = JacobianPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ZERO)
        for (bit in BLS12_381_SCALAR_FIELD.bitLength() - 1 downTo 0) {
            result = jacobianDouble(result)
            if (BLS12_381_SCALAR_FIELD.testBit(bit)) {
                result = jacobianAddAffine(result, x, y)
            }
        }
        return result.z == BigInteger.ZERO
    }

    private fun jacobianDouble(point: JacobianPoint): JacobianPoint {
        if (point.z == BigInteger.ZERO || point.y == BigInteger.ZERO) {
            return JacobianPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ZERO)
        }
        val modulus = BLS12_381_BASE_FIELD
        val a = point.x.multiply(point.x).mod(modulus)
        val b = point.y.multiply(point.y).mod(modulus)
        val c = b.multiply(b).mod(modulus)
        val d = BIG_TWO.multiply(
            point.x.add(b).pow(2).subtract(a).subtract(c),
        ).mod(modulus)
        val e = BigInteger.valueOf(3).multiply(a).mod(modulus)
        val f = e.multiply(e).mod(modulus)
        val x3 = f.subtract(BIG_TWO.multiply(d)).mod(modulus)
        val y3 = e.multiply(d.subtract(x3))
            .subtract(BigInteger.valueOf(8).multiply(c))
            .mod(modulus)
        val z3 = BIG_TWO.multiply(point.y).multiply(point.z).mod(modulus)
        return JacobianPoint(x3, y3, z3)
    }

    private fun jacobianAddAffine(
        point: JacobianPoint,
        affineX: BigInteger,
        affineY: BigInteger,
    ): JacobianPoint {
        if (point.z == BigInteger.ZERO) {
            return JacobianPoint(affineX, affineY, BigInteger.ONE)
        }
        val modulus = BLS12_381_BASE_FIELD
        val z1Squared = point.z.multiply(point.z).mod(modulus)
        val u2 = affineX.multiply(z1Squared).mod(modulus)
        val s2 = affineY.multiply(z1Squared).multiply(point.z).mod(modulus)
        val h = u2.subtract(point.x).mod(modulus)
        if (h == BigInteger.ZERO) {
            return if (s2 == point.y) {
                jacobianDouble(point)
            } else {
                JacobianPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ZERO)
            }
        }
        val hh = h.multiply(h).mod(modulus)
        val i = BigInteger.valueOf(4).multiply(hh).mod(modulus)
        val j = h.multiply(i).mod(modulus)
        val r = BIG_TWO.multiply(s2.subtract(point.y)).mod(modulus)
        val v = point.x.multiply(i).mod(modulus)
        val x3 = r.multiply(r)
            .subtract(j)
            .subtract(BIG_TWO.multiply(v))
            .mod(modulus)
        val y3 = r.multiply(v.subtract(x3))
            .subtract(BIG_TWO.multiply(point.y).multiply(j))
            .mod(modulus)
        val z3 = point.z.add(h).pow(2)
            .subtract(z1Squared)
            .subtract(hh)
            .mod(modulus)
        return JacobianPoint(x3, y3, z3)
    }

    private fun computeValidatorSetHash(validators: List<String>): ConsensusHash =
        ConsensusHash(hashLiteral(validatorVector(validators)))

    private fun computeDescriptorHash(descriptor: ParticipantDescriptor): ConsensusHash {
        val predecessor =
            descriptor.previousLaneBlockDescriptorHash?.let {
                byteArrayOf(1) + field(hashBytes(it))
            } ?: byteArrayOf(0)
        val payload = structure(
            listOf(
                stringBytes("nexus:lane-block-descriptor:v1"),
                byteArrayOf(1),
                field(u32(descriptor.laneId)),
                field(u64(descriptor.dataspaceId)),
                hashBytes(descriptor.laneIncarnation),
                u64(descriptor.proposalHeight),
                u64(descriptor.previousLaneBlockHeight),
                predecessor,
                u64(descriptor.laneBlockHeight),
                u64(descriptor.laneBlockView),
                hashBytes(descriptor.subjectHash),
                hashBytes(descriptor.payloadOwnershipHash),
                hashBytes(descriptor.rbcInstanceHash),
                vector(descriptor.acceptedCandidateIndices, ::u64),
                vector(descriptor.acceptedTransactionHashes) { hashBytes(it.value) },
                u16(descriptor.validatorSetHashVersion),
                hashBytes(descriptor.validatorSetHash),
                validatorVector(descriptor.validatorSet),
                u32(descriptor.validatorCount.toLong()),
                u32(descriptor.minQuorum.toLong()),
                stringBytes(descriptor.qcModeTag),
            ),
        )
        return ConsensusHash(
            hashLiteral(noritoFrame(DESCRIPTOR_PREIMAGE_TYPE, payload)),
        )
    }

    private fun computeProposalHash(descriptor: ParticipantDescriptor): ConsensusHash {
        val payload = structure(
            listOf(
                stringBytes("nexus:lane-block-proposal:v1"),
                byteArrayOf(1),
                u64(descriptor.proposalHeight),
                hashBytes(descriptor.descriptorHash),
                field(u32(descriptor.laneId)),
                field(u64(descriptor.dataspaceId)),
                hashBytes(descriptor.laneIncarnation),
                u64(descriptor.laneBlockHeight),
                u64(descriptor.laneBlockView),
                hashBytes(descriptor.subjectHash),
                hashBytes(descriptor.payloadOwnershipHash),
                hashBytes(descriptor.rbcInstanceHash),
                vector(descriptor.acceptedCandidateIndices, ::u64),
                vector(descriptor.acceptedTransactionHashes) { hashBytes(it.value) },
                u16(descriptor.validatorSetHashVersion),
                hashBytes(descriptor.validatorSetHash),
                validatorVector(descriptor.validatorSet),
                u32(descriptor.validatorCount.toLong()),
                u32(descriptor.minQuorum.toLong()),
                stringBytes(descriptor.qcModeTag),
            ),
        )
        return ConsensusHash(
            hashLiteral(noritoFrame(PROPOSAL_PREIMAGE_TYPE, payload)),
        )
    }

    private fun computeParticipantSettlementHash(
        settlement: ParticipantSettlement,
    ): ConsensusHash {
        val payload = structure(
            listOf(
                u64(settlement.blockHeight),
                field(u32(settlement.laneId)),
                hashBytes(settlement.laneIncarnation),
                field(u64(settlement.dataspaceId)),
                u64(BigInteger.valueOf(settlement.transactionCount)),
                quantity(settlement.totalLocalAmount),
                quantity(settlement.totalXorDue),
                quantity(settlement.totalXorAfterHaircut),
                quantity(settlement.totalXorVariance),
                byteArrayOf(0),
                vector(settlement.receipts, ::settlementReceipt),
                vector(emptyList<ByteArray>()) { it },
                vector(emptyList<ByteArray>()) { it },
            ),
        )
        return ConsensusHash(hashLiteral(noritoFrame(SETTLEMENT_TYPE, payload)))
    }

    private fun settlementReceipt(receipt: SettlementReceipt): ByteArray =
        structure(
            listOf(
                hexBytes(receipt.sourceId.value),
                quantity(receipt.localAmount),
                quantity(receipt.xorDue),
                quantity(receipt.xorAfterHaircut),
                quantity(receipt.xorVariance),
                u64(receipt.timestampMs),
            ),
        )

    private fun quantity(value: String): ByteArray {
        val separator = value.indexOf('.')
        val whole = if (separator < 0) value else value.substring(0, separator)
        val fraction = if (separator < 0) "" else value.substring(separator + 1)
        val mantissa = BigInteger(whole + fraction)
        val signedLittleEndian =
            if (mantissa == BigInteger.ZERO) {
                ByteArray(0)
            } else {
                mantissa.toByteArray().reversedArray()
            }
        return structure(
            listOf(
                u32(signedLittleEndian.size.toLong()) + signedLittleEndian,
                u32(fraction.length.toLong()),
            ),
        )
    }

    private fun validatorVector(validators: List<String>): ByteArray =
        vector(validators) { validator ->
            val peer = decodeBlsNormalPeerId(validator, "Native AMX validator")
            val compactKey = byteArrayOf(2) + peer.compressedKey
            val encodedKey = ByteArrayOutputStream()
            write(encodedKey, u64(BigInteger.valueOf(compactKey.size.toLong())))
            compactKey.forEach { byte ->
                write(encodedKey, field(byteArrayOf(byte)))
            }
            field(encodedKey.toByteArray())
        }

    private fun <T> vector(values: List<T>, encoder: (T) -> ByteArray): ByteArray {
        val output = ByteArrayOutputStream()
        write(output, u64(BigInteger.valueOf(values.size.toLong())))
        values.forEach { value ->
            write(output, field(encoder(value)))
        }
        return output.toByteArray()
    }

    private fun structure(fields: List<ByteArray>): ByteArray {
        val output = ByteArrayOutputStream()
        fields.forEach { value -> write(output, field(value)) }
        return output.toByteArray()
    }

    private fun field(payload: ByteArray): ByteArray =
        compactLength(payload.size) + payload

    private fun stringBytes(value: String): ByteArray {
        val encoded = value.toByteArray(StandardCharsets.UTF_8)
        return compactLength(encoded.size) + encoded
    }

    private fun compactLength(value: Int): ByteArray {
        require(value >= 0) { "compact length must not be negative" }
        var remaining = value.toLong()
        val output = ByteArrayOutputStream()
        do {
            var byte = (remaining and 0x7f).toInt()
            remaining = remaining ushr 7
            if (remaining != 0L) {
                byte = byte or 0x80
            }
            output.write(byte)
        } while (remaining != 0L)
        return output.toByteArray()
    }

    private fun u16(value: Int): ByteArray =
        byteArrayOf(
            (value and 0xff).toByte(),
            (value ushr 8 and 0xff).toByte(),
        )

    private fun u32(value: Long): ByteArray =
        ByteArray(4) { index ->
            (value ushr (index * 8) and 0xff).toByte()
        }

    private fun u64(value: BigInteger): ByteArray {
        require(value.signum() >= 0 && value <= U64_MAX) {
            "value must fit in an unsigned 64-bit integer"
        }
        return ByteArray(8) { index ->
            value.shiftRight(index * 8).and(BigInteger.valueOf(0xff)).toByte()
        }
    }

    private fun hashBytes(value: ConsensusHash): ByteArray =
        HashLiteral.decode(value.value)

    private fun hashBytes(value: String): ByteArray =
        HashLiteral.decode(value)

    private fun hashLiteral(payload: ByteArray): String =
        HashLiteral.canonicalize(Blake2b.digest256(payload))

    private fun noritoFrame(typeName: String, payload: ByteArray): ByteArray {
        val header = NoritoHeader(
            SchemaHash.hash16(typeName),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return header + payload
    }

    private fun write(output: ByteArrayOutputStream, value: ByteArray) {
        output.write(value, 0, value.size)
    }

    private fun compareUnsigned(left: ByteArray, right: ByteArray): Int {
        val shared = minOf(left.size, right.size)
        for (index in 0 until shared) {
            val comparison =
                (left[index].toInt() and 0xff).compareTo(right[index].toInt() and 0xff)
            if (comparison != 0) {
                return comparison
            }
        }
        return left.size.compareTo(right.size)
    }

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0) { "hex value must have an even length" }
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun requireStrictlyOrdered(values: List<String>, path: String) {
        require(values.zipWithNext().all { (left, right) -> left < right }) {
            "$path must be strictly ordered and unique"
        }
    }

    private fun requireCanonicalNonzeroHash(value: String, field: String) {
        require(CANONICAL_HASH.matches(value)) {
            "$field must be a canonical Iroha hash literal"
        }
        val bytes = HashLiteral.decode(value)
        require(bytes.any { it.toInt() != 0 }) { "$field must not be the zero hash" }
        require((bytes[bytes.lastIndex].toInt() and 1) == 1) {
            "$field has an invalid Iroha hash marker bit"
        }
    }

    private val SOURCE_ID = Regex("^[0-9A-F]{64}$")
    private val CANONICAL_HASH = Regex("^hash:[0-9A-F]{64}#[0-9A-F]{4}$")
    private val BLS_NORMAL_PEER_ID = Regex("^ea0130[0-9A-F]{96}$")
    private val QUANTITY = Regex("^(?:0|[1-9][0-9]*)(?:\\.[0-9]{0,27}[1-9])?$")

    private val GROUP_FIELDS = setOf(
        "block_height",
        "lane_id",
        "lane_incarnation",
        "dataspace_id",
        "tx_count",
        "total_local_amount",
        "total_xor_due",
        "total_xor_after_haircut",
        "total_xor_variance",
        "swap_metadata",
        "receipts",
        "nexus_fee_receipts",
        "native_amx_receipts",
    )
    private val RECEIPT_FIELDS = setOf(
        "version",
        "source_id",
        "chain_id_hash",
        "plan_digest",
        "lane_id",
        "dataspace_id",
        "lane_incarnation",
        "authority_context_height",
        "lane_block_height",
        "lane_block_view",
        "coordinator_proposal_hash",
        "legs",
    )
    private val LEG_FIELDS = setOf(
        "lane_id",
        "dataspace_id",
        "participant_proposal",
        "participant_settlement",
        "participant_settlement_hash",
        "prepare_qc",
        "commit_qc",
    )
    private val QC_FIELDS = setOf(
        "body",
        "validator_set_hash_version",
        "validator_set_hash",
        "validator_set",
        "validator_set_pops",
        "signers_bitmap",
        "bls_aggregate_signature",
    )
    private val BODY_FIELDS = setOf(
        "round",
        "epoch",
        "chain_id_hash",
        "source_id",
        "tx_entrypoint_hash",
        "plan_digest",
        "phase",
        "coordinator_lane_id",
        "coordinator_dataspace_id",
        "coordinator_lane_incarnation",
        "participant_lane_id",
        "participant_dataspace_id",
        "participant_lane_incarnation",
        "participant_previous_block_height",
        "participant_previous_block_descriptor_hash",
        "participant_lane_block_height",
        "participant_lane_block_view",
        "participant_proposal_hash",
        "participant_settlement_commitment",
        "participant_validator_set_hash",
        "participant_validator_count",
        "participant_min_quorum",
        "authority_context_height",
        "planned_coordinator_block_height",
        "coordinator_lane_block_view",
        "coordinator_proposal_hash",
    )
    private val ROUND_FIELDS = setOf("context_id", "height", "view")
    private val PHASE_FIELDS = setOf("phase", "detail")
    private val PROPOSAL_FIELDS = setOf("descriptor", "proposal_hash")
    private val DESCRIPTOR_REQUIRED_FIELDS = setOf(
        "lane_id",
        "dataspace_id",
        "lane_incarnation",
        "proposal_height",
        "previous_lane_block_height",
        "lane_block_height",
        "lane_block_view",
        "subject_hash",
        "payload_ownership_hash",
        "rbc_instance_hash",
        "accepted_candidate_indices",
        "accepted_transaction_hashes",
        "validator_set_hash_version",
        "validator_set_hash",
        "validator_set",
        "validator_count",
        "min_quorum",
        "qc_mode_tag",
        "descriptor_hash",
    )
    private val DESCRIPTOR_OPTIONAL_FIELDS = setOf(
        "previous_lane_block_descriptor_hash",
    )
    private val SETTLEMENT_RECEIPT_FIELDS = setOf(
        "source_id",
        "local_amount",
        "xor_due",
        "xor_after_haircut",
        "xor_variance",
        "timestamp_ms",
    )
}
