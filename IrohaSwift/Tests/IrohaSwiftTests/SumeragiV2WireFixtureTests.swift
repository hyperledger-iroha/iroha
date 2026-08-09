// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import XCTest
@testable import IrohaSwift

private func nativeAmxTestCrc16(_ bytes: [UInt8]) -> UInt16 {
    var crc = UInt16.max
    for byte in bytes {
        crc ^= UInt16(byte) << 8
        for _ in 0..<8 {
            crc = (crc & 0x8000) != 0 ? (crc &<< 1) ^ 0x1021 : crc &<< 1
        }
    }
    return crc
}

func nativeAmxTestHash(_ seed: UInt8) -> String {
    var bytes = [UInt8](repeating: seed, count: 32)
    bytes[31] |= 1
    let body = bytes.map { String(format: "%02X", $0) }.joined()
    let checksum = nativeAmxTestCrc16(Array("hash:\(body)".utf8))
    return "hash:\(body)#\(String(format: "%04X", checksum))"
}

func sumeragiV2TestHeightContext(epochEndHeight: UInt64 = 100) -> [String: Any] {
    [
        "epoch": 1,
        "epoch_end_height": epochEndHeight,
        "mode": ["mode": "permissioned", "details": NSNull()],
        "epoch_seed": [UInt8](repeating: 0x42, count: 32),
        "validator_count": 4,
        "quorum": [
            "min_signers": 3,
            "total_power": 4,
        ],
    ]
}

func sumeragiV2TestLiveness() -> [String: Any] {
    let idle: [String: Any] = ["stage": "idle", "details": NSNull()]
    return [
        "generation": 2,
        "prepare_quorums": [],
        "commit_quorums": [],
        "timeout_quorums": [],
        "outbound_intents": [],
        "work": [
            "candidate": idle,
            "body_recovery": idle,
            "body_store": idle,
            "validation": idle,
            "application": idle,
            "successor_height": idle,
        ],
        "queues": [],
        "no_progress_age_ms": 19,
        "ignore_counts": [],
    ]
}

func duplicateSumeragiRootField(_ prefix: String, in payload: Data) -> Data {
    var duplicate = Data(prefix.utf8)
    duplicate.append(contentsOf: payload.dropFirst())
    return duplicate
}

final class SumeragiV2WireFixtureTests: XCTestCase {
    func testExecutionCommitmentCarriesExactMandatoryMergeCarrierOption() throws {
        func hash(_ seed: UInt8) throws -> SumeragiV2Hash {
            var bytes = Data(repeating: seed, count: 32)
            bytes[31] |= 1
            return try SumeragiV2Hash(bytes)
        }
        let emptyManifestRoot = try SumeragiV2Hash(
            SumeragiV2ExecutionCommitment.nativeAmxApplicationManifestEmptyRootBytes()
        )
        let base = try SumeragiV2ExecutionCommitment(
            parentStateRoot: hash(0x21),
            postStateRoot: hash(0x23),
            ordinaryWritesRoot: hash(0x25),
            topUpAnchorRoot: nil,
            topUpAnchorCount: 0,
            nativeAmxApplicationManifestVersion: 1,
            nativeAmxApplicationManifestRoot: emptyManifestRoot,
            nativeAmxApplicationManifestCount: 0,
            executedBlockWireLen: 123,
            executedBlockWireHash: hash(0x27)
        )
        let carrier = try SumeragiV2MergeCarrierCommitment(entryHash: hash(0x29))
        let carried = try SumeragiV2ExecutionCommitment(
            parentStateRoot: base.parentStateRoot,
            postStateRoot: base.postStateRoot,
            ordinaryWritesRoot: base.ordinaryWritesRoot,
            topUpAnchorRoot: nil,
            topUpAnchorCount: 0,
            nativeAmxApplicationManifestVersion:
                base.nativeAmxApplicationManifestVersion,
            nativeAmxApplicationManifestRoot: base.nativeAmxApplicationManifestRoot,
            nativeAmxApplicationManifestCount: 0,
            mergeCarrier: carrier,
            executedBlockWireLen: base.executedBlockWireLen,
            executedBlockWireHash: base.executedBlockWireHash
        )

        XCTAssertNil(base.mergeCarrier)
        XCTAssertEqual(base.executedBlockWireLen, 123)
        XCTAssertEqual(carried.mergeCarrier, carrier)
        XCTAssertGreaterThan(carried.encode().count, base.encode().count)
        XCTAssertThrowsError(
            try SumeragiV2MergeCarrierCommitment(version: 2, entryHash: hash(0x29))
        )
    }

