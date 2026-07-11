import XCTest
@testable import IrohaSwift

final class TransactionParityFixturesTests: XCTestCase {
    private static var cachedFixtures: TransactionFixtureLoader?
    private static var cachedKeypair: Keypair?

    func testExpectedSignedTransactionSchemaHashMatchesRustManifest() throws {
        let loader = try Self.fixtures()
        XCTAssertEqual(
            ToriiNodeCapabilities.expectedSignedTransactionSchemaHashHex,
            loader.schema.signedSchemaHashHex
        )
    }

    func testSwiftParityManifestSignedHashesUseCompactExternalEntrypoint() throws {
        let loader = try Self.fixtures()
        XCTAssertEqual(loader.manifests.count, 3)
        for (name, entry) in loader.manifests {
            let signedBytes = try XCTUnwrap(
                Data(base64Encoded: entry.signedBase64),
                "invalid signed_base64 for \(name)"
            )
            var compact = OfflineCompactNoritoWriter()
            compact.writeUInt32LE(0)
            compact.writeField(signedBytes)
            XCTAssertEqual(
                IrohaHash.hash(compact.data).hexEncodedString(),
                entry.signedHash,
                "compact External hash mismatch for \(name)"
            )
            XCTAssertNotEqual(
                IrohaHash.hash(signedBytes).hexEncodedString(),
                entry.signedHash,
                "raw signed bytes must not alias compact External hash for \(name)"
            )
        }
    }

