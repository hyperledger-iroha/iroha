// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.crypto.IrohaHash

/** Canonical bare-Norito models for the Sumeragi v2 consensus wire protocol. */
object SumeragiV2Wire {
    /** The only protocol revision accepted by live consensus. */
    const val PROTOCOL_VERSION: Int = 2
    /** Maximum number of real Kagemusha top-up leaves committed by one block. */
    const val MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK: Long = 16
    private val KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN =
        "iroha:kagemusha:v2:post-state-root".toByteArray(StandardCharsets.UTF_8)

    /** A 32-byte Iroha hash. The low bit of the final byte must be set. */
    class Hash32(bytes: ByteArray) {
        private val value = bytes.copyOf()

        init {
            require(value.size == 32) { "Iroha hashes must contain 32 bytes" }
            require((value[31].toInt() and 1) == 1) { "Iroha hash low bit must be set" }
        }

        fun bytes(): ByteArray = value.copyOf()

        override fun equals(other: Any?): Boolean =
            other is Hash32 && value.contentEquals(other.value)

        override fun hashCode(): Int = value.contentHashCode()
    }

    /** Exact bare-Norito payload of an Iroha `PeerId`. */
    class PeerIdPayload(encoded: ByteArray) {
        private val value = encoded.copyOf()

        init {
            require(value.isNotEmpty()) { "PeerId payload must not be empty" }
        }

        fun bytes(): ByteArray = value.copyOf()

        override fun equals(other: Any?): Boolean =
            other is PeerIdPayload && value.contentEquals(other.value)

        override fun hashCode(): Int = value.contentHashCode()
    }

    /** Canonical Norito representation of an Iroha chain identifier. */
    class ChainId(@JvmField val value: String) : WireValue() {
        init {
            requireWellFormedUtf16(value, "chain ID")
        }

        override fun encode(): ByteArray = struct(string(value))

        companion object {
            internal fun decode(bytes: ByteArray): ChainId = decodeStruct(bytes) { reader ->
                ChainId(reader.field("chain_id.value") { it.stringOnly("chain_id.value") })
            }
        }
    }

    /** Typed identifier of a frozen height context. */
    class HeightContextId(@JvmField val hash: Hash32) : WireValue() {
        override fun encode(): ByteArray = struct(hash.bytes())

        companion object {
            internal fun decode(bytes: ByteArray): HeightContextId = decodeStruct(bytes) { reader ->
                HeightContextId(Hash32(reader.field("height_context_id.hash") { it.hash() }))
            }
        }
    }

    /** Consensus round under one frozen height context. */
    class ConsensusRound(
        @JvmField val contextId: HeightContextId,
        @JvmField val height: Long,
        @JvmField val view: Long,
    ) : WireValue() {
        override fun encode(): ByteArray = struct(
            contextId.encode(),
            u64(height),
            u64(view),
        )

        companion object {
            internal fun decode(bytes: ByteArray): ConsensusRound = decodeStruct(bytes) { reader ->
                ConsensusRound(
                    reader.field("round.context_id") { HeightContextId.decode(it.remainingBytes()) },
                    reader.field("round.height") { it.u64Only("round.height") },
                    reader.field("round.view") { it.u64Only("round.view") },
                )
            }
        }
    }

    /** The only global voting phases in Sumeragi v2. */
    enum class GlobalPhase(@JvmField val discriminant: Long) {
        PREPARE(1),
        COMMIT(2),
        ;

        internal fun encode(): ByteArray = u32(discriminant)

        companion object {
            internal fun decode(bytes: ByteArray): GlobalPhase {
                val tag = Reader(bytes).u32Only("global phase")
                return entries.firstOrNull { it.discriminant == tag }
                    ?: throw IllegalArgumentException("Unknown Sumeragi v2 global phase: $tag")
            }
        }
    }

    /** Proposal subject authenticated by votes and certificates. */
    class BlockSubject(
        @JvmField val parentBlockHash: Hash32?,
        @JvmField val blockHash: Hash32,
        @JvmField val payloadHash: Hash32,
    ) : WireValue() {
        override fun encode(): ByteArray = struct(
            option(parentBlockHash?.bytes()),
            blockHash.bytes(),
            payloadHash.bytes(),
        )

        companion object {
            internal fun decode(bytes: ByteArray): BlockSubject = decodeStruct(bytes) { reader ->
                BlockSubject(
                    reader.field("subject.parent") { optionHash(it) },
                    Hash32(reader.field("subject.block") { it.hash() }),
                    Hash32(reader.field("subject.payload") { it.hash() }),
                )
            }
        }
    }

    /** Deterministic state-transition result authenticated by votes and certificates. */
    class ExecutionCommitment(
        @JvmField val parentStateRoot: Hash32,
        @JvmField val postStateRoot: Hash32,
        @JvmField val ordinaryWritesRoot: Hash32,
        @JvmField val topupAnchorRoot: Hash32?,
        @JvmField val topupAnchorCount: Long,
    ) : WireValue() {
        init {
            require(topupAnchorCount in 0..0xffff_ffffL) {
                "topupAnchorCount must fit in an unsigned 32-bit integer"
            }
            if (topupAnchorCount == 0L) {
                require(topupAnchorRoot == null) {
                    "zero top-up count must not carry an anchor root"
                }
            } else {
                require(topupAnchorRoot != null) {
                    "non-zero top-up count requires an anchor root"
                }
                require(topupAnchorCount <= MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK) {
                    "top-up anchor count exceeds the consensus bound"
                }
                require(
                    postStateRoot == topupPostStateRoot(
                        topupAnchorCount,
                        ordinaryWritesRoot,
                        topupAnchorRoot,
                    ),
                ) { "post-state root does not bind the top-up anchor projection" }
            }
        }

        override fun encode(): ByteArray = struct(
            parentStateRoot.bytes(),
            postStateRoot.bytes(),
            ordinaryWritesRoot.bytes(),
            option(topupAnchorRoot?.bytes()),
            u32(topupAnchorCount),
        )

        companion object {
            @JvmStatic
            fun withoutTopups(
                parentStateRoot: Hash32,
                postStateRoot: Hash32,
                ordinaryWritesRoot: Hash32,
            ): ExecutionCommitment = ExecutionCommitment(
                parentStateRoot,
                postStateRoot,
                ordinaryWritesRoot,
                null,
                0,
            )

            @JvmStatic
            fun topupPostStateRoot(
                topupAnchorCount: Long,
                ordinaryWritesRoot: Hash32,
                topupAnchorRoot: Hash32,
            ): Hash32 {
                require(
                    topupAnchorCount in 1..MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK,
                ) { "top-up anchor count must fit the non-empty consensus bound" }
                val preimage = ByteArrayOutputStream()
                preimage.write(KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN)
                preimage.write(0)
                preimage.write(u32(topupAnchorCount))
                preimage.write(ordinaryWritesRoot.bytes())
                preimage.write(topupAnchorRoot.bytes())
                return Hash32(IrohaHash.prehash(preimage.toByteArray()))
            }

            internal fun decode(bytes: ByteArray): ExecutionCommitment =
                decodeStruct(bytes) { reader ->
                    ExecutionCommitment(
                        Hash32(reader.field("execution.parent_state_root") { it.hash() }),
                        Hash32(reader.field("execution.post_state_root") { it.hash() }),
                        Hash32(reader.field("execution.ordinary_writes_root") { it.hash() }),
                        reader.field("execution.topup_anchor_root") { optionHash(it) },
                        reader.field("execution.topup_anchor_count") {
                            it.u32Only("execution.topup_anchor_count")
                        },
                    )
                }
        }
    }