    func testUnsafeProposalIgnoreReasonUsesWireDiscriminantEleven() {
        XCTAssertEqual(SumeragiV2IgnoreReason(rawValue: 11), .unsafeProposal)
    }

    func testLivenessBlockerDecoderPinsEveryWireDiscriminant() throws {
        let expected: [SumeragiV2LivenessBlocker] = [
            .missingProposal,
            .bodyUnavailable,
            .prepareQuorumMissing,
            .commitQuorumMissing,
            .timeoutCertificateMissing,
            .schedulerStarvation,
            .applicationPending,
            .successorActivationPending,
            .localControlPending,
        ]
        for (tag, blocker) in expected.enumerated() {
            XCTAssertEqual(
                try SumeragiV2LivenessBlocker.decode(Data([UInt8(tag), 0, 0, 0])),
                blocker
            )
        }
    }

    func testLivenessBlockerDecoderRejectsUnknownTruncatedAndTrailingInputs() {
        let cases: [(Data, SumeragiV2WireError)] = [
            (Data([9, 0, 0, 0]), .invalid("unknown liveness blocker 9")),
            (Data([8, 0, 0]), .invalid("u32 is truncated")),
            (Data([8, 0, 0, 0, 0]), .invalid("u32 contains trailing bytes")),
        ]
        for (encoded, expectedError) in cases {
            XCTAssertThrowsError(try SumeragiV2LivenessBlocker.decode(encoded)) { error in
                XCTAssertEqual(error as? SumeragiV2WireError, expectedError)
            }
        }
    }

    func testDataAvailabilityRejectsRetiredEncodingTagAndZeroShards() throws {
        XCTAssertEqual(SumeragiV2PayloadEncoding(rawValue: 0), .reedSolomon16)
        XCTAssertNil(SumeragiV2PayloadEncoding(rawValue: 1))

        for shards in [(data: UInt16(0), parity: UInt16(1)), (data: 1, parity: 0)] {
            XCTAssertThrowsError(
                try SumeragiV2DataAvailabilityLayout(
                    encoding: .reedSolomon16,
                    chunkSizeBytes: 4,
                    dataShards: shards.data,
                    parityShards: shards.parity,
                    maxPayloadSizeBytes: 4,
                    maxChunkCount: 2
                )
            ) { error in
                XCTAssertEqual(
                    error as? SumeragiV2WireError,
                    .invalid(
                        "ReedSolomon16 data availability requires positive shard counts"
                    )
                )
            }
        }
    }

    func testRustCanonicalMessageFixturesRoundtrip() throws {
        let messages = try fixtureRows().filter { $0.kind == "message" }
        XCTAssertEqual(Set(messages.map(\.name)), expectedMessageNames)

        for row in messages {
            let encoded = try Data(sumeragiV2Hex: row.hex)
            let decoded = try SumeragiV2ConsensusMessage.decodeCanonical(encoded)
            XCTAssertEqual(decoded.encode(), encoded, row.name)
        }
    }

    func testRustMergeCarrierFixturePinsCurrentV4Shape() throws {
        let rows = try fixtureRows()
        let carriedRow = try XCTUnwrap(rows.first {
            $0.kind == "message" && $0.name == "quorum_certificate_merge_carrier"
        })
        let carriedMessage = try SumeragiV2ConsensusMessage.decodeCanonical(
            Data(sumeragiV2Hex: carriedRow.hex)
        )
        guard case .quorumCertificate(let certificate) = carriedMessage.payload else {
            return XCTFail("merge-carrier fixture decoded to the wrong payload")
        }
        let carrier = try XCTUnwrap(certificate.executionCommitment.mergeCarrier)
        XCTAssertEqual(carrier.version, SumeragiV2MergeCarrierCommitment.canonicalVersion)
        XCTAssertEqual(carrier.entryHash.bytes.count, 32)

        for name in [
            "execution_commitment_merge_carrier_wrong_version",
            "execution_commitment_missing_merge_carrier_field",
        ] {
            let row = try XCTUnwrap(rows.first {
                $0.kind == "negative_message" && $0.name == name
            })
            XCTAssertThrowsError(
                try SumeragiV2ConsensusMessage.decodeCanonical(
                    Data(sumeragiV2Hex: row.hex)
                ),
                name
            )
        }
    }

