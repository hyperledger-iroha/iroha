import Foundation
import XCTest
#if canImport(NoritoBridge)
import NoritoBridge
#endif
@testable import IrohaSwift

final class NoritoRpcFixtureParityTests: XCTestCase {
    private static let signedTransactionType = "iroha_data_model::transaction::signed::SignedTransaction"

    func testSignedTransactionFixturesRoundTrip() throws {
        let loader = try NoritoRpcFixtureLoader()
        XCTAssertFalse(loader.names.isEmpty, "the shared fixture corpus must not be empty")
        for name in loader.names {
            try assertFixtureIntegrity(loader: loader, name: name)
        }

        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge native decoder not linked"
        )
        XCTAssertEqual(
            nativeBridgeABIVersion(),
            21,
            "required transaction fixture decode must execute through ABI-21"
        )
        for name in loader.names {
            try assertFixtureNativeRoundTrip(loader: loader, name: name)
        }
    }

    func testFixtureLoaderRejectsDuplicateNamesAndFiles() throws {
        let first = NoritoRpcFixtureLoader.Entry(
            name: "duplicate",
            authority: "authority",
            networkId: FixtureConstants.networkId,
            creationTimeMs: 1,
            timeToLiveMs: 100_000,
            nonce: nil,
            encodedFile: "shared.norito",
            encodedLen: 0,
            signedLen: 0,
            payloadBase64: "AA==",
            signedBase64: "AQ==",
            payloadHash: "payload-hash",
            signedHash: "signed-hash"
        )
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatedEntries([first, first])
        ) { error in
            guard case FixtureError.duplicateFixtureName("duplicate") = error else {
                return XCTFail("unexpected duplicate-name error: \(error)")
            }
        }

        let second = NoritoRpcFixtureLoader.Entry(
            name: "other",
            authority: first.authority,
            networkId: first.networkId,
            creationTimeMs: first.creationTimeMs,
            timeToLiveMs: first.timeToLiveMs,
            nonce: first.nonce,
            encodedFile: first.encodedFile,
            encodedLen: first.encodedLen,
            signedLen: first.signedLen,
            payloadBase64: first.payloadBase64,
            signedBase64: first.signedBase64,
            payloadHash: first.payloadHash,
            signedHash: first.signedHash
        )
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatedEntries([first, second])
        ) { error in
            guard case FixtureError.duplicateEncodedFile("shared.norito") = error else {
                return XCTFail("unexpected duplicate-file error: \(error)")
            }
        }

        let renamedClone = NoritoRpcFixtureLoader.Entry(
            name: "renamed-clone",
            authority: first.authority,
            networkId: first.networkId,
            creationTimeMs: first.creationTimeMs,
            timeToLiveMs: first.timeToLiveMs,
            nonce: first.nonce,
            encodedFile: "renamed-clone.norito",
            encodedLen: first.encodedLen,
            signedLen: first.signedLen,
            payloadBase64: first.payloadBase64,
            signedBase64: first.signedBase64,
            payloadHash: first.payloadHash,
            signedHash: first.signedHash
        )
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatedEntries([first, renamedClone])
        ) { error in
            guard case FixtureError.duplicatePayloadHash("payload-hash") = error else {
                return XCTFail("unexpected renamed-clone error: \(error)")
            }
        }

        for malformed in ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="] {
            let invalid = NoritoRpcFixtureLoader.Entry(
                name: "invalid-base64",
                authority: first.authority,
                networkId: first.networkId,
                creationTimeMs: first.creationTimeMs,
                timeToLiveMs: first.timeToLiveMs,
                nonce: first.nonce,
                encodedFile: "invalid-base64.norito",
                encodedLen: first.encodedLen,
                signedLen: first.signedLen,
                payloadBase64: malformed,
                signedBase64: first.signedBase64,
                payloadHash: first.payloadHash,
                signedHash: first.signedHash
            )
            XCTAssertThrowsError(
                try NoritoRpcFixtureLoader.validatedEntries([invalid])
            ) { error in
                guard case FixtureError.invalidBase64("invalid-base64.payload_base64") = error else {
                    return XCTFail("unexpected base64 error for \(malformed): \(error)")
                }
            }
        }
    }

    func testFixtureLoaderRequiresPositiveIntegerTtl() throws {
        func loadFixture(ttl: Any?) throws -> NoritoRpcFixtureLoader.Entry {
            var fixture: [String: Any] = [
                "name": "ttl-fixture",
                "authority": "authority",
                "network_id": FixtureConstants.networkId,
                "creation_time_ms": 1,
                "nonce": NSNull(),
                "encoded_file": "ttl-fixture.norito",
                "encoded_len": 1,
                "signed_len": 1,
                "payload_base64": "AA==",
                "signed_base64": "AQ==",
                "payload_hash": "payload-hash",
                "signed_hash": "signed-hash",
            ]
            if let ttl {
                fixture["time_to_live_ms"] = ttl
            }
            let data = try JSONSerialization.data(withJSONObject: ["fixtures": [fixture]])
            let manifest = try JSONDecoder().decode(
                NoritoRpcFixtureLoader.Manifest.self,
                from: data
            )
            _ = try NoritoRpcFixtureLoader.validatedEntries(manifest.fixtures)
            return try XCTUnwrap(manifest.fixtures.first)
        }

        XCTAssertEqual(try loadFixture(ttl: 1).timeToLiveMs, 1)
        XCTAssertEqual(try loadFixture(ttl: 100_000).timeToLiveMs, 100_000)

        let invalidValues: [(String, Any?)] = [
            ("missing", nil),
            ("null", NSNull()),
            ("zero", 0),
            ("negative", -1),
            ("true", true),
            ("false", false),
            ("fractional", 1.5),
            ("string", "100000"),
        ]
        for (label, value) in invalidValues {
            XCTAssertThrowsError(try loadFixture(ttl: value), label)
        }
    }

    func testFixtureLoaderRequiresCanonicalNetworkId() {
        let invalid = NoritoRpcFixtureLoader.Entry(
            name: "invalid-network",
            authority: "authority",
            networkId: FixtureConstants.networkId.lowercased(),
            creationTimeMs: 1,
            timeToLiveMs: 100_000,
            nonce: nil,
            encodedFile: "invalid-network.norito",
            encodedLen: 1,
            signedLen: 1,
            payloadBase64: "AA==",
            signedBase64: "AQ==",
            payloadHash: "payload-hash",
            signedHash: "signed-hash"
        )
        XCTAssertThrowsError(try NoritoRpcFixtureLoader.validatedEntries([invalid])) { error in
            guard case FixtureError.invalidNetworkId = error else {
                return XCTFail("unexpected network identity error: \(error)")
            }
        }
    }

    func testFixtureSchemasRejectLegacyChainField() throws {
        var manifestFixture: [String: Any] = [
            "name": "legacy-manifest",
            "authority": "authority",
            "network_id": FixtureConstants.networkId,
            "creation_time_ms": 1,
            "time_to_live_ms": 100_000,
            "nonce": NSNull(),
            "encoded_file": "legacy-manifest.norito",
            "encoded_len": 1,
            "signed_len": 1,
            "payload_base64": "AA==",
            "signed_base64": "AQ==",
            "payload_hash": "payload-hash",
            "signed_hash": "signed-hash",
        ]
        manifestFixture["chain"] = manifestFixture.removeValue(forKey: "network_id")
        let manifestData = try JSONSerialization.data(
            withJSONObject: ["fixtures": [manifestFixture]]
        )
        XCTAssertThrowsError(
            try StrictFixtureJSON.decode(
                NoritoRpcFixtureLoader.Manifest.self,
                from: manifestData,
                using: JSONDecoder(),
                context: "legacy transaction manifest"
            )
        )

        let sharedPayload: [String: Any] = [
            "authority": "authority",
            "network_id": FixtureConstants.networkId,
            "creation_time_ms": 1,
            "executable": ["Instructions": []],
            "fee_payment": [
                "payer": "authority",
                "value": ["charge_limits": []],
            ],
            "metadata": [:],
            "nonce": NSNull(),
            "time_to_live_ms": 100_000,
        ]
        var descriptor: [String: Any] = [
            "name": "legacy-descriptor",
            "authority": "authority",
            "network_id": FixtureConstants.networkId,
            "creation_time_ms": 1,
            "time_to_live_ms": 100_000,
            "nonce": NSNull(),
            "payload": sharedPayload,
            "payload_base64": "AA==",
            "signed_base64": "AQ==",
            "payload_hash": "payload-hash",
            "signed_hash": "signed-hash",
        ]
        descriptor["chain"] = descriptor.removeValue(forKey: "network_id")
        let descriptorData = try JSONSerialization.data(withJSONObject: [descriptor])
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatePayloadDocumentForTesting(descriptorData)
        )

        descriptor["network_id"] = descriptor.removeValue(forKey: "chain")
        var legacyPayload = sharedPayload
        legacyPayload["chain"] = legacyPayload.removeValue(forKey: "network_id")
        descriptor["payload"] = legacyPayload
        let nestedData = try JSONSerialization.data(withJSONObject: [descriptor])
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatePayloadDocumentForTesting(nestedData)
        )
    }

    func testMixedExecutableBatchFixturePreservesItemOrder() throws {
        let loader = try NoritoRpcFixtureLoader()
        let fixture = try loader.fixture(named: "mixed_executable_batch")
        try assertFixtureIntegrity(loader: loader, name: fixture.entry.name)

        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge native decoder not linked"
        )
        let signedBytes = try XCTUnwrap(Data(base64Encoded: fixture.entry.signedBase64))
        let versionedSignedBytes = versionedSignedTransaction(signedBytes)
        let json = try NoritoNativeBridge.shared.withChainDiscriminant(
            FixtureConstants.networkPrefix
        ) {
            NoritoNativeBridge.shared.decodeSignedTransaction(versionedSignedBytes)
        }
        let payload = try XCTUnwrap(json.flatMap { decodeSignedPayload(from: $0) })
        XCTAssertNil(payload["executable"], "legacy executable field alias must not be emitted")
        let executable = try XCTUnwrap(payload["instructions"] as? [String: Any])
        let items = try XCTUnwrap(executable["Batch"] as? [[String: Any]])

        XCTAssertEqual(items.count, 3)
        XCTAssertNotNil(items[0]["Instruction"])
        XCTAssertNotNil(items[1]["ContractCall"])
        XCTAssertNotNil(items[2]["Instruction"])
    }

    private func assertFixtureIntegrity(loader: NoritoRpcFixtureLoader, name: String) throws {
        let fixture = try loader.fixture(named: name)
        let payloadBytes = fixture.payloadBytes
        XCTAssertEqual(
            payloadBytes.count,
            fixture.entry.encodedLen,
            "encoded length mismatch for \(name)"
        )

        let payloadBase64 = try XCTUnwrap(
            Data(base64Encoded: fixture.entry.payloadBase64),
            "payload_base64 missing or invalid for \(name)"
        )
        XCTAssertEqual(payloadBase64.count, fixture.entry.encodedLen, "payload length mismatch for \(name)")
        XCTAssertEqual(payloadBytes, payloadBase64, "payload mismatch for \(name)")

        let signedBytes = try XCTUnwrap(
            Data(base64Encoded: fixture.entry.signedBase64),
            "signed_base64 missing or invalid for \(name)"
        )
        XCTAssertEqual(
            signedBytes.count,
            fixture.entry.signedLen,
            "signed length mismatch for \(name)"
        )
        let payloadHash = IrohaHash.hash(payloadBytes).hexLowercased()
        XCTAssertEqual(payloadHash, fixture.entry.payloadHash, "payload hash mismatch for \(name)")
        let signedPayload = try canonicalSignedTransactionPayload(signedBytes)
        XCTAssertEqual(
            signedPayload,
            payloadBytes,
            "signed transaction payload mismatch for \(name)"
        )
        var entrypoint = CompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(signedPayload)
        let signedHash = IrohaHash.hash(entrypoint.data).hexLowercased()
        XCTAssertEqual(signedHash, fixture.entry.signedHash, "signed hash mismatch for \(name)")
        XCTAssertNotEqual(
            IrohaHash.hash(signedBytes).hexLowercased(),
            fixture.entry.signedHash,
            "raw signed bytes must not alias the compact External hash for \(name)"
        )
    }

    private func assertFixtureNativeRoundTrip(loader: NoritoRpcFixtureLoader, name: String) throws {
        let fixture = try loader.fixture(named: name)
        let signedBytes = try XCTUnwrap(
            Data(base64Encoded: fixture.entry.signedBase64),
            "signed_base64 missing or invalid for \(name)"
        )
        let expectedAuthority = try expectedAuthorityLiteral(from: fixture.entry.authority)
        XCTAssertEqual(
            nativeSignedTransactionDecodeStatus(signedBytes),
            -2,
            "ABI-21 must reject the unversioned bare signed transaction fixture: \(name)"
        )
        let versionedSignedBytes = versionedSignedTransaction(signedBytes)
        XCTAssertEqual(versionedSignedBytes.first, FixtureConstants.signedTransactionVersion)
        XCTAssertEqual(
            Data(versionedSignedBytes.dropFirst()),
            signedBytes,
            "V1 framing must add only the canonical version byte for \(name)"
        )
        let decodedJson = try NoritoNativeBridge.shared.withChainDiscriminant(
            FixtureConstants.networkPrefix
        ) {
            NoritoNativeBridge.shared.decodeSignedTransaction(versionedSignedBytes)
        }
        let json = try XCTUnwrap(
            decodedJson,
            "ABI-22 native bridge must decode every required signed transaction fixture: \(name)"
        )
        guard let payload = decodeSignedPayload(from: json) else {
            return XCTFail("failed to decode signed transaction JSON for \(name)")
        }
        let decodedAuthority = payload["authority"] as? String
        assertAuthorityMatches(decodedAuthority,
                               expected: expectedAuthority,
                               name: name)
        let decodedDomain = try XCTUnwrap(payload["domain"] as? [String: Any])
        XCTAssertEqual(decodedDomain["kind"] as? String, "network")
        XCTAssertEqual(
            decodedDomain["value"] as? String,
            fixture.entry.networkId,
            "network_id mismatch in decode for \(name)"
        )
        if let creation = payload["creation_time_ms"] as? NSNumber {
            XCTAssertEqual(
                creation.uint64Value,
                fixture.entry.creationTimeMs,
                "creation_time_ms mismatch in decode for \(name)"
            )
        } else {
            XCTFail("creation_time_ms missing in decode for \(name)")
        }
        assertOptionalNumberEquals(
            payload["time_to_live_ms"],
            expected: fixture.entry.timeToLiveMs,
            name: name,
            field: "time_to_live_ms"
        )
        assertOptionalNumberEquals(
            payload["nonce"],
            expected: fixture.entry.nonce.map(UInt64.init),
            name: name,
            field: "nonce"
        )
    }
}

