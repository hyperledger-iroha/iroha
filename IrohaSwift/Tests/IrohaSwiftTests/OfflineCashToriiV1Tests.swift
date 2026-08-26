import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineCashToriiV1Tests: XCTestCase {
    private static let operationId = String(repeating: "11", count: 32)
    private static let transactionHash = String(repeating: "23", count: 32)
    private static let statusValidationSymbol =
        "connect_norito_kagemusha_validate_operation_status_v4"

    func testNativeSubmissionProjectionUsesExactFixedWidthLayout() throws {
        let operationId = Data(repeating: 0x11, count: 32)
        let submittedAt: UInt64 = 1_725_000_000_001
        var bytes = TestNetworkIds.canonical.bytes
        bytes.append(operationId)
        for shift in stride(from: 56, through: 0, by: -8) {
            bytes.append(UInt8(truncatingIfNeeded: submittedAt >> UInt64(shift)))
        }

        let projection = try NoritoNativeBridge
            .decodeKagemushaSubmissionRequestProjectionV4(bytes)
        XCTAssertEqual(projection.networkId, TestNetworkIds.canonical)
        XCTAssertEqual(projection.operationId, operationId)
        XCTAssertEqual(projection.submittedAtMilliseconds, submittedAt)
    }

    func testNativeSubmissionProjectionRejectsMalformedOrUnboundFields() throws {
        var canonical = TestNetworkIds.canonical.bytes
        canonical.append(Data(repeating: 0x11, count: 32))
        canonical.append(Data([0, 0, 0, 0, 0, 0, 0, 1]))

        var invalidNetwork = canonical
        invalidNetwork[NetworkId.byteCount - 1] &= 0xfe
        var zeroOperation = canonical
        zeroOperation.replaceSubrange(32..<64, with: Data(repeating: 0, count: 32))
        var zeroTime = canonical
        zeroTime.replaceSubrange(64..<72, with: Data(repeating: 0, count: 8))

        for invalid in [
            Data(),
            Data(canonical.dropLast()),
            canonical + Data([0]),
            invalidNetwork,
            zeroOperation,
            zeroTime,
        ] {
            XCTAssertThrowsError(
                try NoritoNativeBridge.decodeKagemushaSubmissionRequestProjectionV4(invalid)
            ) { error in
                XCTAssertEqual(
                    error as? NativeBridgeError,
                    .invalidKagemushaVerifierOutput
                )
            }
        }
    }

    func testOpaqueReferenceRetainsCanonicalBytesAndProjectsSafeFields() throws {
        let internalReference = try KagemushaOperationReference(
            operationId: Self.operationId,
            kind: .topUp,
            state: .pending,
            transactionHash: Self.transactionHash,
            statusUri: "/v1/offline/operations/\(Self.operationId)",
            submittedAtMs: 1_725_000_000_001
        )
        var source = KagemushaOperationCodec.encodeReference(internalReference)
        let expected = source
        let reference = try OfflineCashOperationReferenceV1(canonicalNorito: source)
        source[source.startIndex] ^= 0xff

        XCTAssertEqual(reference.encodeCanonical(), expected)
        let projected = reference.project()
        XCTAssertEqual(projected.operationId, Self.operationId)
        XCTAssertEqual(projected.kind, .topUp)
        XCTAssertEqual(projected.state, .pending)
        XCTAssertEqual(projected.transactionHash, Self.transactionHash)
        XCTAssertEqual(
            projected.statusURI,
            "/v1/offline/operations/\(Self.operationId)"
        )
        XCTAssertEqual(projected.submittedAtMilliseconds, 1_725_000_000_001)
    }

    func testOpaqueStatusProjectsPendingAndRejectedWithoutSubstrateTypes() throws {
        try requireNativeStatusValidation()
        let pendingArchive = Self.pendingStatusArchive(submittedAtMilliseconds: 42)
        let pending = try OfflineCashOperationStatusV1(
            canonicalNorito: pendingArchive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertEqual(pending.encodeCanonical(), pendingArchive)
        XCTAssertEqual(
            pending.project(),
            OfflineCashOperationStatusProjectionV1(
                state: .pending,
                kind: .topUp,
                operationId: Self.operationId,
                transactionHash: Self.transactionHash,
                submittedAtMilliseconds: 42,
                finalizedBlockHeight: nil,
                serverTimeMilliseconds: nil,
                finalizedTopUp: nil,
                rejection: nil
            )
        )

        let rejected = try OfflineCashOperationStatusV1(
            canonicalNorito: Self.rejectedStatusArchive(),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).project()
        XCTAssertEqual(rejected.state, .rejected)
        XCTAssertEqual(rejected.kind, .redeem)
        XCTAssertEqual(rejected.operationId, Self.operationId)
        XCTAssertEqual(rejected.rejection?.code, "offline_operation_rejected")
        XCTAssertEqual(rejected.rejection?.message, "rejected")
    }

    func testNativeStatusValidationRejectsMismatchedAppliedTopUpProofBindings() throws {
        try requireNativeStatusValidation()
        let fixtures = try Self.offlineCashFixtureRows()
        let valid = try Self.fixtureArchive(named: "top_up_applied_status", in: fixtures)
        let decoded = try OfflineCashOperationStatusV1(
            canonicalNorito: valid,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertEqual(decoded.encodeCanonical(), valid)

        for name in [
            "invalid_top_up_anchor_status",
            "invalid_top_up_proof_status",
            "wrong_top_up_operation_status",
            "wrong_top_up_transaction_status",
            "wrong_top_up_height_status",
            "wrong_top_up_proof_network_status",
            "wrong_top_up_proof_anchor_status",
            "wrong_top_up_proof_height_status",
        ] {
            let archive = try Self.fixtureArchive(named: name, in: fixtures)
            XCTAssertThrowsError(try OfflineCashOperationStatusV1(
                canonicalNorito: archive,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ), name) {
                XCTAssertEqual(
                    $0 as? OfflineCashToriiV1Error,
                    .invalidCanonicalStatus,
                    name
                )
            }
        }
    }

    func testPublicRequestWrappersRejectBoundsBeforeNativeDispatch() {
        XCTAssertThrowsError(try OfflineCashTopUpRequestV1(canonicalNorito: Data())) {
            XCTAssertEqual(
                $0 as? OfflineCashToriiV1Error,
                .invalidCanonicalRequest
            )
        }
        XCTAssertThrowsError(try OfflineCashTopUpRequestV1(
            canonicalNorito: Data(
                repeating: 0xa5,
                count: OfflineCashTopUpRequestV1.maximumCanonicalBytes + 1
            )
        )) {
            XCTAssertEqual(
                $0 as? OfflineCashToriiV1Error,
                .invalidCanonicalRequest
            )
        }
    }

    func testPublicResponseWrappersRejectBoundsBeforeDecodingOrNativeDispatch() {
        for invalid in [
            Data(),
            Data(
                repeating: 0xa5,
                count: OfflineCashOperationReferenceV1.maximumCanonicalBytes + 1
            ),
        ] {
            XCTAssertThrowsError(
                try OfflineCashOperationReferenceV1(canonicalNorito: invalid)
            ) {
                XCTAssertEqual(
                    $0 as? OfflineCashToriiV1Error,
                    .invalidCanonicalReference
                )
            }
        }

        for invalid in [
            Data(),
            Data(
                repeating: 0xa5,
                count: OfflineCashOperationStatusV1.maximumCanonicalBytes + 1
            ),
        ] {
            XCTAssertThrowsError(try OfflineCashOperationStatusV1(
                canonicalNorito: invalid,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )) {
                XCTAssertEqual(
                    $0 as? OfflineCashToriiV1Error,
                    .invalidCanonicalStatus
                )
            }
        }
    }

    func testClientRequiresImmutableLocalSigningContext() {
        let client = ToriiClient(baseURL: URL(string: "https://example.test")!)
        XCTAssertThrowsError(try OfflineCashToriiClientV1(
            client: client,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )) {
            XCTAssertEqual(
                $0 as? OfflineCashToriiV1Error,
                .localSigningContextRequired
            )
        }
    }

    private func requireNativeStatusValidation() throws {
        guard NoritoNativeBridge.shared.hasKagemushaRecursiveSpendV4Symbols([
            Self.statusValidationSymbol,
        ]) else {
            throw XCTSkip("ABI22 Offline Cash status validation bridge is unavailable")
        }
    }

    private static func offlineCashFixtureRows() throws -> [String: String] {
        let environmentName = "IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN"
        guard let configured = ProcessInfo.processInfo.environment[environmentName],
              !configured.isEmpty else {
            throw XCTSkip("\(environmentName) is not configured")
        }
        let repositoryRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let executable = configured.hasPrefix("/")
            ? URL(fileURLWithPath: configured)
            : repositoryRoot.appendingPathComponent(configured)
        guard FileManager.default.isExecutableFile(atPath: executable.path) else {
            throw XCTSkip("\(environmentName) does not name an executable fixture generator")
        }

        let process = Process()
        let output = Pipe()
        process.executableURL = executable
        process.arguments = ["offline-cash-v1"]
        process.currentDirectoryURL = repositoryRoot
        process.standardOutput = output
        process.standardError = output
        try process.run()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0,
              let text = String(data: data, encoding: .utf8) else {
            XCTFail("authoritative Offline Cash fixture generator failed")
            return [:]
        }
        var rows: [String: String] = [:]
        for row in text.split(separator: "\n") {
            guard let separator = row.firstIndex(of: "=") else { continue }
            let name = String(row[..<separator])
            let value = String(row[row.index(after: separator)...])
            guard rows.updateValue(value, forKey: name) == nil else {
                XCTFail("duplicate Offline Cash fixture row \(name)")
                return [:]
            }
        }
        return rows
    }

    private static func fixtureArchive(
        named name: String,
        in fixtures: [String: String]
    ) throws -> Data {
        guard let hexadecimal = fixtures[name],
              !hexadecimal.isEmpty,
              hexadecimal.count.isMultiple(of: 2) else {
            XCTFail("missing canonical Offline Cash fixture \(name)")
            return Data()
        }
        var archive = Data(capacity: hexadecimal.count / 2)
        var index = hexadecimal.startIndex
        while index < hexadecimal.endIndex {
            let next = hexadecimal.index(index, offsetBy: 2)
            guard let byte = UInt8(hexadecimal[index..<next], radix: 16) else {
                XCTFail("Offline Cash fixture \(name) is not hexadecimal")
                return Data()
            }
            archive.append(byte)
            index = next
        }
        return archive
    }

    private static func pendingStatusArchive(submittedAtMilliseconds: UInt64) -> Data {
        var status = CompactNoritoWriter()
        status.writeUInt32LE(0)
        status.writeField(CompactNorito.encodeString(operationId))
        status.writeField(CompactNorito.encodeUInt32(0))
        status.writeField(CompactNorito.encodeString(transactionHash))
        status.writeField(CompactNorito.encodeUInt64(submittedAtMilliseconds))
        return noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationStatus",
            payload: status.data,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
    }

    private static func rejectedStatusArchive() -> Data {
        var error = CompactNoritoWriter()
        error.writeField(CompactNorito.encodeString("offline_operation_rejected"))
        error.writeField(CompactNorito.encodeString("rejected"))

        var status = CompactNoritoWriter()
        status.writeUInt32LE(2)
        status.writeField(CompactNorito.encodeString(operationId))
        status.writeField(CompactNorito.encodeUInt32(1))
        status.writeField(CompactNorito.encodeString(transactionHash))
        status.writeField(error.data)
        return noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationStatus",
            payload: status.data,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
    }
}