    /** Prepare or Commit vote. */
    class Vote(
        @JvmField val round: ConsensusRound,
        @JvmField val phase: GlobalPhase,
        @JvmField val subject: BlockSubject,
        @JvmField val executionCommitment: ExecutionCommitment,
        @JvmField val signer: Long,
        signature: ByteArray,
    ) : WireValue() {
        private val signatureValue = signature.copyOf()

        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            round.encode(),
            phase.encode(),
            subject.encode(),
            executionCommitment.encode(),
            u32(signer),
            byteVector(signatureValue),
        )

        companion object {
            internal fun decode(bytes: ByteArray): Vote = decodeStruct(bytes) { reader ->
                Vote(
                    reader.field("vote.round") { ConsensusRound.decode(it.remainingBytes()) },
                    reader.field("vote.phase") { GlobalPhase.decode(it.remainingBytes()) },
                    reader.field("vote.subject") { BlockSubject.decode(it.remainingBytes()) },
                    reader.field("vote.execution_commitment") {
                        ExecutionCommitment.decode(it.remainingBytes())
                    },
                    reader.field("vote.signer") { it.u32Only("vote.signer") },
                    reader.field("vote.signature") { it.byteVectorOnly("vote.signature") },
                )
            }
        }
    }

    /** Stable reference to a quorum certificate. */
    class QuorumCertificateRef(
        @JvmField val round: ConsensusRound,
        @JvmField val phase: GlobalPhase,
        @JvmField val subject: BlockSubject,
        @JvmField val executionCommitment: ExecutionCommitment,
    ) : WireValue() {
        override fun encode(): ByteArray = struct(
            round.encode(),
            phase.encode(),
            subject.encode(),
            executionCommitment.encode(),
        )

        companion object {
            internal fun decode(bytes: ByteArray): QuorumCertificateRef =
                decodeStruct(bytes) { reader ->
                    QuorumCertificateRef(
                        reader.field("qc_ref.round") { ConsensusRound.decode(it.remainingBytes()) },
                        reader.field("qc_ref.phase") { GlobalPhase.decode(it.remainingBytes()) },
                        reader.field("qc_ref.subject") { BlockSubject.decode(it.remainingBytes()) },
                        reader.field("qc_ref.execution_commitment") {
                            ExecutionCommitment.decode(it.remainingBytes())
                        },
                    )
                }
        }
    }

    /** Aggregate Prepare or Commit certificate. */
    class QuorumCertificate(
        @JvmField val round: ConsensusRound,
        @JvmField val phase: GlobalPhase,
        @JvmField val subject: BlockSubject,
        @JvmField val executionCommitment: ExecutionCommitment,
        signers: List<Long>,
        aggregateSignature: ByteArray,
    ) : WireValue() {
        @JvmField val signers: List<Long> = signers.toList()
        private val aggregateSignatureValue = aggregateSignature.copyOf()

        init {
            requireStrictlyIncreasing(this.signers, "quorum certificate signers")
        }

        fun aggregateSignature(): ByteArray = aggregateSignatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            round.encode(),
            phase.encode(),
            subject.encode(),
            executionCommitment.encode(),
            vector(this.signers) { u32(it) },
            byteVector(aggregateSignatureValue),
        )

        fun reference(): QuorumCertificateRef =
            QuorumCertificateRef(round, phase, subject, executionCommitment)

        companion object {
            internal fun decode(bytes: ByteArray): QuorumCertificate = decodeStruct(bytes) { reader ->
                QuorumCertificate(
                    reader.field("qc.round") { ConsensusRound.decode(it.remainingBytes()) },
                    reader.field("qc.phase") { GlobalPhase.decode(it.remainingBytes()) },
                    reader.field("qc.subject") { BlockSubject.decode(it.remainingBytes()) },
                    reader.field("qc.execution_commitment") {
                        ExecutionCommitment.decode(it.remainingBytes())
                    },
                    reader.field("qc.signers") { vectorDecode(it, "qc.signers") { field ->
                        field.u32Only("qc.signer")
                    } },
                    reader.field("qc.signature") { it.byteVectorOnly("qc.signature") },
                )
            }
        }
    }

    /** One durable timeout vote. */
    class TimeoutVote(
        @JvmField val round: ConsensusRound,
        @JvmField val highestPrepareQc: QuorumCertificate?,
        @JvmField val signer: Long,
        signature: ByteArray,
    ) : WireValue() {
        private val signatureValue = signature.copyOf()

        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            round.encode(),
            option(highestPrepareQc?.encode()),
            u32(signer),
            byteVector(signatureValue),
        )

        companion object {
            internal fun decode(bytes: ByteArray): TimeoutVote = decodeStruct(bytes) { reader ->
                TimeoutVote(
                    reader.field("timeout_vote.round") { ConsensusRound.decode(it.remainingBytes()) },
                    reader.field("timeout_vote.high_qc") {
                        optionDecode(it, "timeout_vote.high_qc") { QuorumCertificate.decode(it) }
                    },
                    reader.field("timeout_vote.signer") { it.u32Only("timeout_vote.signer") },
                    reader.field("timeout_vote.signature") { it.byteVectorOnly("timeout_vote.signature") },
                )
            }
        }
    }

    /** Timeout signatures sharing the same highest PrepareQC. */
    class TimeoutVoteGroup(
        @JvmField val highestPrepareQc: QuorumCertificate?,
        signers: List<Long>,
        aggregateSignature: ByteArray,
    ) : WireValue() {
        @JvmField val signers: List<Long> = signers.toList()
        private val aggregateSignatureValue = aggregateSignature.copyOf()

        init {
            require(this.signers.isNotEmpty()) { "timeout group must contain a signer" }
            requireStrictlyIncreasing(this.signers, "timeout group signers")
        }

        fun aggregateSignature(): ByteArray = aggregateSignatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            option(highestPrepareQc?.encode()),
            vector(signers) { u32(it) },
            byteVector(aggregateSignatureValue),
        )

        companion object {
            internal fun decode(bytes: ByteArray): TimeoutVoteGroup = decodeStruct(bytes) { reader ->
                TimeoutVoteGroup(
                    reader.field("timeout_group.high_qc") {
                        optionDecode(it, "timeout_group.high_qc") { QuorumCertificate.decode(it) }
                    },
                    reader.field("timeout_group.signers") { vectorDecode(it, "timeout_group.signers") { field ->
                        field.u32Only("timeout_group.signer")
                    } },
                    reader.field("timeout_group.signature") { it.byteVectorOnly("timeout_group.signature") },
                )
            }
        }
    }

    /** Certificate authorizing the next view. */
    class TimeoutCertificate(
        @JvmField val round: ConsensusRound,
        groups: List<TimeoutVoteGroup>,
    ) : WireValue() {
        @JvmField val groups: List<TimeoutVoteGroup> = groups.toList()

        init {
            require(this.groups.isNotEmpty()) { "timeout certificate must contain a group" }
            val seen = HashSet<Long>()
            this.groups.forEach { group ->
                group.signers.forEach { signer ->
                    require(seen.add(signer)) { "timeout certificate signer groups overlap" }
                }
            }
        }

        override fun encode(): ByteArray = struct(
            round.encode(),
            vector(groups) { it.encode() },
        )

        companion object {
            internal fun decode(bytes: ByteArray): TimeoutCertificate = decodeStruct(bytes) { reader ->
                TimeoutCertificate(
                    reader.field("tc.round") { ConsensusRound.decode(it.remainingBytes()) },
                    reader.field("tc.groups") { vectorDecode(it, "tc.groups") { field ->
                        TimeoutVoteGroup.decode(field.remainingBytes())
                    } },
                )
            }
        }
    }

    /** Stable reference to a timeout certificate. */
    class TimeoutCertificateRef(
        @JvmField val round: ConsensusRound,
        @JvmField val highestPrepareQc: QuorumCertificateRef?,
        @JvmField val certificateHash: Hash32,
    ) : WireValue() {
        override fun encode(): ByteArray = struct(
            round.encode(),
            option(highestPrepareQc?.encode()),
            certificateHash.bytes(),
        )

        companion object {
            internal fun decode(bytes: ByteArray): TimeoutCertificateRef = decodeStruct(bytes) { reader ->
                TimeoutCertificateRef(
                    reader.field("tc_ref.round") { ConsensusRound.decode(it.remainingBytes()) },
                    reader.field("tc_ref.high_qc") {
                        optionDecode(it, "tc_ref.high_qc") { QuorumCertificateRef.decode(it) }
                    },
                    Hash32(reader.field("tc_ref.hash") { it.hash() }),
                )
            }
        }
    }

    /** Parent CommitQC justification for a view-zero proposal. */
    class ParentCommitJustification(@JvmField val certificate: QuorumCertificate?) : WireValue() {
        override fun encode(): ByteArray = struct(option(certificate?.encode()))

        companion object {
            internal fun decode(bytes: ByteArray): ParentCommitJustification =
                decodeStruct(bytes) { reader ->
                    ParentCommitJustification(
                        reader.field("parent_justification.certificate") {
                            optionDecode(it, "parent_justification.certificate") {
                                QuorumCertificate.decode(it)
                            }
                        },
                    )
                }
        }
    }

    /** Timeout justification for a later-view proposal. */
    class TimeoutJustification(
        @JvmField val timeoutCertificate: TimeoutCertificate,
        @JvmField val highestPrepareQc: QuorumCertificate?,
    ) : WireValue() {
        override fun encode(): ByteArray = struct(
            timeoutCertificate.encode(),
            option(highestPrepareQc?.encode()),
        )

        companion object {
            internal fun decode(bytes: ByteArray): TimeoutJustification =
                decodeStruct(bytes) { reader ->
                    TimeoutJustification(
                        reader.field("timeout_justification.tc") {
                            TimeoutCertificate.decode(it.remainingBytes())
                        },
                        reader.field("timeout_justification.high_qc") {
                            optionDecode(it, "timeout_justification.high_qc") {
                                QuorumCertificate.decode(it)
                            }
                        },
                    )
                }
        }
    }

    /** Proposal justification union. */
    sealed class ProposalJustification : WireValue() {
        class Parent(@JvmField val value: ParentCommitJustification) : ProposalJustification() {
            override fun encode(): ByteArray = enumPayload(0, value.encode())
        }

        class Timeout(@JvmField val value: TimeoutJustification) : ProposalJustification() {
            override fun encode(): ByteArray = enumPayload(1, value.encode())
        }

        companion object {
            internal fun decode(bytes: ByteArray): ProposalJustification {
                val reader = Reader(bytes)
                val tag = reader.u32("proposal justification")
                val payload = reader.compactField("proposal justification payload")
                reader.finish("proposal justification")
                return when (tag) {
                    0L -> Parent(ParentCommitJustification.decode(payload))
                    1L -> Timeout(TimeoutJustification.decode(payload))
                    else -> throw IllegalArgumentException("Unknown proposal justification: $tag")
                }
            }
        }
    }

    /** Deterministic payload encoding. */
    enum class PayloadEncoding(@JvmField val discriminant: Long) {
        PLAIN(0),
        REED_SOLOMON_16(1),
        ;

        internal fun encode(): ByteArray = u32(discriminant)

        companion object {
            internal fun decode(bytes: ByteArray): PayloadEncoding {
                val tag = Reader(bytes).u32Only("payload encoding")
                return entries.firstOrNull { it.discriminant == tag }
                    ?: throw IllegalArgumentException("Unknown payload encoding: $tag")
            }
        }
    }

    /** Payload chunking limits frozen in the height context. */
    class DataAvailabilityLayout(
        @JvmField val encoding: PayloadEncoding,
        @JvmField val chunkSizeBytes: Long,
        @JvmField val dataShards: Int,
        @JvmField val parityShards: Int,
        @JvmField val maxPayloadSizeBytes: Long,
        @JvmField val maxChunkCount: Long,
    ) : WireValue() {
        override fun encode(): ByteArray = struct(
            encoding.encode(),
            u32(chunkSizeBytes),
            u16(dataShards),
            u16(parityShards),
            u64(maxPayloadSizeBytes),
            u32(maxChunkCount),
        )

        companion object {
            internal fun decode(bytes: ByteArray): DataAvailabilityLayout =
                decodeStruct(bytes) { reader ->
                    DataAvailabilityLayout(
                        reader.field("da.encoding") { PayloadEncoding.decode(it.remainingBytes()) },
                        reader.field("da.chunk_size") { it.u32Only("da.chunk_size") },
                        reader.field("da.data_shards") { it.u16Only("da.data_shards") },
                        reader.field("da.parity_shards") { it.u16Only("da.parity_shards") },
                        reader.field("da.max_payload") { it.u64Only("da.max_payload") },
                        reader.field("da.max_chunks") { it.u32Only("da.max_chunks") },
                    )
                }
        }
    }

    /** Manifest committing to an exact canonical block payload. */
    class PayloadManifest(
        @JvmField val round: ConsensusRound,
        @JvmField val subject: BlockSubject,
        @JvmField val payloadSizeBytes: Long,
        @JvmField val layout: DataAvailabilityLayout,
        chunkHashes: List<Hash32>,
        @JvmField val chunkRoot: Hash32,
    ) : WireValue() {
        @JvmField val chunkHashes: List<Hash32> = chunkHashes.toList()

        init {
            require(this.chunkHashes.isNotEmpty()) { "payload manifest must contain a chunk hash" }
        }

        override fun encode(): ByteArray = struct(
            round.encode(),
            subject.encode(),
            u64(payloadSizeBytes),
            layout.encode(),
            vector(chunkHashes) { it.bytes() },
            chunkRoot.bytes(),
        )

        companion object {
            internal fun decode(bytes: ByteArray): PayloadManifest = decodeStruct(bytes) { reader ->
                PayloadManifest(
                    reader.field("manifest.round") { ConsensusRound.decode(it.remainingBytes()) },
                    reader.field("manifest.subject") { BlockSubject.decode(it.remainingBytes()) },
                    reader.field("manifest.size") { it.u64Only("manifest.size") },
                    reader.field("manifest.layout") { DataAvailabilityLayout.decode(it.remainingBytes()) },
                    reader.field("manifest.chunk_hashes") { vectorDecode(it, "manifest.chunk_hashes") { field ->
                        Hash32(field.hashOnly("manifest.chunk_hash"))
                    } },
                    Hash32(reader.field("manifest.chunk_root") { it.hash() }),
                )
            }
        }
    }

    /** One authenticated encoded payload chunk. */
    class PayloadChunk(
        @JvmField val manifestHash: Hash32,
        @JvmField val index: Long,
        bytes: ByteArray,
        @JvmField val sender: Long,
        signature: ByteArray,
    ) : WireValue() {
        private val bytesValue = bytes.copyOf()
        private val signatureValue = signature.copyOf()

        fun bytes(): ByteArray = bytesValue.copyOf()
        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            manifestHash.bytes(),
            u32(index),
            byteVector(bytesValue),
            u32(sender),
            byteVector(signatureValue),
        )

        companion object {
            internal fun decode(bytes: ByteArray): PayloadChunk = decodeStruct(bytes) { reader ->
                PayloadChunk(
                    Hash32(reader.field("chunk.manifest_hash") { it.hash() }),
                    reader.field("chunk.index") { it.u32Only("chunk.index") },
                    reader.field("chunk.bytes") { it.byteVectorOnly("chunk.bytes") },
                    reader.field("chunk.sender") { it.u32Only("chunk.sender") },
                    reader.field("chunk.signature") { it.byteVectorOnly("chunk.signature") },
                )
            }
        }
    }

    /** Signed proposal for one round. */
    class Proposal(
        @JvmField val round: ConsensusRound,
        @JvmField val proposer: Long,
        @JvmField val subject: BlockSubject,
        @JvmField val manifest: PayloadManifest,
        @JvmField val justification: ProposalJustification,
        signature: ByteArray,
    ) : WireValue() {
        private val signatureValue = signature.copyOf()

        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            round.encode(),
            u32(proposer),
            subject.encode(),
            manifest.encode(),
            justification.encode(),
            byteVector(signatureValue),
        )

        companion object {
            internal fun decode(bytes: ByteArray): Proposal = decodeStruct(bytes) { reader ->
                Proposal(
                    reader.field("proposal.round") { ConsensusRound.decode(it.remainingBytes()) },
                    reader.field("proposal.proposer") { it.u32Only("proposal.proposer") },
                    reader.field("proposal.subject") { BlockSubject.decode(it.remainingBytes()) },
                    reader.field("proposal.manifest") { PayloadManifest.decode(it.remainingBytes()) },
                    reader.field("proposal.justification") {
                        ProposalJustification.decode(it.remainingBytes())
                    },
                    reader.field("proposal.signature") { it.byteVectorOnly("proposal.signature") },
                )
            }
        }
    }

    /** Authenticated request for a body covered by a QC. */
    class CertifiedBodyRequest(
        @JvmField val round: ConsensusRound,
        @JvmField val subject: BlockSubject,
        @JvmField val certificate: QuorumCertificate,
        @JvmField val requester: PeerIdPayload,
        signature: ByteArray,
    ) : WireValue() {
        private val signatureValue = signature.copyOf()

        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            round.encode(),
            subject.encode(),
            certificate.encode(),
            requester.bytes(),
            byteVector(signatureValue),
        )

        companion object {
            internal fun decode(bytes: ByteArray): CertifiedBodyRequest =
                decodeStruct(bytes) { reader ->
                    CertifiedBodyRequest(
                        reader.field("body_request.round") { ConsensusRound.decode(it.remainingBytes()) },
                        reader.field("body_request.subject") { BlockSubject.decode(it.remainingBytes()) },
                        reader.field("body_request.certificate") {
                            QuorumCertificate.decode(it.remainingBytes())
                        },
                        PeerIdPayload(reader.compactField("body_request.requester")),
                        reader.field("body_request.signature") {
                            it.byteVectorOnly("body_request.signature")
                        },
                    )
                }
        }
    }

    /** Certified body response. */
    class CertifiedBodyResponse(
        @JvmField val requestHash: Hash32,
        @JvmField val manifest: PayloadManifest,
        body: ByteArray,
        @JvmField val responder: Long,
        signature: ByteArray,
    ) : WireValue() {
        private val bodyValue = body.copyOf()
        private val signatureValue = signature.copyOf()

        fun body(): ByteArray = bodyValue.copyOf()
        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            requestHash.bytes(),
            manifest.encode(),
            byteVector(bodyValue),
            u32(responder),
            byteVector(signatureValue),
        )

        companion object {
            internal fun decode(bytes: ByteArray): CertifiedBodyResponse =
                decodeStruct(bytes) { reader ->
                    CertifiedBodyResponse(
                        Hash32(reader.field("body_response.request_hash") { it.hash() }),
                        reader.field("body_response.manifest") {
                            PayloadManifest.decode(it.remainingBytes())
                        },
                        reader.field("body_response.body") { it.byteVectorOnly("body_response.body") },
                        reader.field("body_response.responder") { it.u32Only("body_response.responder") },
                        reader.field("body_response.signature") {
                            it.byteVectorOnly("body_response.signature")
                        },
                    )
                }
        }
    }

    /** Signed request for the durable CommitQC of one exact height context. */
    class CommitCertificateRequest(
        @JvmField val protocolVersion: Int,
        @JvmField val chainId: ChainId,
        @JvmField val contextId: HeightContextId,
        @JvmField val height: Long,
        @JvmField val requester: PeerIdPayload,
        signature: ByteArray,
    ) : WireValue() {
        private val signatureValue = signature.copyOf()

        constructor(
            chainId: ChainId,
            contextId: HeightContextId,
            height: Long,
            requester: PeerIdPayload,
            signature: ByteArray,
        ) : this(PROTOCOL_VERSION, chainId, contextId, height, requester, signature)

        init {
            require(protocolVersion == PROTOCOL_VERSION) {
                "Unsupported commit-certificate request protocol version $protocolVersion"
            }
            require(signatureValue.isNotEmpty()) { "Commit-certificate request signature is missing" }
        }

        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            u16(protocolVersion),
            chainId.encode(),
            contextId.encode(),
            u64(height),
            requester.bytes(),
            byteVector(signatureValue),
        )

        /** Exact domain-separated bytes authenticated by the requester. */
        fun signaturePreimage(): ByteArray =
            COMMIT_CERTIFICATE_REQUEST_DOMAIN + struct(
                u16(protocolVersion),
                chainId.encode(),
                contextId.encode(),
                u64(height),
                requester.bytes(),
                byteVector(byteArrayOf()),
            )

        /** Iroha hash identifying this exact signed request. */
        fun requestHash(): Hash32 = Hash32(IrohaHash.prehash(encode()))

        companion object {
            private val COMMIT_CERTIFICATE_REQUEST_DOMAIN =
                "iroha:sumeragi:v2:commit-certificate-request".toByteArray(StandardCharsets.UTF_8)

            internal fun decode(bytes: ByteArray): CommitCertificateRequest =
                decodeStruct(bytes) { reader ->
                    CommitCertificateRequest(
                        reader.field("commit_request.protocol_version") {
                            it.u16Only("commit_request.protocol_version")
                        },
                        reader.field("commit_request.chain_id") {
                            ChainId.decode(it.remainingBytes())
                        },
                        reader.field("commit_request.context_id") {
                            HeightContextId.decode(it.remainingBytes())
                        },
                        reader.field("commit_request.height") {
                            it.u64Only("commit_request.height")
                        },
                        PeerIdPayload(reader.compactField("commit_request.requester")),
                        reader.field("commit_request.signature") {
                            it.byteVectorOnly("commit_request.signature")
                        },
                    )
                }
        }
    }

    /** Signed response carrying the durable CommitQC for an exact request. */
    class CommitCertificateResponse(
        @JvmField val requestHash: Hash32,
        @JvmField val certificate: QuorumCertificate,
        @JvmField val responder: PeerIdPayload,
        signature: ByteArray,
    ) : WireValue() {
        private val signatureValue = signature.copyOf()

        init {
            require(certificate.phase == GlobalPhase.COMMIT) {
                "Commit-certificate response must carry a CommitQC"
            }
            require(signatureValue.isNotEmpty()) { "Commit-certificate response signature is missing" }
        }

        fun signature(): ByteArray = signatureValue.copyOf()

        override fun encode(): ByteArray = struct(
            requestHash.bytes(),
            certificate.encode(),
            responder.bytes(),
            byteVector(signatureValue),
        )

        /** Exact domain-separated bytes authenticated by the responder. */
        fun signaturePreimage(): ByteArray =
            COMMIT_CERTIFICATE_RESPONSE_DOMAIN + struct(
                u16(PROTOCOL_VERSION),
                requestHash.bytes(),
                certificate.encode(),
                responder.bytes(),
            )

        /**
         * Fail closed unless this response answers the exact request and its QC is governed by
         * that request's height context. Responder and aggregate-signature verification remains
         * the authenticated transport/consensus caller's responsibility.
         */
        fun validateAgainst(request: CommitCertificateRequest) {
            require(requestHash == request.requestHash()) {
                "Commit-certificate response does not answer the exact signed request"
            }
            require(certificate.round.contextId == request.contextId) {
                "Commit-certificate response uses a different height context"
            }
            require(certificate.round.height == request.height) {
                "Commit-certificate response uses a different height"
            }
        }

        companion object {
            private val COMMIT_CERTIFICATE_RESPONSE_DOMAIN =
                "iroha:sumeragi:v2:commit-certificate-response".toByteArray(StandardCharsets.UTF_8)

            internal fun decode(bytes: ByteArray): CommitCertificateResponse =
                decodeStruct(bytes) { reader ->
                    CommitCertificateResponse(
                        Hash32(reader.field("commit_response.request_hash") { it.hash() }),
                        reader.field("commit_response.certificate") {
                            QuorumCertificate.decode(it.remainingBytes())
                        },
                        PeerIdPayload(reader.compactField("commit_response.responder")),
                        reader.field("commit_response.signature") {
                            it.byteVectorOnly("commit_response.signature")
                        },
                    )
                }
        }
    }

    /** Canonical v2 network payload variants, in Rust declaration order. */
    sealed class ConsensusPayload : WireValue() {
        class ProposalMessage(@JvmField val value: Proposal) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(0, value.encode())
        }

        class VoteMessage(@JvmField val value: Vote) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(1, value.encode())
        }

        class QuorumCertificateMessage(@JvmField val value: QuorumCertificate) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(2, value.encode())
        }

        class TimeoutVoteMessage(@JvmField val value: TimeoutVote) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(3, value.encode())
        }

        class TimeoutCertificateMessage(@JvmField val value: TimeoutCertificate) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(4, value.encode())
        }

        class PayloadManifestMessage(@JvmField val value: PayloadManifest) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(5, value.encode())
        }

        class PayloadChunkMessage(@JvmField val value: PayloadChunk) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(6, value.encode())
        }

        class CertifiedBodyRequestMessage(@JvmField val value: CertifiedBodyRequest) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(7, value.encode())
        }

        class CertifiedBodyResponseMessage(@JvmField val value: CertifiedBodyResponse) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(8, value.encode())
        }

        class CommitCertificateRequestMessage(
            @JvmField val value: CommitCertificateRequest,
        ) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(9, value.encode())
        }

        class CommitCertificateResponseMessage(
            @JvmField val value: CommitCertificateResponse,
        ) : ConsensusPayload() {
            override fun encode(): ByteArray = enumPayload(10, value.encode())
        }

        companion object {
            internal fun decode(bytes: ByteArray): ConsensusPayload {
                val reader = Reader(bytes)
                val tag = reader.u32("consensus payload")
                val payload = reader.compactField("consensus payload value")
                reader.finish("consensus payload")
                return when (tag) {
                    0L -> ProposalMessage(Proposal.decode(payload))
                    1L -> VoteMessage(Vote.decode(payload))
                    2L -> QuorumCertificateMessage(QuorumCertificate.decode(payload))
                    3L -> TimeoutVoteMessage(TimeoutVote.decode(payload))
                    4L -> TimeoutCertificateMessage(TimeoutCertificate.decode(payload))
                    5L -> PayloadManifestMessage(PayloadManifest.decode(payload))
                    6L -> PayloadChunkMessage(PayloadChunk.decode(payload))
                    7L -> CertifiedBodyRequestMessage(CertifiedBodyRequest.decode(payload))
                    8L -> CertifiedBodyResponseMessage(CertifiedBodyResponse.decode(payload))
                    9L -> CommitCertificateRequestMessage(CommitCertificateRequest.decode(payload))
                    10L -> CommitCertificateResponseMessage(CommitCertificateResponse.decode(payload))
                    else -> throw IllegalArgumentException("Unknown Sumeragi v2 payload: $tag")
                }
            }
        }
    }

    /** Explicitly versioned live-consensus envelope. */
    class ConsensusMessageV2(
        @JvmField val protocolVersion: Int,
        @JvmField val payload: ConsensusPayload,
    ) : WireValue() {
        constructor(payload: ConsensusPayload) : this(PROTOCOL_VERSION, payload)

        init {
            require(protocolVersion == PROTOCOL_VERSION) {
                "Unsupported Sumeragi protocol version $protocolVersion; expected $PROTOCOL_VERSION"
            }
        }

        override fun encode(): ByteArray = struct(u16(protocolVersion), payload.encode())

        companion object {
            /** Decode a canonical compact-length bare-Norito v2 envelope. */
            @JvmStatic
            fun decodeCanonical(bytes: ByteArray): ConsensusMessageV2 {
                val decoded = decodeStruct(bytes) { reader ->
                    ConsensusMessageV2(
                        reader.field("message.protocol_version") {
                            it.u16Only("message.protocol_version")
                        },
                        reader.field("message.payload") {
                            ConsensusPayload.decode(it.remainingBytes())
                        },
                    )
                }
                require(decoded.encode().contentEquals(bytes)) {
                    "ConsensusMessageV2 is not in canonical compact-length Norito form"
                }
                return decoded
            }
        }
    }

    /** Reducer phase exposed by the compact status endpoint. */
    enum class StatusPhase(@JvmField val discriminant: Long) {
        AWAITING_PROPOSAL(0),
        RECONSTRUCTING_PAYLOAD(1),
        VALIDATING_PAYLOAD(2),
        PREPARE(3),
        COMMIT(4),
        PENDING_APPLY(5),
        ;

        internal fun encode(): ByteArray = u32(discriminant)

        companion object {
            internal fun decode(bytes: ByteArray): StatusPhase {
                val tag = Reader(bytes).u32Only("status phase")
                return entries.firstOrNull { it.discriminant == tag }
                    ?: throw IllegalArgumentException("Unknown Sumeragi v2 status phase: $tag")
            }
        }
    }

    /** Local body state exposed by the compact status endpoint. */
    enum class BodyState(@JvmField val discriminant: Long) {
        MISSING(0),
        RECONSTRUCTING(1),
        STORED(2),
        VALIDATED(3),
        PENDING_APPLY(4),
        APPLIED(5),
        ;

        internal fun encode(): ByteArray = u32(discriminant)

        companion object {
            internal fun decode(bytes: ByteArray): BodyState {
                val tag = Reader(bytes).u32Only("body state")
                return entries.firstOrNull { it.discriminant == tag }
                    ?: throw IllegalArgumentException("Unknown Sumeragi v2 body state: $tag")
            }
        }
    }

    /** Compact, protocol-v2-only `/v1/sumeragi/status` payload. */
    class SumeragiV2Status(
        @JvmField val protocolVersion: Int,
        @JvmField val nodeFingerprint: Hash32,
        @JvmField val buildFingerprint: Hash32,
        @JvmField val configFingerprint: Hash32,
        @JvmField val heightContextId: HeightContextId,
        @JvmField val height: Long,
        @JvmField val view: Long,
        @JvmField val phase: StatusPhase,
        @JvmField val leader: Long,
        @JvmField val lockedPrepareQc: QuorumCertificateRef?,
        @JvmField val highestPrepareQc: QuorumCertificateRef?,
        @JvmField val lastTimeoutCertificate: TimeoutCertificateRef?,
        @JvmField val bodyState: BodyState,
        @JvmField val pendingPersistenceId: Long?,
        @JvmField val lastCommittedHeight: Long,
        @JvmField val lastCommittedSubject: BlockSubject?,
    ) : WireValue() {
        init {
            require(protocolVersion == PROTOCOL_VERSION) {
                "Unsupported Sumeragi status protocol version $protocolVersion"
            }
        }

        override fun encode(): ByteArray = struct(
            u16(protocolVersion),
            nodeFingerprint.bytes(),
            buildFingerprint.bytes(),
            configFingerprint.bytes(),
            heightContextId.encode(),
            u64(height),
            u64(view),
            phase.encode(),
            u32(leader),
            option(lockedPrepareQc?.encode()),
            option(highestPrepareQc?.encode()),
            option(lastTimeoutCertificate?.encode()),
            bodyState.encode(),
            option(pendingPersistenceId?.let(::u64)),
            u64(lastCommittedHeight),
            option(lastCommittedSubject?.encode()),
        )

        companion object {
            /** Decode a canonical compact-length bare-Norito v2 status value. */
            @JvmStatic
            fun decodeCanonical(bytes: ByteArray): SumeragiV2Status {
                val decoded = decodeStruct(bytes) { reader ->
                    SumeragiV2Status(
                        reader.field("status.protocol_version") { it.u16Only("status.protocol_version") },
                        Hash32(reader.field("status.node_fingerprint") { it.hash() }),
                        Hash32(reader.field("status.build_fingerprint") { it.hash() }),
                        Hash32(reader.field("status.config_fingerprint") { it.hash() }),
                        reader.field("status.context_id") { HeightContextId.decode(it.remainingBytes()) },
                        reader.field("status.height") { it.u64Only("status.height") },
                        reader.field("status.view") { it.u64Only("status.view") },
                        reader.field("status.phase") { StatusPhase.decode(it.remainingBytes()) },
                        reader.field("status.leader") { it.u32Only("status.leader") },
                        reader.field("status.lock") {
                            optionDecode(it, "status.lock") { QuorumCertificateRef.decode(it) }
                        },
                        reader.field("status.high_qc") {
                            optionDecode(it, "status.high_qc") { QuorumCertificateRef.decode(it) }
                        },
                        reader.field("status.last_tc") {
                            optionDecode(it, "status.last_tc") { TimeoutCertificateRef.decode(it) }
                        },
                        reader.field("status.body_state") { BodyState.decode(it.remainingBytes()) },
                        reader.field("status.pending_persistence") {
                            optionDecode(it, "status.pending_persistence") { payload ->
                                Reader(payload).u64Only("status.pending_persistence.value")
                            }
                        },
                        reader.field("status.last_committed_height") {
                            it.u64Only("status.last_committed_height")
                        },
                        reader.field("status.last_committed_subject") {
                            optionDecode(it, "status.last_committed_subject") { BlockSubject.decode(it) }
                        },
                    )
                }
                require(decoded.encode().contentEquals(bytes)) {
                    "SumeragiV2Status is not in canonical compact-length Norito form"
                }
                return decoded
            }
        }
    }

    /** Base equality contract for immutable wire values. */
    abstract class WireValue {
        abstract fun encode(): ByteArray

        final override fun equals(other: Any?): Boolean =
            other != null && javaClass == other.javaClass &&
                encode().contentEquals((other as WireValue).encode())

        final override fun hashCode(): Int = 31 * javaClass.hashCode() + encode().contentHashCode()
    }

    private class Reader(private val bytes: ByteArray) {
        private var offset = 0

        fun u8(label: String): Int {
            requireRemaining(1, label)
            return bytes[offset++].toInt() and 0xff
        }

        fun u16(label: String): Int {
            requireRemaining(2, label)
            val value = ByteBuffer.wrap(bytes, offset, 2).order(ByteOrder.LITTLE_ENDIAN).short
            offset += 2
            return value.toInt() and 0xffff
        }

        fun u32(label: String): Long {
            requireRemaining(4, label)
            val value = ByteBuffer.wrap(bytes, offset, 4).order(ByteOrder.LITTLE_ENDIAN).int
            offset += 4
            return value.toLong() and 0xffff_ffffL
        }

        fun u64(label: String): Long {
            requireRemaining(8, label)
            val value = ByteBuffer.wrap(bytes, offset, 8).order(ByteOrder.LITTLE_ENDIAN).long
            offset += 8
            return value
        }

        fun compactField(label: String): ByteArray {
            val length = varint(label)
            require(length in 0..Int.MAX_VALUE.toLong()) { "$label length exceeds JVM range" }
            return read(length.toInt(), label)
        }

        fun <T> field(label: String, decode: (Reader) -> T): T {
            val child = Reader(compactField(label))
            val value = decode(child)
            child.finish(label)
            return value
        }

        fun hash(): ByteArray = read(32, "hash")

        fun hashOnly(label: String): ByteArray {
            val value = hash()
            finish(label)
            return value
        }

        fun byteVectorOnly(label: String): ByteArray {
            val length = u64("$label length")
            require(length in 0..Int.MAX_VALUE.toLong()) { "$label exceeds JVM range" }
            val value = read(length.toInt(), label)
            finish(label)
            return value
        }

        fun stringOnly(label: String): String {
            val value = compactField("$label bytes")
            finish(label)
            return decodeUtf8(value, label)
        }

        fun u16Only(label: String): Int {
            val value = u16(label)
            finish(label)
            return value
        }

        fun u32Only(label: String): Long {
            val value = u32(label)
            finish(label)
            return value
        }

        fun u64Only(label: String): Long {
            val value = u64(label)
            finish(label)
            return value
        }

        fun remainingBytes(): ByteArray = read(bytes.size - offset, "remaining payload")

        fun finish(label: String) {
            require(offset == bytes.size) { "$label contains trailing Norito bytes" }
        }

        private fun read(count: Int, label: String): ByteArray {
            requireRemaining(count, label)
            val result = bytes.copyOfRange(offset, offset + count)
            offset += count
            return result
        }

        private fun requireRemaining(count: Int, label: String) {
            require(count >= 0 && offset <= bytes.size - count) { "$label is truncated" }
        }

        private fun varint(label: String): Long {
            var value = 0L
            var shift = 0
            var count = 0
            while (true) {
                requireRemaining(1, label)
                val byte = bytes[offset++].toInt() and 0xff
                count++
                require(count <= 10 && shift < 64) { "$label varint overflows u64" }
                if (shift == 63) require((byte and 0x7e) == 0) { "$label varint overflows u64" }
                value = value or ((byte and 0x7f).toLong() shl shift)
                if ((byte and 0x80) == 0) {
                    require(varint(value).size == count) { "$label uses a non-canonical varint" }
                    return value
                }
                shift += 7
            }
        }
    }

    private fun struct(vararg fields: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        fields.forEach { field ->
            out.write(varint(field.size.toLong()))
            out.write(field)
        }
        return out.toByteArray()
    }

    private fun enumPayload(discriminant: Long, payload: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(u32(discriminant))
        out.write(varint(payload.size.toLong()))
        out.write(payload)
        return out.toByteArray()
    }

    private fun u16(value: Int): ByteArray {
        require(value in 0..0xffff) { "u16 value out of range" }
        return ByteBuffer.allocate(2).order(ByteOrder.LITTLE_ENDIAN).putShort(value.toShort()).array()
    }

    private fun u32(value: Long): ByteArray {
        require(value in 0..0xffff_ffffL) { "u32 value out of range" }
        return ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value.toInt()).array()
    }

    private fun u64(value: Long): ByteArray =
        ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()

    private fun byteVector(value: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(u64(value.size.toLong()))
        out.write(value)
        return out.toByteArray()
    }

    private fun string(value: String): ByteArray {
        requireWellFormedUtf16(value, "string")
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        val out = ByteArrayOutputStream()
        out.write(varint(bytes.size.toLong()))
        out.write(bytes)
        return out.toByteArray()
    }

    private fun decodeUtf8(bytes: ByteArray, label: String): String {
        val decoded = StandardCharsets.UTF_8.newDecoder().runCatching {
            decode(ByteBuffer.wrap(bytes)).toString()
        }.getOrElse { throw IllegalArgumentException("$label is not valid UTF-8", it) }
        requireWellFormedUtf16(decoded, label)
        return decoded
    }

    private fun option(value: ByteArray?): ByteArray {
        if (value == null) return byteArrayOf(0)
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(varint(value.size.toLong()))
        out.write(value)
        return out.toByteArray()
    }

    private fun <T> vector(values: List<T>, encode: (T) -> ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(u64(values.size.toLong()))
        values.forEach { value ->
            val payload = encode(value)
            out.write(varint(payload.size.toLong()))
            out.write(payload)
        }
        return out.toByteArray()
    }

    private fun varint(value: Long): ByteArray {
        require(value >= 0) { "Norito compact length must be non-negative" }
        val out = ByteArrayOutputStream()
        var remaining = value
        while (remaining >= 0x80) {
            out.write(((remaining and 0x7f) or 0x80).toInt())
            remaining = remaining ushr 7
        }
        out.write(remaining.toInt())
        return out.toByteArray()
    }

    private fun optionHash(reader: Reader): Hash32? =
        optionDecode(reader, "optional hash") { Hash32(Reader(it).hashOnly("optional hash value")) }

    private fun <T> optionDecode(reader: Reader, label: String, decode: (ByteArray) -> T): T? {
        val tag = reader.u8(label)
        if (tag == 0) {
            reader.finish(label)
            return null
        }
        require(tag == 1) { "$label has invalid Option tag $tag" }
        val payload = reader.compactField("$label payload")
        reader.finish(label)
        return decode(payload)
    }

    private fun <T> vectorDecode(reader: Reader, label: String, decode: (Reader) -> T): List<T> {
        val count = reader.u64("$label count")
        require(count in 0..Int.MAX_VALUE.toLong()) { "$label count exceeds JVM range" }
        val values = ArrayList<T>(count.toInt())
        repeat(count.toInt()) { index ->
            values.add(reader.field("$label[$index]", decode))
        }
        reader.finish(label)
        return values
    }

    private fun <T> decodeStruct(bytes: ByteArray, decode: (Reader) -> T): T {
        val reader = Reader(bytes)
        val value = decode(reader)
        reader.finish("struct")
        return value
    }

    private fun requireStrictlyIncreasing(values: List<Long>, label: String) {
        values.forEach { require(it in 0..0xffff_ffffL) { "$label contains an invalid u32" } }
        values.zipWithNext().forEach { (left, right) ->
            require(left < right) { "$label must be strictly increasing" }
        }
    }

    private fun requireWellFormedUtf16(value: String, label: String) {
        var index = 0
        while (index < value.length) {
            val current = value[index]
            when {
                Character.isHighSurrogate(current) -> {
                    require(index + 1 < value.length && Character.isLowSurrogate(value[index + 1])) {
                        "$label contains an unpaired UTF-16 surrogate"
                    }
                    index += 2
                }
                Character.isLowSurrogate(current) ->
                    throw IllegalArgumentException("$label contains an unpaired UTF-16 surrogate")
                else -> index++
            }
        }
    }
}
