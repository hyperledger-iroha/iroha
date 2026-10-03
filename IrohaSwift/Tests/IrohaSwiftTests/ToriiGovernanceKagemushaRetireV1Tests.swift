import Foundation
import XCTest
@testable import IrohaSwift

/// Exercises the typed retirement projection against the genuine native producer fixture.
final class ToriiGovernanceKagemushaRetireV1Tests: XCTestCase {
    private func fixture() throws -> Data {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { root.deleteLastPathComponent() }
        return try Data(contentsOf: root.appendingPathComponent(
            "fixtures/governance/kagemusha_verifier_release_retire_v1.json"
        ))
    }

    private func mutated(_ mutation: (inout [String: Any]) -> Void) throws -> Data {
        var proposal = try XCTUnwrap(JSONSerialization.jsonObject(with: fixture()) as? [String: Any])
        var payload = try XCTUnwrap(proposal["payload"] as? [String: Any])
        mutation(&payload)
        proposal["payload"] = payload
        return try JSONSerialization.data(withJSONObject: proposal, options: [.sortedKeys])
    }

    func testRetirementDecodesExactNativeFixture() throws {
        let proposal = try ToriiParliamentProposalV1(validating: fixture())
        guard case .kagemushaVerifierReleaseRetire(let retirement) = proposal.kind else {
            return XCTFail("expected KagemushaVerifierReleaseRetire")
        }
        XCTAssertNotNil(retirement.expectedPredecessor.authorityPolicy)
        XCTAssertNotNil(retirement.expectedPredecessor.activeReleaseId)
        XCTAssertEqual(retirement.expectedPredecessor.releases.count, 3)
        for status in [ToriiGovernanceKagemushaGovernedVerifierReleaseStatusV1.active,
                       .standby, .verificationOnly] {
            XCTAssertEqual(retirement.expectedPredecessor.releases.filter { $0.status == status }.count, 1)
        }
        let native = try XCTUnwrap(JSONSerialization.jsonObject(with: fixture()) as? [String: Any])
        let payload = try XCTUnwrap(native["payload"] as? [String: Any])
        let rawPredecessor = try XCTUnwrap(payload["expected_predecessor"] as? [String: Any])
        let originalPredecessor = try JSONDecoder().decode(
            ToriiGovernanceKagemushaGovernedVerifierRegistryV1.self,
            from: JSONSerialization.data(withJSONObject: rawPredecessor)
        )
        XCTAssertEqual(retirement.expectedPredecessor, originalPredecessor)
        let target = try XCTUnwrap(retirement.expectedPredecessor.releases.first {
            $0.releaseId == retirement.standbyReleaseId
        })
        XCTAssertEqual(target.status, .standby)
    }

