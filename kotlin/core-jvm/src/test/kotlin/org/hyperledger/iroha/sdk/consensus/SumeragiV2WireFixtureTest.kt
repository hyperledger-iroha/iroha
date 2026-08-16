// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.core.model.NetworkId

class SumeragiV2WireFixtureTest {
    @Test
    fun `execution commitments carry exact mandatory lane finality and merge carrier options`() {
        fun hash(seed: Int) =
            SumeragiV2Wire.Hash32(
                ByteArray(32) { seed.toByte() }.also {
                    it[31] = (it[31].toInt() or 1).toByte()
                },
            )
        val base = SumeragiV2Wire.ExecutionCommitment.withoutTopups(
            hash(0x21), hash(0x23), hash(0x25), 123, hash(0x27),
        )
        val laneFinality = SumeragiV2Wire.LaneFinalityManifestCommitment(hash(0x2b), 1)
        val carrier = SumeragiV2Wire.MergeCarrierCommitment(1, hash(0x29))
        val carried = SumeragiV2Wire.ExecutionCommitment(
            base.parentStateRoot,
            base.postStateRoot,
            base.ordinaryWritesRoot,
            base.topupAnchorRoot,
            base.topupAnchorCount,
            base.nativeAmxApplicationManifestVersion,
            base.nativeAmxApplicationManifestRoot,
            base.nativeAmxApplicationManifestCount,
            laneFinality,
            carrier,
            base.executedBlockWireLen,
            base.executedBlockWireHash,
        )

        val decodedBase = SumeragiV2Wire.ExecutionCommitment.decode(base.encode())
        assertEquals(null, decodedBase.laneFinalityManifest)
        assertEquals(null, decodedBase.mergeCarrier)
        assertEquals(123L, decodedBase.executedBlockWireLen)
        assertEquals(
            carrier,
            SumeragiV2Wire.ExecutionCommitment.decode(carried.encode()).mergeCarrier,
        )
        assertEquals(
            laneFinality,
            SumeragiV2Wire.ExecutionCommitment.decode(carried.encode()).laneFinalityManifest,
        )
        listOf(0L, SumeragiV2Wire.MAX_LANE_FINALITY_STATEMENTS_PER_BLOCK + 1).forEach { count ->
            assertFailsWith<IllegalArgumentException> {
                SumeragiV2Wire.LaneFinalityManifestCommitment(hash(0x2b), count)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.MergeCarrierCommitment(2, hash(0x29))
        }
    }

    @Test
    fun `unsafe proposal ignore reason decodes wire discriminant eleven`() {
        assertEquals(
            SumeragiV2Wire.IgnoreReason.UNSAFE_PROPOSAL,
            SumeragiV2Wire.IgnoreReason.decode(byteArrayOf(11, 0, 0, 0)),
        )
    }

    @Test
    fun `successor activation blocker uses revision four wire discriminant`() {
        assertEquals(
            SumeragiV2Wire.LivenessBlocker.SUCCESSOR_ACTIVATION_PENDING,
            SumeragiV2Wire.LivenessBlocker.decode(byteArrayOf(7, 0, 0, 0)),
        )
        assertEquals(
            SumeragiV2Wire.LivenessBlocker.LOCAL_CONTROL_PENDING,
            SumeragiV2Wire.LivenessBlocker.decode(byteArrayOf(8, 0, 0, 0)),
        )
    }

    @Test
    fun `data availability rejects retired encoding tag and zero shards`() {
        val retiredTag = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.PayloadEncoding.decode(byteArrayOf(1, 0, 0, 0))
        }
        assertEquals("Unknown payload encoding: 1", retiredTag.message)
        assertEquals(
            SumeragiV2Wire.PayloadEncoding.REED_SOLOMON_16,
            SumeragiV2Wire.PayloadEncoding.decode(byteArrayOf(0, 0, 0, 0)),
        )

        listOf(0 to 1, 1 to 0).forEach { (dataShards, parityShards) ->
            val zeroShard = assertFailsWith<IllegalArgumentException> {
                SumeragiV2Wire.DataAvailabilityLayout(
                    SumeragiV2Wire.PayloadEncoding.REED_SOLOMON_16,
                    4,
                    dataShards,
                    parityShards,
                    4,
                    2,
                )
            }
            assertEquals(
                "ReedSolomon16 data availability requires positive shard counts",
                zeroShard.message,
            )
        }
    }