    func testCommitReproposalsRequireTheirVoteAndCertificateRound() throws {
        func message(_ name: String) throws -> SumeragiV2ConsensusMessage {
            let row = try XCTUnwrap(
                fixtureRows().first { $0.kind == "message" && $0.name == name }
            )
            return try SumeragiV2ConsensusMessage.decodeCanonical(
                Data(sumeragiV2Hex: row.hex)
            )
        }

        guard case .vote(let vote) = try message("commit_vote_reproposal").payload else {
            return XCTFail("Commit reproposal vote fixture decoded to the wrong payload")
        }
        guard case .quorumCertificate(let certificate) = try message(
            "commit_quorum_certificate_reproposal"
        ).payload else {
            return XCTFail("Commit reproposal QC fixture decoded to the wrong payload")
        }
        guard case .commitCertificateResponse(let response) = try message(
            "commit_certificate_response"
        ).payload else {
            return XCTFail("CommitQC response fixture decoded to the wrong payload")
        }

        XCTAssertEqual(vote.phase, .commit)
        XCTAssertEqual(vote.round.view, 9)
        XCTAssertEqual(vote.round, vote.proposalRound)
        XCTAssertEqual(certificate.round, vote.round)
        XCTAssertEqual(certificate.proposalRound, vote.proposalRound)
        XCTAssertEqual(certificate.subject, vote.subject)
        XCTAssertEqual(certificate.executionCommitment, vote.executionCommitment)
        XCTAssertEqual(response.certificate.reference, certificate.reference)

        let splitCases: [(name: String, message: String)] = [
            (
                "commit_vote_split_round",
                "Prepare/Commit vote proposal round must match its round"
            ),
            (
                "commit_quorum_certificate_split_round",
                "Prepare/Commit certificate proposal round must match its round"
            ),
        ]
        for splitCase in splitCases {
            let row = try XCTUnwrap(
                fixtureRows().first {
                    $0.kind == "negative_message" && $0.name == splitCase.name
                }
            )
            XCTAssertThrowsError(
                try SumeragiV2ConsensusMessage.decodeCanonical(
                    Data(sumeragiV2Hex: row.hex)
                )
            ) { error in
                XCTAssertEqual(
                    error as? SumeragiV2WireError,
                    .invalid(splitCase.message)
                )
            }
        }
    }

    func testStatusValidatorsRejectAnOlderCommitProposalRound() throws {
        let row = try XCTUnwrap(
            fixtureRows().first {
                $0.kind == "message" &&
                    $0.name == "commit_quorum_certificate_reproposal"
            }
        )
        let message = try SumeragiV2ConsensusMessage.decodeCanonical(
            Data(sumeragiV2Hex: row.hex)
        )
        guard case .quorumCertificate(let certificate) = message.payload else {
            return XCTFail("Commit reproposal QC fixture decoded to the wrong payload")
        }
        let olderProposalRound = SumeragiV2ConsensusRound(
            contextID: certificate.round.contextID,
            height: certificate.round.height,
            view: certificate.round.view - 1
        )

        XCTAssertThrowsError(
            try SumeragiV2QuorumCertificateRef(
                round: certificate.round,
                proposalRound: olderProposalRound,
                phase: .commit,
                subject: certificate.subject,
                executionCommitment: certificate.executionCommitment
            )
        ) { error in
            XCTAssertEqual(
                error as? SumeragiV2WireError,
                .invalid(
                    "Prepare/Commit certificate reference proposal round must match its round"
                )
            )
        }

        XCTAssertThrowsError(
            try SumeragiV2VoteQuorumStatus(
                round: certificate.round,
                proposalRound: olderProposalRound,
                subject: certificate.subject,
                executionCommitment: certificate.executionCommitment,
                signerCount: 1,
                signedPower: 1,
                minSigners: 3,
                totalPower: 4
            )
        ) { error in
            XCTAssertEqual(
                error as? SumeragiV2WireError,
                .invalid("Prepare/Commit quorum status proposal round must match its round")
            )
        }

        XCTAssertThrowsError(
            try SumeragiV2OutboundIntentStatus(
                kind: .commitVote,
                round: certificate.round,
                proposalRound: olderProposalRound,
                subject: certificate.subject,
                executionCommitment: certificate.executionCommitment,
                stage: .queued
            )
        ) { error in
            XCTAssertEqual(
                error as? SumeragiV2WireError,
                .invalid(
                    "Proposal/Prepare/Commit outbound intent origin must match its round"
                )
            )
        }
    }

