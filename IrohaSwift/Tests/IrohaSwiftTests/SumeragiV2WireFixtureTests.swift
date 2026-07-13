// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import XCTest
@testable import IrohaSwift

final class SumeragiV2WireFixtureTests: XCTestCase {
    func testRustCanonicalMessageFixturesRoundtrip() throws {
        let messages = try fixtureRows().filter { $0.kind == "message" }
        XCTAssertEqual(Set(messages.map(\.name)), expectedMessageNames)

        for row in messages {
            let encoded = try Data(sumeragiV2Hex: row.hex)
            let decoded = try SumeragiV2ConsensusMessage.decodeCanonical(encoded)
            XCTAssertEqual(decoded.encode(), encoded, row.name)
        }
    }

    func testCommitCertificateSigningPreimagesMatchRustExactly() throws {
        let rows = try fixtureRows()
        let requestMessage = try XCTUnwrap(rows.first {
            $0.kind == "message" && $0.name == "commit_certificate_request"
        })
        let responseMessage = try XCTUnwrap(rows.first {
            $0.kind == "message" && $0.name == "commit_certificate_response"
        })
        let requestPreimage = try XCTUnwrap(rows.first {
            $0.kind == "preimage" && $0.name == "commit_certificate_request"
        })
        let responsePreimage = try XCTUnwrap(rows.first {
            $0.kind == "preimage" && $0.name == "commit_certificate_response"
        })

        let decodedRequest = try SumeragiV2ConsensusMessage.decodeCanonical(
            Data(sumeragiV2Hex: requestMessage.hex)
        )
        guard case .commitCertificateRequest(let request) = decodedRequest.payload else {
            return XCTFail("request fixture decoded to the wrong v2 payload")
        }
        XCTAssertEqual(request.protocolVersion, SumeragiV2ConsensusMessage.protocolVersion)
        XCTAssertEqual(request.chainID.value, "sumeragi-v2-test")
        XCTAssertEqual(request.height, 1)
        XCTAssertEqual(request.signature.count, 48)
        XCTAssertEqual(request.signaturePreimage(), try Data(sumeragiV2Hex: requestPreimage.hex))
        let reSignedRequest = try SumeragiV2CommitCertificateRequest(
            protocolVersion: request.protocolVersion,
            chainID: request.chainID,
            contextID: request.contextID,
            height: request.height,
            requester: request.requester,
            signature: Data([1])
        )
        XCTAssertEqual(request.signaturePreimage(), reSignedRequest.signaturePreimage())
        let crossChainRequest = try SumeragiV2CommitCertificateRequest(
            protocolVersion: request.protocolVersion,
            chainID: SumeragiV2ChainID("other-chain"),
            contextID: request.contextID,
            height: request.height,
            requester: request.requester,
            signature: Data([1])
        )
        XCTAssertNotEqual(request.signaturePreimage(), crossChainRequest.signaturePreimage())

        let decodedResponse = try SumeragiV2ConsensusMessage.decodeCanonical(
            Data(sumeragiV2Hex: responseMessage.hex)
        )
        guard case .commitCertificateResponse(let response) = decodedResponse.payload else {
            return XCTFail("response fixture decoded to the wrong v2 payload")
        }
        XCTAssertEqual(response.certificate.phase, .commit)
        XCTAssertEqual(response.signature.count, 48)
        XCTAssertEqual(response.requestHash, try request.requestHash())
        try response.validate(against: request)
        XCTAssertEqual(response.signaturePreimage(), try Data(sumeragiV2Hex: responsePreimage.hex))
        let reSignedResponse = try SumeragiV2CommitCertificateResponse(
            requestHash: response.requestHash,
            certificate: response.certificate,
            responder: response.responder,
            signature: Data([1])
        )
        XCTAssertEqual(response.signaturePreimage(), reSignedResponse.signaturePreimage())
        XCTAssertThrowsError(try response.validate(against: reSignedRequest))
        let changedResponder = try SumeragiV2CommitCertificateResponse(
            requestHash: response.requestHash,
            certificate: response.certificate,
            responder: request.requester,
            signature: Data([1])
        )
        XCTAssertNotEqual(response.signaturePreimage(), changedResponder.signaturePreimage())

        var changedContextBytes = request.contextID.hash.bytes
        changedContextBytes[changedContextBytes.startIndex] ^= 1
        let changedContextRequest = try SumeragiV2CommitCertificateRequest(
            protocolVersion: request.protocolVersion,
            chainID: request.chainID,
            contextID: SumeragiV2HeightContextID(
                hash: try SumeragiV2Hash(changedContextBytes)
            ),
            height: request.height,
            requester: request.requester,
            signature: request.signature
        )
        let mismatchedContextResponse = try SumeragiV2CommitCertificateResponse(
            requestHash: changedContextRequest.requestHash(),
            certificate: response.certificate,
            responder: response.responder,
            signature: response.signature
        )
        XCTAssertThrowsError(
            try mismatchedContextResponse.validate(against: changedContextRequest)
        )

        let changedHeightRequest = try SumeragiV2CommitCertificateRequest(
            protocolVersion: request.protocolVersion,
            chainID: request.chainID,
            contextID: request.contextID,
            height: request.height + 1,
            requester: request.requester,
            signature: request.signature
        )
        let mismatchedHeightResponse = try SumeragiV2CommitCertificateResponse(
            requestHash: changedHeightRequest.requestHash(),
            certificate: response.certificate,
            responder: response.responder,
            signature: response.signature
        )
        XCTAssertThrowsError(
            try mismatchedHeightResponse.validate(against: changedHeightRequest)
        )

        let changedSubject = SumeragiV2BlockSubject(
            parentBlockHash: response.certificate.subject.parentBlockHash,
            blockHash: response.certificate.subject.payloadHash,
            payloadHash: response.certificate.subject.blockHash
        )
        let changedSubjectCertificate = try SumeragiV2QuorumCertificate(
            round: response.certificate.round,
            phase: response.certificate.phase,
            subject: changedSubject,
            executionCommitment: response.certificate.executionCommitment,
            signers: response.certificate.signers,
            aggregateSignature: response.certificate.aggregateSignature
        )
        let changedSubjectResponse = try SumeragiV2CommitCertificateResponse(
            requestHash: response.requestHash,
            certificate: changedSubjectCertificate,
            responder: response.responder,
            signature: response.signature
        )
        XCTAssertNotEqual(
            response.signaturePreimage(),
            changedSubjectResponse.signaturePreimage()
        )
    }

