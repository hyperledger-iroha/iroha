// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
@testable import IrohaSwift
import XCTest

/// Rust-authored canonical validator and staking DTO bytes consumed by Swift.
final class ValidatorStakingNoritoV1FixtureTests: XCTestCase {
    private let expectedKinds: Set<String> = [
        "authority_generation", "epoch_authorization", "dkg_session", "dkg_transcript",
        "committee_transition", "monetary_plan", "monetary_bond_plan", "monetary_unbond_plan",
        "monetary_slash_plan", "reward_claim_plan", "fee_reward_claim_plan", "rebind_peer",
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

        for kind in ["reward_claim_plan", "fee_reward_claim_plan"] {
            let claim = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: rows[kind]!)
            XCTAssertNotNil(claim.networkScope)
            XCTAssertEqual(claim.validUntilHeight, 210)
            XCTAssertEqual(claim.expectedState?.throughEpoch, 200)
            XCTAssertEqual(claim.records.count, 1)
            XCTAssertEqual(claim.records.first?.epoch, 201)
            XCTAssertEqual(claim.records.first?.recordHash.count, 32)
            XCTAssertEqual(claim.sources.count, 1)
            XCTAssertEqual(claim.sources.first?.expectedAccrued?.mantissaLittleEndian, Data([5]))
            XCTAssertEqual(claim.sources.first?.payout.mantissaLittleEndian, Data([15]))
            XCTAssertEqual(claim.sources.first?.sourceAsset.noritoPayload, plan.destinationAsset.noritoPayload)
            XCTAssertEqual(claim.sources.first?.destinationAsset.noritoPayload, plan.sourceAsset.noritoPayload)
            XCTAssertEqual(claim.noritoPayload, rows[kind])
            if kind == "reward_claim_plan" {
                XCTAssertNil(claim.feeClaim)
            } else {
                let fee = try XCTUnwrap(claim.feeClaim)
                XCTAssertEqual(fee.lifecycleSeal, Data(repeating: 0x77, count: 32))
                XCTAssertEqual(fee.beneficiaryID, plan.sourceAsset.account)
                XCTAssertEqual(fee.beneficiaryRevision, 4)
                XCTAssertEqual(fee.sourceAsset.noritoPayload, plan.destinationAsset.noritoPayload)
                XCTAssertEqual(fee.destinationAsset.noritoPayload, plan.sourceAsset.noritoPayload)
                XCTAssertEqual(fee.amount.mantissaLittleEndian, Data([7]))
                XCTAssertEqual(fee.expectedClaimSequence, 5)
                XCTAssertEqual(try ValidatorStakingNoritoV1.FeeRewardClaim(noritoPayload: fee.noritoPayload).noritoPayload,
                               fee.noritoPayload)
            }
        }