// MARK: - Fixtures

private struct NoritoRpcFixtureLoader {
    struct Manifest: Decodable {
        let fixtures: [Entry]

        private enum CodingKeys: String, CodingKey {
            case fixtures
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["fixtures"],
                context: "shared transaction manifest"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            fixtures = try container.decode([Entry].self, forKey: .fixtures)
        }
    }

    struct Entry: Decodable {
        let name: String
        let authority: String
        let networkId: String
        let creationTimeMs: UInt64
        let timeToLiveMs: UInt64
        let nonce: UInt32?
        let encodedFile: String
        let encodedLen: Int
        let signedLen: Int
        let payloadBase64: String
        let signedBase64: String
        let payloadHash: String
        let signedHash: String

        enum CodingKeys: String, CodingKey {
            case name
            case authority
            case networkId = "network_id"
            case creationTimeMs = "creation_time_ms"
            case timeToLiveMs = "time_to_live_ms"
            case nonce
            case encodedFile = "encoded_file"
            case encodedLen = "encoded_len"
            case signedLen = "signed_len"
            case payloadBase64 = "payload_base64"
            case signedBase64 = "signed_base64"
            case payloadHash = "payload_hash"
            case signedHash = "signed_hash"
        }

        init(
            name: String,
            authority: String,
            networkId: String,
            creationTimeMs: UInt64,
            timeToLiveMs: UInt64,
            nonce: UInt32?,
            encodedFile: String,
            encodedLen: Int,
            signedLen: Int,
            payloadBase64: String,
            signedBase64: String,
            payloadHash: String,
            signedHash: String
        ) {
            self.name = name
            self.authority = authority
            self.networkId = networkId
            self.creationTimeMs = creationTimeMs
            self.timeToLiveMs = timeToLiveMs
            self.nonce = nonce
            self.encodedFile = encodedFile
            self.encodedLen = encodedLen
            self.signedLen = signedLen
            self.payloadBase64 = payloadBase64
            self.signedBase64 = signedBase64
            self.payloadHash = payloadHash
            self.signedHash = signedHash
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                [
                    "authority", "network_id", "creation_time_ms", "encoded_file",
                    "encoded_len", "name", "nonce", "payload_base64", "payload_hash",
                    "signed_base64", "signed_hash", "signed_len", "time_to_live_ms",
                ],
                context: "shared transaction manifest fixture"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            name = try container.decode(String.self, forKey: .name)
            authority = try container.decode(String.self, forKey: .authority)
            networkId = try container.decode(String.self, forKey: .networkId)
            guard ToriiNativeAmxWire.isCanonicalHash(networkId) else {
                throw DecodingError.dataCorruptedError(
                    forKey: .networkId,
                    in: container,
                    debugDescription: "network_id must be an exact canonical NetworkId hash literal"
                )
            }
            creationTimeMs = try container.decode(UInt64.self, forKey: .creationTimeMs)
            timeToLiveMs = try container.decode(UInt64.self, forKey: .timeToLiveMs)
            nonce = try container.decodeIfPresent(UInt32.self, forKey: .nonce)
            encodedFile = try container.decode(String.self, forKey: .encodedFile)
            encodedLen = try container.decode(Int.self, forKey: .encodedLen)
            signedLen = try container.decode(Int.self, forKey: .signedLen)
            payloadBase64 = try container.decode(String.self, forKey: .payloadBase64)
            signedBase64 = try container.decode(String.self, forKey: .signedBase64)
            payloadHash = try container.decode(String.self, forKey: .payloadHash)
            signedHash = try container.decode(String.self, forKey: .signedHash)
        }
    }

