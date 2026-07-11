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

class SumeragiV2WireFixtureTest {
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
        assertEquals("sumeragi-v2-test", request.chainId.value)
        assertEquals(1L, request.height)
        assertEquals(48, request.signature().size)
        assertContentEquals(requestPreimage.hex.hexBytes(), request.signaturePreimage())
        val reSignedRequest = SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            request.chainId,
            request.contextId,
            request.height,
            request.requester,
            byteArrayOf(1),
        )
        assertContentEquals(request.signaturePreimage(), reSignedRequest.signaturePreimage())
        val crossChainRequest = SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            SumeragiV2Wire.ChainId("other-chain"),
            request.contextId,
            request.height,
            request.requester,
            byteArrayOf(1),
        )
        assertFalse(request.signaturePreimage().contentEquals(crossChainRequest.signaturePreimage()))

        val response = (
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(responseMessage.hex.hexBytes()).payload
                as SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage
            ).value
        assertEquals(SumeragiV2Wire.GlobalPhase.COMMIT, response.certificate.phase)
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
            request.chainId,
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
            request.chainId,
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
            response.certificate.phase,
            changedSubject,
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
    }

    @Test
    fun `rust canonical compact status fixture roundtrips`() {
        val row = fixtureRows().single { it.kind == "status" && it.name == "compact" }
        val encoded = row.hex.hexBytes()
        val decoded = SumeragiV2Wire.SumeragiV2Status.decodeCanonical(encoded)
        assertContentEquals(encoded, decoded.encode())
        assertEquals(SumeragiV2Wire.PROTOCOL_VERSION, decoded.protocolVersion)
        assertEquals(1L, decoded.height)
        assertEquals(3L, decoded.view)
        assertEquals(SumeragiV2Wire.StatusPhase.PREPARE, decoded.phase)
        assertEquals(2L, decoded.leader)
        assertEquals(SumeragiV2Wire.BodyState.VALIDATED, decoded.bodyState)
        assertEquals(17L, decoded.pendingPersistenceId)
        assertEquals(0L, decoded.lastCommittedHeight)
        requireNotNull(decoded.lockedPrepareQc)
        requireNotNull(decoded.highestPrepareQc)
        requireNotNull(decoded.lastTimeoutCertificate)
        requireNotNull(decoded.lastCommittedSubject)
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