    func testSwiftTransferAssetFixtureMatchesRustEncoder() throws {
        try ensureBridgeAvailable()
        try assertFixture(named: "swift_transfer_asset_basic") { fixture, keypair in
            let instruction = try fixture.payload.instruction(kind: "Transfer", action: "TransferAsset")
            let assetDefinitionId = try instruction.argument(named: "asset_definition_id")
            let destination = try instruction.argument(named: "destination")
            let quantity = try instruction.argument(named: "quantity")
            let authority = try TransactionParityFixturesTests.canonicalAccountId(
                fixture.payload.authority,
                field: "payload.authority"
            )
            let canonicalDestination = try TransactionParityFixturesTests.canonicalAccountId(
                destination,
                field: "Transfer.TransferAsset.destination"
            )
            let request = TransferRequest(chainId: fixture.payload.chain,
                                          authority: authority,
                                          assetDefinitionId: assetDefinitionId,
                                          quantity: quantity,
                                          destination: canonicalDestination,
                                          description: fixture.payload.metadata["memo"],
                                          ttlMs: fixture.payload.timeToLiveMs,
                                          nonce: fixture.payload.nonce)
            return try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                              keypair: keypair,
                                                              creationTimeMs: fixture.payload.creationTimeMs)
        }
    }

    func testSwiftMintAssetFixtureMatchesRustEncoder() throws {
        try ensureBridgeAvailable()
        try assertFixture(named: "swift_mint_asset_basic") { fixture, keypair in
            let instruction = try fixture.payload.instruction(kind: "Mint", action: "MintAsset")
            let assetDefinitionId = try instruction.argument(named: "asset_definition_id")
            let destination = try instruction.argument(named: "destination")
            let quantity = try instruction.argument(named: "quantity")
            let authority = try TransactionParityFixturesTests.canonicalAccountId(
                fixture.payload.authority,
                field: "payload.authority"
            )
            let canonicalDestination = try TransactionParityFixturesTests.canonicalAccountId(
                destination,
                field: "Mint.MintAsset.destination"
            )
            let request = MintRequest(chainId: fixture.payload.chain,
                                      authority: authority,
                                      assetDefinitionId: assetDefinitionId,
                                      quantity: quantity,
                                      destination: canonicalDestination,
                                      ttlMs: fixture.payload.timeToLiveMs,
                                      nonce: fixture.payload.nonce)
            return try SwiftTransactionEncoder.encodeMint(request: request,
                                                          keypair: keypair,
                                                          creationTimeMs: fixture.payload.creationTimeMs)
        }
    }

    func testSwiftBurnAssetFixtureMatchesRustEncoder() throws {
        try ensureBridgeAvailable()
        try assertFixture(named: "swift_burn_asset_basic") { fixture, keypair in
            let instruction = try fixture.payload.instruction(kind: "Burn", action: "BurnAsset")
            let assetDefinitionId = try instruction.argument(named: "asset_definition_id")
            let destination = try instruction.argument(named: "destination")
            let quantity = try instruction.argument(named: "quantity")
            let authority = try TransactionParityFixturesTests.canonicalAccountId(
                fixture.payload.authority,
                field: "payload.authority"
            )
            let canonicalDestination = try TransactionParityFixturesTests.canonicalAccountId(
                destination,
                field: "Burn.BurnAsset.destination"
            )
            let request = BurnRequest(chainId: fixture.payload.chain,
                                      authority: authority,
                                      assetDefinitionId: assetDefinitionId,
                                      quantity: quantity,
                                      destination: canonicalDestination,
                                      ttlMs: fixture.payload.timeToLiveMs,
                                      nonce: fixture.payload.nonce)
            return try SwiftTransactionEncoder.encodeBurn(request: request,
                                                          keypair: keypair,
                                                          creationTimeMs: fixture.payload.creationTimeMs)
        }
    }

    func testFixtureSeedKeypairMatchesBridge() throws {
        try ensureBridgeAvailable()
        guard let seed = Data(hexString: FixtureConstants.signingSeedHex) else {
            throw FixtureError.invalidSigningSeed
        }
        guard let derived = NoritoNativeBridge.shared.keypairFromSeed(algorithm: .ed25519, seed: seed) else {
            throw FixtureError.bridgeKeypairUnavailable
        }
        let keypair = try Keypair(privateKeyBytes: derived.privateKey)
        XCTAssertEqual(keypair.privateKeyBytes, derived.privateKey)
        XCTAssertEqual(keypair.publicKey, derived.publicKey)
    }

    func testCompactAndroidGoldenMatchesSwiftEntrypointHasher() throws {
        let fixtureURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/norito_rpc/iroha_compact_hash_vector.properties")
        let fixtureText = try String(contentsOf: fixtureURL, encoding: .utf8)
        let fixture = try Self.properties(fixtureText)
        let versioned = try TransactionFixtureLoader.decodeCanonicalBase64(
            try XCTUnwrap(fixture["versioned.base64"]),
            context: "versioned.base64"
        )
        XCTAssertEqual(versioned.count, Int(try XCTUnwrap(fixture["versioned.bytes"])))
        XCTAssertEqual(versioned.first, 1)

        let bare = Data(versioned.dropFirst())
        XCTAssertEqual(bare.count, Int(try XCTUnwrap(fixture["bare.bytes"])))
        var compact = OfflineCompactNoritoWriter()
        compact.writeUInt32LE(0)
        compact.writeField(bare)
        XCTAssertEqual(
            Data(compact.data.prefix(6)).hexEncodedString(),
            fixture["canonical.prefix.hex"]
        )
        XCTAssertEqual(
            IrohaHash.hash(compact.data).hexEncodedString(),
            fixture["canonical.hash"]
        )

        var fixedWidth = OfflineNoritoWriter()
        fixedWidth.writeUInt32LE(0)
        fixedWidth.writeField(bare)
        XCTAssertNotEqual(
            IrohaHash.hash(fixedWidth.data).hexEncodedString(),
            fixture["canonical.hash"],
            "fixed-u64 field lengths must not alias canonical COMPACT_LEN framing"
        )
    }

    func testParityFixtureLoadersRejectDuplicateIdentitiesAndBase64Aliases() throws {
        let first = TransactionFixtureLoader.ManifestEntry(
            name: "first",
            encodedFile: "first.norito",
            payloadBase64: "AA==",
            payloadHash: "payload-hash",
            signedBase64: "AQ==",
            signedHash: "signed-hash"
        )
        let renamedClone = TransactionFixtureLoader.ManifestEntry(
            name: "renamed-clone",
            encodedFile: "renamed-clone.norito",
            payloadBase64: first.payloadBase64,
            payloadHash: first.payloadHash,
            signedBase64: first.signedBase64,
            signedHash: first.signedHash
        )
        XCTAssertThrowsError(
            try TransactionFixtureLoader.validatedManifests([first, renamedClone])
        ) { error in
            guard case FixtureError.duplicatePayloadHash("payload-hash") = error else {
                return XCTFail("unexpected renamed-clone error: \(error)")
            }
        }

        for malformed in ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="] {
            XCTAssertThrowsError(
                try TransactionFixtureLoader.decodeCanonicalBase64(
                    malformed,
                    context: "adversarial.fixture"
                )
            ) { error in
                guard case FixtureError.invalidBase64("adversarial.fixture") = error else {
                    return XCTFail("unexpected base64 error for \(malformed): \(error)")
                }
            }
        }
    }

    func testCompactPropertiesRejectDuplicateKeys() throws {
        let fixtureURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/norito_rpc/iroha_compact_hash_vector.properties")
        let fixtureText = try String(contentsOf: fixtureURL, encoding: .utf8)
        XCTAssertThrowsError(
            try Self.properties(fixtureText + "\ncanonical.hash=duplicate\n")
        ) { error in
            guard case FixtureError.duplicateManifestProperty("canonical.hash") = error else {
                return XCTFail("unexpected duplicate-property error: \(error)")
            }
        }
    }

    // MARK: - Helpers

    private func assertFixture(
        named name: String,
        encoder: (CombinedTransactionFixture, Keypair) throws -> SignedTransactionEnvelope
    ) throws {
        let loader = try Self.fixtures()
        let fixture = try loader.fixture(named: name)
        let keypair = try Self.fixtureKeypair()
        do {
            let envelope = try NoritoNativeBridge.shared.withChainDiscriminant(FixtureConstants.networkPrefix) {
                try encoder(fixture, keypair)
            }
            let actual = envelope.signedTransaction.base64EncodedString()
            XCTAssertEqual(
                actual,
                fixture.manifest.signedBase64,
                "encoded transaction for \(name) did not match Rust fixture"
            )
            XCTAssertEqual(
                envelope.transactionHash.hexEncodedString(),
                fixture.manifest.signedHash,
                "entrypoint hash for \(name) did not match compact Rust framing"
            )
        } catch let SwiftTransactionEncoderError.nativeBridgeError(error) {
            throw error
        }
    }

    private func ensureBridgeAvailable() throws {
        guard NoritoNativeBridge.shared.supportsTransactions(using: .ed25519) else {
            throw XCTSkip("NoritoBridge ed25519 transaction encoder unavailable")
        }
        guard let seed = Data(hexString: FixtureConstants.signingSeedHex) else {
            throw FixtureError.invalidSigningSeed
        }
        guard NoritoNativeBridge.shared.keypairFromSeed(algorithm: .ed25519, seed: seed) != nil else {
            throw XCTSkip("NoritoBridge ed25519 seed derivation unavailable")
        }
    }

    private static func fixtures() throws -> TransactionFixtureLoader {
        if let cached = cachedFixtures { return cached }
        let loader = try TransactionFixtureLoader()
        cachedFixtures = loader
        return loader
    }

    private static func fixtureKeypair() throws -> Keypair {
        if let cached = cachedKeypair { return cached }
        guard let bytes = Data(hexString: FixtureConstants.signingSeedHex) else {
            throw FixtureError.invalidSigningSeed
        }
        guard let derived = NoritoNativeBridge.shared.keypairFromSeed(algorithm: .ed25519, seed: bytes) else {
            throw FixtureError.bridgeKeypairUnavailable
        }
        let keypair = try Keypair(privateKeyBytes: derived.privateKey)
        cachedKeypair = keypair
        return keypair
    }

    private static func canonicalAccountId(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw FixtureError.invalidAccountId(field: field, value: value)
        }
        do {
            _ = try AccountAddress.parseEncoded(trimmed)
            return trimmed
        } catch {
            throw FixtureError.invalidAccountId(field: field, value: value)
        }
    }

    private static func properties(_ contents: String) throws -> [String: String] {
        let expectedKeys: Set<String> = [
            "schema.version",
            "source.tag",
            "source.commit",
            "reference",
            "versioned.bytes",
            "versioned.sha256",
            "bare.bytes",
            "compact.length.hex",
            "canonical.prefix.hex",
            "canonical.hash",
            "payload.prehash",
            "pinned.sdk.defective.hash",
            "versioned.base64",
        ]
        var result: [String: String] = [:]
        for line in contents.split(whereSeparator: { $0.isNewline }) {
            if line.isEmpty || line.hasPrefix("#") { continue }
            guard let separator = line.firstIndex(of: "=") else {
                throw FixtureError.invalidManifestProperty(String(line))
            }
            let key = String(line[..<separator])
            let value = String(line[line.index(after: separator)...])
            guard !key.isEmpty, !value.isEmpty else {
                throw FixtureError.invalidManifestProperty(String(line))
            }
            guard result[key] == nil else {
                throw FixtureError.duplicateManifestProperty(key)
            }
            result[key] = value
        }
        guard Set(result.keys) == expectedKeys else {
            throw FixtureError.unexpectedManifestProperties(Set(result.keys))
        }
        return result
    }
}