    private struct PayloadDocumentEntry: Decodable {
        let name: String
        let authority: String
        let networkId: String
        let creationTimeMs: UInt64
        let timeToLiveMs: UInt64
        let nonce: UInt32?
        let payload: SharedPayload
        let payloadBase64: String
        let signedBase64: String
        let payloadHash: String
        let signedHash: String

        private enum CodingKeys: String, CodingKey {
            case name
            case authority
            case networkId = "network_id"
            case creationTimeMs = "creation_time_ms"
            case timeToLiveMs = "time_to_live_ms"
            case nonce
            case payload
            case payloadBase64 = "payload_base64"
            case signedBase64 = "signed_base64"
            case payloadHash = "payload_hash"
            case signedHash = "signed_hash"
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                [
                    "authority", "network_id", "creation_time_ms", "name", "nonce", "payload",
                    "payload_base64", "payload_hash", "signed_base64", "signed_hash",
                    "time_to_live_ms",
                ],
                context: "shared transaction payload descriptor"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            name = try container.decode(String.self, forKey: .name)
            authority = try container.decode(String.self, forKey: .authority)
            networkId = try container.decode(String.self, forKey: .networkId)
            guard ToriiNativeAmxWire.isCanonicalHash(networkId) else {
                throw DecodingError.dataCorruptedError(
                    forKey: .networkId,
                    in: container,
                    debugDescription: "network_id must be an exact canonical NetworkId hash literal"
                )
            }
            creationTimeMs = try container.decode(UInt64.self, forKey: .creationTimeMs)
            timeToLiveMs = try container.decode(UInt64.self, forKey: .timeToLiveMs)
            nonce = try container.decodeIfPresent(UInt32.self, forKey: .nonce)
            payload = try container.decode(SharedPayload.self, forKey: .payload)
            payloadBase64 = try container.decode(String.self, forKey: .payloadBase64)
            signedBase64 = try container.decode(String.self, forKey: .signedBase64)
            payloadHash = try container.decode(String.self, forKey: .payloadHash)
            signedHash = try container.decode(String.self, forKey: .signedHash)
        }
    }

