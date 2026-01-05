import XCTest
@testable import IrohaSwift

final class OfflineVerdictJournalTests: XCTestCase {

    func testAllowanceDecodingIncludesMetadata() throws {
        let allowances = try decodeAllowances(
            expiresAtMs: 2_000,
            policyExpiresAtMs: 3_000,
            refreshAtMs: 1_500,
            remainingAmount: "42"
        )
        XCTAssertEqual(allowances.total, 1)
        guard let item = allowances.items.first else {
            return XCTFail("missing decoded item")
        }
        XCTAssertEqual(item.certificateIdHex, "deadbeef")
        XCTAssertEqual(item.expiresAtMs, 2_000)
        XCTAssertEqual(item.policyExpiresAtMs, 3_000)
        XCTAssertEqual(item.refreshAtMs, 1_500)
        XCTAssertEqual(item.verdictIdHex, "c0ffee")
        XCTAssertEqual(item.attestationNonceHex, "abcd")
        XCTAssertEqual(item.remainingAmount, "42")
    }

    func testVerdictJournalUpsertsRawAllowances() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let certificateIdHex = try certificate.certificateIdHex().lowercased()
        let verdictHex = String(repeating: "b", count: 64)
        let verdictLiteral = try SocialKeyedHash(pepperId: "pepper", digest: verdictHex).digest
        let record = try makeRawAllowanceRecord(certificate: certificate,
                                                verdictLiteral: verdictLiteral,
                                                remainingAmount: "12")
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: tempURL)
        let inserted = try journal.upsert(rawAllowances: [record], recordedAtMs: 222)
        XCTAssertEqual(inserted.count, 1)
        let metadata = try XCTUnwrap(journal.metadata(for: certificateIdHex))
        XCTAssertEqual(metadata.certificateIdHex, certificateIdHex)
        XCTAssertEqual(metadata.verdictIdHex, verdictHex)
        XCTAssertEqual(metadata.attestationNonceHex, verdictHex)
        XCTAssertEqual(metadata.remainingAmount, "12")
        XCTAssertEqual(metadata.recordedAtMs, 222)
    }

    func testVerdictJournalCapturesProvisionedMetadata() throws {
        let metadata: [String: Any] = [
            "android.integrity.policy": "provisioned",
            "android.provisioned.inspector_public_key": "ed0120ABCDEF",
            "android.provisioned.manifest_schema": "offline_provisioning_v1",
            "android.provisioned.manifest_version": 3,
            "android.provisioned.max_manifest_age_ms": 604_800_000,
            "android.provisioned.manifest_digest": "aa55"
        ]
        let allowances = try decodeAllowances(
            expiresAtMs: 4_000,
            policyExpiresAtMs: 4_000,
            refreshAtMs: 3_000,
            remainingAmount: "5",
            metadata: metadata
        )
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: tempURL)
        try journal.upsert(allowances: allowances.items, recordedAtMs: 1_000)
        let stored = try XCTUnwrap(journal.metadata(for: "deadbeef"))
        XCTAssertEqual(stored.integrityPolicy, "provisioned")
        let provisioned = try XCTUnwrap(stored.provisionedMetadata)
        XCTAssertEqual(provisioned.inspectorPublicKeyHex, "ed0120ABCDEF")
        XCTAssertEqual(provisioned.manifestSchema, "offline_provisioning_v1")
        XCTAssertEqual(provisioned.manifestVersion, 3)
        XCTAssertEqual(provisioned.maxManifestAgeMs, 604_800_000)
        XCTAssertEqual(provisioned.manifestDigestHex, "aa55")
    }

    func testVerdictJournalCapturesPlayIntegrityMetadata() throws {
        let metadata: [String: Any] = [
            "android.integrity.policy": "play_integrity",
            "android.play_integrity.cloud_project_number": 424242,
            "android.play_integrity.environment": "production",
            "android.play_integrity.package_names": ["tech.iroha.wallet"],
            "android.play_integrity.signing_digests_sha256": [
                "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF"
            ],
            "android.play_integrity.allowed_app_verdicts": ["play_recognized", "licensed"],
            "android.play_integrity.allowed_device_verdicts": ["strong", "device"],
            "android.play_integrity.max_token_age_ms": 30_000
        ]
        let allowances = try decodeAllowances(
            expiresAtMs: 4_000,
            policyExpiresAtMs: 4_000,
            refreshAtMs: 3_000,
            remainingAmount: "5",
            metadata: metadata
        )
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: tempURL)
        try journal.upsert(allowances: allowances.items, recordedAtMs: 1_000)
        let stored = try XCTUnwrap(journal.metadata(for: "deadbeef"))
        XCTAssertEqual(stored.integrityPolicy, "play_integrity")
        let playIntegrity = try XCTUnwrap(stored.playIntegrityMetadata)
        XCTAssertEqual(playIntegrity.cloudProjectNumber, 424242)
        XCTAssertEqual(playIntegrity.environment, "production")
        XCTAssertEqual(playIntegrity.packageNames, ["tech.iroha.wallet"])
        XCTAssertEqual(playIntegrity.allowedAppVerdicts, ["play_recognized", "licensed"])
        XCTAssertEqual(playIntegrity.allowedDeviceVerdicts, ["strong", "device"])
        XCTAssertEqual(playIntegrity.maxTokenAgeMs, 30_000)
    }

    func testVerdictJournalCapturesHmsSafetyDetectMetadata() throws {
        let metadata: [String: Any] = [
            "android.integrity.policy": "hms_safety_detect",
            "android.hms_safety_detect.app_id": "103000042",
            "android.hms_safety_detect.package_names": ["tech.iroha.wallet"],
            "android.hms_safety_detect.signing_digests_sha256": [
                "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789"
            ],
            "android.hms_safety_detect.required_evaluations": ["strong_integrity"],
            "android.hms_safety_detect.max_token_age_ms": 3_600_000
        ]
        let allowances = try decodeAllowances(
            expiresAtMs: 4_000,
            policyExpiresAtMs: 4_000,
            refreshAtMs: 3_000,
            remainingAmount: "5",
            metadata: metadata
        )
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: tempURL)
        try journal.upsert(allowances: allowances.items, recordedAtMs: 1_000)
        let stored = try XCTUnwrap(journal.metadata(for: "deadbeef"))
        XCTAssertEqual(stored.integrityPolicy, "hms_safety_detect")
        let hms = try XCTUnwrap(stored.hmsSafetyDetectMetadata)
        XCTAssertEqual(hms.appId, "103000042")
        XCTAssertEqual(hms.packageNames, ["tech.iroha.wallet"])
        XCTAssertEqual(hms.requiredEvaluations, ["strong_integrity"])
        XCTAssertEqual(hms.maxTokenAgeMs, 3_600_000)
    }

    func testVerdictJournalWarnsBeforeDeadlineAndPersists() throws {
        let allowances = try decodeAllowances(
            expiresAtMs: 4_000,
            policyExpiresAtMs: 4_000,
            refreshAtMs: 3_000,
            remainingAmount: "5"
        )
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: tempURL)
        try journal.upsert(allowances: allowances.items, recordedAtMs: 1_000)
        guard let refreshAt = allowances.items.first?.refreshAtMs else {
            return XCTFail("missing refresh timestamp")
        }
        let warnings = journal.warnings(nowMs: refreshAt - 500, warningThresholdMs: 1_000)
        XCTAssertEqual(warnings.count, 1)
        XCTAssertEqual(warnings.first?.state, .warning)
        let expired = journal.warning(for: "DEADBEEF",
                                      nowMs: refreshAt + 1,
                                      warningThresholdMs: 1_000)
        XCTAssertEqual(expired?.state, .expired)

        // Reload from disk to ensure persistence.
        let reloaded = try OfflineVerdictJournal(storageURL: tempURL)
        XCTAssertNotNil(
            reloaded.warning(for: "deadbeef", nowMs: refreshAt + 1, warningThresholdMs: 1_000)
        )
    }

    func testOfflineWalletExposesWarnings() throws {
        let allowances = try decodeAllowances(
            expiresAtMs: 10_000,
            policyExpiresAtMs: 10_000,
            refreshAtMs: 8_000,
            remainingAmount: "7"
        )
        let torii = ToriiClient(baseURL: URL(string: "https://example.invalid")!)
        let auditURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journalURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: journalURL)
        let wallet = try OfflineWallet(
            toriiClient: torii,
            auditLoggingEnabled: false,
            auditStorageURL: auditURL,
            auditLogger: nil,
            circulationMode: .ledgerReconcilable,
            onCirculationModeChange: nil,
            verdictJournal: journal
        )
        try wallet.recordVerdictMetadata(from: allowances, recordedAt: 1_000)
        let refreshAt = allowances.items.first?.refreshAtMs ?? 0
        let now = Date(timeIntervalSince1970: TimeInterval(refreshAt - 500) / 1_000)
        let warnings = wallet.verdictWarnings(warningThresholdMs: 1_000, now: now)
        XCTAssertEqual(warnings.count, 1)
        XCTAssertEqual(warnings.first?.deadlineKind, .refresh)
    }

    func testEnsureFreshVerdictRejectsExpiredRefresh() throws {
        let allowances = try decodeAllowances(
            expiresAtMs: 5_000,
            policyExpiresAtMs: 5_000,
            refreshAtMs: 2_000,
            remainingAmount: "3"
        )
        let torii = ToriiClient(baseURL: URL(string: "https://example.invalid")!)
        let auditURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journalURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: journalURL)
        let wallet = try OfflineWallet(
            toriiClient: torii,
            auditLoggingEnabled: false,
            auditStorageURL: auditURL,
            auditLogger: nil,
            circulationMode: .ledgerReconcilable,
            onCirculationModeChange: nil,
            verdictJournal: journal
        )
        try wallet.recordVerdictMetadata(from: allowances, recordedAt: 1_000)
        let refreshAt = allowances.items.first?.refreshAtMs ?? 0
        let before = Date(timeIntervalSince1970: TimeInterval(refreshAt - 500) / 1_000)
        XCTAssertNoThrow(
            try wallet.ensureFreshVerdict(for: "DEADBEEF",
                                          attestationNonceHex: "abcd",
                                          now: before)
        )
        let after = Date(timeIntervalSince1970: TimeInterval(refreshAt + 1) / 1_000)
        XCTAssertThrowsError(
            try wallet.ensureFreshVerdict(for: "deadbeef",
                                          attestationNonceHex: "abcd",
                                          now: after)
        ) { error in
            guard case let OfflineVerdictError.expired(_, kind, deadlineMs) = error else {
                return XCTFail("expected expired error")
            }
            XCTAssertEqual(kind, .refresh)
            XCTAssertEqual(deadlineMs, refreshAt)
        }
    }

    func testEnsureFreshVerdictDetectsNonceMismatch() throws {
        let allowances = try decodeAllowances(
            expiresAtMs: 9_000,
            policyExpiresAtMs: 9_000,
            refreshAtMs: 8_000,
            remainingAmount: "2"
        )
        let torii = ToriiClient(baseURL: URL(string: "https://example.invalid")!)
        let auditURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journalURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let journal = try OfflineVerdictJournal(storageURL: journalURL)
        let wallet = try OfflineWallet(
            toriiClient: torii,
            auditLoggingEnabled: false,
            auditStorageURL: auditURL,
            auditLogger: nil,
            circulationMode: .ledgerReconcilable,
            onCirculationModeChange: nil,
            verdictJournal: journal
        )
        try wallet.recordVerdictMetadata(from: allowances, recordedAt: 1_000)
        XCTAssertThrowsError(
            try wallet.ensureFreshVerdict(for: "deadbeef",
                                          attestationNonceHex: "ffff")
        ) { error in
            guard case let OfflineVerdictError.nonceMismatch(_, expected, provided) = error else {
                return XCTFail("expected nonce mismatch")
            }
            XCTAssertEqual(expected, "abcd")
            XCTAssertEqual(provided, "ffff")
        }
    }

    // MARK: - Helpers

    private func decodeAllowances(expiresAtMs: UInt64,
                                  policyExpiresAtMs: UInt64,
                                  refreshAtMs: UInt64,
                                  remainingAmount: String,
                                  metadata: [String: Any]? = nil) throws -> ToriiOfflineAllowanceList {
        var record: [String: Any] = [
            "remaining_amount": remainingAmount
        ]
        if let metadata {
            record["certificate"] = [
                "metadata": metadata
            ]
        }

        let allowance: [String: Any] = [
            "certificate_id_hex": "deadbeef",
            "controller_id": "alice@sora",
            "controller_display": "Alice",
            "asset_id": "xor#wonderland",
            "registered_at_ms": NSNumber(value: 123),
            "expires_at_ms": NSNumber(value: expiresAtMs),
            "policy_expires_at_ms": NSNumber(value: policyExpiresAtMs),
            "refresh_at_ms": NSNumber(value: refreshAtMs),
            "verdict_id_hex": "c0ffee",
            "attestation_nonce_hex": "abcd",
            "remaining_amount": remainingAmount,
            "record": record
        ]
        let root: [String: Any] = [
            "items": [allowance],
            "total": 1
        ]
        let data = try JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
        return try JSONDecoder().decode(ToriiOfflineAllowanceList.self, from: data)
    }

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }

    private func makeRawAllowanceRecord(certificate: OfflineWalletCertificate,
                                        verdictLiteral: String,
                                        remainingAmount: String) throws -> ToriiJSONValue {
        let certificateData = try JSONEncoder().encode(certificate)
        guard let certificateObject = try JSONSerialization.jsonObject(with: certificateData) as? [String: Any] else {
            throw XCTSkip("certificate fixture failed to encode as JSON object")
        }
        let record: [String: Any] = [
            "certificate": certificateObject,
            "current_commitment": [NSNumber(value: 1), NSNumber(value: 2), NSNumber(value: 3)],
            "registered_at_ms": NSNumber(value: 123),
            "remaining_amount": remainingAmount,
            "counter_state": [
                "apple_key_counters": [:],
                "android_series_counters": [:]
            ],
            "verdict_id_hex": verdictLiteral,
            "attestation_nonce_hex": verdictLiteral,
            "refresh_at_ms": NSNumber(value: 777)
        ]
        let data = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
        return try JSONDecoder().decode(ToriiJSONValue.self, from: data)
    }
}