    func testRustCanonicalCompactStatusFixtureRoundtrips() throws {
        let row = try XCTUnwrap(
            fixtureRows().first { $0.kind == "status" && $0.name == "compact" }
        )
        let encoded = try Data(sumeragiV2Hex: row.hex)
        let decoded = try SumeragiV2Status.decodeCanonical(encoded)
        XCTAssertEqual(decoded.encode(), encoded)
        XCTAssertEqual(decoded.protocolVersion, SumeragiV2ConsensusMessage.protocolVersion)
        XCTAssertEqual(decoded.height, 1)
        XCTAssertEqual(decoded.view, 3)
        XCTAssertEqual(decoded.phase, .prepare)
        XCTAssertEqual(decoded.leader, 2)
        XCTAssertEqual(decoded.bodyState, .validated)
        XCTAssertEqual(decoded.pendingPersistenceID, 17)
        XCTAssertEqual(decoded.lastCommittedHeight, 0)
        XCTAssertNotNil(decoded.lockedPrepareQC)
        XCTAssertNotNil(decoded.highestPrepareQC)
        XCTAssertNotNil(decoded.lastTimeoutCertificate)
        XCTAssertNil(decoded.lastCommittedSubject)
        XCTAssertEqual(decoded.heightContext.epoch, 2)
        XCTAssertEqual(decoded.heightContext.epochEndHeight, 100)
        XCTAssertEqual(decoded.heightContext.mode, .npos)
        XCTAssertEqual(decoded.heightContext.validatorCount, 4)
        XCTAssertEqual(decoded.heightContext.quorum.minSigners, 3)
        XCTAssertEqual(decoded.heightContext.quorum.totalPower, 4)
        XCTAssertNil(decoded.lastCommitQC)
    }

