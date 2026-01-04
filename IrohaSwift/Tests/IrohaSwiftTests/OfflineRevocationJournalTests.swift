import XCTest
@testable import IrohaSwift

final class OfflineRevocationJournalTests: XCTestCase {
    func testRevocationJournalStoresListEntries() throws {
        let verdictHex = String(repeating: "a", count: 64)
        let list = try makeRevocationList(verdictHex: verdictHex)
        let journalURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineRevocationJournal(storageURL: journalURL)
        let inserted = try journal.upsert(revocations: list.items, recordedAtMs: 1_234)
        XCTAssertEqual(inserted.count, 1)
        XCTAssertTrue(journal.isRevoked(verdictIdHex: verdictHex.uppercased()))
        let entry = try XCTUnwrap(journal.revocation(for: verdictHex))
        XCTAssertEqual(entry.issuerId, "alice@wonderland")
        XCTAssertEqual(entry.issuerDisplay, "Alice")
        XCTAssertEqual(entry.revokedAtMs, 777)
        XCTAssertEqual(entry.reason, "compromised")
        XCTAssertEqual(entry.note, "lost device")
        XCTAssertEqual(entry.recordedAtMs, 1_234)
        if case let .object(metadata)? = entry.metadata {
            XCTAssertEqual(metadata["case_id"]?.normalizedString, "inc-1")
        } else {
            XCTFail("expected metadata object")
        }

        let reloaded = try OfflineRevocationJournal(storageURL: journalURL)
        XCTAssertTrue(reloaded.isRevoked(verdictIdHex: verdictHex))
    }

    func testRevocationJournalParsesRawState() throws {
        let verdictHex = String(repeating: "b", count: 64)
        let hashLiteral = try SocialKeyedHash(pepperId: "pepper", digest: verdictHex).digest
        let record: [String: Any] = [
            "verdict_id_hex": hashLiteral,
            "issuer_id": "bob@wonderland",
            "revoked_at_ms": NSNumber(value: 999),
            "reason": "compromised",
            "metadata": ["ticket": "T-1"]
        ]
        let data = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
        let value = try JSONDecoder().decode(ToriiJSONValue.self, from: data)
        let journalURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineRevocationJournal(storageURL: journalURL)
        let inserted = try journal.upsert(rawRevocations: [value], recordedAtMs: 500)
        XCTAssertEqual(inserted.count, 1)
        let entry = try XCTUnwrap(journal.revocation(for: verdictHex.uppercased()))
        XCTAssertEqual(entry.issuerId, "bob@wonderland")
        XCTAssertEqual(entry.revokedAtMs, 999)
        XCTAssertEqual(entry.recordedAtMs, 500)
    }

    // MARK: - Helpers

    private func makeRevocationList(verdictHex: String) throws -> ToriiOfflineRevocationList {
        let hashLiteral = try SocialKeyedHash(pepperId: "pepper", digest: verdictHex).digest
        let record: [String: Any] = [
            "verdict_id_hex": hashLiteral,
            "issuer_id": "alice@wonderland",
            "revoked_at_ms": NSNumber(value: 777),
            "reason": "compromised",
            "note": "lost device",
            "metadata": ["case_id": "inc-1"]
        ]
        let item: [String: Any] = [
            "verdict_id_hex": verdictHex,
            "issuer_id": "alice@wonderland",
            "issuer_display": "Alice",
            "revoked_at_ms": NSNumber(value: 777),
            "reason": "compromised",
            "note": "lost device",
            "metadata": ["case_id": "inc-1"],
            "record": record
        ]
        let root: [String: Any] = [
            "items": [item],
            "total": NSNumber(value: 1)
        ]
        let data = try JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
        return try JSONDecoder().decode(ToriiOfflineRevocationList.self, from: data)
    }
}