// MARK: - Fixture Loading

private struct TransactionFixtureLoader {
    struct PayloadEntry: Decodable {
        let name: String
        let payload: TransactionPayloadSpec
    }

    struct ManifestEntry: Decodable {
        let name: String
        let encodedFile: String
        let payloadBase64: String
        let payloadHash: String
        let signedBase64: String
        let signedHash: String
    }

    struct ManifestSchema: Decodable {
        let signedSchemaHashHex: String
    }

    struct ManifestFile: Decodable {
        let schema: ManifestSchema
        let fixtures: [ManifestEntry]
    }

    let schema: ManifestSchema
    let payloads: [String: TransactionPayloadSpec]
    let manifests: [String: ManifestEntry]

    init() throws {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase

        let payloadURL = TransactionFixtureLoader.fixturesRoot()
            .appendingPathComponent("swift_parity_payloads.json")
        let payloadData = try Data(contentsOf: payloadURL)
        let payloadEntries = try decoder.decode([PayloadEntry].self, from: payloadData)
        payloads = try Self.validatedPayloads(payloadEntries)

        let manifestURL = TransactionFixtureLoader.fixturesRoot()
            .appendingPathComponent("swift_parity_manifest.json")
        let manifestData = try Data(contentsOf: manifestURL)
        let manifest = try decoder.decode(ManifestFile.self, from: manifestData)
        schema = manifest.schema
        manifests = try Self.validatedManifests(manifest.fixtures)
        guard Set(payloads.keys) == Set(manifests.keys) else {
            throw FixtureError.fixtureNameSetMismatch(
                payloads: Set(payloads.keys),
                manifests: Set(manifests.keys)
            )
        }
    }