    func testExecutionCommitmentRejectsNoncanonicalTopUpProjection() throws {
        let row = try XCTUnwrap(
            fixtureRows().first {
                $0.kind == "message" && $0.name == "commit_certificate_response"
            }
        )
        let message = try SumeragiV2ConsensusMessage.decodeCanonical(
            Data(sumeragiV2Hex: row.hex)
        )
        guard case .commitCertificateResponse(let response) = message.payload else {
            return XCTFail("response fixture decoded to the wrong v2 payload")
        }
        let commitment = response.certificate.executionCommitment
        XCTAssertEqual(response.certificate.reference.executionCommitment, commitment)

        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: commitment.parentStateRoot,
                topUpAnchorCount: 0,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: nil,
                topUpAnchorCount: 1,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: commitment.parentStateRoot,
                topUpAnchorCount: SumeragiV2ExecutionCommitment.maximumTopUpAnchorCount + 1,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: commitment.parentStateRoot,
                topUpAnchorCount: 1,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertEqual(commitment.executedBlockWireHash.bytes.count, 32)
    }

    func testMalformedAndSemanticallyNoncanonicalFixturesFailClosed() throws {
        for row in try fixtureRows() where row.kind == "negative_message" {
            let encoded = try Data(sumeragiV2Hex: row.hex)
            XCTAssertThrowsError(try SumeragiV2ConsensusMessage.decodeCanonical(encoded), row.name)
        }
        for row in try fixtureRows() where row.kind == "negative_status" {
            let encoded = try Data(sumeragiV2Hex: row.hex)
            XCTAssertThrowsError(try SumeragiV2Status.decodeCanonical(encoded), row.name)
        }
    }

    func testCommitCertificateBindingCorruptionsFailAgainstExactRequest() throws {
        let rows = try fixtureRows()
        let requestRow = try XCTUnwrap(rows.first {
            $0.kind == "message" && $0.name == "commit_certificate_request"
        })
        let requestMessage = try SumeragiV2ConsensusMessage.decodeCanonical(
            Data(sumeragiV2Hex: requestRow.hex)
        )
        guard case .commitCertificateRequest(let request) = requestMessage.payload else {
            return XCTFail("request fixture decoded to the wrong payload")
        }

        for row in rows where row.kind == "negative_binding" {
            let message = try SumeragiV2ConsensusMessage.decodeCanonical(
                Data(sumeragiV2Hex: row.hex)
            )
            guard case .commitCertificateResponse(let response) = message.payload else {
                return XCTFail("\(row.name) decoded to the wrong payload")
            }
            XCTAssertThrowsError(try response.validate(against: request), row.name)
        }
    }

    private struct FixtureRow {
        let kind: String
        let name: String
        let hex: String
    }

    private func fixtureRows() throws -> [FixtureRow] {
        let contents = try String(contentsOf: fixtureURL(), encoding: .utf8)
        return try contents.split(whereSeparator: \.isNewline).compactMap { rawLine in
            if rawLine.isEmpty || rawLine.first == "#" { return nil }
            let columns = rawLine.split(separator: "\t", omittingEmptySubsequences: false)
            guard columns.count == 4 else {
                throw SumeragiV2WireError.invalid("malformed fixture row")
            }
            guard columns[3] == "accept" || columns[3] == "reject" else {
                throw SumeragiV2WireError.invalid("unknown fixture expectation")
            }
            return FixtureRow(
                kind: String(columns[0]), name: String(columns[1]), hex: String(columns[2])
            )
        }
    }

    private func fixtureURL() throws -> URL {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while directory.path != "/" {
            let candidate = directory.appendingPathComponent(fixtureRelativePath)
            if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
            directory.deleteLastPathComponent()
        }
        throw SumeragiV2WireError.invalid("unable to locate \(fixtureRelativePath)")
    }

    private let fixtureRelativePath = "fixtures/sumeragi_v2/wire_v2.tsv"
    private let expectedMessageNames: Set<String> = [
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
    ]
}

private extension Data {
    init(sumeragiV2Hex hex: String) throws {
        guard hex.count.isMultiple(of: 2) else {
            throw SumeragiV2WireError.invalid("hex fixture has odd length")
        }
        var bytes = Data()
        bytes.reserveCapacity(hex.count / 2)
        var cursor = hex.startIndex
        while cursor < hex.endIndex {
            let next = hex.index(cursor, offsetBy: 2)
            guard let byte = UInt8(hex[cursor..<next], radix: 16) else {
                throw SumeragiV2WireError.invalid("hex fixture contains a non-hex byte")
            }
            bytes.append(byte)
            cursor = next
        }
        self = bytes
    }
}