    private struct SharedPayload: Decodable {
        let authority: String
        let networkId: String
        let creationTimeMs: UInt64
        let timeToLiveMs: UInt64
        let nonce: UInt32?
        let executable: SharedExecutable
        let metadata: [String: ToriiJSONValue]

        private enum CodingKeys: String, CodingKey {
            case authority
            case networkId = "network_id"
            case creationTimeMs = "creation_time_ms"
            case timeToLiveMs = "time_to_live_ms"
            case nonce
            case executable
            case feePayment = "fee_payment"
            case metadata
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                [
                    "authority", "network_id", "creation_time_ms", "executable", "fee_payment",
                    "metadata", "nonce", "time_to_live_ms",
                ],
                context: "shared transaction payload"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            authority = try container.decode(String.self, forKey: .authority)
            networkId = try container.decode(String.self, forKey: .networkId)
            guard ToriiNativeAmxWire.isCanonicalHash(networkId) else {
                throw DecodingError.dataCorruptedError(
                    forKey: .networkId,
                    in: container,
                    debugDescription: "network_id must be an exact canonical NetworkId hash literal"
                )
            }
            creationTimeMs = try container.decode(UInt64.self, forKey: .creationTimeMs)
            timeToLiveMs = try container.decode(UInt64.self, forKey: .timeToLiveMs)
            guard timeToLiveMs > 0 else {
                throw DecodingError.dataCorruptedError(
                    forKey: .timeToLiveMs,
                    in: container,
                    debugDescription: "time_to_live_ms must be positive"
                )
            }
            nonce = try container.decodeIfPresent(UInt32.self, forKey: .nonce)
            if nonce == 0 {
                throw DecodingError.dataCorruptedError(
                    forKey: .nonce,
                    in: container,
                    debugDescription: "nonce must be null or positive"
                )
            }
            executable = try container.decode(SharedExecutable.self, forKey: .executable)
            _ = try container.decode(SharedFeePayment.self, forKey: .feePayment)
            metadata = try container.decode([String: ToriiJSONValue].self, forKey: .metadata)
        }
    }