    func testRetirementRejectsMissingMalformedExtendedAndReplayedTargets() throws {
        let mutations: [(String, (inout [String: Any]) -> Void)] = [
            ("unknown field", { $0["retired_alias"] = true }),
            ("missing target", { $0.removeValue(forKey: "standby_release_id") }),
            ("short target", { $0["standby_release_id"] = [UInt8](repeating: 1, count: 31) }),
            ("hex target", { $0["standby_release_id"] = String(repeating: "11", count: 32) }),
            ("zero target", { $0["standby_release_id"] = [UInt8](repeating: 0, count: 32) }),
            ("absent target", { $0["standby_release_id"] = [UInt8](repeating: 99, count: 32) }),
            ("ungoverned predecessor", { payload in
                var registry = payload["expected_predecessor"] as! [String: Any]
                registry["authority_policy"] = NSNull()
                payload["expected_predecessor"] = registry
            }),
            ("replay after removal", { payload in
                var registry = payload["expected_predecessor"] as! [String: Any]
                let target = payload["standby_release_id"] as! [Int]
                let rows = registry["releases"] as! [[String: Any]]
                registry["releases"] = rows.filter { ($0["release_id"] as! [Int]) != target }
                payload["expected_predecessor"] = registry
            }),
            ("unknown nested field", { payload in
                var registry = payload["expected_predecessor"] as! [String: Any]
                var rows = registry["releases"] as! [[String: Any]]
                rows[0]["legacy_alias"] = true
                registry["releases"] = rows
                payload["expected_predecessor"] = registry
            }),
        ]
        for (name, mutation) in mutations {
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: mutated(mutation)), name)
        }
    }

    private func mixed(_ mutation: (inout [String: Any]) -> Void = { _ in }) throws -> Data {
        // Shape-only mutations do not create authenticated registry or release authority.
        try mutated { payload in
            var registry = payload["expected_predecessor"] as! [String: Any]
            let original = (registry["releases"] as! [[String: Any]])[0]
            registry["releases"] = [(1, 1), (2, 3), (3, 2)].map { id, status in
                var row = original
                row["release_id"] = [UInt8](repeating: UInt8(id), count: 32)
                row["status"] = status
                return row
            }
            registry["active_release_id"] = String(repeating: "01", count: 32)
            payload["expected_predecessor"] = registry
            payload["standby_release_id"] = [UInt8](repeating: 3, count: 32)
            mutation(&payload)
        }
    }

    func testRetirementPreservesGenuineCompleteActiveAndHistoricalPredecessor() throws {
        let nativeBytes = try fixture()
        let proposal = try ToriiParliamentProposalV1(validating: nativeBytes)
        guard case .kagemushaVerifierReleaseRetire(let retirement) = proposal.kind else {
            return XCTFail("expected KagemushaVerifierReleaseRetire")
        }
        let native = try XCTUnwrap(JSONSerialization.jsonObject(with: nativeBytes) as? [String: Any])
        let payload = try XCTUnwrap(native["payload"] as? [String: Any])
        let rawPredecessor = try XCTUnwrap(payload["expected_predecessor"] as? [String: Any])
        let original = try JSONDecoder().decode(
            ToriiGovernanceKagemushaGovernedVerifierRegistryV1.self,
            from: JSONSerialization.data(withJSONObject: rawPredecessor)
        )
        XCTAssertEqual(retirement.expectedPredecessor, original)
        XCTAssertEqual(original.releases.count, 3)
        let active = try XCTUnwrap(original.releases.first { $0.status == .active })
        let historical = try XCTUnwrap(original.releases.first { $0.status == .verificationOnly })
        let standby = try XCTUnwrap(original.releases.first { $0.status == .standby })
        XCTAssertEqual(retirement.expectedPredecessor.activeReleaseId, active.releaseId.bytes)
        XCTAssertEqual(retirement.standbyReleaseId, standby.releaseId)
        for row in [active, historical] {
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: mutated {
                $0["standby_release_id"] = Array(row.releaseId.bytes)
            }), "genuine nonstandby row must not be removable")
        }
    }

    func testRetirementValidatesParserOnlyMixedRowShapes() throws {
        let proposal = try ToriiParliamentProposalV1(validating: mixed())
        guard case .kagemushaVerifierReleaseRetire(let retirement) = proposal.kind else {
            return XCTFail("expected KagemushaVerifierReleaseRetire")
        }
        XCTAssertEqual(retirement.expectedPredecessor.activeReleaseId, Data(repeating: 1, count: 32))
        XCTAssertEqual(retirement.expectedPredecessor.releases.map(\.status), [.active, .verificationOnly, .standby])
        XCTAssertEqual(retirement.expectedPredecessor.releases.map(\.releaseId.bytes), [
            Data(repeating: 1, count: 32), Data(repeating: 2, count: 32), Data(repeating: 3, count: 32)
        ])
        XCTAssertEqual(retirement.standbyReleaseId.bytes, Data(repeating: 3, count: 32))
        for id in [UInt8(1), 2] {
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: mixed {
                $0["standby_release_id"] = [UInt8](repeating: id, count: 32)
            }))
        }
    }

    func testRetirementRejectsInconsistentDuplicateAndUnorderedRegistryRows() throws {
        for defect in 0..<5 {
            let invalid = try mixed { payload in
                var registry = payload["expected_predecessor"] as! [String: Any]
                var rows = registry["releases"] as! [[String: Any]]
                switch defect {
                case 0: registry["active_release_id"] = String(repeating: "02", count: 32)
                case 1: rows.reverse()
                case 2: rows[1]["release_id"] = rows[0]["release_id"]
                case 3: rows[1]["profile_digest"] = [UInt8](repeating: 0, count: 32)
                default: registry["active_release_id"] = NSNull()
                }
                registry["releases"] = rows
                payload["expected_predecessor"] = registry
            }
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: invalid), "defect \(defect)")
        }
    }
}