    static func validatedPayloads(_ entries: [PayloadEntry]) throws -> [String: TransactionPayloadSpec] {
        var names = Set<String>()
        for entry in entries {
            guard names.insert(entry.name).inserted else {
                throw FixtureError.duplicateFixtureName(entry.name)
            }
        }
        return Dictionary(uniqueKeysWithValues: entries.map { ($0.name, $0.payload) })
    }

    static func validatedManifests(_ entries: [ManifestEntry]) throws -> [String: ManifestEntry] {
        var names = Set<String>()
        var encodedFiles = Set<String>()
        var payloadHashes = Set<String>()
        var payloadBytes = Set<Data>()
        var signedHashes = Set<String>()
        var signedBytes = Set<Data>()
        for entry in entries {
            guard names.insert(entry.name).inserted else {
                throw FixtureError.duplicateFixtureName(entry.name)
            }
            guard encodedFiles.insert(entry.encodedFile).inserted else {
                throw FixtureError.duplicateEncodedFile(entry.encodedFile)
            }
            guard payloadHashes.insert(entry.payloadHash).inserted else {
                throw FixtureError.duplicatePayloadHash(entry.payloadHash)
            }
            let decodedPayload = try decodeCanonicalBase64(
                entry.payloadBase64,
                context: "\(entry.name).payload_base64"
            )
            guard payloadBytes.insert(decodedPayload).inserted else {
                throw FixtureError.duplicatePayloadBytes(entry.name)
            }
            guard signedHashes.insert(entry.signedHash).inserted else {
                throw FixtureError.duplicateSignedHash(entry.signedHash)
            }
            let decodedSigned = try decodeCanonicalBase64(
                entry.signedBase64,
                context: "\(entry.name).signed_base64"
            )
            guard signedBytes.insert(decodedSigned).inserted else {
                throw FixtureError.duplicateSignedBytes(entry.name)
            }
        }
        return Dictionary(uniqueKeysWithValues: entries.map { ($0.name, $0) })
    }