    private enum SharedExecutable: Decodable {
        case instructions([SharedInstruction])
        case ivm(Data)
        case batch([SharedBatchItem])
        case contractCall(SharedContractCall)

        private enum CodingKeys: String, CodingKey {
            case instructions = "Instructions"
            case ivm = "Ivm"
            case batch = "Batch"
            case contractCall = "ContractCall"
        }

        init(from decoder: Decoder) throws {
            let dynamic = try decoder.container(keyedBy: FixtureJSONCodingKey.self)
            let variants = Set(dynamic.allKeys.map(\.stringValue))
            guard variants.count == 1,
                  let variant = variants.first,
                  ["Batch", "ContractCall", "Instructions", "Ivm"].contains(variant) else {
                throw DecodingError.dataCorrupted(
                    .init(
                        codingPath: decoder.codingPath,
                        debugDescription: "executable must contain exactly one known variant"
                    )
                )
            }
            let container = try decoder.container(keyedBy: CodingKeys.self)
            switch variant {
            case "Instructions":
                self = try .instructions(container.decode([SharedInstruction].self, forKey: .instructions))
            case "Ivm":
                let encoded = try container.decode(String.self, forKey: .ivm)
                guard let bytes = Data(base64Encoded: encoded),
                      !bytes.isEmpty,
                      bytes.base64EncodedString() == encoded else {
                    throw DecodingError.dataCorruptedError(
                        forKey: .ivm,
                        in: container,
                        debugDescription: "Ivm must use non-empty canonical base64"
                    )
                }
                self = .ivm(bytes)
            case "Batch":
                self = try .batch(container.decode([SharedBatchItem].self, forKey: .batch))
            case "ContractCall":
                self = try .contractCall(container.decode(SharedContractCall.self, forKey: .contractCall))
            default:
                throw DecodingError.dataCorrupted(
                    .init(codingPath: decoder.codingPath, debugDescription: "unknown executable")
                )
            }
        }
    }

    private struct SharedInstruction: Decodable {
        let payloadBase64: String
        let wireName: String

        private enum CodingKeys: String, CodingKey {
            case payloadBase64 = "payload_base64"
            case wireName = "wire_name"
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["payload_base64", "wire_name"],
                context: "shared instruction descriptor"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            payloadBase64 = try container.decode(String.self, forKey: .payloadBase64)
            wireName = try container.decode(String.self, forKey: .wireName)
            guard let bytes = Data(base64Encoded: payloadBase64),
                  bytes.base64EncodedString() == payloadBase64,
                  !wireName.isEmpty else {
                throw DecodingError.dataCorrupted(
                    .init(codingPath: decoder.codingPath, debugDescription: "invalid instruction descriptor")
                )
            }
        }
    }

    private enum SharedBatchItem: Decodable {
        case instruction(SharedInstruction)
        case contractCall(SharedContractCall)

        private enum CodingKeys: String, CodingKey {
            case instruction = "Instruction"
            case contractCall = "ContractCall"
        }

        init(from decoder: Decoder) throws {
            let dynamic = try decoder.container(keyedBy: FixtureJSONCodingKey.self)
            let variants = Set(dynamic.allKeys.map(\.stringValue))
            guard variants.count == 1,
                  let variant = variants.first,
                  ["ContractCall", "Instruction"].contains(variant) else {
                throw DecodingError.dataCorrupted(
                    .init(
                        codingPath: decoder.codingPath,
                        debugDescription: "batch item must contain exactly one known variant"
                    )
                )
            }
            let container = try decoder.container(keyedBy: CodingKeys.self)
            if variant == "Instruction" {
                self = try .instruction(container.decode(SharedInstruction.self, forKey: .instruction))
            } else {
                self = try .contractCall(container.decode(SharedContractCall.self, forKey: .contractCall))
            }
        }
    }