    @Test
    fun `rust canonical message fixtures roundtrip`() {
        val messages = fixtureRows().filter { it.kind == "message" }
        assertEquals(EXPECTED_MESSAGE_NAMES, messages.map { it.name }.toSet())

        messages.forEach { row ->
            val encoded = row.hex.hexBytes()
            val decoded = SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(encoded)
            assertContentEquals(encoded, decoded.encode(), row.name)
        }
    }

    @Test
    fun `Rust merge carrier fixture pins the current v4 shape`() {
        val rows = fixtureRows()
        val carried = SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
            rows.single {
                it.kind == "message" && it.name == "quorum_certificate_merge_carrier"
            }.hex.hexBytes(),
        )
        val certificate = (
            carried.payload as SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage
            ).value
        val carrier = requireNotNull(certificate.executionCommitment.mergeCarrier)
        assertEquals(1, carrier.version)
        assertEquals(32, carrier.entryHash.bytes().size)

        setOf(
            "execution_commitment_merge_carrier_wrong_version",
            "execution_commitment_missing_merge_carrier_field",
        ).forEach { name ->
            val row = rows.single { it.kind == "negative_message" && it.name == name }
            assertFailsWith<IllegalArgumentException>(name) {
                SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(row.hex.hexBytes())
            }
        }
    }

    @Test
    fun `commit reproposals require their vote and certificate round`() {
        fun message(name: String): SumeragiV2Wire.ConsensusMessageV2 =
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                fixtureRows().single { it.kind == "message" && it.name == name }.hex.hexBytes(),
            )

        val vote = (
            message("commit_vote_reproposal").payload
                as SumeragiV2Wire.ConsensusPayload.VoteMessage
            ).value
        val certificate = (
            message("commit_quorum_certificate_reproposal").payload
                as SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage
            ).value
        val response = (
            message("commit_certificate_response").payload
                as SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage
            ).value

        assertEquals(SumeragiV2Wire.GlobalPhase.COMMIT, vote.phase)
        assertEquals(9L, vote.round.view)
        assertEquals(vote.round, vote.proposalRound)
        assertEquals(vote.round, certificate.round)
        assertEquals(vote.proposalRound, certificate.proposalRound)
        assertEquals(vote.subject, certificate.subject)
        assertEquals(vote.executionCommitment, certificate.executionCommitment)
        assertEquals(certificate.reference(), response.certificate.reference())

        val splitVote = fixtureRows().single {
            it.kind == "negative_message" && it.name == "commit_vote_split_round"
        }
        val voteError = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(splitVote.hex.hexBytes())
        }
        assertEquals(
            "Prepare/Commit vote proposal round must match its round",
            voteError.message,
        )

        val splitCertificate = fixtureRows().single {
            it.kind == "negative_message" &&
                it.name == "commit_quorum_certificate_split_round"
        }
        val certificateError = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(splitCertificate.hex.hexBytes())
        }
        assertEquals(
            "Prepare/Commit certificate proposal round must match its round",
            certificateError.message,
        )
    }

    @Test
    fun `status validators reject an older commit proposal round`() {
        val certificate = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                fixtureRows().single {
                    it.kind == "message" &&
                        it.name == "commit_quorum_certificate_reproposal"
                }.hex.hexBytes(),
            ).payload as SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage
            ).value
        val olderProposalRound = SumeragiV2Wire.ConsensusRound(
            certificate.round.contextId,
            certificate.round.height,
            certificate.round.view - 1,
        )

        val referenceError = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.QuorumCertificateRef(
                certificate.round,
                olderProposalRound,
                SumeragiV2Wire.GlobalPhase.COMMIT,
                certificate.subject,
                certificate.executionCommitment,
            )
        }
        assertEquals(
            "Prepare/Commit certificate reference proposal round must match its round",
            referenceError.message,
        )

        val quorumError = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.VoteQuorumStatus(
                certificate.round,
                olderProposalRound,
                certificate.subject,
                certificate.executionCommitment,
                1,
                1,
                3,
                4,
            )
        }
        assertEquals(
            "Prepare/Commit quorum status proposal round must match its round",
            quorumError.message,
        )

        val outboundError = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.OutboundIntentStatus(
                SumeragiV2Wire.OutboundIntentKind.COMMIT_VOTE,
                certificate.round,
                olderProposalRound,
                certificate.subject,
                certificate.executionCommitment,
                SumeragiV2Wire.OutboundIntentStage.QUEUED,
            )
        }
        assertEquals(
            "Proposal/Prepare/Commit outbound intent origin must match its round",
            outboundError.message,
        )
    }

    @Test
    fun `prepare votes and certificates reject split rounds`() {
        val prepare = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                fixtureRows().single {
                    it.kind == "message" && it.name == "quorum_certificate"
                }.hex.hexBytes(),
            ).payload as SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage
            ).value
        val laterRound = SumeragiV2Wire.ConsensusRound(
            prepare.round.contextId,
            prepare.round.height,
            prepare.round.view + 1,
        )

        val voteError = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.Vote(
                laterRound,
                prepare.round,
                SumeragiV2Wire.GlobalPhase.PREPARE,
                prepare.subject,
                prepare.executionCommitment,
                0,
                byteArrayOf(1),
            )
        }
        assertEquals(
            "Prepare/Commit vote proposal round must match its round",
            voteError.message,
        )

        val certificateError = assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.QuorumCertificate(
                laterRound,
                prepare.round,
                SumeragiV2Wire.GlobalPhase.PREPARE,
                prepare.subject,
                prepare.executionCommitment,
                prepare.signers,
                prepare.aggregateSignature(),
            )
        }
        assertEquals(
            "Prepare/Commit certificate proposal round must match its round",
            certificateError.message,
        )
    }

    @Test
    fun `timeout vote carries the complete prepare certificate`() {
        val rows = fixtureRows()
        val timeoutVote = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                rows.single {
                    it.kind == "message" && it.name == "timeout_vote"
                }.hex.hexBytes(),
            ).payload as SumeragiV2Wire.ConsensusPayload.TimeoutVoteMessage
            ).value
        val standalonePrepare = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                rows.single {
                    it.kind == "message" && it.name == "quorum_certificate"
                }.hex.hexBytes(),
            ).payload as SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage
            ).value

        val embeddedPrepare = requireNotNull(timeoutVote.highestPrepareQc)
        assertEquals(standalonePrepare, embeddedPrepare)
        assertEquals(embeddedPrepare.round, embeddedPrepare.proposalRound)
        assertEquals(embeddedPrepare.proposalRound, embeddedPrepare.reference().proposalRound)
        assertEquals(listOf(0L, 1L, 2L), embeddedPrepare.signers)
        assertEquals(48, embeddedPrepare.aggregateSignature().size)

        val changedSignature = embeddedPrepare.aggregateSignature().also {
            it[0] = (it[0].toInt() xor 1).toByte()
        }
        val changedPrepare = SumeragiV2Wire.QuorumCertificate(
            embeddedPrepare.round,
            embeddedPrepare.proposalRound,
            embeddedPrepare.phase,
            embeddedPrepare.subject,
            embeddedPrepare.executionCommitment,
            embeddedPrepare.signers,
            changedSignature,
        )
        val changedVote = SumeragiV2Wire.TimeoutVote(
            timeoutVote.round,
            changedPrepare,
            timeoutVote.signer,
            timeoutVote.signature(),
        )
        assertFalse(
            timeoutVote.encode().contentEquals(changedVote.encode()),
            "timeout-vote wire bytes did not bind the embedded PrepareQC evidence",
        )
    }

    @Test
    fun `commit certificate signing preimages match rust exactly`() {
        val rows = fixtureRows()
        val requestMessage = rows.single {
            it.kind == "message" && it.name == "commit_certificate_request"
        }
        val responseMessage = rows.single {
            it.kind == "message" && it.name == "commit_certificate_response"
        }
        val requestPreimage = rows.single {
            it.kind == "preimage" && it.name == "commit_certificate_request"
        }
        val responsePreimage = rows.single {
            it.kind == "preimage" && it.name == "commit_certificate_response"
        }

        val request = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(requestMessage.hex.hexBytes()).payload
                as SumeragiV2Wire.ConsensusPayload.CommitCertificateRequestMessage
            ).value
        assertEquals(SumeragiV2Wire.PROTOCOL_VERSION, request.protocolVersion)
        assertContentEquals(ByteArray(32) { 0x71 }, request.networkId.bytes())
        assertEquals(1L, request.height)
        assertEquals(48, request.signature().size)
        assertContentEquals(requestPreimage.hex.hexBytes(), request.signaturePreimage())
        val reSignedRequest = SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            request.networkId,
            request.contextId,
            request.height,
            request.requester,
            byteArrayOf(1),
        )
        assertContentEquals(request.signaturePreimage(), reSignedRequest.signaturePreimage())
        val otherNetworkBytes = request.networkId.bytes().also {
            it[0] = (it[0].toInt() xor 1).toByte()
        }
        val crossNetworkRequest = SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            NetworkId.fromBytes(otherNetworkBytes),
            request.contextId,
            request.height,
            request.requester,
            byteArrayOf(1),
        )
        assertFalse(
            request.signaturePreimage().contentEquals(crossNetworkRequest.signaturePreimage()),
        )

        val response = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(responseMessage.hex.hexBytes()).payload
                as SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage
            ).value
        assertEquals(SumeragiV2Wire.GlobalPhase.COMMIT, response.certificate.phase)
        assertEquals(9L, response.certificate.round.view)
        assertEquals(response.certificate.round, response.certificate.proposalRound)
        assertEquals(48, response.signature().size)
        assertEquals(response.requestHash, request.requestHash())
        response.validateAgainst(request)
        assertContentEquals(responsePreimage.hex.hexBytes(), response.signaturePreimage())
        val reSignedResponse = SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash,
            response.certificate,
            response.responder,
            byteArrayOf(1),
        )
        assertContentEquals(response.signaturePreimage(), reSignedResponse.signaturePreimage())
        assertFailsWith<IllegalArgumentException> { response.validateAgainst(reSignedRequest) }
        val changedResponder = SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash,
            response.certificate,
            request.requester,
            byteArrayOf(1),
        )
        assertFalse(response.signaturePreimage().contentEquals(changedResponder.signaturePreimage()))

        val changedContextBytes = request.contextId.hash.bytes()
        changedContextBytes[0] = (changedContextBytes[0].toInt() xor 1).toByte()
        val changedContextRequest = SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            request.networkId,
            SumeragiV2Wire.HeightContextId(SumeragiV2Wire.Hash32(changedContextBytes)),
            request.height,
            request.requester,
            request.signature(),
        )
        val mismatchedContextResponse = SumeragiV2Wire.CommitCertificateResponse(
            changedContextRequest.requestHash(),
            response.certificate,
            response.responder,
            response.signature(),
        )
        assertFailsWith<IllegalArgumentException> {
            mismatchedContextResponse.validateAgainst(changedContextRequest)
        }

        val changedHeightRequest = SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            request.networkId,
            request.contextId,
            request.height + 1,
            request.requester,
            request.signature(),
        )
        val mismatchedHeightResponse = SumeragiV2Wire.CommitCertificateResponse(
            changedHeightRequest.requestHash(),
            response.certificate,
            response.responder,
            response.signature(),
        )
        assertFailsWith<IllegalArgumentException> {
            mismatchedHeightResponse.validateAgainst(changedHeightRequest)
        }

        val changedSubject = SumeragiV2Wire.BlockSubject(
            response.certificate.subject.parentBlockHash,
            response.certificate.subject.payloadHash,
            response.certificate.subject.blockHash,
        )
        val changedSubjectCertificate = SumeragiV2Wire.QuorumCertificate(
            response.certificate.round,
            response.certificate.proposalRound,
            response.certificate.phase,
            changedSubject,
            response.certificate.executionCommitment,
            response.certificate.signers,
            response.certificate.aggregateSignature(),
        )
        val changedSubjectResponse = SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash,
            changedSubjectCertificate,
            response.responder,
            response.signature(),
        )
        assertFalse(response.signaturePreimage().contentEquals(changedSubjectResponse.signaturePreimage()))

        val changedParentState = response.certificate.executionCommitment.parentStateRoot.bytes()
        changedParentState[0] = (changedParentState[0].toInt() xor 1).toByte()
        val changedExecutionCommitment = SumeragiV2Wire.ExecutionCommitment.withoutTopups(
            SumeragiV2Wire.Hash32(changedParentState),
            response.certificate.executionCommitment.postStateRoot,
            response.certificate.executionCommitment.ordinaryWritesRoot,
            response.certificate.executionCommitment.executedBlockWireLen,
            response.certificate.executionCommitment.executedBlockWireHash,
        )
        val changedExecutionCertificate = SumeragiV2Wire.QuorumCertificate(
            response.certificate.round,
            response.certificate.proposalRound,
            response.certificate.phase,
            response.certificate.subject,
            changedExecutionCommitment,
            response.certificate.signers,
            response.certificate.aggregateSignature(),
        )
        val changedExecutionResponse = SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash,
            changedExecutionCertificate,
            response.responder,
            response.signature(),
        )
        assertFalse(
            response.signaturePreimage().contentEquals(changedExecutionResponse.signaturePreimage()),
            "commit response signature preimage did not bind the execution commitment",
        )
    }

    @Test
    fun `execution commitments reject noncanonical topup bindings`() {
        val responseMessage = fixtureRows().single {
            it.kind == "message" && it.name == "commit_certificate_response"
        }
        val responsePayload =
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                responseMessage.hex.hexBytes(),
            ).payload as SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage
        val base = responsePayload.value.certificate.executionCommitment
        val topupRoot = base.parentStateRoot

        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                topupRoot,
                0,
                base.nativeAmxApplicationManifestVersion,
                base.nativeAmxApplicationManifestRoot,
                base.nativeAmxApplicationManifestCount,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                null,
                1,
                base.nativeAmxApplicationManifestVersion,
                base.nativeAmxApplicationManifestRoot,
                base.nativeAmxApplicationManifestCount,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                topupRoot,
                SumeragiV2Wire.MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK + 1,
                base.nativeAmxApplicationManifestVersion,
                base.nativeAmxApplicationManifestRoot,
                base.nativeAmxApplicationManifestCount,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                topupRoot,
                1,
                base.nativeAmxApplicationManifestVersion,
                base.nativeAmxApplicationManifestRoot,
                base.nativeAmxApplicationManifestCount,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }

        val canonicalPostState = SumeragiV2Wire.ExecutionCommitment.topupPostStateRoot(
            1,
            base.ordinaryWritesRoot,
            topupRoot,
        )
        val valid = SumeragiV2Wire.ExecutionCommitment(
            base.parentStateRoot,
            canonicalPostState,
            base.ordinaryWritesRoot,
            topupRoot,
            1,
            base.nativeAmxApplicationManifestVersion,
            base.nativeAmxApplicationManifestRoot,
            base.nativeAmxApplicationManifestCount,
            base.laneFinalityManifest,
            null,
            base.executedBlockWireLen,
            base.executedBlockWireHash,
        )
        assertEquals(base.executedBlockWireHash, valid.executedBlockWireHash)
        assertContentEquals(
            valid.encode(),
            SumeragiV2Wire.ExecutionCommitment.decode(valid.encode()).encode(),
        )
    }

    @Test
    fun `execution commitments reject noncanonical Native AMX manifest bindings`() {
        val responseMessage = fixtureRows().single {
            it.kind == "message" && it.name == "commit_certificate_response"
        }
        val responsePayload =
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                responseMessage.hex.hexBytes(),
            ).payload as SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage
        val base = responsePayload.value.certificate.executionCommitment
        val nonEmptyRoot = base.parentStateRoot

        assertEquals(
            SumeragiV2Wire.ExecutionCommitment.nativeAmxApplicationManifestEmptyRoot(),
            base.nativeAmxApplicationManifestRoot,
        )
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                null,
                0,
                SumeragiV2Wire.NATIVE_AMX_APPLICATION_MANIFEST_VERSION + 1,
                base.nativeAmxApplicationManifestRoot,
                0,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                null,
                0,
                SumeragiV2Wire.NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                nonEmptyRoot,
                0,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                null,
                0,
                SumeragiV2Wire.NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                base.nativeAmxApplicationManifestRoot,
                1,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                null,
                0,
                SumeragiV2Wire.NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                nonEmptyRoot,
                SumeragiV2Wire.MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES + 1,
                base.laneFinalityManifest,
                null,
                base.executedBlockWireLen,
                base.executedBlockWireHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                base.topupAnchorRoot,
                base.topupAnchorCount,
                base.nativeAmxApplicationManifestVersion,
                base.nativeAmxApplicationManifestRoot,
                base.nativeAmxApplicationManifestCount,
                base.laneFinalityManifest,
                base.mergeCarrier,
                0,
                base.executedBlockWireHash,
            )
        }
    }

    @Test
    fun `rust canonical compact status fixture roundtrips`() {
        val row = fixtureRows().single { it.kind == "status" && it.name == "compact" }
        val encoded = row.hex.hexBytes()
        val decoded = SumeragiV2Wire.SumeragiV2Status.decodeCanonical(encoded)
        assertContentEquals(encoded, decoded.encode())
        assertEquals(SumeragiV2Wire.PROTOCOL_VERSION, decoded.protocolVersion)
        assertEquals(false, decoded.restartRequired)
        assertEquals(1L, decoded.height)
        assertEquals(3L, decoded.view)
        assertEquals(SumeragiV2Wire.StatusPhase.COMMIT, decoded.phase)
        assertEquals(2L, decoded.leader)
        assertEquals(SumeragiV2Wire.BodyState.VALIDATED, decoded.bodyState)
        assertEquals(17L, decoded.pendingPersistenceId)
        assertEquals(0L, decoded.lastCommittedHeight)
        requireNotNull(decoded.lockedPrepareQc)
        requireNotNull(decoded.highestPrepareQc)
        requireNotNull(decoded.lastTimeoutCertificate)
        assertEquals(null, decoded.lastCommittedSubject)
        assertEquals(2L, decoded.heightContext.epoch)
        assertEquals(100L, decoded.heightContext.epochEndHeight)
        assertEquals(SumeragiV2Wire.ConsensusMode.NPOS, decoded.heightContext.mode)
        assertEquals(4L, decoded.heightContext.validatorCount)
        assertEquals(3L, decoded.heightContext.quorum.minSigners)
        assertEquals(4L, decoded.heightContext.quorum.totalPower)
        assertEquals(null, decoded.lastCommitQc)
        assertEquals(3L, decoded.liveness.generation)
        assertEquals(1, decoded.liveness.prepareQuorums.size)
        assertEquals(1, decoded.liveness.commitQuorums.size)
        assertEquals(1L, decoded.liveness.prepareQuorums.single().round.view)
        assertEquals(1L, decoded.liveness.prepareQuorums.single().proposalRound.view)
        assertEquals(3L, decoded.liveness.commitQuorums.single().round.view)
        assertEquals(3L, decoded.liveness.commitQuorums.single().proposalRound.view)
        assertEquals(1, decoded.liveness.timeoutQuorums.size)
        assertEquals(SumeragiV2Wire.OutboundIntentKind.COMMIT_VOTE, decoded.liveness.outboundIntents.single().kind)
        assertEquals(3L, decoded.liveness.outboundIntents.single().round.view)
        assertEquals(3L, decoded.liveness.outboundIntents.single().proposalRound?.view)
        assertEquals(1, decoded.liveness.queues.size)
        assertEquals(SumeragiV2Wire.QueueKind.EFFECT_DISPATCH, decoded.liveness.queues.single().queue)
        assertEquals(SumeragiV2Wire.LivenessBlocker.LOCAL_CONTROL_PENDING, decoded.liveness.blocker)

        // The fifth struct field follows four fixed-width fields and is the
        // canonical one-byte `restart_required` boolean.
        assertEquals(1, encoded[102].toInt())
        val invalidBoolean = encoded.copyOf().also { it[103] = 2 }
        assertFailsWith<IllegalArgumentException> {
            SumeragiV2Wire.SumeragiV2Status.decodeCanonical(invalidBoolean)
        }
    }

    @Test
    fun `malformed and semantically noncanonical fixtures fail closed`() {
        fixtureRows().filter { it.kind == "negative_message" }.forEach { row ->
            assertFailsWith<IllegalArgumentException>(row.name) {
                SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(row.hex.hexBytes())
            }
        }
        fixtureRows().filter { it.kind == "negative_status" }.forEach { row ->
            assertFailsWith<IllegalArgumentException>(row.name) {
                SumeragiV2Wire.SumeragiV2Status.decodeCanonical(row.hex.hexBytes())
            }
        }
    }

    @Test
    fun `commit certificate binding corruptions fail against exact request`() {
        val rows = fixtureRows()
        val request = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                rows.single {
                    it.kind == "message" && it.name == "commit_certificate_request"
                }.hex.hexBytes(),
            ).payload as SumeragiV2Wire.ConsensusPayload.CommitCertificateRequestMessage
            ).value

        rows.filter { it.kind == "negative_binding" }.forEach { row ->
            val response = (
                SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(row.hex.hexBytes()).payload
                    as SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage
                ).value
            assertFailsWith<IllegalArgumentException>(row.name) {
                response.validateAgainst(request)
            }
        }
    }

    private class FixtureRow(
        val kind: String,
        val name: String,
        val hex: String,
        val expectation: String,
    )

    private fun fixtureRows(): List<FixtureRow> = Files.readAllLines(fixturePath())
        .filter { it.isNotBlank() && !it.startsWith("#") }
        .map { line ->
            val columns = line.split('\t')
            require(columns.size == 4) { "Malformed Sumeragi v2 fixture row" }
            FixtureRow(columns[0], columns[1], columns[2], columns[3]).also {
                require(it.expectation == "accept" || it.expectation == "reject")
            }
        }

    private fun fixturePath(): Path {
        var directory: Path? = Paths.get("").toAbsolutePath().normalize()
        while (directory != null) {
            val candidate = directory.resolve(FIXTURE_RELATIVE_PATH)
            if (Files.isRegularFile(candidate)) return candidate
            directory = directory.parent
        }
        error("Unable to locate $FIXTURE_RELATIVE_PATH")
    }

    private fun String.hexBytes(): ByteArray {
        require(length % 2 == 0) { "hex fixture has odd length" }
        return ByteArray(length / 2) { index ->
            val offset = index * 2
            substring(offset, offset + 2).toInt(16).toByte()
        }
    }

    companion object {
        private const val FIXTURE_RELATIVE_PATH = "fixtures/sumeragi_v2/wire_v2.tsv"
        private val EXPECTED_MESSAGE_NAMES = setOf(
            "proposal",
            "vote",
            "quorum_certificate",
            "quorum_certificate_merge_carrier",
            "commit_vote_reproposal",
            "commit_quorum_certificate_reproposal",
            "timeout_vote",
            "timeout_certificate",
            "payload_manifest",
            "payload_chunk",
            "certified_body_request",
            "certified_body_response",
            "commit_certificate_request",
            "commit_certificate_response",
        )
    }
}
