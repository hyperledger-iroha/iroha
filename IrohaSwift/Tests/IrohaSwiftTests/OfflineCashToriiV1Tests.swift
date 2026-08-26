import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineCashToriiV1Tests: XCTestCase {
    private static let operationId = String(repeating: "11", count: 32)
    private static let transactionHash = String(repeating: "23", count: 32)
    private static let statusValidationSymbol =
        "connect_norito_kagemusha_validate_operation_status_v4"
    private static let fixtureNames = [
        "network_id",
        "top_up_operation_id",
        "top_up_submitted_at_ms",
        "top_up_request",
        "top_up_reference",
        "top_up_pending_status",
        "top_up_finalized_block_height",
        "top_up_server_time_ms",
        "top_up_applied_status",
        "invalid_top_up_anchor_status",
        "invalid_top_up_proof_status",
        "wrong_top_up_operation_status",
        "wrong_top_up_transaction_status",
        "wrong_top_up_height_status",
        "wrong_top_up_proof_network_status",
        "foreign_network_top_up_status",
        "wrong_top_up_proof_anchor_status",
        "wrong_top_up_proof_height_status",
        "redeem_operation_id",
        "redeem_submitted_at_ms",
        "redeem_request",
        "redeem_reference",
        "redeem_pending_status",
        "redeem_applied_status",
        "rejected_status",
        "invalid_binding_top_up_request",
        "wrong_id_reference",
        "wrong_kind_reference",
        "wrong_time_reference",
        "zero_time_reference",
        "wrong_uri_reference",
        "invalid_transaction_hash_reference",
        "wrong_id_status",
        "zero_submitted_pending_status",
        "zero_height_status",
        "zero_time_status",
        "invalid_transaction_hash_status",
        "wrong_rejection_code_status",
        "rejection_details_status",
        "oversized_rejection_message_status",
    ]
    private static let fixtureDigestNames: Set<String> = [
        "network_id",
        "top_up_operation_id",
        "redeem_operation_id",
    ]
    private static let fixturePositiveDecimalNames: Set<String> = [
        "top_up_submitted_at_ms",
        "top_up_finalized_block_height",
        "top_up_server_time_ms",
        "redeem_submitted_at_ms",
    ]

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

    func testOfflineCashFixtureInventoryRejectsDrift() throws {
        let fixtures = try Self.offlineCashFixtureRows()
        let canonicalRows = try Self.fixtureNames.map { name in
            "\(name)=\(try XCTUnwrap(fixtures[name]))"
        }
        let canonical = canonicalRows.joined(separator: "\n") + "\n"
        XCTAssertEqual(try Self.parseOfflineCashFixtureRows(canonical), fixtures)

        var reorderedRows = canonicalRows
        reorderedRows.swapAt(0, 1)
        var duplicateRows = canonicalRows
        duplicateRows[1] = canonicalRows[0]
        var malformedValueRows = canonicalRows
        malformedValueRows[3] = "top_up_request=0G"
        let invalidInventories = [
            ("missing", canonicalRows.dropLast().joined(separator: "\n") + "\n"),
            ("additional", (canonicalRows + ["unexpected=00"]).joined(separator: "\n") + "\n"),
            ("reordered", reorderedRows.joined(separator: "\n") + "\n"),
            ("duplicate", duplicateRows.joined(separator: "\n") + "\n"),
            ("malformed-row", canonical.replacingOccurrences(
                of: "network_id=",
                with: "network_id:",
                options: .anchored
            )),
            ("malformed-value", malformedValueRows.joined(separator: "\n") + "\n"),
            ("missing-final-lf", String(canonical.dropLast())),
        ]
        for (label, inventory) in invalidInventories {
            XCTAssertThrowsError(
                try Self.parseOfflineCashFixtureRows(inventory),
                label
            ) { error in
                XCTAssertTrue(
                    error is RequiredNativeTestCapabilityError,
                    "\(label): \(error)"
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
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.hasKagemushaRecursiveSpendV4Symbols([
                Self.statusValidationSymbol,
            ]),
            "ABI22 Offline Cash status validation bridge is unavailable"
        )
    }

    private static func offlineCashFixtureRows() throws -> [String: String] {
        let environmentName = "IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN"
        guard let configured = ProcessInfo.processInfo.environment[environmentName],
              !configured.isEmpty else {
            try failRequiredNativeTestCapability("\(environmentName) is not configured")
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
            try failRequiredNativeTestCapability(
                "\(environmentName) does not name an executable fixture generator"
            )
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
            try failRequiredNativeTestCapability(
                "authoritative Offline Cash fixture generator failed"
            )
        }
        return try parseOfflineCashFixtureRows(text)
    }

    private static func parseOfflineCashFixtureRows(
        _ text: String
    ) throws -> [String: String] {
        guard text.hasSuffix("\n") else {
            try failRequiredNativeTestCapability(
                "authoritative Offline Cash fixture output must end with one LF"
            )
        }
        var fixtureRows = text.split(
            separator: "\n",
            omittingEmptySubsequences: false
        )
        fixtureRows.removeLast()
        guard fixtureRows.count == fixtureNames.count else {
            try failRequiredNativeTestCapability(
                "Offline Cash fixture output must contain exactly \(fixtureNames.count) rows"
            )
        }
        var rows: [String: String] = [:]
        for (expectedName, row) in zip(fixtureNames, fixtureRows) {
            guard let separator = row.firstIndex(of: "="),
                  separator != row.startIndex,
                  separator != row.index(before: row.endIndex),
                  row.lastIndex(of: "=") == separator else {
                try failRequiredNativeTestCapability(
                    "invalid Offline Cash fixture row"
                )
            }
            let name = String(row[..<separator])
            let value = String(row[row.index(after: separator)...])
            guard name == expectedName else {
                try failRequiredNativeTestCapability(
                    "unexpected Offline Cash fixture row \(name); expected \(expectedName)"
                )
            }
            try validateFixtureValue(value, named: name)
            guard rows.updateValue(value, forKey: name) == nil else {
                try failRequiredNativeTestCapability(
                    "duplicate Offline Cash fixture row \(name)"
                )
            }
        }
        return rows
    }

    private static func validateFixtureValue(
        _ value: String,
        named name: String
    ) throws {
        let isLowercaseHexadecimal = value.allSatisfy {
            ("0"..."9").contains($0) || ("a"..."f").contains($0)
        }
        if fixtureDigestNames.contains(name) {
            guard value.count == 64,
                  value.contains(where: { $0 != "0" }),
                  isLowercaseHexadecimal else {
                try failRequiredNativeTestCapability(
                    "\(name) must be exactly 32 non-zero lowercase hexadecimal bytes"
                )
            }
            if name == "network_id" {
                guard let marker = UInt8(value.suffix(2), radix: 16),
                      marker & 1 == 1 else {
                    try failRequiredNativeTestCapability(
                        "network_id must contain a canonical marked Iroha hash"
                    )
                }
            }
            return
        }
        if fixturePositiveDecimalNames.contains(name) {
            guard let first = value.first,
                  ("1"..."9").contains(first),
                  value.dropFirst().allSatisfy({ ("0"..."9").contains($0) }),
                  let parsed = Int64(value),
                  parsed > 0 else {
                try failRequiredNativeTestCapability(
                    "\(name) must be a canonical positive signed 64-bit decimal"
                )
            }
            return
        }
        guard !value.isEmpty,
              value.count.isMultiple(of: 2),
              isLowercaseHexadecimal else {
            try failRequiredNativeTestCapability(
                "\(name) must be non-empty even-length lowercase hexadecimal"
            )
        }
    }

    private static func fixtureArchive(
        named name: String,
        in fixtures: [String: String]
    ) throws -> Data {
        guard let hexadecimal = fixtures[name],
              !hexadecimal.isEmpty,
              hexadecimal.count.isMultiple(of: 2) else {
            try failRequiredNativeTestCapability(
                "missing canonical Offline Cash fixture \(name)"
            )
        }
        var archive = Data(capacity: hexadecimal.count / 2)
        var index = hexadecimal.startIndex
        while index < hexadecimal.endIndex {
            let next = hexadecimal.index(index, offsetBy: 2)
            guard let byte = UInt8(hexadecimal[index..<next], radix: 16) else {
                try failRequiredNativeTestCapability(
                    "Offline Cash fixture \(name) is not hexadecimal"
                )
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
