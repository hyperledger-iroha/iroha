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
        XCTAssertEqual(transition.preparation.committee.count, 4)
        let policy = transition.preparation.eligibility
        XCTAssertEqual(policy.assetScope, .global)
        XCTAssertEqual(policy.assetScale, 9)
        XCTAssertEqual(policy.epochLengthBlocks, 100)
        XCTAssertFalse(policy.minSelfBond.mantissaLittleEndian.isEmpty)
        XCTAssertFalse(policy.minNominationBond.mantissaLittleEndian.isEmpty)
        XCTAssertTrue((4...31).contains(policy.maxValidators))
        XCTAssertEqual(try ValidatorStakingNoritoV1.ElectionPolicy(noritoPayload: policy.noritoPayload).noritoPayload,
                       policy.noritoPayload)
        for member in transition.preparation.committee {
            XCTAssertEqual(member.blsPublicKey.count, 48)
            XCTAssertEqual(member.proofOfPossession.count, 96)
            XCTAssertEqual(try ValidatorStakingNoritoV1.CommitteeMember(noritoPayload: member.noritoPayload).noritoPayload,
                           member.noritoPayload)
        }
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

    func testElectionPolicyRejectsSyntheticScopedImpreciseAndUnboundedStake() throws {
        let policy = try fields(preparationFields()[11])
        let synthetic: [UInt8] = [
            0x5e, 0xcd, 0x1e, 0x80, 0xac, 0x7d, 0x4d, 0x18,
            0xb2, 0x27, 0x72, 0x09, 0x1a, 0x73, 0xfc, 0x13,
        ]
        let invalid: [(Int, Data)] = [
            (0, record(synthetic.map { Data([$0]) })),
            (0, record(Array(repeating: Data([0]), count: 16))),
            (1, uint(1, bytes: 4) + record([uint(7)])),
            (1, uint(0, bytes: 4) + Data([0])),
            (2, uint(8, bytes: 4)), (2, uint(10, bytes: 4)),
            (3, quantity([], scale: 0)), (4, quantity([], scale: 0)),
            (3, quantity([1], scale: 10)), (4, quantity([1], scale: 10)),
            (5, uint(3, bytes: 4)), (5, uint(5, bytes: 4)), (5, uint(34, bytes: 4)),
            (6, uint(0)), (6, uint(2)),
        ]
        for (index, value) in invalid {
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.ElectionPolicy(
                noritoPayload: replacing(policy, index: index, value: value)), "policy field \(index)")
        }
        for maximum in [UInt32(4), 7, 31] {
            XCTAssertEqual(try ValidatorStakingNoritoV1.ElectionPolicy(
                noritoPayload: replacing(policy, index: 5, value: uint(UInt64(maximum), bytes: 4))).maxValidators,
                maximum)
        }
        XCTAssertEqual(try ValidatorStakingNoritoV1.ElectionPolicy(
            noritoPayload: replacing(policy, index: 6, value: uint(UInt64.max))).epochLengthBlocks, UInt64.max)
        for index in [3, 4] {
            let exact = try ValidatorStakingNoritoV1.ElectionPolicy(
                noritoPayload: replacing(policy, index: index, value: quantity([1], scale: 9)))
            XCTAssertEqual(index == 3 ? exact.minSelfBond.scale : exact.minNominationBond.scale, 9)
        }
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.ElectionPolicy(noritoPayload: record(Array(policy.dropLast()))))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.ElectionPolicy(noritoPayload: record(policy + [uint(1, bytes: 4)])))
    }

    func testPreparationRejectsRetiredLayoutMalformedMembersAndOverflow() throws {
        let preparation = try preparationFields()
        let members = try vectorFields(preparation[12])
        let retiredRoster = try members.map { record([try fields($0)[0], uint(1)]) }
        let retiredPops = try members.map { try fields($0)[1] }
        var retired = preparation
        retired[11] = vector(retiredRoster)
        retired[12] = vector(retiredPops)
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.CommitteePreparation(noritoPayload: record(retired)))
        var duplicated = members
        duplicated[1] = duplicated[0]
        let invalid: [(Int, Data)] = [
            (0, uint(2, bytes: 2)), (1, Data(repeating: 0, count: 32)), (3, uint(0)),
            (4, Data(repeating: 0, count: 32)), (5, uint(3)), (6, uint(101)),
            (7, uint(200)), (7, uint(301)), (8, uint(0)),
            (9, Data(repeating: 0, count: 32)), (10, Data(repeating: 0, count: 32)),
            (12, vector(Array(members.prefix(3)))), (12, vector(members + [members[0]])),
            (12, vector(Array(members.reversed()))), (12, vector(duplicated)),
            (12, vector(Array(repeating: members[0], count: 32))),
        ]
        for (index, value) in invalid {
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.CommitteePreparation(
                noritoPayload: replacing(preparation, index: index, value: value)), "preparation field \(index)")
        }
        var overflow = preparation
        overflow[2] = uint(UInt64.max)
        overflow[5] = uint(1)
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.CommitteePreparation(noritoPayload: record(overflow)))
        var heightOverflow = preparation
        heightOverflow[3] = uint(UInt64.max)
        heightOverflow[6] = uint(1)
        heightOverflow[7] = uint(100)
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.CommitteePreparation(noritoPayload: record(heightOverflow)))

        let member = try fields(members[0])
        for length in [0, 95, 97] {
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.CommitteeMember(
                noritoPayload: replacing(member, index: 1, value: uint(UInt64(length)) + Data(repeating: 0, count: length))))
        }
        var key = try vectorFields(fields(member[0])[0])
        key[0] = Data([0])
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.CommitteeMember(
            noritoPayload: replacing(member, index: 0, value: record([vector(key)]))))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.CommitteeMember(noritoPayload: record(Array(member.dropLast()))))
    }

    func testPreparationRetainsUnsignedHeightsAndValueData() throws {
        var high = try preparationFields()
        high[2] = uint(UInt64.max - 2)
        high[5] = uint(UInt64.max)
        high[3] = uint(UInt64.max - 109)
        high[6] = uint(UInt64.max - 100)
        high[7] = uint(UInt64.max - 1)
        var bytes = record(high)
        let original = bytes
        let decoded = try ValidatorStakingNoritoV1.CommitteePreparation(noritoPayload: bytes)
        XCTAssertEqual(decoded.targetEpoch, UInt64.max)
        XCTAssertEqual(decoded.lastHeight, UInt64.max - 1)
        bytes.resetBytes(in: 0..<bytes.count)
        var members = decoded.committee
        members.removeAll()
        var definition = decoded.eligibility.xorAssetDefinitionID
        definition.resetBytes(in: 0..<definition.count)
        XCTAssertEqual(decoded.committee.count, 4)
        XCTAssertEqual(decoded.noritoPayload, original)
    }

    private func preparationFields() throws -> [Data] {
        try fields(fields(fixtureRows()["committee_transition"]!)[0])
    }

    func testDkgBoundsAndUnsignedCutoffs() throws {
        let rows = try fixtureRows()
        var session = try fields(XCTUnwrap(rows["dkg_session"]))
        session[8] = uint(UInt64.max - 3)
        session[9] = uint(UInt64.max - 2)
        session[10] = uint(UInt64.max - 1)
        session[11] = uint(UInt64.max)
        XCTAssertEqual(try ValidatorStakingNoritoV1.DkgSession(noritoPayload: record(session)).acceptancesEndHeight, UInt64.max)
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.DkgSession(noritoPayload: replacing(session, index: 8, value: uint(0))))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.DkgSession(noritoPayload: replacing(session, index: 11, value: uint(UInt64.max - 2))))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.DkgSession(noritoPayload: replacing(session, index: 6, value: uint(34, bytes: 2))))
    }

    private func fields(_ payload: Data) throws -> [Data] {
        var reader = CanonicalNoritoReader(data: payload)
        var values = [Data]()
        while reader.remaining() > 0 {
            values.append(try reader.readCompactField())
        }
        return values
    }

    private func vectorFields(_ payload: Data) throws -> [Data] {
        var reader = CanonicalNoritoReader(data: payload)
        let count = Int(try reader.readUInt64LE())
        var values = [Data]()
        for _ in 0..<count {
            values.append(try reader.readCompactField())
        }
        XCTAssertEqual(reader.remaining(), 0)
        return values
    }

    private func record(_ fields: [Data]) -> Data {
        var output = Data()
        for field in fields {
            var length = UInt64(field.count)
            while length >= 0x80 {
                output.append(UInt8(length & 0x7f) | 0x80)
                length >>= 7
            }
            output.append(UInt8(length))
            output.append(field)
        }
        return output
    }

    private func replacing(_ fields: [Data], index: Int, value: Data) -> Data {
        var changed = fields
        changed[index] = value
        return record(changed)
    }

    private func vector(_ fields: [Data]) -> Data { uint(UInt64(fields.count)) + record(fields) }

    private func uint(_ value: UInt64, bytes: Int = 8) -> Data {
        Data((0..<bytes).map { UInt8(truncatingIfNeeded: value >> ($0 * 8)) })
    }

    private func quantity(_ mantissa: [UInt8], scale: UInt32) -> Data {
        record([uint(UInt64(mantissa.count), bytes: 4) + Data(mantissa), uint(UInt64(scale), bytes: 4)])
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
