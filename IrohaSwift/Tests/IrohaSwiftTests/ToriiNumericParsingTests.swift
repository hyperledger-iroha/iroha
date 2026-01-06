import XCTest
@testable import IrohaSwift

final class ToriiNumericParsingTests: XCTestCase {
    func testConnectStatusPolicySkipsFractionalLimits() {
        let policy = ToriiConnectStatusPolicySnapshot(raw: [
            "ws_max_sessions": .number(5.5),
            "session_ttl_ms": .number(60.25),
            "relay_enabled": .bool(true)
        ])

        XCTAssertNil(policy.wsMaxSessions)
        XCTAssertNil(policy.sessionTtlMs)
        XCTAssertEqual(policy.relayEnabled, true)
    }

    func testAdmissionManifestSkipsFractionalVersion() throws {
        let manifest = try ToriiConnectAdmissionManifest(raw: [
            "version": .number(1.5),
            "entries": .array([])
        ])
        XCTAssertNil(manifest.version)
    }

    func testConnectPerIpSessionsRejectsFractionalCounts() {
        let raw: [String: ToriiJSONValue] = [
            "ip": .string("1.1.1.1"),
            "sessions": .number(1.5)
        ]
        XCTAssertThrowsError(try ToriiConnectPerIpSessions(raw: raw))
    }

    func testStatusPayloadRejectsFractionalCounts() {
        let raw: [String: ToriiJSONValue] = [
            "peers": .number(1),
            "queue_size": .number(1.5),
            "commit_time_ms": .number(10),
            "txs_approved": .number(0),
            "txs_rejected": .number(0),
            "view_changes": .number(0),
            "lane_governance_sealed_total": .number(0),
            "lane_governance_sealed_aliases": .array([])
        ]
        XCTAssertThrowsError(try ToriiStatusPayload(raw: raw))
    }

    func testDaManifestBundleRejectsFractionalLaneId() {
        let manifestB64 = Data("m".utf8).base64EncodedString()
        let raw: [String: ToriiJSONValue] = [
            "storage_ticket": .string(String(repeating: "aa", count: 32)),
            "client_blob_id": .string(String(repeating: "bb", count: 32)),
            "blob_hash": .string(String(repeating: "cc", count: 32)),
            "manifest_hash": .string(String(repeating: "dd", count: 32)),
            "chunk_root": .string(String(repeating: "ee", count: 32)),
            "lane_id": .number(1.5),
            "epoch": .number(1),
            "manifest_len": .number(1),
            "manifest_norito": .string(manifestB64),
            "chunk_plan": .array([])
        ]
        XCTAssertThrowsError(try ToriiDaManifestBundle(raw: raw))
    }

    func testDaSamplingPlanRejectsFractionalWindow() {
        let plan: ToriiJSONValue = .object([
            "assignment_hash": .string(String(repeating: "bb", count: 32)),
            "sample_window": .number(2.5),
            "samples": .array([])
        ])
        XCTAssertThrowsError(try ToriiDaSamplingPlan.parse(plan))
    }
}
