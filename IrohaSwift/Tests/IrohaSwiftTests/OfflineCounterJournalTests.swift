import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineCounterJournalTests: XCTestCase {
    private let encodedControllerId = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"

    func testSummaryHashMatchesAndPersists() throws {
        let appleCounters: [String: UInt64] = ["key-1": 2, "key-2": 5]
        let androidCounters: [String: UInt64] = ["series-1": 7]
        let summaryHash = try OfflineCounterJournal.computeSummaryHashHex(
            appleKeyCounters: appleCounters,
            androidSeriesCounters: androidCounters
        ).lowercased()

        let payload: [String: Any] = [
            "items": [
                [
                    "certificate_id_hex": "DEADBEEF",
                    "controller_id": encodedControllerId,
                    "controller_display": "Alice",
                    "summary_hash_hex": summaryHash,
                    "apple_key_counters": appleCounters,
                    "android_series_counters": androidCounters
                ]
            ],
            "total": 1
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [])
        let list = try JSONDecoder().decode(ToriiOfflineSummaryList.self, from: data)

        let journal = try OfflineCounterJournal(storageURL: temporaryURL())
        try journal.upsert(summaries: list.items, recordedAtMs: 1_000)
        let checkpoint = try XCTUnwrap(journal.checkpoint(for: "deadbeef"))
        XCTAssertEqual(checkpoint.summaryHashHex, summaryHash)
        XCTAssertEqual(checkpoint.appleKeyCounters["key-1"], 2)
    }

    func testCounterJumpThrows() throws {
        let journal = try OfflineCounterJournal(storageURL: temporaryURL())
        _ = try journal.updateCounter(
            certificateIdHex: "deadbeef",
            controllerId: encodedControllerId,
            controllerDisplay: nil,
            platform: .appleKey,
            scope: "key-1",
            counter: 1,
            recordedAtMs: 1
        )

        XCTAssertThrowsError(
            try journal.updateCounter(
                certificateIdHex: "deadbeef",
                controllerId: encodedControllerId,
                controllerDisplay: nil,
                platform: .appleKey,
                scope: "key-1",
                counter: 3,
                recordedAtMs: 2
            )
        ) { error in
            guard case OfflineCounterError.counterJump = error else {
                return XCTFail("expected counter jump error")
            }
        }
    }

    func testCounterOverflowThrows() throws {
        let journal = try OfflineCounterJournal(storageURL: temporaryURL())
        _ = try journal.updateCounter(
            certificateIdHex: "deadbeef",
            controllerId: encodedControllerId,
            controllerDisplay: nil,
            platform: .appleKey,
            scope: "key-1",
            counter: UInt64.max,
            recordedAtMs: 1
        )

        XCTAssertThrowsError(
            try journal.updateCounter(
                certificateIdHex: "deadbeef",
                controllerId: encodedControllerId,
                controllerDisplay: nil,
                platform: .appleKey,
                scope: "key-1",
                counter: 0,
                recordedAtMs: 2
            )
        ) { error in
            guard case OfflineCounterError.counterOverflow = error else {
                return XCTFail("expected counter overflow error")
            }
        }
    }

    func testSummaryHashMismatchThrows() throws {
        let payload: [String: Any] = [
            "items": [
                [
                    "certificate_id_hex": "DEADBEEF",
                    "controller_id": encodedControllerId,
                    "controller_display": "Alice",
                    "summary_hash_hex": String(repeating: "00", count: 32),
                    "apple_key_counters": ["key-1": 1],
                    "android_series_counters": [:]
                ]
            ],
            "total": 1
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [])
        let list = try JSONDecoder().decode(ToriiOfflineSummaryList.self, from: data)
        let journal = try OfflineCounterJournal(storageURL: temporaryURL())

        XCTAssertThrowsError(try journal.upsert(summaries: list.items, recordedAtMs: 1_000)) { error in
            guard case OfflineCounterError.summaryHashMismatch = error else {
                return XCTFail("expected summary hash mismatch")
            }
        }
    }

    private func temporaryURL() -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".json")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }
}