    static func decodeCanonicalBase64(_ value: String, context: String) throws -> Data {
        guard let decoded = Data(base64Encoded: value),
              decoded.base64EncodedString() == value else {
            throw FixtureError.invalidBase64(context)
        }
        return decoded
    }

    func fixture(named name: String) throws -> CombinedTransactionFixture {
        guard let payload = payloads[name] else {
            throw FixtureError.missingFixture(name)
        }
        guard let manifest = manifests[name] else {
            throw FixtureError.missingFixture(name)
        }
        return CombinedTransactionFixture(payload: payload, manifest: manifest)
    }

    private static func fixturesRoot() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // TransactionParityFixturesTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .appendingPathComponent("Fixtures", isDirectory: true)
    }
}

private struct CombinedTransactionFixture {
    let payload: TransactionPayloadSpec
    let manifest: TransactionFixtureLoader.ManifestEntry
}

private struct TransactionPayloadSpec: Decodable {
    let chain: String
    let authority: String
    let creationTimeMs: UInt64
    let executable: TransactionExecutable
    let timeToLiveMs: UInt64?
    let nonce: UInt32?
    let metadata: [String: String]

    private enum CodingKeys: String, CodingKey {
        case chain
        case authority
        case creationTimeMs
        case executable
        case timeToLiveMs
        case nonce
        case metadata
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        chain = try container.decode(String.self, forKey: .chain)
        authority = try container.decode(String.self, forKey: .authority)
        creationTimeMs = try container.decode(UInt64.self, forKey: .creationTimeMs)
        executable = try container.decode(TransactionExecutable.self, forKey: .executable)
        timeToLiveMs = try container.decodeIfPresent(UInt64.self, forKey: .timeToLiveMs)
        nonce = try container.decodeIfPresent(UInt32.self, forKey: .nonce)
        metadata = try container.decodeIfPresent([String: String].self, forKey: .metadata) ?? [:]
    }

    func instruction(kind: String, action: String) throws -> TransactionInstruction {
        guard case .instructions(let items) = executable else {
            throw FixtureError.unsupportedExecutable(kind)
        }
        guard let instruction = items.first(where: { instruction in
            instruction.kind == kind && instruction.arguments["action"] == action
        }) else {
            throw FixtureError.missingInstruction("\(kind)::\(action)")
        }
        return instruction
    }
}

private enum TransactionExecutable: Decodable {
    case instructions([TransactionInstruction])
    case ivm(Data)