    private struct SharedContractCall: Decodable {
        let contractAddress: String
        let expectedCodeHash: String
        let entrypoint: String
        let arguments: [UInt8]?

        private enum CodingKeys: String, CodingKey {
            case contractAddress = "contract_address"
            case expectedCodeHash = "expected_code_hash"
            case entrypoint
            case arguments
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["arguments", "contract_address", "entrypoint", "expected_code_hash"],
                context: "shared contract call"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            contractAddress = try container.decode(String.self, forKey: .contractAddress)
            expectedCodeHash = try container.decode(String.self, forKey: .expectedCodeHash)
            entrypoint = try container.decode(String.self, forKey: .entrypoint)
            arguments = try container.decodeIfPresent([UInt8].self, forKey: .arguments)
        }
    }

    private struct SharedFeePayment: Decodable {
        private enum CodingKeys: String, CodingKey {
            case payer
            case value
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["payer", "value"],
                context: "shared fee payment"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            _ = try container.decode(String.self, forKey: .payer)
            _ = try container.decode(SharedFeeValue.self, forKey: .value)
        }
    }

    private struct SharedFeeValue: Decodable {
        private enum CodingKeys: String, CodingKey {
            case chargeLimits = "charge_limits"
            case gasLimit = "gas_limit"
        }

        init(from decoder: Decoder) throws {
            let dynamic = try decoder.container(keyedBy: FixtureJSONCodingKey.self)
            let actual = Set(dynamic.allKeys.map(\.stringValue))
            guard actual == ["charge_limits"] || actual == ["charge_limits", "gas_limit"] else {
                throw DecodingError.dataCorrupted(
                    .init(codingPath: decoder.codingPath, debugDescription: "invalid fee value fields")
                )
            }
            let container = try decoder.container(keyedBy: CodingKeys.self)
            _ = try container.decode([SharedChargeLimit].self, forKey: .chargeLimits)
            if container.contains(.gasLimit) {
                let gasLimit = try container.decode(UInt64.self, forKey: .gasLimit)
                guard gasLimit > 0 else {
                    throw DecodingError.dataCorruptedError(
                        forKey: .gasLimit,
                        in: container,
                        debugDescription: "gas_limit must be positive"
                    )
                }
            }
        }
    }

    private struct SharedChargeLimit: Decodable {
        private enum CodingKeys: String, CodingKey {
            case assetDefinitionId = "asset_definition_id"
            case kind
            case maxAmount = "max_amount"
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["asset_definition_id", "kind", "max_amount"],
                context: "shared charge limit"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            _ = try container.decode(String.self, forKey: .assetDefinitionId)
            _ = try container.decode(SharedChargeKind.self, forKey: .kind)
            _ = try container.decode(String.self, forKey: .maxAmount)
        }
    }

    private struct SharedChargeKind: Decodable {
        private enum CodingKeys: String, CodingKey {
            case kind
            case value
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["kind", "value"],
                context: "shared charge kind"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            _ = try container.decode(String.self, forKey: .kind)
            _ = try container.decodeIfPresent(ToriiJSONValue.self, forKey: .value)
        }
    }

    private let entries: [String: Entry]
    private let root: URL
    let names: [String]

