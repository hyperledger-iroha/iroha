import XCTest
@testable import IrohaSwift

final class TransactionParityFixturesTests: XCTestCase {
    private static var cachedFixtures: TransactionFixtureLoader?
    private static var cachedKeypair: Keypair?

    func testSwiftTransferAssetFixtureMatchesRustEncoder() throws {
        try ensureBridgeAvailable()
        try assertFixture(named: "swift_transfer_asset_basic") { fixture, keypair in
            let instruction = try fixture.payload.instruction(kind: "Transfer", action: "TransferAsset")
            let assetLiteral = try instruction.argument(named: "asset")
            let destination = try instruction.argument(named: "destination")
            let quantity = try instruction.argument(named: "quantity")
            let components = try TransactionParityFixturesTests.parseAssetLiteral(assetLiteral)
            let request = TransferRequest(chainId: fixture.payload.chain,
                                          authority: fixture.payload.authority,
                                          assetDefinitionId: components.definitionId,
                                          quantity: quantity,
                                          destination: destination,
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
            let assetLiteral = try instruction.argument(named: "asset")
            let quantity = try instruction.argument(named: "quantity")
            let components = try TransactionParityFixturesTests.parseAssetLiteral(assetLiteral)
            let request = MintRequest(chainId: fixture.payload.chain,
                                      authority: fixture.payload.authority,
                                      assetDefinitionId: components.definitionId,
                                      quantity: quantity,
                                      destination: components.accountId,
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
            let assetLiteral = try instruction.argument(named: "asset")
            let quantity = try instruction.argument(named: "quantity")
            let components = try TransactionParityFixturesTests.parseAssetLiteral(assetLiteral)
            let request = BurnRequest(chainId: fixture.payload.chain,
                                      authority: fixture.payload.authority,
                                      assetDefinitionId: components.definitionId,
                                      quantity: quantity,
                                      destination: components.accountId,
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
        } catch let SwiftTransactionEncoderError.nativeBridgeError(error) {
            throw error
        }
    }

    private func ensureBridgeAvailable() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
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

    private static func parseAssetLiteral(_ literal: String) throws -> AssetLiteral {
        let parts = literal.split(separator: "#", omittingEmptySubsequences: false)
        guard parts.count >= 3 else {
            throw FixtureError.invalidAssetLiteral(literal)
        }
        let definitionId = parts[0...1].joined(separator: "#")
        let accountId = parts[2...].joined(separator: "#")
        guard !definitionId.isEmpty, !accountId.isEmpty else {
            throw FixtureError.invalidAssetLiteral(literal)
        }
        return AssetLiteral(definitionId: String(definitionId), accountId: String(accountId))
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
        let signedBase64: String
    }

    struct ManifestFile: Decodable {
        let fixtures: [ManifestEntry]
    }

    let payloads: [String: TransactionPayloadSpec]
    let manifests: [String: ManifestEntry]

    init() throws {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase

        let payloadURL = TransactionFixtureLoader.fixturesRoot()
            .appendingPathComponent("swift_parity_payloads.json")
        let payloadData = try Data(contentsOf: payloadURL)
        let payloadEntries = try decoder.decode([PayloadEntry].self, from: payloadData)
        var payloadMap: [String: TransactionPayloadSpec] = [:]
        for entry in payloadEntries {
            payloadMap[entry.name] = entry.payload
        }
        payloads = payloadMap

        let manifestURL = TransactionFixtureLoader.fixturesRoot()
            .appendingPathComponent("swift_parity_manifest.json")
        let manifestData = try Data(contentsOf: manifestURL)
        let manifest = try decoder.decode(ManifestFile.self, from: manifestData)
        var manifestMap: [String: ManifestEntry] = [:]
        for entry in manifest.fixtures {
            manifestMap[entry.name] = entry
        }
        manifests = manifestMap
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

private struct AssetLiteral {
    let definitionId: String
    let accountId: String
}

private enum FixtureError: Error, LocalizedError {
    case missingFixture(String)
    case unsupportedExecutable(String)
    case missingInstruction(String)
    case missingArgument(String, instruction: String)
    case invalidAssetLiteral(String)
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
        case let .invalidAssetLiteral(literal):
            return "asset literal '\(literal)' is invalid"
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
