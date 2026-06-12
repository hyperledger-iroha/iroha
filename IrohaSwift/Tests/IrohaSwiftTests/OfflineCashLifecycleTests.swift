import XCTest
@testable import IrohaSwift

final class OfflineCashLifecycleTests: XCTestCase {
    func testLoadSyncsPendingAuditReceiptsBeforeIssuingMoreCash() async throws {
        let events = RecordingLifecycleEvents()
        let synchronizer = RecordingAuditReceiptSynchronizer(hasPending: true, events: events)
        let controller = OfflineCashLifecycleController(
            auditReceiptSynchronizer: synchronizer,
            listNotes: { [] },
            load: { _, _ in
                await events.append("load")
                throw TestLifecycleError.stop
            },
            prepareReceive: { _, _ in throw TestLifecycleError.stop },
            createPayment: { _ in throw TestLifecycleError.stop },
            acceptPayment: { _ in throw TestLifecycleError.stop },
            publishAudit: { _ in },
            redeem: { _, _ in throw TestLifecycleError.stop },
            syncNotes: { [] }
        )

        do {
            _ = try await controller.load(assetDefinitionId: "pkr#sbp", amount: "10")
            XCTFail("expected test sentinel")
        } catch TestLifecycleError.stop {
            let snapshot = await events.snapshot()
            XCTAssertEqual(snapshot, ["hasPending", "sync", "load"])
        }
    }

    func testLoadSkipsAuditSyncWhenNoPendingReceiptsExist() async throws {
        let events = RecordingLifecycleEvents()
        let synchronizer = RecordingAuditReceiptSynchronizer(hasPending: false, events: events)
        let controller = OfflineCashLifecycleController(
            auditReceiptSynchronizer: synchronizer,
            listNotes: { [] },
            load: { _, _ in
                await events.append("load")
                throw TestLifecycleError.stop
            },
            prepareReceive: { _, _ in throw TestLifecycleError.stop },
            createPayment: { _ in throw TestLifecycleError.stop },
            acceptPayment: { _ in throw TestLifecycleError.stop },
            publishAudit: { _ in },
            redeem: { _, _ in throw TestLifecycleError.stop },
            syncNotes: { [] }
        )

        do {
            _ = try await controller.load(assetDefinitionId: "pkr#sbp", amount: "10")
            XCTFail("expected test sentinel")
        } catch TestLifecycleError.stop {
            let snapshot = await events.snapshot()
            XCTAssertEqual(snapshot, ["hasPending", "load"])
        }
    }

    func testLoadDoesNotIssueCashWhenAuditReceiptSyncFails() async throws {
        let events = RecordingLifecycleEvents()
        let synchronizer = RecordingAuditReceiptSynchronizer(
            hasPending: true,
            events: events,
            syncError: TestLifecycleError.auditSyncFailed
        )
        let controller = OfflineCashLifecycleController(
            auditReceiptSynchronizer: synchronizer,
            listNotes: { [] },
            load: { _, _ in
                await events.append("load")
                throw TestLifecycleError.stop
            },
            prepareReceive: { _, _ in throw TestLifecycleError.stop },
            createPayment: { _ in throw TestLifecycleError.stop },
            acceptPayment: { _ in throw TestLifecycleError.stop },
            publishAudit: { _ in },
            redeem: { _, _ in throw TestLifecycleError.stop },
            syncNotes: { [] }
        )

        do {
            _ = try await controller.load(assetDefinitionId: "pkr#sbp", amount: "10")
            XCTFail("expected sync failure")
        } catch TestLifecycleError.auditSyncFailed {
            let snapshot = await events.snapshot()
            XCTAssertEqual(snapshot, ["hasPending", "sync"])
        } catch {
            XCTFail("expected auditSyncFailed, got \(error)")
        }
    }