    init() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // NoritoRpcFixtureParityTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift package root
        let manifestURL = root.appendingPathComponent("fixtures/norito_rpc/transaction_fixtures.manifest.json")
        let payloadURL = root.appendingPathComponent("fixtures/norito_rpc/transaction_payloads.json")
        let decoder = JSONDecoder()
        let manifest = try StrictFixtureJSON.decode(
            Manifest.self,
            from: Data(contentsOf: manifestURL),
            using: decoder,
            context: manifestURL.path
        )
        let payloadDescriptors = try StrictFixtureJSON.decode(
            [PayloadDocumentEntry].self,
            from: Data(contentsOf: payloadURL),
            using: decoder,
            context: payloadURL.path
        )
        let entries = try Self.validatedEntries(manifest.fixtures)
        let payloads = try Self.validatedPayloadEntries(payloadDescriptors)
        guard Set(entries.keys) == Set(payloads.keys) else {
            throw FixtureError.fixtureNameSetMismatch(
                payloads: Set(payloads.keys),
                manifests: Set(entries.keys)
            )
        }
        for name in entries.keys {
            guard let entry = entries[name], let payload = payloads[name] else {
                throw FixtureError.missingFixture(name)
            }
            try Self.validateDescriptorParity(payload, manifest: entry)
            try Self.validateEntryIdentity(entry, root: root)
        }
        self.entries = entries
        self.root = root
        names = entries.keys.sorted()
    }

    static func validatedEntries(_ fixtures: [Entry]) throws -> [String: Entry] {
        var names = Set<String>()
        var encodedFiles = Set<String>()
        var payloadHashes = Set<String>()
        var payloadBytesValues = Set<Data>()
        var signedHashes = Set<String>()
        var signedBytesValues = Set<Data>()
        for entry in fixtures {
            guard ToriiNativeAmxWire.isCanonicalHash(entry.networkId) else {
                throw FixtureError.invalidNetworkId(entry.networkId)
            }
            guard entry.timeToLiveMs > 0 else {
                throw FixtureError.invalidTimeToLiveMs(entry.name)
            }
            guard names.insert(entry.name).inserted else {
                throw FixtureError.duplicateFixtureName(entry.name)
            }
            guard encodedFiles.insert(entry.encodedFile).inserted else {
                throw FixtureError.duplicateEncodedFile(entry.encodedFile)
            }
            guard payloadHashes.insert(entry.payloadHash).inserted else {
                throw FixtureError.duplicatePayloadHash(entry.payloadHash)
            }
            let payloadBytes = try decodeCanonicalBase64(
                entry.payloadBase64,
                context: "\(entry.name).payload_base64"
            )
            guard payloadBytesValues.insert(payloadBytes).inserted else {
                throw FixtureError.duplicatePayloadBytes(entry.name)
            }
            guard signedHashes.insert(entry.signedHash).inserted else {
                throw FixtureError.duplicateSignedHash(entry.signedHash)
            }
            let signedBytes = try decodeCanonicalBase64(
                entry.signedBase64,
                context: "\(entry.name).signed_base64"
            )
            guard signedBytesValues.insert(signedBytes).inserted else {
                throw FixtureError.duplicateSignedBytes(entry.name)
            }
        }
        for entry in fixtures {
            guard !entry.name.isEmpty,
                  !entry.name.contains("/"),
                  !entry.name.contains("\\"),
                  entry.encodedFile == "\(entry.name).norito" else {
                throw FixtureError.invalidEncodedFile(entry.encodedFile)
            }
            guard entry.timeToLiveMs > 0 else {
                throw FixtureError.invalidTimeToLiveMs(entry.name)
            }
            guard entry.nonce != 0 else {
                throw FixtureError.invalidNonce(entry.name)
            }
            guard entry.encodedLen > 0,
                  entry.encodedLen <= 16 * 1024 * 1024,
                  entry.signedLen > 0,
                  entry.signedLen <= 16 * 1024 * 1024 else {
                throw FixtureError.invalidLength(entry.name)
            }
        }
        return Dictionary(uniqueKeysWithValues: fixtures.map { ($0.name, $0) })
    }

    private static func validatedPayloadEntries(
        _ fixtures: [PayloadDocumentEntry]
    ) throws -> [String: PayloadDocumentEntry] {
        var entries: [String: PayloadDocumentEntry] = [:]
        for entry in fixtures {
            guard entries[entry.name] == nil else {
                throw FixtureError.duplicateFixtureName(entry.name)
            }
            guard ToriiNativeAmxWire.isCanonicalHash(entry.networkId),
                  ToriiNativeAmxWire.isCanonicalHash(entry.payload.networkId) else {
                throw FixtureError.invalidNetworkId(entry.networkId)
            }
            guard entry.authority == entry.payload.authority,
                  entry.networkId == entry.payload.networkId,
                  entry.creationTimeMs == entry.payload.creationTimeMs,
                  entry.timeToLiveMs == entry.payload.timeToLiveMs,
                  entry.nonce == entry.payload.nonce else {
                throw FixtureError.payloadMetadataMismatch(entry.name)
            }
            entries[entry.name] = entry
        }
        return entries
    }

    static func validatePayloadDocumentForTesting(_ data: Data) throws {
        let fixtures = try StrictFixtureJSON.decode(
            [PayloadDocumentEntry].self,
            from: data,
            using: JSONDecoder(),
            context: "test transaction payload descriptor"
        )
        _ = try validatedPayloadEntries(fixtures)
    }

    private static func validateDescriptorParity(
        _ payload: PayloadDocumentEntry,
        manifest: Entry
    ) throws {
        guard payload.authority == manifest.authority,
              payload.networkId == manifest.networkId,
              payload.creationTimeMs == manifest.creationTimeMs,
              payload.timeToLiveMs == manifest.timeToLiveMs,
              payload.nonce == manifest.nonce,
              payload.payloadBase64 == manifest.payloadBase64,
              payload.signedBase64 == manifest.signedBase64,
              payload.payloadHash == manifest.payloadHash,
              payload.signedHash == manifest.signedHash else {
            throw FixtureError.manifestPayloadMismatch(manifest.name)
        }
    }

    private static func validateEntryIdentity(_ entry: Entry, root: URL) throws {
        guard isLowerHex(entry.payloadHash, count: 64),
              isLowerHex(entry.signedHash, count: 64) else {
            throw FixtureError.invalidHash(entry.name)
        }
        let payloadBytes = try decodeCanonicalBase64(
            entry.payloadBase64,
            context: "\(entry.name).payload_base64"
        )
        let signedBytes = try decodeCanonicalBase64(
            entry.signedBase64,
            context: "\(entry.name).signed_base64"
        )
        guard payloadBytes.count == entry.encodedLen,
              signedBytes.count == entry.signedLen,
              IrohaHash.hash(payloadBytes).hexLowercased() == entry.payloadHash,
              try canonicalSignedTransactionPayload(signedBytes) == payloadBytes else {
            throw FixtureError.invalidIdentity(entry.name)
        }
        var compact = CompactNoritoWriter()
        compact.writeUInt32LE(0)
        compact.writeField(payloadBytes)
        guard IrohaHash.hash(compact.data).hexLowercased() == entry.signedHash else {
            throw FixtureError.invalidIdentity(entry.name)
        }
        let fixtureURL = root.appendingPathComponent("fixtures/norito_rpc/\(entry.encodedFile)")
        guard try Data(contentsOf: fixtureURL) == payloadBytes else {
            throw FixtureError.invalidIdentity(entry.name)
        }
    }

    private static func isLowerHex(_ value: String, count: Int) -> Bool {
        value.count == count && value.allSatisfy { "0123456789abcdef".contains($0) }
    }

    private static func decodeCanonicalBase64(_ value: String, context: String) throws -> Data {
        guard let decoded = Data(base64Encoded: value),
              decoded.base64EncodedString() == value else {
            throw FixtureError.invalidBase64(context)
        }
        return decoded
    }

    func fixture(named name: String) throws -> NoritoRpcFixture {
        guard let entry = entries[name] else {
            throw FixtureError.missingFixture(name)
        }
        let path = root.appendingPathComponent("fixtures/norito_rpc/\(entry.encodedFile)")
        let data = try Data(contentsOf: path)
        return NoritoRpcFixture(entry: entry, payloadBytes: data)
    }
}