    private enum CodingKeys: String, CodingKey {
        case instructions = "Instructions"
        case ivm = "Ivm"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let instructions = try container.decodeIfPresent([TransactionInstruction].self, forKey: .instructions) {
            self = .instructions(instructions)
        } else if let ivmBase64 = try container.decodeIfPresent(String.self, forKey: .ivm) {
            guard let decoded = Data(base64Encoded: ivmBase64) else {
                throw DecodingError.dataCorruptedError(forKey: .ivm,
                                                       in: container,
                                                       debugDescription: "invalid base64 payload")
            }
            self = .ivm(decoded)
        } else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(codingPath: decoder.codingPath,
                                      debugDescription: "unsupported executable variant")
            )
        }
    }
}

private struct TransactionInstruction: Decodable {
    let kind: String
    let arguments: [String: String]

    func argument(named name: String) throws -> String {
        guard let value = arguments[name], !value.isEmpty else {
            throw FixtureError.missingArgument(name, instruction: kind)
        }
        return value
    }
}

private enum FixtureError: Error, LocalizedError {
    case missingFixture(String)
    case unsupportedExecutable(String)
    case missingInstruction(String)
    case missingArgument(String, instruction: String)
    case invalidAccountId(field: String, value: String)
    case invalidManifestProperty(String)
    case duplicateManifestProperty(String)
    case unexpectedManifestProperties(Set<String>)
    case duplicateFixtureName(String)
    case duplicateEncodedFile(String)
    case duplicatePayloadHash(String)
    case duplicatePayloadBytes(String)
    case duplicateSignedHash(String)
    case duplicateSignedBytes(String)
    case invalidBase64(String)
    case fixtureNameSetMismatch(payloads: Set<String>, manifests: Set<String>)
    case invalidSigningSeed
    case bridgeKeypairUnavailable

    var errorDescription: String? {
        switch self {
        case let .missingFixture(name):
            return "fixture '\(name)' is not available"
        case let .unsupportedExecutable(kind):
            return "executable for '\(kind)' fixture is not instruction-based"
        case let .missingInstruction(label):
            return "instruction \(label) not found in fixture"
        case let .missingArgument(arg, instruction):
            return "instruction \(instruction) missing argument '\(arg)'"
        case let .invalidAccountId(field, value):
            return "account id for \(field) must be encoded-only (received '\(value)')"
        case let .invalidManifestProperty(line):
            return "fixture property is missing '=': \(line)"
        case let .duplicateManifestProperty(key):
            return "fixture property '\(key)' is duplicated"
        case let .unexpectedManifestProperties(keys):
            return "fixture properties did not match the required key set: \(keys.sorted())"
        case let .duplicateFixtureName(name):
            return "fixture name '\(name)' is duplicated"
        case let .duplicateEncodedFile(file):
            return "fixture encoded file '\(file)' is duplicated"
        case let .duplicatePayloadHash(hash):
            return "fixture payload hash '\(hash)' is duplicated"
        case let .duplicatePayloadBytes(name):
            return "fixture payload bytes are duplicated by '\(name)'"
        case let .duplicateSignedHash(hash):
            return "fixture signed hash '\(hash)' is duplicated"
        case let .duplicateSignedBytes(name):
            return "fixture signed bytes are duplicated by '\(name)'"
        case let .invalidBase64(context):
            return "fixture base64 is invalid or non-canonical: \(context)"
        case let .fixtureNameSetMismatch(payloads, manifests):
            return "payload/manifest fixture names differ: payloads=\(payloads.sorted()) manifests=\(manifests.sorted())"
        case .invalidSigningSeed:
            return "fixture signing seed could not be decoded"
        case .bridgeKeypairUnavailable:
            return "fixture signing seed could not be derived using the native bridge"
        }
    }
}

private enum FixtureConstants {
    static let signingSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032"
    static let networkPrefix: UInt16 = 42
}
