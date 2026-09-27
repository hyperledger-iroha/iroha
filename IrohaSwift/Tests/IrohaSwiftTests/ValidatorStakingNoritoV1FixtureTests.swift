// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
@testable import IrohaSwift
import XCTest

/// Rust-authored canonical validator and staking DTO bytes consumed by Swift.
final class ValidatorStakingNoritoV1FixtureTests: XCTestCase {
    private let expectedKinds: Set<String> = [
        "authority_generation", "epoch_authorization", "dkg_session", "dkg_transcript",
        "committee_transition", "monetary_plan", "rebind_peer",
    ]

    func testCanonicalRustFixtures() throws {
        let rows = try fixtureRows()
        XCTAssertEqual(Set(rows.keys), expectedKinds)

        let authority = try ValidatorStakingNoritoV1.AuthorityGeneration(
            noritoPayload: rows["authority_generation"]!
        )
        XCTAssertEqual(authority.generation, 0)
        XCTAssertEqual(authority.validators.count, 4)
        XCTAssertEqual(authority.noritoPayload, rows["authority_generation"])

        let epoch = try ValidatorStakingNoritoV1.EpochAuthorization(
            noritoPayload: rows["epoch_authorization"]!
        )
        XCTAssertEqual(epoch.decision, .genesis)
        XCTAssertEqual(epoch.authorityGeneration, 0)
        XCTAssertEqual(epoch.noritoPayload, rows["epoch_authorization"])

        let session = try ValidatorStakingNoritoV1.DkgSession(
            noritoPayload: rows["dkg_session"]!
        )
        XCTAssertEqual(session.committeeSize, 4)
        XCTAssertEqual(session.threshold, 2)
        XCTAssertEqual(session.authorityGeneration, 1)
        XCTAssertEqual(session.noritoPayload, rows["dkg_session"])

        let transcript = try ValidatorStakingNoritoV1.DkgTranscript(
            noritoPayload: rows["dkg_transcript"]!
        )
        XCTAssertEqual(transcript.recipientKeys.count, 4)
        XCTAssertEqual(transcript.dealerCommitments.count, 4)
        XCTAssertEqual(transcript.encryptedShares.count, 16)
        XCTAssertEqual(transcript.shareAcceptances.count, 16)
        XCTAssertEqual(transcript.finalizedAtHeight, 131)
        XCTAssertEqual(transcript.noritoPayload, rows["dkg_transcript"])

        let transition = try ValidatorStakingNoritoV1.CommitteeTransition(
            noritoPayload: rows["committee_transition"]!
        )
        XCTAssertEqual(transition.preparation.targetEpoch, 2)
        XCTAssertEqual(transition.preparation.roster.count, 4)
        XCTAssertEqual(transition.credentials?.authority.generation, 1)
        XCTAssertEqual(transition.outcome?.decision, .activate)
        XCTAssertEqual(transition.noritoPayload, rows["committee_transition"])

        let plan = try ValidatorStakingNoritoV1.MonetaryPlan(
            noritoPayload: rows["monetary_plan"]!
        )
        XCTAssertNotNil(plan.networkScope)
        XCTAssertEqual(plan.amount.mantissaLittleEndian, Data([0xE8, 0x03]))
        XCTAssertEqual(plan.precondition.kind, .registration)
        XCTAssertEqual(plan.precondition.activationHeight, 201)
        XCTAssertEqual(plan.sourceAsset.definition, plan.destinationAsset.definition)
        XCTAssertEqual(plan.noritoPayload, rows["monetary_plan"])

        let rebind = try ValidatorStakingNoritoV1.RebindPeer(
            noritoPayload: rows["rebind_peer"]!
        )
        XCTAssertEqual(rebind.laneID, 0)
        XCTAssertEqual(rebind.noritoPayload, rows["rebind_peer"])
    }

    func testTruncatedRecordsFailClosed() throws {
        let rows = try fixtureRows()
        let decoders: [String: (Data) throws -> Void] = [
            "authority_generation": { _ = try ValidatorStakingNoritoV1.AuthorityGeneration(noritoPayload: $0) },
            "epoch_authorization": { _ = try ValidatorStakingNoritoV1.EpochAuthorization(noritoPayload: $0) },
            "dkg_session": { _ = try ValidatorStakingNoritoV1.DkgSession(noritoPayload: $0) },
            "dkg_transcript": { _ = try ValidatorStakingNoritoV1.DkgTranscript(noritoPayload: $0) },
            "committee_transition": { _ = try ValidatorStakingNoritoV1.CommitteeTransition(noritoPayload: $0) },
            "monetary_plan": { _ = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: $0) },
            "rebind_peer": { _ = try ValidatorStakingNoritoV1.RebindPeer(noritoPayload: $0) },
        ]
        for (kind, bytes) in rows {
            XCTAssertThrowsError(try decoders[kind]!(Data(bytes.dropLast())), kind)
        }
    }

    func testQuantityRejectsNoncanonicalDecimals() throws {
        func quantity(_ mantissa: [UInt8], scale: UInt8) -> Data {
            let mantissaField = [UInt8(mantissa.count), 0, 0, 0] + mantissa
            let scaleField: [UInt8] = [scale, 0, 0, 0]
            return Data([UInt8(mantissaField.count)] + mantissaField + [4] + scaleField)
        }

        XCTAssertEqual(
            try ValidatorStakingNoritoV1.Quantity(noritoPayload: quantity([0xC8, 0], scale: 0)).mantissaLittleEndian,
            Data([0xC8, 0])
        )
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.Quantity(noritoPayload: quantity([10], scale: 1)))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.Quantity(noritoPayload: quantity([], scale: 1)))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.Quantity(noritoPayload: quantity([1], scale: 29)))
    }

    private func fixtureRows() throws -> [String: Data] {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while directory.path != "/" {
            let fixture = directory.appendingPathComponent("fixtures/validator_staking/norito_v1.tsv")
            if FileManager.default.fileExists(atPath: fixture.path) {
                var rows = [String: Data]()
                for line in try String(contentsOf: fixture, encoding: .utf8).split(separator: "\n") {
                    if line.isEmpty || line.hasPrefix("#") { continue }
                    let fields = line.split(separator: "\t", omittingEmptySubsequences: false)
                    guard fields.count == 2, expectedKinds.contains(String(fields[0])), rows[String(fields[0])] == nil,
                          fields[1].count.isMultiple(of: 2) else {
                        throw FixtureError.invalidRow(String(line))
                    }
                    var bytes = Data()
                    let hex = Array(fields[1].utf8)
                    for index in stride(from: 0, to: hex.count, by: 2) {
                        guard let value = UInt8(String(decoding: hex[index..<(index + 2)], as: UTF8.self), radix: 16) else {
                            throw FixtureError.invalidRow(String(line))
                        }
                        bytes.append(value)
                    }
                    rows[String(fields[0])] = bytes
                }
                return rows
            }
            directory.deleteLastPathComponent()
        }
        throw FixtureError.missing
    }

    private enum FixtureError: Error {
        case missing
        case invalidRow(String)
    }
}