private struct NoritoRpcFixture {
    let entry: NoritoRpcFixtureLoader.Entry
    let payloadBytes: Data
}

private enum FixtureConstants {
    static let networkPrefix: UInt16 = 753
    static let signedTransactionVersion: UInt8 = 1
    static let networkId =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
}

private func versionedSignedTransaction(_ bareSignedTransaction: Data) -> Data {
    var versioned = Data([FixtureConstants.signedTransactionVersion])
    versioned.append(bareSignedTransaction)
    return versioned
}

private func nativeSignedTransactionDecodeStatus(_ data: Data) -> Int32 {
    #if canImport(NoritoBridge)
    var jsonPointer: UnsafeMutablePointer<UInt8>?
    var jsonLength: UInt = 0
    let status = data.withUnsafeBytes { buffer in
        connect_norito_decode_signed_transaction_json(
            buffer.bindMemory(to: UInt8.self).baseAddress,
            UInt(data.count),
            &jsonPointer,
            &jsonLength
        )
    }
    if let jsonPointer {
        connect_norito_free(jsonPointer)
    }
    return status
    #else
    return Int32.min
    #endif
}

private func nativeBridgeABIVersion() -> UInt32? {
    #if canImport(NoritoBridge)
    connect_norito_bridge_abi_version()
    #else
    nil
    #endif
}

private func expectedAuthorityLiteral(from label: String) throws -> String {
    guard !label.contains("@") else {
        throw FixtureError.invalidAuthority(label)
    }
    guard let parsed = try? AccountAddress.parseEncoded(label,
                                                        expectedPrefix: FixtureConstants.networkPrefix) else {
        throw FixtureError.invalidAuthority(label)
    }
    return try parsed.toI105(networkPrefix: FixtureConstants.networkPrefix)
}

private func decodeSignedPayload(from json: String) -> [String: Any]? {
    guard let data = json.data(using: .utf8) else {
        return nil
    }
    guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        return nil
    }
    return object["payload"] as? [String: Any]
}

private func assertOptionalNumberEquals(
    _ value: Any?,
    expected: UInt64?,
    name: String,
    field: String
) {
    if let expected {
        guard let number = value as? NSNumber else {
            return XCTFail("\(field) missing in decode for \(name)")
        }
        XCTAssertEqual(number.uint64Value, expected, "\(field) mismatch in decode for \(name)")
        return
    }
    if value == nil || value is NSNull {
        return
    }
    XCTFail("\(field) should be null in decode for \(name)")
}

private func assertAuthorityMatches(_ decoded: String?,
                                    expected: String,
                                    name: String) {
    guard let decoded else {
        return XCTFail("authority missing in decode for \(name)")
    }
    if expected.contains("@") {
        XCTAssertEqual(decoded, expected, "authority mismatch in decode for \(name)")
        return
    }
    if decoded == expected {
        return
    }
    if let atIndex = decoded.firstIndex(of: "@") {
        let prefix = String(decoded[..<atIndex])
        XCTAssertEqual(prefix, expected, "authority mismatch in decode for \(name)")
        return
    }
    XCTAssertEqual(decoded, expected, "authority mismatch in decode for \(name)")
}

private enum FixtureError: Error {
    case missingFixture(String)
    case invalidAuthority(String)
    case bridgeKeypairUnavailable
    case duplicateFixtureName(String)
    case duplicateEncodedFile(String)
    case duplicatePayloadHash(String)
    case duplicatePayloadBytes(String)
    case duplicateSignedHash(String)
    case duplicateSignedBytes(String)
    case invalidBase64(String)
    case invalidTimeToLiveMs(String)
    case invalidEncodedFile(String)
    case invalidNonce(String)
    case invalidNetworkId(String)
    case invalidLength(String)
    case payloadMetadataMismatch(String)
    case manifestPayloadMismatch(String)
    case fixtureNameSetMismatch(payloads: Set<String>, manifests: Set<String>)
    case invalidHash(String)
    case invalidIdentity(String)
}