    func testOfflineCashConfigurationSnapshotRequiresCachedIssuerKeyForOfflineExchange() throws {
        let snapshot = OfflineCashConfigurationSnapshot(
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: "issuer-public-key",
            nativeBridgeAbiVersion: 7,
            artifactSetId: "artifact-set",
            circuitId: "kagemusha-recursive-compact-v1",
            createdAtMs: 100,
            expiresAtMs: 1_000
        )

        XCTAssertNoThrow(
            try snapshot.requireUsableForOfflineExchange(nowMs: 999, requiredNativeBridgeAbiVersion: 7)
        )

        let missingKey = OfflineCashConfigurationSnapshot(
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: " ",
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100
        )
        XCTAssertThrowsError(
            try missingKey.requireUsableForOfflineExchange(nowMs: 200, requiredNativeBridgeAbiVersion: 7)
        ) { error in
            XCTAssertEqual(error as? OfflineCashConfigurationSnapshotError, .missingIssuerPublicKey)
        }

        for issuerPublicKeyBase64 in [
            "",
            " issuer-public-key",
            "issuer-public-key ",
            "issuer public key",
            "issuer-public-key\n",
            "issuer-public-key\u{2603}"
        ] {
            XCTAssertThrowsError(
                try OfflineCashConfigurationSnapshot(
                    chainId: "00000042",
                    assetDefinitionId: "pkr#sbp",
                    offlinePaymentsEnabled: true,
                    issuerPublicKeyBase64: issuerPublicKeyBase64,
                    nativeBridgeAbiVersion: 7,
                    createdAtMs: 100
                ).requireUsableForOfflineExchange(nowMs: 200, requiredNativeBridgeAbiVersion: 7)
            ) { error in
                XCTAssertEqual(error as? OfflineCashConfigurationSnapshotError, .missingIssuerPublicKey)
            }
        }

        XCTAssertThrowsError(
            try snapshot.requireUsableForOfflineExchange(nowMs: 1_000, requiredNativeBridgeAbiVersion: 7)
        ) { error in
            XCTAssertEqual(
                error as? OfflineCashConfigurationSnapshotError,
                .expired(expiresAtMs: 1_000, nowMs: 1_000)
            )
        }

        XCTAssertThrowsError(
            try OfflineCashConfigurationSnapshot(
                chainId: "00000042",
                assetDefinitionId: "pkr#sbp",
                offlinePaymentsEnabled: false,
                issuerPublicKeyBase64: "issuer-public-key",
                nativeBridgeAbiVersion: 7,
                createdAtMs: 100
            ).requireUsableForOfflineExchange(nowMs: 200, requiredNativeBridgeAbiVersion: 7)
        ) { error in
            XCTAssertEqual(error as? OfflineCashConfigurationSnapshotError, .offlinePaymentsDisabled)
        }

        XCTAssertThrowsError(
            try OfflineCashConfigurationSnapshot(
                chainId: "00000042",
                assetDefinitionId: "pkr#sbp",
                offlinePaymentsEnabled: true,
                issuerPublicKeyBase64: "issuer-public-key",
                nativeBridgeAbiVersion: 6,
                createdAtMs: 100
            ).requireUsableForOfflineExchange(nowMs: 200, requiredNativeBridgeAbiVersion: 7)
        ) { error in
            XCTAssertEqual(
                error as? OfflineCashConfigurationSnapshotError,
                .unsupportedNativeBridgeAbi(required: 7, actual: 6)
            )
        }
    }

    func testKagemushaWireNamesArePublicAndCanonical() {
        XCTAssertEqual(
            KagemushaInstructionType.transfer.wireName,
            KagemushaWireNames.transferInstruction
        )
        XCTAssertEqual(
            KagemushaInstructionType.redeemRecursive.wireName,
            KagemushaWireNames.redeemRecursiveInstruction
        )
        XCTAssertEqual(
            KagemushaRecursiveRedeemRequestArchive.schemaName,
            KagemushaWireNames.recursiveRedeemRequest
        )
    }
}

private enum TestLifecycleError: Error {
    case stop
    case auditSyncFailed
}

private actor RecordingLifecycleEvents {
    private var events: [String] = []

    func append(_ event: String) {
        events.append(event)
    }

    func snapshot() -> [String] {
        events
    }
}

private final class RecordingAuditReceiptSynchronizer: OfflineCashAuditReceiptSynchronizing {
    private let hasPending: Bool
    private let events: RecordingLifecycleEvents
    private let syncError: Error?

    init(hasPending: Bool, events: RecordingLifecycleEvents, syncError: Error? = nil) {
        self.hasPending = hasPending
        self.events = events
        self.syncError = syncError
    }

    func hasPendingAuditReceipts() async throws -> Bool {
        await events.append("hasPending")
        return hasPending
    }

    func syncPendingAuditReceipts() async throws {
        await events.append("sync")
        if let syncError {
            throw syncError
        }
    }
}