    func testPrepareVotesAndCertificatesRejectSplitRounds() throws {
        let row = try XCTUnwrap(
            fixtureRows().first {
                $0.kind == "message" && $0.name == "quorum_certificate"
            }
        )
        let message = try SumeragiV2ConsensusMessage.decodeCanonical(
            Data(sumeragiV2Hex: row.hex)
        )
        guard case .quorumCertificate(let prepare) = message.payload else {
            return XCTFail("PrepareQC fixture decoded to the wrong payload")
        }
        let laterRound = SumeragiV2ConsensusRound(
            contextID: prepare.round.contextID,
            height: prepare.round.height,
            view: prepare.round.view + 1
        )

        XCTAssertThrowsError(
            try SumeragiV2Vote(
                round: laterRound,
                proposalRound: prepare.round,
                phase: .prepare,
                subject: prepare.subject,
                executionCommitment: prepare.executionCommitment,
                signer: 0,
                signature: Data([1])
            )
        ) { error in
            XCTAssertEqual(
                error as? SumeragiV2WireError,
                .invalid("Prepare/Commit vote proposal round must match its round")
            )
        }

        XCTAssertThrowsError(
            try SumeragiV2QuorumCertificate(
                round: laterRound,
                proposalRound: prepare.round,
                phase: .prepare,
                subject: prepare.subject,
                executionCommitment: prepare.executionCommitment,
                signers: prepare.signers,
                aggregateSignature: prepare.aggregateSignature
            )
        ) { error in
            XCTAssertEqual(
                error as? SumeragiV2WireError,
                .invalid("Prepare/Commit certificate proposal round must match its round")
            )
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
        XCTAssertEqual(response.certificate.round.view, 9)
        XCTAssertEqual(response.certificate.round, response.certificate.proposalRound)
        XCTAssertEqual(
            response.certificate.reference.proposalRound,
            response.certificate.proposalRound
        )
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
            proposalRound: response.certificate.proposalRound,
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
        XCTAssertFalse(decoded.restartRequired)
        XCTAssertEqual(decoded.height, 1)
        XCTAssertEqual(decoded.view, 3)
        XCTAssertEqual(decoded.phase, .commit)
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
        XCTAssertEqual(decoded.liveness.generation, 3)
        XCTAssertEqual(decoded.liveness.prepareQuorums.count, 1)
        XCTAssertEqual(decoded.liveness.commitQuorums.count, 1)
        XCTAssertEqual(decoded.liveness.prepareQuorums.first?.round.view, 1)
        XCTAssertEqual(decoded.liveness.prepareQuorums.first?.proposalRound.view, 1)
        XCTAssertEqual(decoded.liveness.commitQuorums.first?.round.view, 3)
        XCTAssertEqual(decoded.liveness.commitQuorums.first?.proposalRound.view, 3)
        XCTAssertEqual(decoded.liveness.timeoutQuorums.count, 1)
        XCTAssertEqual(decoded.liveness.outboundIntents.first?.kind, .commitVote)
        XCTAssertEqual(decoded.liveness.outboundIntents.first?.round.view, 3)
        XCTAssertEqual(decoded.liveness.outboundIntents.first?.proposalRound?.view, 3)
        XCTAssertEqual(decoded.liveness.queues.count, 1)
        XCTAssertEqual(decoded.liveness.queues.first?.queue, .effectDispatch)
        XCTAssertEqual(decoded.liveness.blocker, .localControlPending)

        // The fifth struct field follows four fixed-width fields and is the
        // canonical one-byte `restart_required` boolean.
        XCTAssertEqual(encoded[102], 1)
        var invalidBoolean = encoded
        invalidBoolean[103] = 2
        XCTAssertThrowsError(try SumeragiV2Status.decodeCanonical(invalidBoolean))
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
                nativeAmxApplicationManifestVersion:
                    commitment.nativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot:
                    commitment.nativeAmxApplicationManifestRoot,
                nativeAmxApplicationManifestCount:
                    commitment.nativeAmxApplicationManifestCount,
                executedBlockWireLen: commitment.executedBlockWireLen,
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
                nativeAmxApplicationManifestVersion:
                    commitment.nativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot:
                    commitment.nativeAmxApplicationManifestRoot,
                nativeAmxApplicationManifestCount:
                    commitment.nativeAmxApplicationManifestCount,
                executedBlockWireLen: commitment.executedBlockWireLen,
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
                nativeAmxApplicationManifestVersion:
                    commitment.nativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot:
                    commitment.nativeAmxApplicationManifestRoot,
                nativeAmxApplicationManifestCount:
                    commitment.nativeAmxApplicationManifestCount,
                executedBlockWireLen: commitment.executedBlockWireLen,
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
                nativeAmxApplicationManifestVersion:
                    commitment.nativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot:
                    commitment.nativeAmxApplicationManifestRoot,
                nativeAmxApplicationManifestCount:
                    commitment.nativeAmxApplicationManifestCount,
                executedBlockWireLen: commitment.executedBlockWireLen,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: commitment.topUpAnchorRoot,
                topUpAnchorCount: commitment.topUpAnchorCount,
                nativeAmxApplicationManifestVersion:
                    commitment.nativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot:
                    commitment.nativeAmxApplicationManifestRoot,
                nativeAmxApplicationManifestCount:
                    commitment.nativeAmxApplicationManifestCount,
                mergeCarrier: commitment.mergeCarrier,
                executedBlockWireLen: 0,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertEqual(commitment.executedBlockWireHash.bytes.count, 32)
    }

    func testExecutionCommitmentRejectsNoncanonicalNativeAmxManifestProjection() throws {
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
        XCTAssertEqual(
            commitment.nativeAmxApplicationManifestRoot.bytes,
            SumeragiV2ExecutionCommitment.nativeAmxApplicationManifestEmptyRootBytes()
        )

        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: nil,
                topUpAnchorCount: 0,
                nativeAmxApplicationManifestVersion:
                    SumeragiV2ExecutionCommitment
                        .canonicalNativeAmxApplicationManifestVersion + 1,
                nativeAmxApplicationManifestRoot:
                    commitment.nativeAmxApplicationManifestRoot,
                nativeAmxApplicationManifestCount: 0,
                executedBlockWireLen: commitment.executedBlockWireLen,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: nil,
                topUpAnchorCount: 0,
                nativeAmxApplicationManifestVersion:
                    SumeragiV2ExecutionCommitment
                        .canonicalNativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot: commitment.parentStateRoot,
                nativeAmxApplicationManifestCount: 0,
                executedBlockWireLen: commitment.executedBlockWireLen,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: nil,
                topUpAnchorCount: 0,
                nativeAmxApplicationManifestVersion:
                    SumeragiV2ExecutionCommitment
                        .canonicalNativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot:
                    commitment.nativeAmxApplicationManifestRoot,
                nativeAmxApplicationManifestCount: 1,
                executedBlockWireLen: commitment.executedBlockWireLen,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
        XCTAssertThrowsError(
            try SumeragiV2ExecutionCommitment(
                parentStateRoot: commitment.parentStateRoot,
                postStateRoot: commitment.postStateRoot,
                ordinaryWritesRoot: commitment.ordinaryWritesRoot,
                topUpAnchorRoot: nil,
                topUpAnchorCount: 0,
                nativeAmxApplicationManifestVersion:
                    SumeragiV2ExecutionCommitment
                        .canonicalNativeAmxApplicationManifestVersion,
                nativeAmxApplicationManifestRoot: commitment.parentStateRoot,
                nativeAmxApplicationManifestCount:
                    SumeragiV2ExecutionCommitment
                        .maximumNativeAmxApplicationManifestLeafCount + 1,
                executedBlockWireLen: commitment.executedBlockWireLen,
                executedBlockWireHash: commitment.executedBlockWireHash
            )
        )
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