        let rebind = try ValidatorStakingNoritoV1.RebindPeer(
            noritoPayload: rows["rebind_peer"]!
        )
        XCTAssertEqual(rebind.laneID, 0)
        XCTAssertEqual(rebind.noritoPayload, rows["rebind_peer"])
    }

    func testMonetaryOperationFixturesBindExactTypedFieldsAndNetworkXor() throws {
        let rows = try fixtureRows()
        let registration = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: rows["monetary_plan"]!)
        let transition = try ValidatorStakingNoritoV1.CommitteeTransition(noritoPayload: rows["committee_transition"]!)
        let bond = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: rows["monetary_bond_plan"]!)
        let unbond = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: rows["monetary_unbond_plan"]!)
        let slash = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: rows["monetary_slash_plan"]!)
        guard case let .bond(bondBinding) = bond.precondition,
              case let .unbond(unbondBinding) = unbond.precondition,
              case let .slash(slashBinding) = slash.precondition else {
            XCTFail("operation-specific fixtures must decode to their exact typed variants")
            return
        }
        XCTAssertEqual(bondBinding.peerID.algorithm, .blsNormal)
        XCTAssertEqual(bondBinding.peerID.noritoPayload, transition.preparation.committee[0].validator)
        XCTAssertEqual(bondBinding.peerID.publicKey, transition.preparation.committee[0].blsPublicKey)
        XCTAssertEqual(unbondBinding.requestHash, Data(repeating: 0x75, count: 32))
        XCTAssertEqual(slashBinding.slashableExposure.mantissaLittleEndian, Data([0xdc, 0x05]))
        XCTAssertEqual(slashBinding.slashableExposure.scale, 0)
        for (name, plan) in [("monetary_bond_plan", bond), ("monetary_unbond_plan", unbond), ("monetary_slash_plan", slash)] {
            XCTAssertEqual(plan.networkScope, transition.preparation.networkID)
            XCTAssertEqual(plan.validUntilHeight, 210)
            XCTAssertEqual(plan.amount.mantissaLittleEndian, registration.amount.mantissaLittleEndian)
            XCTAssertEqual(plan.amount.scale, 0)
            XCTAssertEqual(plan.precondition.activationHeight, 201)
            for asset in [plan.sourceAsset, plan.destinationAsset] {
                XCTAssertEqual(asset.definition, transition.preparation.eligibility.xorAssetDefinitionID)
                XCTAssertNil(asset.scopeDataspace)
            }
            let deposits = plan.precondition.kind == .bond
            XCTAssertEqual(plan.sourceAsset.noritoPayload, deposits ? registration.sourceAsset.noritoPayload : registration.destinationAsset.noritoPayload)
            XCTAssertEqual(plan.destinationAsset.noritoPayload, deposits ? registration.destinationAsset.noritoPayload : registration.sourceAsset.noritoPayload)
            XCTAssertEqual(plan.noritoPayload, rows[name])
        }
    }

    func testMonetaryBindingsRejectMalformedPeerHashAndExposure() throws {
        let rows = try fixtureRows()
        func assertRejected(_ kind: String, _ tag: UInt64, _ body: [Data], file: StaticString = #filePath, line: UInt = #line) throws {
            let original = try fields(rows[kind]!)
            let variant = uint(tag, bytes: 4) + record([record(body)])
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.MonetaryPrecondition(noritoPayload: variant), file: file, line: line)
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.MonetaryPlan(
                noritoPayload: replacing(original, index: 5, value: variant)), file: file, line: line)
        }
        let peer = try fields(fields(fields(rows["monetary_bond_plan"]!)[5].dropFirst(4))[0])[1]
        let key = try vectorFields(fields(peer)[0])
        let badPeers = [
            Data(), record([]), record([Data()]), record([vector([])]),
            record([vector([Data([0xff])] + Array(key.dropFirst()))]),
            record([vector(Array(key.dropLast()))]),
            record([vector(key + [Data([1])])]),
            record([vector([Data([2])] + Array(repeating: Data([0]), count: 48))]),
            record([vector([Data([2, 0])] + Array(key.dropFirst()))]),
            record([vector(key), Data([0])]),
        ]
        for malformed in badPeers { try assertRejected("monetary_bond_plan", 1, [uint(201), malformed]) }
        for width in [0, 1, 31, 33] {
            try assertRejected("monetary_unbond_plan", 2, [uint(201), Data(repeating: 0x75, count: width)])
        }
        var unmarked = Data(repeating: 0x75, count: 32)
        unmarked[31] = 0x74
        try assertRejected("monetary_unbond_plan", 2, [uint(201), unmarked])
        for malformed in [quantity([0xff], scale: 0), quantity([10], scale: 1), quantity([1], scale: 29), Data()] {
            try assertRejected("monetary_slash_plan", 3, [uint(201), malformed])
        }
        for (kind, tag, binding) in [("monetary_bond_plan", UInt64(1), peer),
                                   ("monetary_unbond_plan", 2, Data(repeating: 0x75, count: 32)),
                                   ("monetary_slash_plan", 3, quantity([0xdc, 5], scale: 0))] {
            try assertRejected(kind, tag, [uint(201)])
            try assertRejected(kind, tag, [uint(201), binding, Data([0])])
            for width in [0, 7, 9] { try assertRejected(kind, tag, [Data(repeating: 0, count: width), binding]) }
            try assertRejected(kind, 4, [uint(201), binding])
            try assertRejected(kind, UInt64(UInt32.max), [uint(201), binding])
        }
        let body = record([uint(201), Data(repeating: 0x75, count: 32)])
        let nonminimal = uint(2, bytes: 4) + Data([UInt8(body.count) | 0x80, 0]) + body
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.MonetaryPrecondition(noritoPayload: nonminimal))
    }

    func testMonetaryBindingsPreserveUnsignedHeightAndOwnedValues() throws {
        let rows = try fixtureRows()
        for (kind, tag) in [("monetary_bond_plan", UInt64(1)), ("monetary_unbond_plan", 2), ("monetary_slash_plan", 3)] {
            var plan = try fields(rows[kind]!)
            var binding = try fields(fields(plan[5].dropFirst(4))[0])
            for height in [UInt64(0), UInt64.max] {
                binding[0] = uint(height)
                plan[5] = uint(tag, bytes: 4) + record([record(binding)])
                var bytes = record(plan)
                let original = bytes
                let decoded = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: bytes)
                bytes.resetBytes(in: 0..<bytes.count)
                XCTAssertEqual(decoded.precondition.activationHeight, height)
                XCTAssertEqual(decoded.noritoPayload, original)
                switch decoded.precondition {
                case .bond(let value):
                    var key = value.peerID.publicKey
                    key.resetBytes(in: 0..<key.count)
                    XCTAssertNotEqual(value.peerID.publicKey, key)
                case .unbond(let value):
                    var hash = value.requestHash
                    hash.resetBytes(in: 0..<hash.count)
                    XCTAssertEqual(value.requestHash, Data(repeating: 0x75, count: 32))
                case .slash(let value): XCTAssertEqual(value.slashableExposure.mantissaLittleEndian, Data([0xdc, 5]))
                case .registration: XCTFail("wrong monetary variant")
                }
            }
        }
        // PeerId is a generic Rust key identity, not a BLS-only wire alias.
        let replacementPeer = try fields(rows["rebind_peer"]!)[2]
        let peer = try ValidatorStakingNoritoV1.PeerID(noritoPayload: replacementPeer)
        XCTAssertEqual(peer.algorithm, .ed25519)
        XCTAssertEqual(peer.noritoPayload, replacementPeer)
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
            "monetary_bond_plan": { _ = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: $0) },
            "monetary_unbond_plan": { _ = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: $0) },
            "monetary_slash_plan": { _ = try ValidatorStakingNoritoV1.MonetaryPlan(noritoPayload: $0) },
            "reward_claim_plan": { _ = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: $0) },
            "fee_reward_claim_plan": { _ = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: $0) },
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

    func testRewardPlanRequiresExplicitFeeOptionAndBoundedOrderedRecords() throws {
        let plan = try fields(fixtureRows()["reward_claim_plan"]!)
        let records = try vectorFields(plan[3])
        let sources = try vectorFields(plan[4])
        let invalid: [Data] = [
            record(Array(plan.dropLast())), record(plan + [Data([0])]),
            replacing(plan, index: 5, value: Data([2])),
            replacing(plan, index: 5, value: Data([0, 0])),
            replacing(plan, index: 1, value: uint(0)),
            replacing(plan, index: 3, value: vector(Array(repeating: records[0], count: 65))),
            replacing(plan, index: 4, value: vector(Array(repeating: sources[0], count: 65))),
            replacing(plan, index: 3, value: vector([records[0], records[0]])),
            replacing(plan, index: 4, value: vector([sources[0], sources[0]])),
            replacing(plan, index: 2, value: option(record([option(uint(201))]))),
        ]
        for bytes in invalid {
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: bytes))
        }
        let recordFields = try fields(records[0])
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardRecordRef(
            noritoPayload: replacing(recordFields, index: 1, value: Data(repeating: 1, count: 31))))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardRecordRef(noritoPayload: record(Array(recordFields.dropLast()))))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardClaimState(noritoPayload: Data()))
        XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardClaimState(noritoPayload: record([Data([0]), Data([0])])))
        XCTAssertNil(try ValidatorStakingNoritoV1.RewardClaimState(noritoPayload: record([Data([0])])).throughEpoch)
        XCTAssertEqual(try ValidatorStakingNoritoV1.RewardClaimState(noritoPayload: record([option(uint(0))])).throughEpoch, 0)
        var high = plan
        high[1] = uint(UInt64.max)
        high[2] = option(record([option(uint(UInt64.max - 1))]))
        high[3] = vector([replacing(recordFields, index: 0, value: uint(UInt64.max))])
        let decoded = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: record(high))
        XCTAssertEqual(decoded.validUntilHeight, UInt64.max)
        XCTAssertEqual(decoded.records[0].epoch, UInt64.max)
        XCTAssertEqual(decoded.noritoPayload, record(high))
    }

    func testRewardPlanRetainsAllSixtyFourSourcesInTheirSignedOrder() throws {
        var plan = try fields(fixtureRows()["reward_claim_plan"]!)
        let reference = try fields(vectorFields(plan[3])[0])
        let source = try fields(vectorFields(plan[4])[0])
        let sourceAsset = try fields(source[0])
        let destinationAsset = try fields(source[1])
        let sources = (0..<64).map { index -> Data in
            let definition = record([Data([UInt8(index)])] + Array(repeating: Data([9]), count: 15))
            var changed = source
            changed[0] = replacing(sourceAsset, index: 1, value: definition)
            changed[1] = replacing(destinationAsset, index: 1, value: definition)
            return record(changed)
        }
        plan[2] = Data([0])
        plan[3] = vector((0..<64).map { replacing(reference, index: 0, value: uint(UInt64($0))) })
        plan[4] = vector(sources)
        let payload = record(plan)
        let decoded = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: payload)
        XCTAssertNil(decoded.expectedState)
        XCTAssertEqual(decoded.records.count, 64)
        XCTAssertEqual(decoded.sources.map(\.noritoPayload), sources)
        XCTAssertEqual(decoded.noritoPayload, payload)
    }

    func testRewardSourcesRequireSemanticAccountDefinitionAndScopeOrder() throws {
        let plan = try fields(fixtureRows()["reward_claim_plan"]!)
        let source = try fields(vectorFields(plan[4])[0])
        let asset = try fields(source[0])
        let destination = try fields(source[1])
        func claim(_ owner: Data, _ scope: Data) -> Data {
            var changed = source
            changed[0] = record([owner, asset[1], scope])
            changed[1] = replacing(destination, index: 2, value: scope)
            return record(changed)
        }
        func assertOrdered(_ sources: [Data]) throws {
            let payload = replacing(plan, index: 4, value: vector(sources))
            XCTAssertEqual(try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: payload).noritoPayload, payload)
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardClaimPlan(
                noritoPayload: replacing(plan, index: 4, value: vector(Array(sources.reversed())))))
        }
        let scopes = [uint(0, bytes: 4)] + [UInt64(255), 256, UInt64.max].map {
            uint(1, bytes: 4) + record([uint($0)])
        }
        try assertOrdered(scopes.map { claim(asset[0], $0) })
        // Rust compares the u16 threshold numerically; its little-endian frame would reverse 255/256.
        let key = try fields(Data(asset[0].dropFirst(4)))[0]
        func multisig(_ threshold: UInt64) -> Data {
            uint(1, bytes: 4) + record([record([
                Data([1]), uint(threshold, bytes: 2),
                vector([record([key, uint(256, bytes: 2)])]),
            ])])
        }
        try assertOrdered([asset[0], multisig(255), multisig(256)].map { claim($0, uint(0, bytes: 4)) })
        let laterDefinition = record(Array(repeating: Data([255]), count: 16))
        let later = record([
            replacing(asset, index: 1, value: laterDefinition),
            replacing(destination, index: 1, value: laterDefinition), source[2], source[3],
        ])
        try assertOrdered([record(source), later])
    }

    func testRewardSourcesBindAssetsAndRetainZeroPayoutDust() throws {
        let plan = try fields(fixtureRows()["reward_claim_plan"]!)
        let source = try fields(vectorFields(plan[4])[0])
        let destination = try fields(source[1])
        let invalid: [Data] = [
            record(Array(source.dropLast())), record(source + [Data([0])]),
            replacing(source, index: 2, value: option(quantity([], scale: 0))),
            replacing(source, index: 1, value: replacing(destination, index: 1,
                value: record(Array(repeating: Data([9]), count: 16)))),
            replacing(source, index: 1, value: replacing(destination, index: 2,
                value: uint(1, bytes: 4) + record([uint(7)]))),
        ]
        for bytes in invalid {
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardClaimSource(noritoPayload: bytes))
        }
        let dust = try ValidatorStakingNoritoV1.RewardClaimSource(
            noritoPayload: replacing(source, index: 3, value: quantity([], scale: 0)))
        XCTAssertTrue(dust.payout.mantissaLittleEndian.isEmpty)
        XCTAssertEqual(dust.expectedAccrued?.mantissaLittleEndian, Data([5]))
        XCTAssertNil(try ValidatorStakingNoritoV1.RewardClaimSource(
            noritoPayload: replacing(source, index: 2, value: Data([0]))).expectedAccrued)
    }

    func testFeeRewardClaimRejectsChangedCustodyAndZeroAmount() throws {
        let bytes = try fixtureRows()["fee_reward_claim_plan"]!
        let decoded = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: bytes)
        let fee = try XCTUnwrap(decoded.feeClaim)
        let claim = try fields(fee.noritoPayload)
        let source = try fields(claim[3])
        let destination = try fields(claim[4])
        let invalid: [Data] = [
            record(Array(claim.dropLast())), record(claim + [Data([0])]),
            replacing(claim, index: 0, value: Data(repeating: 0, count: 32)),
            replacing(claim, index: 0, value: Data(repeating: 0x77, count: 31)),
            replacing(claim, index: 0, value: Data(repeating: 0x77, count: 33)),
            replacing(claim, index: 0, value: record(Array(repeating: Data([0x77]), count: 32))),
            replacing(claim, index: 5, value: quantity([], scale: 0)),
            replacing(claim, index: 3, value: replacing(source, index: 2,
                value: uint(1, bytes: 4) + record([uint(7)]))),
            replacing(claim, index: 4, value: replacing(destination, index: 1,
                value: record(Array(repeating: Data([9]), count: 16)))),
            replacing(claim, index: 4, value: replacing(destination, index: 2,
                value: uint(1, bytes: 4) + record([uint(7)]))),
        ]
        for payload in invalid {
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.FeeRewardClaim(noritoPayload: payload))
            XCTAssertThrowsError(try ValidatorStakingNoritoV1.RewardClaimPlan(
                noritoPayload: replacing(fields(bytes), index: 5, value: option(payload))))
        }
        var high = claim
        high[2] = uint(UInt64.max)
        high[6] = uint(UInt64.max)
        let max = try ValidatorStakingNoritoV1.FeeRewardClaim(noritoPayload: record(high))
        XCTAssertEqual(max.beneficiaryRevision, UInt64.max)
        XCTAssertEqual(max.expectedClaimSequence, UInt64.max)
    }

    func testFeeRewardClaimRetainsExactSelfCustodyPayment() throws {
        let plan = try fields(fixtureRows()["fee_reward_claim_plan"]!)
        let original = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: record(plan))
        let claim = try fields(XCTUnwrap(original.feeClaim).noritoPayload)
        let selfCustody = replacing(claim, index: 3, value: claim[4])
        let payload = replacing(plan, index: 5, value: option(selfCustody))
        let decoded = try ValidatorStakingNoritoV1.RewardClaimPlan(noritoPayload: payload)
        let fee = try XCTUnwrap(decoded.feeClaim)
        XCTAssertEqual(fee.sourceAsset.noritoPayload, fee.destinationAsset.noritoPayload)
        XCTAssertEqual(fee.amount.mantissaLittleEndian, Data([7]))
        XCTAssertEqual(fee.noritoPayload, selfCustody)
        XCTAssertEqual(decoded.noritoPayload, payload)
    }

    private func option(_ value: Data) -> Data { Data([1]) + record([value]) }

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
